//! The probe of every row of `options.table` on the running kernel, through the generated trait methods.
//!
//! Each call goes through the method a caller sees, so the signature the generator made is what is measured. Memory a
//! caller lends a method sits between canary bytes and is compared after the call: a set the table marks `reads` must
//! leave its value as it was, one marked `writes-back` must be seen writing into it, and no call may write before or past
//! it. Memory the method owns (the room of a read, the copy of a number, a program's header) carries the
//! implementation's own canaries, which stop the call when the kernel writes past what it was told; the probe counts
//! each such stop. A get marked `within` must answer no longer a length than its room, `past` must refuse every query
//! asking more answers than its room holds, and `through` must be seen writing into the rooms its value names.
//!
//! Canaries see only what the kernel writes, so two checks see what it reads. Every value a caller lends is also lent
//! once with each of its words set to an address no process maps: a kernel that followed an address the value holds
//! answers `EFAULT`, and any `EFAULT` fails the row. And a `within` room, or a number read, must be seen given less
//! than the kernel had to write, so that a socket left with nothing to write does not pass for one that stops at its
//! room; a number read is given each room from one byte to its width below the method, with the same level and option.
//! Runs as root, in a network namespace of its own, as the conformance suite does.

use std::ffi::c_int;
use std::fmt;
use std::io::{IoSlice, IoSliceMut};
use std::mem::{offset_of, size_of, size_of_val};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::panic::AssertUnwindSafe;

use linux_raw_sys::net as uapi;
use nix::sys::socket::{setsockopt, sockopt};
use nix::sys::uio::{RemoteIoVec, process_vm_readv, process_vm_writev};
use nix::unistd::Pid;
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use wcontainers_abi::HostError;
use wcontainers_linux_socket_options::{
    GroupFilter, KEY_QUERY_LENGTH, LinuxSocketOptions, MptcpFullInformation, Written,
};
use wcontainers_linux_socket_options_conformance::{
    EIGHT_BYTES, FAST_OPEN_QUEUE, HEADER_COVERAGE, LARGE_ROOM, NO_CHECKSUM, ONE_BYTE,
    QUICK_THEN_DELAYING, RETRANSMISSION_MAXIMUM_MILLISECONDS, RETRANSMISSION_MINIMUM_MICROSECONDS,
    SEGMENT_SIZE, SEQUENCE, own_network, storage_of,
};

use crate::DirectLinuxSocketOptions;
use crate::generated::ROWS;

/// The implementation every call of the probe goes through, by its trait.
const OPTIONS: DirectLinuxSocketOptions = DirectLinuxSocketOptions;

/// What the kernel does with the caller's memory, as the table's kernel column names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kernel {
    Reads,
    WritesBack,
    Within,
    Past,
    Through,
}

/// One row of `options.table`.
pub(crate) struct Row {
    pub(crate) method: &'static str,
    pub(crate) value: &'static str,
    pub(crate) kernel: Kernel,
    /// The level the row's method passes, none for an ioctl request, and the option or request number.
    pub(crate) level: Option<u32>,
    pub(crate) option: u32,
}

/// The value forms of a number, and the bytes each is read into.
const NUMBER_FORMS: [(&str, usize); 3] = [
    ("c_int", size_of::<c_int>()),
    ("u32", size_of::<u32>()),
    ("u64", size_of::<u64>()),
];

/// The canary bytes on each side of a value a caller lends.
const GUARD: usize = 1024;

/// The two canary patterns, so that a byte the kernel writes equal to one is seen through the other.
const CANARIES: [u8; 2] = [0xa5, 0x5a];

/// The empty room, the first of those a read of varying length is read into.
const NO_ROOM: usize = 0;

/// The words a lent value is filled with to see whether the kernel follows an address in it.
///
/// Each is an address below the lowest any process may map (`vm.mmap_min_addr`) and a count small enough for the
/// kernel to go on and use the address beside it.
///
/// https://docs.kernel.org/admin-guide/sysctl/vm.html#mmap-min-addr
const UNMAPPED_WORDS: [u64; 2] = [1, 16];

/// A room longer than a control message's header and shorter than the message, which the kernel fills and cuts.
///
/// https://man7.org/linux/man-pages/man3/cmsg.3.html
const PART_OF_A_RECORD: usize = 20;

/// The forwarding setting of the namespace's loopback, which lets it join any anycast address.
///
/// https://docs.kernel.org/networking/ip-sysctl.html
const LOOPBACK_FORWARDING: &str = "/proc/sys/net/ipv6/conf/lo/forwarding";

/// A room of `MPTCP_FULL_INFO` shorter than each structure it holds, and one longer.
const SHORT_ENTRY: usize = 8;
const LONG_ENTRY: usize = 512;

/// The subflow counts `MPTCP_FULL_INFO` is told: none and one.
const SUBFLOW_COUNTS: [usize; 2] = [0, 1];

/// A value of a number option that every socket here takes, and the value that turns one off.
const ON: c_int = 1;
const OFF: c_int = 0;

/// The classic BPF instruction that returns the group's first socket, `BPF_RET | BPF_K` with 0.
///
/// https://docs.kernel.org/networking/filter.html
const RETURN_FIRST: libc::sock_filter = libc::sock_filter {
    code: 0x06,
    jt: 0,
    jf: 0,
    k: 0,
};

/// A program of a few instructions, longer than one.
const FEW_INSTRUCTIONS: usize = 4;

/// An IPv4 multicast group of the organization's own scope.
///
/// https://www.rfc-editor.org/rfc/rfc2365#section-6.2
const GROUP_V4: Ipv4Addr = Ipv4Addr::new(239, 1, 2, 3);

/// An IPv6 multicast group of the link.
///
/// https://www.rfc-editor.org/rfc/rfc4291#section-2.7
const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0x1234);

/// An IPv6 address of the documentation prefix, joined as anycast.
///
/// https://www.rfc-editor.org/rfc/rfc3849
const ANYCAST: Ipv6Addr = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);

/// A flow label below the kernel's reserved half.
///
/// https://docs.kernel.org/networking/ip-sysctl.html
const FLOW_LABEL: u32 = 0x1_2345;

/// A secret of sixteen bytes, as TCP MD5, TCP-AO and Fast Open keys take it.
const SECRET: [u8; 16] = [0x5c; 16];

/// The MAC algorithm the TCP-AO keys use, as the kernel's crypto layer names it.
///
/// https://docs.kernel.org/networking/tcp_ao.html
const AO_ALGORITHM: &[u8] = b"hmac(sha1)";

/// The bit of `get_all` in the bit field of `struct tcp_ao_getsockopt`, its third on a little-endian machine.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/tcp.h
const GET_ALL: u8 = 1 << 2;

/// The TCP-AO key identifiers of the two keys a keyed socket holds.
const KEY_IDENTIFIERS: [u8; 2] = [1, 2];

/// The prefix length of one IPv4 address.
const ONE_ADDRESS_V4: u8 = 32;

/// The `nkeys` of key queries: one answer, two (more than the room of one holds), none, and the most a count names.
const KEY_COUNTS: [u32; 4] = [1, 2, 0, u32::MAX];

/// The most answers the room of a key query holds.
const ANSWERS_IN_ROOM: u32 = 1;

/// The kind of TCP's maximum segment size option.
///
/// https://www.rfc-editor.org/rfc/rfc9293#section-3.2
const MAXIMUM_SEGMENT_SIZE: u32 = 2;

/// The congestion control whose `TCP_CC_INFO` Linux fills, `struct tcpvegas_info`; the job loads its module.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/inet_diag.h
const VEGAS: &[u8] = b"vegas";

/// An IPv6 extension header of eight bytes holding only padding: no next header, no further units, a PadN of four.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-4.2
const PADDING_HEADER: [u8; 8] = [0, 0, 1, 4, 0, 0, 0, 0];

/// IPv4 options of four bytes: three no-operations and the end.
///
/// https://www.rfc-editor.org/rfc/rfc791#section-3.1
const PADDING_OPTIONS: [u8; 4] = [1, 1, 1, 0];

/// A socket a row is called on, with the sockets that must stay open beside it.
struct Prepared {
    socket: OwnedFd,
    _keep: Vec<OwnedFd>,
}

/// One way of calling a row: the socket, made afresh for every call, how many calls, and the call by its index.
struct Case {
    prepare: fn() -> Prepared,
    count: Box<dyn Fn(BorrowedFd<'_>) -> usize>,
    call: Box<dyn Fn(&mut Seen, BorrowedFd<'_>, usize)>,
}

impl Case {
    /// Makes one call per value, each value made from the socket of its own call.
    fn new<T: 'static>(
        prepare: fn() -> Prepared,
        values: fn(BorrowedFd<'_>) -> Vec<T>,
        call: impl Fn(&mut Seen, BorrowedFd<'_>, T) + 'static,
    ) -> Self {
        Self {
            prepare,
            count: Box::new(move |socket| values(socket).len()),
            call: Box::new(move |seen, socket, index| {
                let value = values(socket).swap_remove(index);
                call(seen, socket, value);
            }),
        }
    }
}

/// What the calls of one row did to the memory around and inside their values.
#[derive(Default)]
struct Seen {
    calls: usize,
    succeeded: usize,
    /// Calls after which a byte of a lent value differed from what was given.
    inside: usize,
    /// Calls after which a byte past a lent value differed.
    past: usize,
    /// Calls after which a byte before a lent value differed.
    before: usize,
    /// Reads whose answered length was below the room.
    shorter: usize,
    /// Reads whose answered length was above the room.
    longer: usize,
    /// Reads that answered a length above zero.
    written: usize,
    /// Calls that wrote into a room the value named.
    through: usize,
    /// Key queries asking more answers than the room holds that the method took.
    beyond_taken: usize,
    /// Key queries asking more answers than the room holds that the method refused.
    beyond_refused: usize,
    /// Calls the kernel answered `EFAULT`: it followed an address the value holds.
    followed: usize,
    /// Calls lent a value whose words are all unmapped addresses.
    poisoned: usize,
    /// Each room read's room and the length answered, or none when the read failed.
    rooms: Vec<(usize, Option<usize>)>,
    /// What stopped a call or a preparation: an implementation's canary, or a socket that could not be made.
    stops: Vec<String>,
    errors: Vec<c_int>,
}

impl Seen {
    fn record<T>(&mut self, result: &Result<T, HostError>) {
        self.calls += 1;
        match result {
            Ok(_) => self.succeeded += 1,
            Err(HostError::Errno(number)) => {
                self.followed += usize::from(*number == libc::EFAULT);
                if !self.errors.contains(number) {
                    self.errors.push(*number);
                }
            }
            Err(_) => {}
        }
    }

    /// Tells whether some room was given less than the kernel had to write, and the kernel stopped at it or refused.
    ///
    /// The longest answer is what the kernel had; a room above zero and below it must be answered whole or refused.
    fn pressed(&self) -> bool {
        let longest = self
            .rooms
            .iter()
            .filter_map(|(_, answered)| *answered)
            .max()
            .unwrap_or(NO_ROOM);
        self.rooms.iter().any(|(room, answered)| {
            *room > NO_ROOM && *room < longest && answered.is_none_or(|length| length == *room)
        })
    }

    /// Gives what the calls did that the row's kernel column does not allow, if anything.
    fn against(&self, kernel: Kernel, value: &str) -> Option<String> {
        if let Some(stop) = self.stops.first() {
            return Some(format!("a call or its socket stopped: {stop}"));
        }
        let fault = if self.followed > 0 {
            "was answered EFAULT: the kernel followed an address the value holds"
        } else if self.before > 0 {
            "wrote before a value it was lent"
        } else if self.succeeded == 0 {
            "never succeeded, so its column is not measured"
        } else {
            match kernel {
                Kernel::Reads | Kernel::WritesBack | Kernel::Within | Kernel::Past
                    if self.through > 0 =>
                {
                    "wrote at an address its value holds, which only a `through` column allows"
                }
                Kernel::Reads if self.inside > 0 => {
                    "wrote into a value its column says it only reads"
                }
                Kernel::WritesBack if self.inside == 0 => "was never seen writing into its value",
                Kernel::Within | Kernel::Past | Kernel::Through if self.inside > 0 => {
                    "wrote into a value it was lent to read from"
                }
                _ if self.past > 0 => "wrote past a value it was lent",
                Kernel::Within | Kernel::Past | Kernel::Through if self.longer > 0 => {
                    "answered a length longer than its room"
                }
                Kernel::Past if self.beyond_taken > 0 => {
                    "took a query asking more answers than its room holds"
                }
                Kernel::Past if self.beyond_refused == 0 => {
                    "was never asked for more answers than its room holds"
                }
                Kernel::Through if self.through == 0 => "was never seen writing into its rooms",
                Kernel::Within
                    if (value == "room" || NUMBER_FORMS.iter().any(|(form, _)| *form == value))
                        && !self.pressed() =>
                {
                    "was never given less room than the kernel had to write, so `within` is not measured"
                }
                _ => return None,
            }
        };
        Some(fault.to_owned())
    }
}

impl fmt::Display for Seen {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "calls {} ok {} inside {} past {} before {} shorter {} longer {} written {} through {} beyond taken {} \
             refused {} poisoned {} followed {} pressed {} stops {} errors {:?}",
            self.calls,
            self.succeeded,
            self.inside,
            self.past,
            self.before,
            self.shorter,
            self.longer,
            self.written,
            self.through,
            self.beyond_taken,
            self.beyond_refused,
            self.poisoned,
            self.followed,
            self.pressed(),
            self.stops.len(),
            self.errors
        )
    }
}

/// Gives the bytes of `value` as they sit in memory, read through the kernel's copy of this process's own memory.
///
/// linux-raw-sys's structures give no byte view, so this is how a lent structure is compared after a call.
fn bytes_of<T: ?Sized>(value: &T) -> Vec<u8> {
    let mut bytes = vec![u8::MIN; size_of_val(value)];
    let remote = [RemoteIoVec {
        base: std::ptr::from_ref(value).cast::<u8>().expose_provenance(),
        len: bytes.len(),
    }];
    let read = process_vm_readv(Pid::this(), &mut [IoSliceMut::new(&mut bytes)], &remote)
        .expect("the probe reads its own memory, as root");
    assert_eq!(read, bytes.len(), "the whole value read");
    bytes
}

/// Gives `value` in a box of its own with every word of it, and every byte past the last whole word, set from `word`.
///
/// The bytes are written through the kernel's copy into this process's own memory, at the box's exposed address, as
/// linux-raw-sys's structures give no byte view; every structure lent here is made of numbers, so any bytes are one.
fn unmapped<T>(value: T, word: u64) -> Box<T> {
    let mut boxed = Box::new(value);
    let bytes = unmapped_bytes(size_of::<T>(), word);
    let remote = [RemoteIoVec {
        base: std::ptr::from_mut(&mut *boxed)
            .cast::<u8>()
            .expose_provenance(),
        len: bytes.len(),
    }];
    let written = process_vm_writev(Pid::this(), &[IoSlice::new(&bytes)], &remote)
        .expect("the probe writes its own memory, as root");
    assert_eq!(written, bytes.len(), "the whole value written");
    boxed
}

/// Gives `length` bytes of `word` repeated in the machine's order.
fn unmapped_bytes(length: usize, word: u64) -> Vec<u8> {
    word.to_ne_bytes()
        .into_iter()
        .cycle()
        .take(length)
        .collect()
}

/// A value a caller lends, between canary bytes.
#[repr(C)]
struct Fenced<T> {
    before: [u8; GUARD],
    value: T,
    after: [u8; GUARD],
}

impl<T> Fenced<T> {
    fn new(canary: u8, value: T) -> Self {
        Self {
            before: [canary; GUARD],
            value,
            after: [canary; GUARD],
        }
    }

    /// Records how the fence and the value differ from what they held.
    fn compare(&self, seen: &mut Seen, canary: u8, given: &[u8]) {
        seen.before += usize::from(self.before.iter().any(|byte| *byte != canary));
        seen.inside += usize::from(bytes_of(&self.value) != given);
        seen.past += usize::from(self.after.iter().any(|byte| *byte != canary));
    }
}

/// A set whose value the method copies: the probe sees the outcome, the implementation's canaries the memory.
fn numbers<T: Copy + 'static>(
    prepare: fn() -> Prepared,
    values: fn(BorrowedFd<'_>) -> Vec<T>,
    method: fn(BorrowedFd<'_>, T) -> Result<(), HostError>,
) -> Case {
    Case::new(prepare, values, move |seen, socket, value| {
        seen.record(&method(socket, value));
    })
}

/// A set of one structure lent by shared reference, between canaries.
fn structures<T: Copy + 'static>(
    prepare: fn() -> Prepared,
    values: fn(BorrowedFd<'_>) -> Vec<T>,
    method: fn(BorrowedFd<'_>, &T) -> Result<(), HostError>,
) -> Case {
    Case::new(prepare, values, move |seen, socket, value| {
        for canary in CANARIES {
            let fenced = Fenced::new(canary, value);
            let given = bytes_of(&fenced.value);
            seen.record(&method(socket, &fenced.value));
            fenced.compare(seen, canary, &given);
        }
        for word in UNMAPPED_WORDS {
            seen.poisoned += 1;
            seen.record(&method(socket, &unmapped(value, word)));
        }
    })
}

/// A set of one structure lent by exclusive reference, between canaries.
fn structures_written<T: Copy + 'static>(
    prepare: fn() -> Prepared,
    values: fn(BorrowedFd<'_>) -> Vec<T>,
    method: fn(BorrowedFd<'_>, &mut T) -> Result<(), HostError>,
) -> Case {
    Case::new(prepare, values, move |seen, socket, value| {
        for canary in CANARIES {
            let mut fenced = Fenced::new(canary, value);
            let given = bytes_of(&fenced.value);
            seen.record(&method(socket, &mut fenced.value));
            fenced.compare(seen, canary, &given);
        }
        for word in UNMAPPED_WORDS {
            seen.poisoned += 1;
            seen.record(&method(socket, &mut unmapped(value, word)));
        }
    })
}

/// A set of a byte string lent by shared reference, between canaries.
fn byte_strings(
    prepare: fn() -> Prepared,
    values: fn(BorrowedFd<'_>) -> Vec<Vec<u8>>,
    method: fn(BorrowedFd<'_>, &[u8]) -> Result<(), HostError>,
) -> Case {
    Case::new(prepare, values, move |seen, socket, value| {
        for canary in CANARIES {
            let mut buffer = vec![canary; GUARD];
            buffer.extend_from_slice(&value);
            buffer.extend(std::iter::repeat_n(canary, GUARD));
            seen.record(&method(socket, &buffer[GUARD..GUARD + value.len()]));
            let (before, rest) = buffer.split_at(GUARD);
            let (inside, past) = rest.split_at(value.len());
            seen.before += usize::from(before.iter().any(|byte| *byte != canary));
            seen.inside += usize::from(*inside != *value);
            seen.past += usize::from(past.iter().any(|byte| *byte != canary));
        }
        for word in UNMAPPED_WORDS {
            seen.poisoned += 1;
            seen.record(&method(socket, &unmapped_bytes(value.len(), word)));
        }
    })
}

/// A set of a run of `N` values lent by shared reference, between canaries.
fn runs<T: Copy + 'static, const N: usize>(
    prepare: fn() -> Prepared,
    values: fn(BorrowedFd<'_>) -> Vec<[T; N]>,
    method: fn(BorrowedFd<'_>, &[T]) -> Result<(), HostError>,
) -> Case {
    Case::new(prepare, values, move |seen, socket, value| {
        for canary in CANARIES {
            let fenced = Fenced::new(canary, value);
            let given = bytes_of(&fenced.value);
            seen.record(&method(socket, &fenced.value[..]));
            fenced.compare(seen, canary, &given);
        }
        for word in UNMAPPED_WORDS {
            seen.poisoned += 1;
            seen.record(&method(socket, &unmapped(value, word)[..]));
        }
    })
}

/// A program of `N` instructions, each the one that returns the first socket.
fn program_of<const N: usize>(_: BorrowedFd<'_>) -> Vec<[libc::sock_filter; N]> {
    vec![[RETURN_FIRST; N]]
}

/// A multicast source filter: the group, the mode, and the sources.
type Filter = (SocketAddr, u32, Vec<SocketAddr>);

/// A set of a source filter, which the method writes into memory of its own.
fn group_filters(
    prepare: fn() -> Prepared,
    values: fn(BorrowedFd<'_>) -> Vec<Filter>,
    method: fn(BorrowedFd<'_>, &GroupFilter<'_>) -> Result<(), HostError>,
) -> Case {
    Case::new(
        prepare,
        values,
        move |seen, socket, (group, mode, sources)| {
            let sources: Vec<_> = sources.into_iter().map(storage_of).collect();
            let filter = GroupFilter {
                interface: loopback_index(),
                group: storage_of(group),
                mode,
                sources: &sources,
            };
            seen.record(&method(socket, &filter));
        },
    )
}

/// A read of a number or an ioctl request's int, into memory the method owns.
fn number_reads<T: 'static>(
    prepare: fn() -> Prepared,
    method: fn(BorrowedFd<'_>) -> Result<T, HostError>,
) -> Case {
    Case::new(
        prepare,
        |_| vec![()],
        move |seen, socket, ()| seen.record(&method(socket)),
    )
}

/// What a read of varying length answered: its room, the length the kernel wrote, and the bytes.
struct Answer {
    room: usize,
    length: usize,
}

fn answer<const ROOM: usize>(written: Written<ROOM>) -> Answer {
    Answer {
        room: ROOM,
        length: written.length,
    }
}

/// A read of varying length into a room of a size its method's constant names.
type RoomRead = fn(BorrowedFd<'_>) -> Result<Answer, HostError>;

fn room_read(read: RoomRead) -> RoomRead {
    read
}

/// Reads of varying length, one per room, with the size of each room.
fn room_reads(prepare: fn() -> Prepared, reads: Vec<RoomRead>, room_sizes: Vec<usize>) -> Case {
    Case {
        prepare,
        count: Box::new({
            let count = reads.len();
            move |_| count
        }),
        call: Box::new(move |seen, socket, index| {
            let result = reads[index](socket);
            seen.record(&result);
            seen.rooms.push((
                room_sizes[index],
                result.as_ref().ok().map(|answer| answer.length),
            ));
            if let Ok(answer) = result {
                seen.shorter += usize::from(answer.length < answer.room);
                seen.longer += usize::from(answer.length > answer.room);
                seen.written += usize::from(answer.length > NO_ROOM);
            }
        }),
    }
}

/// A key query asking `keys` answers for every key.
fn every_key_query(keys: u32) -> [u8; KEY_QUERY_LENGTH] {
    let mut bytes = [u8::MIN; KEY_QUERY_LENGTH];
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_getsockopt, nkeys),
        &keys.to_ne_bytes(),
    );
    bytes[offset_of!(uapi::tcp_ao_getsockopt, _bitfield_1)] = GET_ALL;
    bytes
}

fn key_queries(_: BorrowedFd<'_>) -> Vec<u32> {
    KEY_COUNTS.to_vec()
}

/// Key queries lent by shared reference between canaries, each asking the answers `KEY_COUNTS` names.
fn key_reads(
    prepare: fn() -> Prepared,
    method: fn(
        BorrowedFd<'_>,
        &[u8; KEY_QUERY_LENGTH],
    ) -> Result<Written<KEY_QUERY_LENGTH>, HostError>,
) -> Case {
    Case::new(prepare, key_queries, move |seen, socket, keys| {
        for canary in CANARIES {
            let fenced = Fenced::new(canary, every_key_query(keys));
            let given = bytes_of(&fenced.value);
            let result = method(socket, &fenced.value);
            seen.record(&result);
            fenced.compare(seen, canary, &given);
            match &result {
                Ok(_) if keys > ANSWERS_IN_ROOM => seen.beyond_taken += 1,
                Err(HostError::Errno(libc::EINVAL)) if keys > ANSWERS_IN_ROOM => {
                    seen.beyond_refused += 1;
                }
                Ok(written) => {
                    seen.longer += usize::from(written.length > KEY_QUERY_LENGTH);
                    seen.written += usize::from(written.length > NO_ROOM);
                }
                Err(_) => {}
            }
        }
        for word in UNMAPPED_WORDS {
            seen.poisoned += 1;
            seen.record(&method(socket, &unmapped(every_key_query(keys), word)));
        }
    })
}

/// What `MPTCP_FULL_INFO` gave, by run of rooms: how many rooms, and whether a byte of one is not zero.
struct FullAnswer {
    rooms: [usize; 2],
    written: bool,
}

fn full_answer<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize>(
    information: MptcpFullInformation<SUBFLOW_ROOM, INFORMATION_ROOM>,
) -> FullAnswer {
    let written = information
        .subflow_information
        .iter()
        .flatten()
        .chain(information.tcp_information.iter().flatten())
        .any(|byte| *byte != u8::MIN);
    FullAnswer {
        rooms: [
            information.subflow_information.len(),
            information.tcp_information.len(),
        ],
        written,
    }
}

/// A read of `MPTCP_FULL_INFO` with room for the subflows given, of the entry sizes its method's constants name.
type FullRead = fn(BorrowedFd<'_>, usize) -> Result<FullAnswer, HostError>;

fn full_read(read: FullRead) -> FullRead {
    read
}

/// Reads of `MPTCP_FULL_INFO`, each of `SUBFLOW_COUNTS` with each entry size.
fn full_information_reads(prepare: fn() -> Prepared, reads: Vec<FullRead>) -> Case {
    Case {
        prepare,
        count: Box::new({
            let count = reads.len() * SUBFLOW_COUNTS.len();
            move |_| count
        }),
        call: Box::new(move |seen, socket, index| {
            let subflows = SUBFLOW_COUNTS[index % SUBFLOW_COUNTS.len()];
            let result = reads[index / SUBFLOW_COUNTS.len()](socket, subflows);
            seen.record(&result);
            if let Ok(answer) = result {
                seen.past += usize::from(answer.rooms != [subflows; 2]);
                seen.through += usize::from(answer.written);
            }
        }),
    }
}

/// A row's name and its cases, each calling, through the trait, the method the row names.
macro_rules! row {
    ($method:ident, rooms($prepare:expr; $($room:expr),+)) => {
        (
            stringify!($method),
            vec![room_reads(
                $prepare,
                vec![$(room_read(|socket| OPTIONS.$method::<{ $room }>(socket).map(answer))),+],
                vec![$($room),+],
            )],
        )
    };
    ($method:ident, full_information($prepare:expr; $($entry:expr),+)) => {
        (
            stringify!($method),
            vec![full_information_reads(
                $prepare,
                vec![$(full_read(|socket, subflows| {
                    OPTIONS.$method::<{ $entry }, { $entry }>(socket, subflows).map(full_answer)
                })),+],
            )],
        )
    };
    ($method:ident, programs($prepare:expr; $($length:expr),+)) => {
        (
            stringify!($method),
            vec![$(runs::<libc::sock_filter, { $length }>(
                $prepare,
                program_of::<{ $length }>,
                |socket, program| OPTIONS.$method(socket, program),
            )),+],
        )
    };
    ($method:ident, read($prepare:expr)) => {
        (stringify!($method), vec![number_reads($prepare, |socket| OPTIONS.$method(socket))])
    };
    ($method:ident, $builder:ident($($argument:expr),+)) => {
        (
            stringify!($method),
            vec![$builder($($argument,)+ |socket, value| OPTIONS.$method(socket, value))],
        )
    };
}

fn new_socket(domain: Domain, kind: Type, protocol: Protocol) -> Socket {
    Socket::new(domain, kind, Some(protocol)).expect("a socket")
}

fn alone(socket: impl Into<OwnedFd>) -> Prepared {
    Prepared {
        socket: socket.into(),
        _keep: Vec::new(),
    }
}

fn udp4() -> Prepared {
    alone(new_socket(Domain::IPV4, Type::DGRAM, Protocol::UDP))
}

fn udp6() -> Prepared {
    alone(new_socket(Domain::IPV6, Type::DGRAM, Protocol::UDP))
}

fn udplite4() -> Prepared {
    alone(new_socket(
        Domain::IPV4,
        Type::DGRAM,
        Protocol::from(libc::IPPROTO_UDPLITE),
    ))
}

fn tcp4() -> Prepared {
    alone(new_socket(Domain::IPV4, Type::STREAM, Protocol::TCP))
}

fn tcp6() -> Prepared {
    alone(new_socket(Domain::IPV6, Type::STREAM, Protocol::TCP))
}

/// A raw IPv4 socket of ICMP, which router alerts need.
fn raw4() -> Prepared {
    alone(new_socket(Domain::IPV4, Type::RAW, Protocol::ICMPV4))
}

fn raw6() -> Prepared {
    alone(new_socket(
        Domain::IPV6,
        Type::RAW,
        Protocol::from(libc::IPPROTO_RAW),
    ))
}

/// The client end of a connected loopback TCP pair at the address given.
fn pair_at(address: SocketAddr) -> Prepared {
    let listener = TcpListener::bind(address).expect("a listener");
    let client =
        TcpStream::connect(listener.local_addr().expect("its address")).expect("a connect");
    let (accepted, _) = listener.accept().expect("an accept");
    Prepared {
        socket: client.into(),
        _keep: vec![accepted.into(), listener.into()],
    }
}

fn tcp4_pair() -> Prepared {
    pair_at(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
}

fn tcp6_pair() -> Prepared {
    pair_at(SocketAddr::from((Ipv6Addr::LOCALHOST, 0)))
}

/// A connected TCP pair whose client asked for packet information, which `IP_PKTOPTIONS` then writes as a record.
fn pktinfo_pair() -> Prepared {
    let prepared = tcp4_pair();
    setsockopt(&prepared.socket, sockopt::Ipv4PacketInfo, &true).expect("packet information");
    prepared
}

/// A connected IPv6 TCP pair whose client asked for packet information, which `IPV6_2292PKTOPTIONS` then writes.
fn pktinfo6_pair() -> Prepared {
    let prepared = tcp6_pair();
    setsockopt(&prepared.socket, sockopt::Ipv6RecvPacketInfo, &true).expect("packet information");
    prepared
}

/// A connected TCP pair whose client runs Vegas, whose state `TCP_CC_INFO` gives.
fn vegas_pair() -> Prepared {
    let prepared = tcp4_pair();
    SockRef::from(&prepared.socket)
        .set_tcp_congestion(VEGAS)
        .expect("Vegas, its module loaded by the job");
    prepared
}

/// Puts a TCP socket in repair mode, as root.
fn repaired(prepared: Prepared) -> Prepared {
    setsockopt(&prepared.socket, sockopt::TcpRepair, &u32::from(true))
        .expect("repair mode, as root");
    prepared
}

fn repair_connected() -> Prepared {
    repaired(tcp4_pair())
}

/// A TCP socket in repair mode, not connected, with its send queue chosen.
fn send_queue_chosen() -> Prepared {
    let prepared = repaired(tcp4());
    OPTIONS
        .set_tcp_repair_queue(prepared.socket.as_fd(), uapi::TCP_SEND_QUEUE as c_int)
        .expect("the send queue chosen");
    prepared
}

/// A connection accepted by a listener that saved its SYN.
fn saved_syn() -> Prepared {
    let listener = new_socket(Domain::IPV4, Type::STREAM, Protocol::TCP);
    OPTIONS
        .set_tcp_save_syn(listener.as_fd(), ON)
        .expect("SYNs saved");
    listener
        .bind(&SocketAddr::from((Ipv4Addr::LOCALHOST, 0)).into())
        .expect("a binding");
    listener.listen(1).expect("listening");
    let address = listener
        .local_addr()
        .expect("its address")
        .as_socket()
        .expect("an IP address");
    let client = TcpStream::connect(address).expect("a connect");
    let (accepted, _) = listener.accept().expect("an accept");
    Prepared {
        socket: accepted.into(),
        _keep: vec![client.into(), listener.into()],
    }
}

/// A UDP socket of a reuse-port group, bound.
fn reuse_port_group() -> Prepared {
    let udp = new_socket(Domain::IPV4, Type::DGRAM, Protocol::UDP);
    udp.set_reuse_port(true).expect("port sharing");
    udp.bind(&SocketAddr::from((Ipv4Addr::LOCALHOST, 0)).into())
        .expect("a binding");
    alone(udp)
}

/// A reuse-port group's socket with a program attached.
fn reuse_port_program() -> Prepared {
    let prepared = reuse_port_group();
    OPTIONS
        .set_so_attach_reuseport_cbpf(prepared.socket.as_fd(), &[RETURN_FIRST])
        .expect("a program attached");
    prepared
}

fn loopback_index() -> u32 {
    nix::net::if_::if_nametoindex("lo").expect("the loopback's index")
}

/// Writes `value` into `bytes` at `at`.
fn put(bytes: &mut [u8], at: usize, value: &[u8]) {
    bytes[at..at + value.len()].copy_from_slice(value);
}

/// Gives a byte as the C character type, signed or not as the target has it.
fn character(byte: u8) -> libc::c_char {
    libc::c_char::from_ne_bytes([byte])
}

/// Gives the socket address in the kernel's storage structure.
fn kernel_storage(address: SocketAddr) -> uapi::__kernel_sockaddr_storage {
    let storage = storage_of(address);
    let (family, data) = storage.bytes.split_at(size_of::<u16>());
    uapi::__kernel_sockaddr_storage {
        __bindgen_anon_1: uapi::__kernel_sockaddr_storage__bindgen_ty_1 {
            __bindgen_anon_1: uapi::__kernel_sockaddr_storage__bindgen_ty_1__bindgen_ty_1 {
                ss_family: u16::from_ne_bytes([family[0], family[1]]),
                __data: core::array::from_fn(|index| character(data[index])),
            },
        },
    }
}

fn address_v6(address: Ipv6Addr) -> uapi::in6_addr {
    uapi::in6_addr {
        in6_u: uapi::in6_addr__bindgen_ty_1 {
            u6_addr8: address.octets(),
        },
    }
}

/// A `struct group_req` for the group on loopback.
fn group_request(group: SocketAddr) -> uapi::group_req {
    uapi::group_req {
        gr_interface: loopback_index(),
        gr_group: kernel_storage(group),
    }
}

fn group_v4() -> SocketAddr {
    SocketAddr::from((GROUP_V4, 0))
}

fn group_v6() -> SocketAddr {
    SocketAddr::from((GROUP_V6, 0))
}

fn group_v4_joined() -> Prepared {
    let prepared = udp4();
    OPTIONS
        .set_ip_mcast_join_group(prepared.socket.as_fd(), &group_request(group_v4()))
        .expect("the group joined");
    prepared
}

fn group_v6_joined() -> Prepared {
    let prepared = udp6();
    OPTIONS
        .set_ipv6_mcast_join_group(prepared.socket.as_fd(), &group_request(group_v6()))
        .expect("the group joined");
    prepared
}

/// A `struct ipv6_mreq` for the anycast address on loopback.
fn anycast_request() -> uapi::ipv6_mreq {
    uapi::ipv6_mreq {
        ipv6mr_multiaddr: address_v6(ANYCAST),
        ipv6mr_ifindex: loopback_index().cast_signed(),
    }
}

fn anycast_joined() -> Prepared {
    let prepared = udp6();
    OPTIONS
        .set_ipv6_join_anycast(prepared.socket.as_fd(), &anycast_request())
        .expect("the anycast address joined");
    prepared
}

/// A `struct in6_flowlabel_req` to loopback.
fn flow_label_request(label: u32, action: u32, share: u32, flags: u32) -> uapi::in6_flowlabel_req {
    uapi::in6_flowlabel_req {
        flr_dst: address_v6(Ipv6Addr::LOCALHOST),
        flr_label: label.to_be(),
        flr_action: u8::try_from(action).expect("an action"),
        flr_share: u8::try_from(share).expect("a share"),
        flr_flags: u16::try_from(flags).expect("flags"),
        flr_expires: u16::MIN,
        flr_linger: u16::MIN,
        __flr_pad: u32::MIN,
    }
}

fn loopback_v4() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
}

/// A `struct tcp_md5sig` for loopback, with the key given or none.
fn md5_key(key: &[u8]) -> uapi::tcp_md5sig {
    let mut request = uapi::tcp_md5sig {
        tcpm_addr: kernel_storage(loopback_v4()),
        tcpm_flags: u8::MIN,
        tcpm_prefixlen: u8::MIN,
        tcpm_keylen: u16::try_from(key.len()).expect("a short key"),
        tcpm_ifindex: c_int::default(),
        tcpm_key: [u8::MIN; _],
    };
    put(&mut request.tcpm_key, 0, key);
    request
}

/// A `struct tcp_ao_add` of a key for loopback with the identifier given both ways.
fn ao_key(identifier: u8) -> uapi::tcp_ao_add {
    let mut request = uapi::tcp_ao_add {
        addr: kernel_storage(loopback_v4()),
        alg_name: [libc::c_char::default(); _],
        ifindex: c_int::default(),
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        reserved2: u16::MIN,
        prefix: ONE_ADDRESS_V4,
        sndid: identifier,
        rcvid: identifier,
        maclen: u8::MIN,
        keyflags: u8::MIN,
        keylen: u8::try_from(SECRET.len()).expect("a short key"),
        key: [u8::MIN; _],
    };
    for (slot, byte) in request.alg_name.iter_mut().zip(AO_ALGORITHM) {
        *slot = character(*byte);
    }
    put(&mut request.key, 0, &SECRET);
    request
}

/// A `struct tcp_ao_del` of the key with the identifier given both ways.
fn ao_deletion(identifier: u8) -> uapi::tcp_ao_del {
    uapi::tcp_ao_del {
        addr: kernel_storage(loopback_v4()),
        ifindex: c_int::default(),
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        reserved2: u16::MIN,
        prefix: ONE_ADDRESS_V4,
        sndid: identifier,
        rcvid: identifier,
        current_key: u8::MIN,
        rnext: u8::MIN,
        keyflags: u8::MIN,
    }
}

/// A `struct tcp_ao_info_opt` asking nothing, which gives a socket the option's state.
fn ao_information() -> uapi::tcp_ao_info_opt {
    uapi::tcp_ao_info_opt {
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        reserved2: u16::MIN,
        current_key: u8::MIN,
        rnext: u8::MIN,
        pkt_good: u64::MIN,
        pkt_bad: u64::MIN,
        pkt_key_not_found: u64::MIN,
        pkt_ao_required: u64::MIN,
        pkt_dropped_icmp: u64::MIN,
    }
}

/// Gives the socket the TCP-AO keys of `KEY_IDENTIFIERS`; the first becomes the current key.
fn keyed(prepared: Prepared) -> Prepared {
    for identifier in KEY_IDENTIFIERS {
        OPTIONS
            .set_tcp_ao_add_key(prepared.socket.as_fd(), &ao_key(identifier))
            .expect("a TCP-AO key");
    }
    prepared
}

fn ao_keyed() -> Prepared {
    keyed(tcp4())
}

/// A TCP socket with TCP-AO keys, connected in repair mode to a listener.
fn ao_repaired() -> Prepared {
    let listener = TcpListener::bind(loopback_v4()).expect("a listener");
    let prepared = keyed(repaired(tcp4()));
    let socket = Socket::from(prepared.socket);
    socket
        .connect(&listener.local_addr().expect("its address").into())
        .expect("a connect in repair mode");
    Prepared {
        socket: socket.into(),
        _keep: vec![listener.into()],
    }
}

/// A connected multipath TCP pair's client end.
fn mptcp_pair() -> Prepared {
    let listener = new_socket(Domain::IPV4, Type::STREAM, Protocol::MPTCP);
    listener
        .bind(&loopback_v4().into())
        .expect("a multipath binding");
    listener.listen(1).expect("listening");
    let client = new_socket(Domain::IPV4, Type::STREAM, Protocol::MPTCP);
    client
        .connect(&listener.local_addr().expect("its address"))
        .expect("a multipath connect");
    let (accepted, _) = listener.accept().expect("an accept");
    Prepared {
        socket: client.into(),
        _keep: vec![accepted.into(), listener.into()],
    }
}

/// The repair window the socket has, read through its method, to be written back unchanged.
fn current_window(socket: BorrowedFd<'_>) -> Vec<uapi::tcp_repair_window> {
    let window = OPTIONS
        .get_tcp_repair_window::<{ size_of::<uapi::tcp_repair_window>() }>(socket)
        .expect("the window");
    let field = |at: usize| {
        u32::from_ne_bytes(
            window.bytes[at..at + size_of::<u32>()]
                .try_into()
                .expect("four bytes"),
        )
    };
    vec![uapi::tcp_repair_window {
        snd_wl1: field(offset_of!(uapi::tcp_repair_window, snd_wl1)),
        snd_wnd: field(offset_of!(uapi::tcp_repair_window, snd_wnd)),
        max_window: field(offset_of!(uapi::tcp_repair_window, max_window)),
        rcv_wnd: field(offset_of!(uapi::tcp_repair_window, rcv_wnd)),
        rcv_wup: field(offset_of!(uapi::tcp_repair_window, rcv_wup)),
    }]
}

fn both(_: BorrowedFd<'_>) -> Vec<bool> {
    vec![true, false]
}

fn on(_: BorrowedFd<'_>) -> Vec<bool> {
    vec![true]
}

fn small(_: BorrowedFd<'_>) -> Vec<c_int> {
    vec![ON]
}

fn zero_word(_: BorrowedFd<'_>) -> Vec<u32> {
    vec![u32::MIN]
}

fn empty_or_padding(_: BorrowedFd<'_>) -> Vec<Vec<u8>> {
    vec![Vec::new(), PADDING_HEADER.to_vec()]
}

/// The way each row is called, by method, each through the trait.
fn cases() -> Vec<(&'static str, Vec<Case>)> {
    vec![
        row!(set_so_rcvmark, numbers(udp4, both)),
        row!(set_so_rcvpriority, numbers(udp4, both)),
        row!(set_so_attach_reuseport_cbpf, programs(reuse_port_group; 0, 1, FEW_INSTRUCTIONS)),
        row!(
            set_so_detach_reuseport_bpf,
            numbers(reuse_port_program, |_| vec![OFF])
        ),
        row!(
            set_so_max_pacing_rate,
            numbers(udp4, |_| vec![u64::MAX, u64::from(u32::MAX), u64::MIN])
        ),
        row!(set_so_rcvlowat, numbers(udp4, small)),
        row!(get_so_rcvlowat, read(udp4)),
        row!(
            set_so_buf_lock,
            numbers(udp4, |_| vec![uapi::SOCK_SNDBUF_LOCK])
        ),
        row!(
            set_so_txrehash,
            numbers(tcp4, |_| vec![uapi::SOCK_TXREHASH_ENABLED.cast_signed()])
        ),
        row!(set_so_cnx_advice, numbers(tcp4_pair, on)),
        row!(set_so_wifi_status, numbers(udp4, both)),
        row!(set_so_peek_off, numbers(tcp4, small)),
        row!(get_so_meminfo, rooms(udp4; NO_ROOM, ONE_BYTE, EIGHT_BYTES, LARGE_ROOM)),
        row!(set_so_no_check, numbers(udp4, both)),
        row!(
            set_so_timestamping,
            structures(udp4, |_| vec![uapi::so_timestamping {
                flags: uapi::SOF_TIMESTAMPING_RX_SOFTWARE as c_int,
                bind_phc: c_int::default(),
            }])
        ),
        row!(
            set_ip_options,
            byte_strings(udp4, |_| vec![Vec::new(), PADDING_OPTIONS.to_vec()])
        ),
        row!(set_ip_unicast_if, numbers(udp4, zero_word)),
        row!(
            set_ip_local_port_range,
            numbers(udp4, |_| vec![
                u32::from(u16::MAX) << u16::BITS | u32::from(u16::MAX / 2)
            ])
        ),
        row!(set_ip_router_alert, numbers(raw4, both)),
        row!(set_ip_nodefrag, numbers(raw4, both)),
        row!(set_ip_recvopts, numbers(udp4, both)),
        row!(set_ip_recvfragsize, numbers(udp4, both)),
        row!(set_ip_minttl, numbers(tcp4, small)),
        row!(get_ip_pktoptions, rooms(pktinfo_pair; NO_ROOM, ONE_BYTE, EIGHT_BYTES, PART_OF_A_RECORD, LARGE_ROOM)),
        row!(
            set_ip_mcast_join_group,
            structures(udp4, |_| vec![group_request(group_v4())])
        ),
        row!(
            set_ip_mcast_leave_group,
            structures(group_v4_joined, |_| vec![group_request(group_v4())])
        ),
        row!(
            set_ip_mcast_msfilter,
            group_filters(group_v4_joined, |_| vec![(
                group_v4(),
                uapi::MCAST_INCLUDE,
                vec![loopback_v4()]
            )])
        ),
        row!(set_ipv6_hopopts, byte_strings(udp6, empty_or_padding)),
        row!(set_ipv6_rthdrdstopts, byte_strings(udp6, empty_or_padding)),
        row!(set_ipv6_rthdr, byte_strings(udp6, |_| vec![Vec::new()])),
        row!(set_ipv6_dstopts, byte_strings(udp6, empty_or_padding)),
        row!(set_ipv6_flowinfo_send, numbers(udp6, both)),
        row!(set_ipv6_autoflowlabel, numbers(udp6, both)),
        row!(
            set_ipv6_flowlabel_mgr,
            structures_written(udp6, |_| vec![
                flow_label_request(
                    u32::MIN,
                    uapi::IPV6_FL_A_GET,
                    uapi::IPV6_FL_S_EXCL,
                    uapi::IPV6_FL_F_CREATE,
                ),
                flow_label_request(
                    FLOW_LABEL,
                    uapi::IPV6_FL_A_GET,
                    uapi::IPV6_FL_S_ANY,
                    uapi::IPV6_FL_F_CREATE,
                ),
                flow_label_request(FLOW_LABEL, uapi::IPV6_FL_A_PUT, u32::MIN, u32::MIN),
                flow_label_request(
                    u32::MIN,
                    uapi::IPV6_FL_A_GET,
                    u32::MIN,
                    uapi::IPV6_FL_F_REFLECT
                ),
            ])
        ),
        row!(
            set_ipv6_addr_preferences,
            numbers(udp6, |_| vec![
                uapi::IPV6_PREFER_SRC_PUBLIC | uapi::IPV6_PREFER_SRC_HOME
            ])
        ),
        row!(
            set_ipv6_join_anycast,
            structures(udp6, |_| vec![anycast_request()])
        ),
        row!(
            set_ipv6_leave_anycast,
            structures(anycast_joined, |_| vec![anycast_request()])
        ),
        row!(set_ipv6_mtu, numbers(udp6, |_| vec![c_int::from(u16::MAX)])),
        row!(set_ipv6_unicast_if, numbers(udp6, zero_word)),
        row!(set_ipv6_router_alert, numbers(raw6, |_| vec![OFF])),
        row!(set_ipv6_router_alert_isolate, numbers(raw6, both)),
        row!(set_ipv6_checksum, numbers(raw6, |_| vec![NO_CHECKSUM])),
        row!(set_ipv6_recvhopopts, numbers(udp6, both)),
        row!(set_ipv6_recvdstopts, numbers(udp6, both)),
        row!(set_ipv6_recvrthdr, numbers(udp6, both)),
        row!(set_ipv6_flowinfo, numbers(udp6, both)),
        row!(set_ipv6_recvfragsize, numbers(udp6, both)),
        row!(set_ipv6_minhopcount, numbers(tcp6, small)),
        row!(
            get_ipv6_2292pktoptions,
            rooms(pktinfo6_pair; NO_ROOM, ONE_BYTE, EIGHT_BYTES, PART_OF_A_RECORD, LARGE_ROOM)
        ),
        row!(
            set_ipv6_mcast_join_group,
            structures(udp6, |_| vec![group_request(group_v6())])
        ),
        row!(
            set_ipv6_mcast_leave_group,
            structures(group_v6_joined, |_| vec![group_request(group_v6())])
        ),
        row!(
            set_ipv6_mcast_msfilter,
            group_filters(group_v6_joined, |_| vec![(
                group_v6(),
                uapi::MCAST_EXCLUDE,
                Vec::new()
            )])
        ),
        row!(
            set_tcp_md5sig_ext,
            structures(tcp4, |_| vec![md5_key(&SECRET), md5_key(&[])])
        ),
        row!(
            set_tcp_ao_add_key,
            structures(tcp4, |_| vec![ao_key(KEY_IDENTIFIERS[0])])
        ),
        row!(get_tcp_ao_get_keys, key_reads(ao_keyed)),
        row!(
            set_tcp_ao_del_key,
            structures(ao_keyed, |_| vec![ao_deletion(KEY_IDENTIFIERS[1])])
        ),
        row!(
            set_tcp_ao_info,
            structures(tcp4, |_| vec![ao_information()])
        ),
        row!(
            get_tcp_ao_info,
            rooms(ao_keyed; NO_ROOM, ONE_BYTE, EIGHT_BYTES, size_of::<uapi::tcp_ao_info_opt>(), LARGE_ROOM)
        ),
        row!(
            set_tcp_ao_repair,
            structures(ao_repaired, |_| vec![uapi::tcp_ao_repair {
                snt_isn: u32::MIN,
                rcv_isn: u32::MIN,
                snd_sne: u32::MIN,
                rcv_sne: u32::MIN,
            }])
        ),
        row!(
            get_tcp_ao_repair,
            rooms(ao_repaired; NO_ROOM, ONE_BYTE, EIGHT_BYTES, size_of::<uapi::tcp_ao_repair>(), LARGE_ROOM)
        ),
        row!(
            set_tcp_window_clamp,
            numbers(tcp4_pair, |_| vec![c_int::from(u16::MAX)])
        ),
        row!(get_tcp_window_clamp, read(tcp4_pair)),
        row!(set_tcp_syncnt, numbers(tcp4, small)),
        row!(set_tcp_defer_accept, numbers(tcp4, small)),
        row!(set_tcp_linger2, numbers(tcp4, small)),
        row!(
            set_tcp_rto_min_us,
            numbers(tcp4, |_| vec![RETRANSMISSION_MINIMUM_MICROSECONDS])
        ),
        row!(
            set_tcp_rto_max_ms,
            numbers(tcp4, |_| vec![RETRANSMISSION_MAXIMUM_MILLISECONDS])
        ),
        row!(
            set_tcp_delack_max_us,
            numbers(tcp4, |_| vec![RETRANSMISSION_MINIMUM_MICROSECONDS])
        ),
        row!(set_tcp_tx_delay, numbers(tcp4, small)),
        row!(set_tcp_fastopen_no_cookie, numbers(tcp4, both)),
        row!(set_tcp_inq, numbers(tcp4, both)),
        row!(
            set_tcp_queue_seq,
            numbers(send_queue_chosen, |_| vec![SEQUENCE])
        ),
        row!(get_tcp_queue_seq, read(send_queue_chosen)),
        row!(get_tcp_saved_syn, rooms(saved_syn; NO_ROOM, ONE_BYTE, EIGHT_BYTES, LARGE_ROOM)),
        row!(set_tcp_fastopen, numbers(tcp4, |_| vec![FAST_OPEN_QUEUE])),
        row!(
            set_tcp_fastopen_key,
            byte_strings(tcp4, |_| vec![SECRET.to_vec(), [SECRET, SECRET].concat()])
        ),
        row!(set_tcp_save_syn, numbers(tcp4, small)),
        row!(
            set_tcp_repair_queue,
            numbers(repair_connected, |_| vec![uapi::TCP_SEND_QUEUE as c_int])
        ),
        row!(get_tcp_repair_queue, read(send_queue_chosen)),
        row!(
            set_tcp_timestamp,
            numbers(repair_connected, |_| vec![SEQUENCE])
        ),
        row!(get_tcp_timestamp, read(tcp4_pair)),
        row!(
            set_tcp_quickack,
            numbers(tcp4_pair, |_| vec![QUICK_THEN_DELAYING, ON])
        ),
        row!(get_tcp_info, rooms(tcp4_pair; NO_ROOM, ONE_BYTE, EIGHT_BYTES, LARGE_ROOM)),
        row!(get_tcp_cc_info, rooms(vegas_pair; NO_ROOM, ONE_BYTE, EIGHT_BYTES, LARGE_ROOM)),
        row!(
            set_tcp_repair_window,
            structures(repair_connected, current_window)
        ),
        row!(
            get_tcp_repair_window,
            rooms(repair_connected; NO_ROOM, ONE_BYTE, EIGHT_BYTES, size_of::<uapi::tcp_repair_window>(), LARGE_ROOM)
        ),
        row!(
            set_tcp_repair_options,
            runs(repair_connected, |_| vec![[uapi::tcp_repair_opt {
                opt_code: MAXIMUM_SEGMENT_SIZE,
                opt_val: SEGMENT_SIZE,
            }]])
        ),
        row!(get_mptcp_info, rooms(mptcp_pair; NO_ROOM, ONE_BYTE, EIGHT_BYTES, LARGE_ROOM)),
        row!(get_mptcp_full_info, full_information(mptcp_pair; SHORT_ENTRY, LONG_ENTRY)),
        row!(set_udp_no_check6_rx, numbers(udp6, both)),
        row!(set_udp_no_check6_tx, numbers(udp6, both)),
        row!(
            set_udplite_recv_cscov,
            numbers(udplite4, |_| vec![HEADER_COVERAGE])
        ),
        row!(
            set_udplite_send_cscov,
            numbers(udplite4, |_| vec![HEADER_COVERAGE])
        ),
        row!(get_siocoutq, read(tcp4_pair)),
        row!(get_siocoutqnsd, read(tcp4_pair)),
        row!(get_siocatmark, read(tcp4_pair)),
    ]
}

/// Runs `work`, giving what stopped it if it panicked.
fn caught<R>(work: impl FnOnce() -> R) -> Result<R, String> {
    std::panic::catch_unwind(AssertUnwindSafe(work)).map_err(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
            })
            .unwrap_or_default()
    })
}

/// Reads a number row's option into each room from one byte to the number's own width, a fresh socket for each, below
/// the method, so that the room is less than the kernel has to write; the implementation's canaries guard every room.
fn number_rooms(seen: &mut Seen, prepare: fn() -> Prepared, level: u32, option: u32, width: usize) {
    for told in 1..=width {
        let outcome = caught(|| {
            let prepared = prepare();
            let mut buffer = vec![u8::MIN; told + crate::GUARD];
            crate::get_guarded(prepared.socket.as_fd(), level, option, &mut buffer, told)
        });
        match outcome {
            Ok(result) => {
                seen.record(&result);
                let answered = result.ok();
                seen.rooms.push((told, answered));
                if let Some(length) = answered {
                    seen.shorter += usize::from(length < told);
                    seen.longer += usize::from(length > told);
                    seen.written += usize::from(length > NO_ROOM);
                }
            }
            Err(stop) => seen.stops.push(stop),
        }
    }
}

/// Makes the case's calls, a fresh socket for each, recording what stopped any of them.
fn run(seen: &mut Seen, case: &Case) {
    let count = match caught(|| (case.count)((case.prepare)().socket.as_fd())) {
        Ok(count) => count,
        Err(stop) => {
            seen.stops.push(stop);
            return;
        }
    };
    for index in 0..count {
        let outcome = caught(|| {
            let prepared = (case.prepare)();
            (case.call)(seen, prepared.socket.as_fd(), index);
        });
        if let Err(stop) = outcome {
            seen.stops.push(stop);
        }
    }
}

#[test]
fn every_row_is_probed_through_its_method_and_the_kernel_does_what_its_column_says() {
    own_network();
    std::fs::write(LOOPBACK_FORWARDING, "1").expect("loopback forwarding, as root");
    let cases = cases();
    let unknown: Vec<&str> = cases
        .iter()
        .map(|(method, _)| *method)
        .filter(|method| !ROWS.iter().any(|row| row.method == *method))
        .collect();
    assert!(unknown.is_empty(), "cases for no row: {unknown:?}");
    let mut missing = Vec::new();
    let mut faults = Vec::new();
    for row in &ROWS {
        let Some((_, row_cases)) = cases.iter().find(|(method, _)| *method == row.method) else {
            missing.push(row.method);
            continue;
        };
        let mut seen = Seen::default();
        for case in row_cases {
            run(&mut seen, case);
        }
        let number = NUMBER_FORMS.iter().find(|(form, _)| *form == row.value);
        if let (Some((_, width)), Some(level), Some(case)) = (number, row.level, row_cases.first())
            && row.kernel == Kernel::Within
        {
            number_rooms(&mut seen, case.prepare, level, row.option, *width);
        }
        println!("PROBE {} {:?} {} {seen}", row.method, row.kernel, row.value);
        if let Some(fault) = seen.against(row.kernel, row.value) {
            faults.push(format!("{}: {fault} ({seen})", row.method));
        }
    }
    assert!(
        missing.is_empty(),
        "rows the probe does not call: {missing:?}"
    );
    assert!(
        faults.is_empty(),
        "rows whose kernel column the running kernel contradicts:\n{}",
        faults.join("\n")
    );
}
