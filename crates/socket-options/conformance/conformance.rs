//! The checks every implementation of wcontainers-socket-options passes on a Linux host, run as root.

use std::mem::offset_of;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::num::NonZero;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use rustix::net::{
    AddressFamily, Protocol, RecvFlags, SendFlags, SocketFlags, SocketType, ipproto,
};
use wcontainers_socket_options::{
    AUTHENTICATION_KEY_QUERY, MulticastLevel, PlainOption, QueuedCount, SocketOptions,
    SourceFilterOptions, StorageBytes, Value,
};

/// The length an option of any length is read with: more than any listed option gives on a fresh socket.
const ANY_READ: usize = 1 << 10;

/// A fresh socket of a kind that takes the option: raw for the IP layers, which take every option of their level there,
/// TCP for the records of the last packet, which only TCP keeps.
fn socket_for(option: PlainOption) -> Option<OwnedFd> {
    let (level, _) = option.level_and_name();
    let (family, kind, protocol) = match option {
        PlainOption::IpPktoptions => (AddressFamily::INET, SocketType::STREAM, ipproto::TCP),
        PlainOption::Ipv62292pktoptions => (AddressFamily::INET6, SocketType::STREAM, ipproto::TCP),
        _ if level == uapi::SOL_IP => (AddressFamily::INET, SocketType::RAW, ipproto::ICMP),
        _ if level == uapi::SOL_IPV6 => (AddressFamily::INET6, SocketType::RAW, ipproto::ICMPV6),
        _ if level == uapi::SOL_MPTCP => (AddressFamily::INET, SocketType::STREAM, ipproto::MPTCP),
        _ if level == wcontainers_socket_options::SOL_UDP => {
            (AddressFamily::INET6, SocketType::DGRAM, ipproto::UDP)
        }
        _ if level == wcontainers_socket_options::SOL_UDPLITE => {
            (AddressFamily::INET6, SocketType::DGRAM, ipproto::UDPLITE)
        }
        _ => (AddressFamily::INET6, SocketType::STREAM, ipproto::TCP),
    };
    rustix::net::socket(family, kind, Some(protocol)).ok()
}

fn length_of(option: PlainOption, any: usize) -> usize {
    option.value().exact_length().unwrap_or(any)
}

/// Checks that the kernel knows every listed option at its level: a read, or for an option it only sets, a write of
/// zeros, answers something other than `ENOPROTOOPT`.
///
/// An option listed at the wrong level or under the wrong name answers `ENOPROTOOPT` to both.
pub fn every_option_is_known_at_its_level<T: SocketOptions>(subject: T) {
    let mut unknown = Vec::new();
    for &option in PlainOption::ALL {
        let Some(socket) = socket_for(option) else {
            eprintln!("{option:?}: no socket of its kind on this host");
            continue;
        };
        let mut read = vec![u8::MIN; length_of(option, ANY_READ)];
        let answer = match subject.get_option(socket.as_fd(), option, &mut read) {
            Err(Errno::NOPROTOOPT) if option.value() != Value::Filled => subject.set_option(
                socket.as_fd(),
                option,
                &vec![u8::MIN; length_of(option, usize::MIN)],
            ),
            other => other.map(drop),
        };
        if answer == Err(Errno::NOPROTOOPT) {
            unknown.push(option);
        }
    }
    assert!(
        unknown.is_empty(),
        "the kernel knows none of {unknown:?} at its level"
    );
}

/// The receive low water mark the checks set, any but Linux's default of one.
///
/// https://man7.org/linux/man-pages/man7/socket.7.html
const LOW_WATER: i32 = 7;

/// Checks that an option reads back what was set.
pub fn an_option_reads_back_what_was_set<T: SocketOptions>(subject: T) {
    let socket =
        rustix::net::socket(AddressFamily::INET6, SocketType::STREAM, None).expect("a TCP socket");
    subject
        .set_option(
            socket.as_fd(),
            PlainOption::SoRcvlowat,
            &LOW_WATER.to_ne_bytes(),
        )
        .expect("the low water mark is set");
    let mut read = [u8::MIN; size_of::<i32>()];
    let length = subject
        .get_option(socket.as_fd(), PlainOption::SoRcvlowat, &mut read)
        .expect("the low water mark is read");
    assert_eq!(
        (length, i32::from_ne_bytes(read)),
        (size_of::<i32>(), LOW_WATER)
    );
}

/// Checks that a value or buffer of another length than the option's is refused, though the kernel would take it.
pub fn another_length_is_refused<T: SocketOptions>(subject: T) {
    let socket =
        rustix::net::socket(AddressFamily::INET6, SocketType::STREAM, None).expect("a TCP socket");
    let longer = [u8::MIN; size_of::<i64>()];
    assert_eq!(
        subject.set_option(socket.as_fd(), PlainOption::SoRcvlowat, &longer),
        Err(Errno::INVAL)
    );
    let mut longer = [u8::MIN; size_of::<i64>()];
    assert_eq!(
        subject.get_option(socket.as_fd(), PlainOption::SoRcvlowat, &mut longer),
        Err(Errno::INVAL)
    );
}

/// A connected loopback TCP or MPTCP pair: the connecting side and the accepted one.
fn connected(protocol: Protocol) -> Option<(OwnedFd, OwnedFd)> {
    let listener =
        rustix::net::socket(AddressFamily::INET, SocketType::STREAM, Some(protocol)).ok()?;
    rustix::net::bind(
        &listener,
        &SocketAddr::from((Ipv4Addr::LOCALHOST, u16::MIN)),
    )
    .ok()?;
    rustix::net::listen(&listener, i32::from(u8::MAX)).ok()?;
    let address = rustix::net::getsockname(&listener).ok()?;
    let connecting =
        rustix::net::socket(AddressFamily::INET, SocketType::STREAM, Some(protocol)).ok()?;
    rustix::net::connect(&connecting, &address).ok()?;
    let accepted = rustix::net::accept(&listener).ok()?;
    Some((connecting, accepted))
}

/// Checks that a connection that sent nothing has nothing queued and is not at an urgent mark.
pub fn a_quiet_connection_has_nothing_queued<T: SocketOptions>(subject: T) {
    let (connecting, _accepted) = connected(ipproto::TCP).expect("a loopback connection");
    for count in [
        QueuedCount::SendQueue,
        QueuedCount::Unsent,
        QueuedCount::AtUrgentMark,
    ] {
        assert_eq!(
            subject.count_queued(connecting.as_fd(), count),
            Ok(0),
            "{count:?}"
        );
    }
}

/// Checks that bytes written and not read are queued until acknowledged, and none are left unsent on loopback.
pub fn written_bytes_are_counted<T: SocketOptions>(subject: T) {
    let (connecting, _accepted) = connected(ipproto::TCP).expect("a loopback connection");
    let written = rustix::io::write(&connecting, b"queued").expect("the bytes are written");
    let unsent = subject
        .count_queued(connecting.as_fd(), QueuedCount::Unsent)
        .expect("the unsent count");
    let queued = subject
        .count_queued(connecting.as_fd(), QueuedCount::SendQueue)
        .expect("the queued count");
    assert!(
        unsent <= queued && queued <= i32::try_from(written).expect("a short write"),
        "{unsent} {queued}"
    );
}

/// Checks that a multipath connection's full information names its one subflow and fills the arrays.
pub fn a_multipath_connection_has_one_subflow<T: SocketOptions>(subject: T) {
    let Some((connecting, _accepted)) = connected(ipproto::MPTCP) else {
        eprintln!("no MPTCP on this host");
        return;
    };
    let mut connection = [u8::MIN; size_of::<u64>()];
    let mut subflows = [[u8::MIN; size_of::<u32>()]; 2];
    let mut information = [[u8::MIN; size_of::<u64>()]; 2];
    let counts = subject
        .get_multipath_full_information(
            connecting.as_fd(),
            &mut connection,
            &mut subflows,
            &mut information,
        )
        .expect("the full information");
    assert_eq!(counts.subflows, 1);
    assert!(counts.tcp_info_size > u32::MIN, "{counts:?}");
    assert_eq!(
        information[1],
        [u8::MIN; size_of::<u64>()],
        "only the one subflow is written"
    );
}

/// The longest value the guard check reads every option with.
pub const LONGEST_TRIED: usize = 1 << 10;
/// The bytes past a value, and the bytes of the decoy, that must keep their mark.
const GUARDED: usize = 1 << 12;
/// What every guarded byte holds.
const MARK: u8 = 0xA5;
/// The counts planted in values: the smallest that is more than one, more than any value tried holds, a page apart
/// and so a page-aligned address, and the largest.
const COUNTS: [u64; 4] = [2, 1 << 6, 1 << 16, u64::MAX];
/// The count that fills the second background of single planted fields.
const PAGE_COUNT: u64 = COUNTS[2];
/// A value of an option of exact length up to this long is also tried with every pair of planted fields.
const PAIRS_UP_TO: usize = 1 << 9;
/// The bytes a connected kind keeps queued, less than a page.
const QUEUED: usize = 1 << 4;
/// How many failures of one option on one kind are written out.
const FAILURES_SHOWN: usize = 3;

/// A field planted in a value at an offset.
#[derive(Clone, Copy, Debug)]
enum Field {
    Count32(u64),
    Count64(u64),
    /// The address of the decoy, a marked buffer the kernel must never write.
    Decoy,
    /// One byte with a single bit set, as a flag.
    Flag(u8),
}

impl Field {
    const fn width(self) -> usize {
        match self {
            Field::Count32(_) => size_of::<u32>(),
            Field::Count64(_) | Field::Decoy => size_of::<u64>(),
            Field::Flag(_) => size_of::<u8>(),
        }
    }

    fn plant(self, value: &mut [u8], at: usize, decoy: u64) {
        let range = at..at + self.width();
        match self {
            Field::Count32(count) => value[range].copy_from_slice(&(count as u32).to_ne_bytes()),
            Field::Count64(count) => value[range].copy_from_slice(&count.to_ne_bytes()),
            Field::Decoy => value[range].copy_from_slice(&decoy.to_ne_bytes()),
            Field::Flag(bit) => value[range].copy_from_slice(&[bit]),
        }
    }
}

/// Every place a field of each kind fits in a value of the length: counts and the decoy's address at every 4-byte
/// offset, and, when `flags`, every single bit of every byte; the flags come last.
fn placements(length: usize, flags: bool) -> Vec<(usize, Field)> {
    let mut placements = Vec::new();
    for at in (usize::MIN..length).step_by(size_of::<u32>()) {
        for count in COUNTS {
            placements.push((at, Field::Count32(count)));
            placements.push((at, Field::Count64(count)));
        }
        placements.push((at, Field::Decoy));
    }
    if flags {
        for at in usize::MIN..length {
            for bit in usize::MIN..u8::BITS as usize {
                placements.push((at, Field::Flag(1 << bit)));
            }
        }
    }
    placements.retain(|&(at, field)| at + field.width() <= length);
    placements
}

/// What a value is made on before fields are planted in it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Background {
    Zeros,
    Ones,
    /// The count in every 4-byte place.
    Count32(u64),
    /// The count in every 8-byte place.
    Count64(u64),
    /// The decoy's address in every 8-byte place.
    Decoy,
}

/// Every background, with its bytes up to `LONGEST_TRIED`.
fn backgrounds(decoy: u64) -> Vec<(Background, Vec<u8>)> {
    let fill = |word: &[u8]| word.iter().copied().cycle().take(LONGEST_TRIED).collect();
    let mut backgrounds = vec![
        (Background::Zeros, vec![u8::MIN; LONGEST_TRIED]),
        (Background::Ones, vec![u8::MAX; LONGEST_TRIED]),
    ];
    for count in COUNTS {
        backgrounds.push((Background::Count32(count), fill(&(count as u32).to_ne_bytes())));
        backgrounds.push((Background::Count64(count), fill(&count.to_ne_bytes())));
    }
    backgrounds.push((Background::Decoy, fill(&decoy.to_ne_bytes())));
    backgrounds
}

/// Calls `read` with every value of the length: each background; each planted field on zeros and on the page count;
/// and, when `pairs`, each pair of fields that do not overlap, on zeros, at least one of them not a flag.
fn each_value(
    length: usize,
    pairs: bool,
    backgrounds: &[(Background, Vec<u8>)],
    mut read: impl FnMut(Background, &[u8], &[(usize, Field)]),
) {
    for (background, bytes) in backgrounds {
        read(*background, &bytes[..length], &[]);
    }
    let fields = placements(length, pairs);
    let on = |wanted: Background| {
        backgrounds.iter().find(|(background, _)| *background == wanted).map(|(_, bytes)| &bytes[..length])
    };
    for background in [Background::Zeros, Background::Count32(PAGE_COUNT)] {
        let Some(bytes) = on(background) else { continue };
        for &field in &fields {
            read(background, bytes, &[field]);
        }
    }
    let Some(zeros) = on(Background::Zeros).filter(|_| pairs) else { return };
    for (index, &first) in fields.iter().enumerate() {
        if matches!(first.1, Field::Flag(_)) {
            break;
        }
        for &second in &fields[index + 1..] {
            let apart = first.0 + first.1.width() <= second.0 || second.0 + second.1.width() <= first.0;
            if apart {
                read(Background::Zeros, zeros, &[first, second]);
            }
        }
    }
}

/// The kinds of socket the guard check reads every option on, each in a state that gives the kernel something to
/// copy: authentication keys, queued bytes, a saved SYN, IP and hop-by-hop options, a group's source filter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GuardedKind {
    TcpV4WithKeys,
    TcpV6WithKeys,
    TcpV4Connected,
    TcpV6Connected,
    MultipathConnected,
    UdpV4InGroup,
    UdpV6InGroup,
    UdpLiteV6,
    RawIcmpV4,
    RawIcmpV6,
}

impl GuardedKind {
    pub const ALL: [GuardedKind; 10] = [
        GuardedKind::TcpV4WithKeys,
        GuardedKind::TcpV6WithKeys,
        GuardedKind::TcpV4Connected,
        GuardedKind::TcpV6Connected,
        GuardedKind::MultipathConnected,
        GuardedKind::UdpV4InGroup,
        GuardedKind::UdpV6InGroup,
        GuardedKind::UdpLiteV6,
        GuardedKind::RawIcmpV4,
        GuardedKind::RawIcmpV6,
    ];

    /// The kinds every Linux host the suite runs on must give.
    const REQUIRED: [GuardedKind; 5] = [
        GuardedKind::TcpV4WithKeys,
        GuardedKind::TcpV6WithKeys,
        GuardedKind::TcpV4Connected,
        GuardedKind::TcpV6Connected,
        GuardedKind::UdpV4InGroup,
    ];
}

/// A socket of a guarded kind, and the peer that keeps bytes queued on it.
struct Guarded {
    socket: OwnedFd,
    peer: Option<OwnedFd>,
}

impl Guarded {
    /// Leaves `QUEUED` bytes, and only those, waiting on a connected socket: bytes unlike the mark, so that a copy of
    /// them into the decoy shows.
    fn refill(&self) {
        let Some(peer) = &self.peer else { return };
        let mut sink = [u8::MIN; GUARDED];
        while rustix::net::recv(&self.socket, &mut sink, RecvFlags::DONTWAIT).is_ok_and(|(read, _)| read > usize::MIN)
        {}
        let _ = rustix::net::send(peer, &[!MARK; QUEUED], SendFlags::DONTWAIT);
    }
}

/// The name of the key algorithm the keyed kinds add.
const KEY_ALGORITHM: &[u8] = b"hmac(sha256)";
/// The keys a keyed kind holds: enough that a read of all of them overruns any one `tcp_ao_getsockopt`.
const KEYS_HELD: u8 = 3;
/// The length of each key.
const KEY_LENGTH: u8 = 16;
/// The source-specific group the group kinds join over IPv4, with `SOURCES` sources.
const GROUP_V4: Ipv4Addr = Ipv4Addr::new(232, 1, 2, 3);
/// The group the group kinds join over IPv6.
const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff3e, u16::MIN, u16::MIN, u16::MIN, u16::MIN, u16::MIN, u16::MIN, 0x8003);
/// The group the source filter check joins, apart from the guard check's.
const FILTERED_GROUP: Ipv4Addr = Ipv4Addr::new(232, 1, 2, 4);
/// How many sources a group kind's filter holds: more than one `group_filter` has room for.
const SOURCES: u8 = 6;
/// Four no-operation IP options.
const IP_OPTIONS: [u8; 4] = [uapi::IPOPT_NOOP as u8; 4];
/// An IPv6 hop-by-hop header holding one PadN option of four bytes.
const HOP_BY_HOP: [u8; 8] = [0, 0, 1, 4, 0, 0, 0, 0];
/// An IPv6 routing header of type 2, the one Mobile IPv6 sets, with one segment left: the loopback address.
const ROUTING_TYPE_2: [u8; 24] = [0, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
/// A fast open key.
const FAST_OPEN_KEY: [u8; 16] = [7; 16];
/// A congestion control whose `TCP_CC_INFO` has fields, unlike cubic's.
const CONGESTION_WITH_INFORMATION: &str = "vegas";

/// The network the group kinds' sources are numbered in.
const SOURCE_NETWORK_V4: Ipv4Addr = Ipv4Addr::new(10, 0, 0, 0);
const SOURCE_NETWORK_V6: Ipv6Addr = Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 0);
/// Where Linux gives the loopback interface's index.
const LOOPBACK_INDEX_FILE: &str = "/sys/class/net/lo/ifindex";
/// Where Linux lists the sources of every IPv4 source filter, each group in hexadecimal.
const SOURCE_FILTERS_FILE: &str = "/proc/net/mcfilter";

fn source_v4(last: u8) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(SOURCE_NETWORK_V4) | u32::from(last))
}

fn source_v6(last: u8) -> Ipv6Addr {
    Ipv6Addr::from(u128::from(SOURCE_NETWORK_V6) | u128::from(last))
}

/// The index of the loopback interface.
fn loopback() -> Option<u32> {
    std::fs::read_to_string(LOOPBACK_INDEX_FILE).ok()?.trim().parse().ok()
}

/// A socket address in the kernel's storage form, as bytes.
fn storage(address: IpAddr) -> StorageBytes {
    let mut storage = [u8::MIN; size_of::<StorageBytes>()];
    match address {
        IpAddr::V4(v4) => {
            storage[..size_of::<u16>()].copy_from_slice(&(uapi::AF_INET as u16).to_ne_bytes());
            let at = offset_of!(uapi::sockaddr_in, sin_addr);
            storage[at..at + size_of::<Ipv4Addr>()].copy_from_slice(&v4.octets());
        }
        IpAddr::V6(v6) => {
            storage[..size_of::<u16>()].copy_from_slice(&(uapi::AF_INET6 as u16).to_ne_bytes());
            let at = offset_of!(uapi::sockaddr_in6, sin6_addr);
            storage[at..at + size_of::<Ipv6Addr>()].copy_from_slice(&v6.octets());
        }
    }
    storage
}

/// Adds `KEYS_HELD` authentication keys for the loopback address, numbered from one.
fn add_keys<T: SocketOptions>(subject: &T, socket: &OwnedFd, loopback: IpAddr) -> Result<(), Errno> {
    for id in 1..=KEYS_HELD {
        let mut add = vec![u8::MIN; size_of::<uapi::tcp_ao_add>()];
        add[..size_of::<StorageBytes>()].copy_from_slice(&storage(loopback));
        let at = offset_of!(uapi::tcp_ao_add, alg_name);
        add[at..at + KEY_ALGORITHM.len()].copy_from_slice(KEY_ALGORITHM);
        let prefix = match loopback {
            IpAddr::V4(_) => Ipv4Addr::BITS,
            IpAddr::V6(_) => Ipv6Addr::BITS,
        };
        add[offset_of!(uapi::tcp_ao_add, prefix)] = prefix as u8;
        add[offset_of!(uapi::tcp_ao_add, sndid)] = id;
        add[offset_of!(uapi::tcp_ao_add, rcvid)] = id;
        add[offset_of!(uapi::tcp_ao_add, keylen)] = KEY_LENGTH;
        let at = offset_of!(uapi::tcp_ao_add, key);
        add[at..at + usize::from(KEY_LENGTH)].fill(id);
        subject.set_option(socket.as_fd(), PlainOption::TcpAoAddKey, &add)?;
    }
    Ok(())
}

/// A loopback connection over the family: the accepted side and the connecting side, `listening` given the listener.
fn connected_pair(
    family: AddressFamily,
    protocol: Protocol,
    listening: impl Fn(&OwnedFd),
) -> Option<(OwnedFd, OwnedFd)> {
    let loopback = if family == AddressFamily::INET6 {
        IpAddr::V6(Ipv6Addr::LOCALHOST)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };
    let listener = rustix::net::socket(family, SocketType::STREAM, Some(protocol)).ok()?;
    listening(&listener);
    rustix::net::bind(&listener, &SocketAddr::new(loopback, u16::MIN)).ok()?;
    rustix::net::listen(&listener, i32::from(u8::MAX)).ok()?;
    let address = rustix::net::getsockname(&listener).ok()?;
    let connecting = rustix::net::socket(family, SocketType::STREAM, Some(protocol)).ok()?;
    rustix::net::connect(&connecting, &address).ok()?;
    let accepted = rustix::net::accept(&listener).ok()?;
    Some((accepted, connecting))
}

/// A socket of the kind in its state, or None when this host cannot make one.
fn guarded<T: SocketOptions>(subject: &T, kind: GuardedKind) -> Option<Guarded> {
    let plain = |family, kind, protocol| rustix::net::socket(family, kind, Some(protocol)).ok();
    let alone = |socket: OwnedFd| Guarded { socket, peer: None };
    let save_syn = |socket: &OwnedFd| {
        let _ = subject.set_option(socket.as_fd(), PlainOption::TcpSaveSyn, &1i32.to_ne_bytes());
    };
    let connected = |family, protocol, prepare: &dyn Fn(&OwnedFd)| {
        let (socket, peer) = connected_pair(family, protocol, save_syn)?;
        prepare(&socket);
        let guarded = Guarded { socket, peer: Some(peer) };
        guarded.refill();
        Some(guarded)
    };
    let in_group = |family, protocol, group: IpAddr, level| {
        let socket = plain(family, SocketType::DGRAM, protocol)?;
        let interface = loopback()?;
        let mut join = vec![u8::MIN; size_of::<uapi::group_req>()];
        join[..size_of::<u32>()].copy_from_slice(&interface.to_ne_bytes());
        let at = offset_of!(uapi::group_req, gr_group);
        join[at..at + size_of::<StorageBytes>()].copy_from_slice(&storage(group));
        let option = match level {
            MulticastLevel::Ipv4 => PlainOption::IpMcastJoinGroup,
            MulticastLevel::Ipv6 => PlainOption::Ipv6McastJoinGroup,
        };
        subject.set_option(socket.as_fd(), option, &join).ok()?;
        let sources: Vec<StorageBytes> = (1..=SOURCES)
            .map(|last| match group {
                IpAddr::V4(_) => storage(IpAddr::V4(source_v4(last))),
                IpAddr::V6(_) => storage(IpAddr::V6(source_v6(last))),
            })
            .collect();
        let filter = SourceFilterOptions {
            level,
            interface,
            group: storage(group),
            mode: uapi::MCAST_INCLUDE,
            sources: &sources,
        };
        subject.set_source_filter(socket.as_fd(), &filter).ok()?;
        Some(alone(socket))
    };
    match kind {
        GuardedKind::TcpV4WithKeys | GuardedKind::TcpV6WithKeys => {
            let (family, loopback) = if kind == GuardedKind::TcpV4WithKeys {
                (AddressFamily::INET, IpAddr::V4(Ipv4Addr::LOCALHOST))
            } else {
                (AddressFamily::INET6, IpAddr::V6(Ipv6Addr::LOCALHOST))
            };
            let socket = plain(family, SocketType::STREAM, ipproto::TCP)?;
            add_keys(subject, &socket, loopback).ok()?;
            let _ = subject.set_option(socket.as_fd(), PlainOption::TcpFastopenKey, &FAST_OPEN_KEY);
            Some(alone(socket))
        }
        GuardedKind::TcpV4Connected => connected(AddressFamily::INET, ipproto::TCP, &|socket| {
            let _ = subject.set_option(socket.as_fd(), PlainOption::IpOptions, &IP_OPTIONS);
            let _ = nix::sys::socket::setsockopt(socket, nix::sys::socket::sockopt::Ipv4PacketInfo, &true);
            let _ = rustix::net::sockopt::set_tcp_congestion(socket, CONGESTION_WITH_INFORMATION);
        }),
        GuardedKind::TcpV6Connected => connected(AddressFamily::INET6, ipproto::TCP, &|socket| {
            let _ = subject.set_option(socket.as_fd(), PlainOption::Ipv6Hopopts, &HOP_BY_HOP);
            let _ = subject.set_option(socket.as_fd(), PlainOption::Ipv6Dstopts, &HOP_BY_HOP);
            let _ = subject.set_option(socket.as_fd(), PlainOption::Ipv6Rthdrdstopts, &HOP_BY_HOP);
            let _ = subject.set_option(socket.as_fd(), PlainOption::Ipv6Rthdr, &ROUTING_TYPE_2);
            let _ = nix::sys::socket::setsockopt(socket, nix::sys::socket::sockopt::Ipv6RecvPacketInfo, &true);
        }),
        GuardedKind::MultipathConnected => connected(AddressFamily::INET, ipproto::MPTCP, &|_| {}),
        GuardedKind::UdpV4InGroup => {
            in_group(AddressFamily::INET, ipproto::UDP, IpAddr::V4(GROUP_V4), MulticastLevel::Ipv4)
        }
        GuardedKind::UdpV6InGroup => {
            in_group(AddressFamily::INET6, ipproto::UDP, IpAddr::V6(GROUP_V6), MulticastLevel::Ipv6)
        }
        GuardedKind::UdpLiteV6 => plain(AddressFamily::INET6, SocketType::DGRAM, ipproto::UDPLITE).map(alone),
        GuardedKind::RawIcmpV4 => plain(AddressFamily::INET, SocketType::RAW, ipproto::ICMP).map(alone),
        GuardedKind::RawIcmpV6 => plain(AddressFamily::INET6, SocketType::RAW, ipproto::ICMPV6).map(alone),
    }
}

/// What the kernel gives for an option on the kind of socket set up for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fills {
    /// A read on the kind answers with at least one byte: the state in which the kernel fills the option.
    On(GuardedKind),
    /// The option is only set; a read on the kind answers an error, as the kernel has no read of it, or none without
    /// a request or a repair mode the primitive never gives it.
    ReadFails(GuardedKind),
}

/// The state in which the kernel fills each option: a `match` with no catch-all, so that an entry added to the list
/// does not compile until it names one, and `the_state_that_fills_each_option` checks it.
pub const fn fills(option: PlainOption) -> Fills {
    use Fills::{On, ReadFails};
    use GuardedKind::*;
    match option {
        PlainOption::SoMaxPacingRate
        | PlainOption::SoTimestamping
        | PlainOption::SoBufLock
        | PlainOption::SoRcvlowat
        | PlainOption::SoTxrehash
        | PlainOption::SoWifiStatus
        | PlainOption::SoPeekOff
        | PlainOption::SoMeminfo
        | PlainOption::SoRcvmark
        | PlainOption::SoRcvpriority
        | PlainOption::SoNoCheck
        | PlainOption::SoTxtime => On(TcpV4Connected),
        PlainOption::SoDetachReuseportBpf | PlainOption::SoCnxAdvice => ReadFails(TcpV4Connected),
        PlainOption::IpOptions
        | PlainOption::IpMinttl
        | PlainOption::IpPktoptions
        | PlainOption::IpLocalPortRange
        | PlainOption::IpUnicastIf
        | PlainOption::IpNodefrag
        | PlainOption::IpRecvopts
        | PlainOption::IpRecvfragsize => On(TcpV4Connected),
        PlainOption::IpRouterAlert => On(RawIcmpV4),
        PlainOption::IpMcastJoinGroup | PlainOption::IpMcastLeaveGroup => ReadFails(UdpV4InGroup),
        PlainOption::Ipv6Hopopts
        | PlainOption::Ipv6Rthdrdstopts
        | PlainOption::Ipv6Rthdr
        | PlainOption::Ipv6Dstopts
        | PlainOption::Ipv6FlowinfoSend
        | PlainOption::Ipv6Autoflowlabel
        | PlainOption::Ipv6AddrPreferences
        | PlainOption::Ipv6Recvhopopts
        | PlainOption::Ipv6Recvdstopts
        | PlainOption::Ipv6Recvrthdr
        | PlainOption::Ipv6Flowinfo
        | PlainOption::Ipv6Recvfragsize
        | PlainOption::Ipv6Minhopcount
        | PlainOption::Ipv62292pktoptions
        | PlainOption::Ipv6Mtu
        | PlainOption::Ipv6UnicastIf => On(TcpV6Connected),
        PlainOption::Ipv6RouterAlert | PlainOption::Ipv6RouterAlertIsolate | PlainOption::Ipv6Checksum => {
            On(RawIcmpV6)
        }
        PlainOption::Ipv6FlowlabelMgr
        | PlainOption::Ipv6JoinAnycast
        | PlainOption::Ipv6LeaveAnycast
        | PlainOption::Ipv6McastJoinGroup
        | PlainOption::Ipv6McastLeaveGroup => ReadFails(UdpV6InGroup),
        PlainOption::TcpMd5sigExt
        | PlainOption::TcpAoAddKey
        | PlainOption::TcpAoDelKey
        | PlainOption::TcpAoRepair
        | PlainOption::TcpQueueSeq
        | PlainOption::TcpRepairQueue
        | PlainOption::TcpRepairWindow
        | PlainOption::TcpRepairOptions => ReadFails(TcpV4WithKeys),
        PlainOption::TcpAoInfo | PlainOption::TcpFastopenKey => On(TcpV4WithKeys),
        PlainOption::TcpInq
        | PlainOption::TcpSavedSyn
        | PlainOption::TcpFastopen
        | PlainOption::TcpFastopenNoCookie
        | PlainOption::TcpSaveSyn
        | PlainOption::TcpTimestamp
        | PlainOption::TcpInfo
        | PlainOption::TcpCcInfo
        | PlainOption::TcpQuickack
        | PlainOption::TcpWindowClamp
        | PlainOption::TcpSyncnt
        | PlainOption::TcpDeferAccept
        | PlainOption::TcpLinger2
        | PlainOption::TcpRtoMinUs
        | PlainOption::TcpRtoMaxMs
        | PlainOption::TcpDelackMaxUs
        | PlainOption::TcpTxDelay => On(TcpV4Connected),
        PlainOption::MptcpInfo => On(MultipathConnected),
        PlainOption::UdpNoCheck6Tx | PlainOption::UdpNoCheck6Rx => On(UdpV6InGroup),
        PlainOption::UdpliteSendCscov | PlainOption::UdpliteRecvCscov => On(UdpLiteV6),
    }
}

/// Checks, against the kernel, the state `fills` names for every option: a read there gives at least one byte, or,
/// for an option only set, fails.
pub fn the_state_that_fills_each_option<T: SocketOptions>(subject: T) {
    let mut wrong = Vec::new();
    let mut skipped = Vec::new();
    for &option in PlainOption::ALL {
        let named = fills(option);
        let (Fills::On(kind) | Fills::ReadFails(kind)) = named;
        let Some(on) = guarded(&subject, kind) else {
            assert!(!GuardedKind::REQUIRED.contains(&kind), "{kind:?} could not be made");
            skipped.push(option);
            continue;
        };
        let mut read = vec![u8::MIN; length_of(option, ANY_READ)];
        let answer = subject.get_option(on.socket.as_fd(), option, &mut read);
        let right = match named {
            Fills::On(_) => matches!(answer, Ok(length) if length > usize::MIN),
            Fills::ReadFails(_) => answer.is_err(),
        };
        if !right {
            wrong.push(format!("{option:?} {named:?}: {answer:?}"));
        }
    }
    eprintln!(
        "{} options, each in its state; not made on this host: {skipped:?}; wrong: {wrong:#?}",
        PlainOption::ALL.len()
    );
    assert!(wrong.is_empty(), "options not filled where `fills` says: {wrong:#?}");
}

/// Reads the option into the front of a marked buffer with every length up to `LONGEST_TRIED` and every value
/// `each_value` makes, and returns how many reads it made and what went outside the front, or into the decoy.
fn guard_option<T: SocketOptions>(
    subject: &T,
    kind: GuardedKind,
    on: &Guarded,
    option: PlainOption,
) -> (u64, u64, Vec<String>) {
    let mut decoy = vec![MARK; GUARDED];
    let decoy_address = decoy.as_mut_ptr().expose_provenance() as u64;
    let backgrounds = backgrounds(decoy_address);
    let ones = &backgrounds[1].1;
    let mut buffer = vec![MARK; LONGEST_TRIED + GUARDED];
    let (mut reads, mut failed, mut shown) = (u64::MIN, u64::MIN, Vec::new());
    for length in usize::MIN..=LONGEST_TRIED {
        let exact = option.value().exact_length();
        let mut read = |background: Background, value: &[u8], fields: &[(usize, Field)]| {
            buffer[..length].copy_from_slice(value);
            for &(at, field) in fields {
                field.plant(&mut buffer[..length], at, decoy_address);
            }
            let result = subject.get_option(on.socket.as_fd(), option, &mut buffer[..length]);
            reads += 1;
            let past = buffer[length..length + GUARDED].iter().filter(|&&byte| byte != MARK).count();
            let followed = decoy.iter().filter(|&&byte| byte != MARK).count();
            let longer = matches!(result, Ok(written) if written > length);
            if past > usize::MIN || followed > usize::MIN || longer {
                failed += 1;
                if shown.len() < FAILURES_SHOWN {
                    shown.push(format!(
                        "{kind:?} {option:?} into {length} bytes with {fields:?} on {background:?}: {result:?}, {past} bytes changed past the value, {followed} in the decoy",
                    ));
                }
                buffer[length..].fill(MARK);
                decoy.fill(MARK);
            }
            buffer[..length].fill(MARK);
            if result.is_ok() {
                on.refill();
            }
        };
        match exact {
            Some(exact) if exact != length => read(Background::Ones, &ones[..length], &[]),
            _ => each_value(length, exact.is_some() && length <= PAIRS_UP_TO, &backgrounds, read),
        }
    }
    (reads, failed, shown)
}

/// Checks that a read of every listed option, into the front of a marked buffer, changes no byte past the length
/// given and none of a decoy whose address the value holds, against the real kernel, run as root.
///
/// Every option is read on every `GuardedKind`, at every length from zero to `LONGEST_TRIED`, with values of zeros,
/// ones, each of `COUNTS` in every 4-byte and in every 8-byte place, and the decoy's address in every 8-byte place;
/// with each count or the decoy's address planted alone at every 4-byte offset; and, at an exact option's own length,
/// with every pair of planted counts, addresses and single-bit flags. A new entry of the list is read without anyone
/// naming it here.
pub fn no_listed_option_writes_past_its_length<T: SocketOptions + Sync>(subject: T) {
    let jobs: Vec<(GuardedKind, PlainOption)> = GuardedKind::ALL
        .iter()
        .flat_map(|&kind| PlainOption::ALL.iter().map(move |&option| (kind, option)))
        .collect();
    let next = AtomicUsize::new(usize::MIN);
    let (reads, failed) = (AtomicU64::new(u64::MIN), AtomicU64::new(u64::MIN));
    let shown = Mutex::new(Vec::new());
    let missing = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism().map_or(1, NonZero::get);
    std::thread::scope(|scope| {
        for _ in usize::MIN..workers {
            scope.spawn(|| {
                while let Some(&(kind, option)) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let Some(on) = guarded(&subject, kind) else {
                        missing.lock().unwrap().push(kind);
                        continue;
                    };
                    let (made, outside, lines) = guard_option(&subject, kind, &on, option);
                    reads.fetch_add(made, Ordering::Relaxed);
                    failed.fetch_add(outside, Ordering::Relaxed);
                    shown.lock().unwrap().extend(lines);
                }
            });
        }
    });
    let mut missing = missing.into_inner().unwrap();
    missing.sort_by_key(|kind| GuardedKind::ALL.iter().position(|other| other == kind));
    missing.dedup();
    let shown = shown.into_inner().unwrap();
    let (reads, failed) = (reads.into_inner(), failed.into_inner());
    eprintln!(
        "{} options on {} kinds of socket: {reads} reads, {failed} outside; kinds this host could not make: {missing:?}",
        PlainOption::ALL.len(),
        GuardedKind::ALL.len() - missing.len(),
    );
    for line in &shown {
        eprintln!("{line}");
    }
    assert!(
        missing.iter().all(|kind| !GuardedKind::REQUIRED.contains(kind)),
        "a kind every host gives is missing: {missing:?}"
    );
    assert_eq!(failed, u64::MIN, "the kernel wrote outside the value: {shown:#?}");
}

/// The descriptors the options and the queue counts are refused on.
#[derive(Clone, Copy, Debug)]
enum Refused {
    UnixSocket,
    NetlinkSocket,
    File,
    Terminal,
    Pipe,
}

/// Checks that the options and the queue counts reach only an IPv4 or IPv6 socket, and the counts only the
/// protocols that answer them: on anything else they are refused before the call.
pub fn only_internet_sockets_are_reached<T: SocketOptions>(subject: T) {
    let (unix, _other) =
        rustix::net::socketpair(AddressFamily::UNIX, SocketType::STREAM, SocketFlags::empty(), None)
            .expect("a Unix socket pair");
    let netlink = rustix::net::socket(AddressFamily::NETLINK, SocketType::RAW, None).expect("a netlink socket");
    let file = std::fs::File::open(std::env::current_exe().expect("the test's own file")).expect("a file");
    let terminal = rustix::pty::openpt(rustix::pty::OpenptFlags::RDWR).expect("a terminal");
    let (pipe, _writer) = std::io::pipe().expect("a pipe");
    let refused = [
        (Refused::UnixSocket, unix.as_fd(), Errno::NOPROTOOPT),
        (Refused::NetlinkSocket, netlink.as_fd(), Errno::NOPROTOOPT),
        (Refused::File, file.as_fd(), Errno::NOTSOCK),
        (Refused::Terminal, terminal.as_fd(), Errno::NOTSOCK),
        (Refused::Pipe, pipe.as_fd(), Errno::NOTSOCK),
    ];
    for (name, descriptor, error) in refused {
        let mut value = [u8::MIN; size_of::<i32>()];
        assert_eq!(subject.get_option(descriptor, PlainOption::IpMinttl, &mut value), Err(error), "{name:?}");
        assert_eq!(subject.set_option(descriptor, PlainOption::IpMinttl, &value), Err(error), "{name:?}");
        for count in [QueuedCount::SendQueue, QueuedCount::Unsent, QueuedCount::AtUrgentMark] {
            assert_eq!(subject.count_queued(descriptor, count), Err(error), "{name:?} {count:?}");
        }
    }
    for family in [AddressFamily::INET, AddressFamily::INET6] {
        let raw = rustix::net::socket(family, SocketType::RAW, Some(ipproto::TCP)).expect("a raw socket of TCP's number");
        for count in [QueuedCount::SendQueue, QueuedCount::Unsent, QueuedCount::AtUrgentMark] {
            assert_eq!(subject.count_queued(raw.as_fd(), count), Err(Errno::NOTTY), "raw {family:?} {count:?}");
        }
    }
    let udp = rustix::net::socket(AddressFamily::INET, SocketType::DGRAM, None).expect("a UDP socket");
    for count in [QueuedCount::SendQueue, QueuedCount::Unsent, QueuedCount::AtUrgentMark] {
        assert_eq!(subject.count_queued(udp.as_fd(), count), Err(Errno::NOTTY), "UDP {count:?}");
    }
    if let Some((multipath, _peer)) = connected(ipproto::MPTCP) {
        assert_eq!(
            subject.count_queued(multipath.as_fd(), QueuedCount::AtUrgentMark),
            Err(Errno::NOTTY)
        );
        assert!(subject.count_queued(multipath.as_fd(), QueuedCount::Unsent).is_ok());
    }
}

/// Checks that a key read names one key and writes only its `tcp_ao_getsockopt`, though its `nkeys` asked for more.
pub fn an_authentication_key_read_writes_one_key<T: SocketOptions>(subject: T) {
    let socket =
        rustix::net::socket(AddressFamily::INET, SocketType::STREAM, Some(ipproto::TCP)).expect("a TCP socket");
    add_keys(&subject, &socket, IpAddr::V4(Ipv4Addr::LOCALHOST)).expect("the keys are added");
    let mut buffer = vec![MARK; AUTHENTICATION_KEY_QUERY + GUARDED];
    let query = &mut buffer[..AUTHENTICATION_KEY_QUERY];
    query.fill(u8::MIN);
    query[..size_of::<StorageBytes>()].copy_from_slice(&storage(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    query[offset_of!(uapi::tcp_ao_getsockopt, prefix)] = Ipv4Addr::BITS as u8;
    query[offset_of!(uapi::tcp_ao_getsockopt, sndid)] = KEYS_HELD;
    query[offset_of!(uapi::tcp_ao_getsockopt, rcvid)] = KEYS_HELD;
    let at = offset_of!(uapi::tcp_ao_getsockopt, nkeys);
    query[at..at + size_of::<u32>()].copy_from_slice(&u32::MAX.to_ne_bytes());
    let query = <&mut [u8; AUTHENTICATION_KEY_QUERY]>::try_from(query).expect("one query");
    subject.get_authentication_key(socket.as_fd(), query).expect("the key is read");
    let found = u32::from_ne_bytes(query[at..at + size_of::<u32>()].try_into().expect("four bytes"));
    let sent = query[offset_of!(uapi::tcp_ao_getsockopt, sndid)];
    assert_eq!((found, sent), (1, KEYS_HELD));
    assert!(buffer[AUTHENTICATION_KEY_QUERY..].iter().all(|&byte| byte == MARK), "a key was written past the query");
}

/// Checks that a source filter carries every source it is given, as the kernel lists them.
pub fn a_source_filter_carries_every_source<T: SocketOptions>(subject: T) {
    let socket = rustix::net::socket(AddressFamily::INET, SocketType::DGRAM, None).expect("a UDP socket");
    let interface = loopback().expect("the loopback interface");
    let mut join = vec![u8::MIN; size_of::<uapi::group_req>()];
    join[..size_of::<u32>()].copy_from_slice(&interface.to_ne_bytes());
    let at = offset_of!(uapi::group_req, gr_group);
    join[at..at + size_of::<StorageBytes>()].copy_from_slice(&storage(IpAddr::V4(FILTERED_GROUP)));
    subject.set_option(socket.as_fd(), PlainOption::IpMcastJoinGroup, &join).expect("the group is joined");
    let sources: Vec<StorageBytes> = (1..=SOURCES)
        .map(|last| storage(IpAddr::V4(source_v4(last))))
        .collect();
    let filter = SourceFilterOptions {
        level: MulticastLevel::Ipv4,
        interface,
        group: storage(IpAddr::V4(FILTERED_GROUP)),
        mode: uapi::MCAST_INCLUDE,
        sources: &sources,
    };
    subject.set_source_filter(socket.as_fd(), &filter).expect("the filter is set");
    let listed = std::fs::read_to_string(SOURCE_FILTERS_FILE).expect("the kernel's source filters");
    let group = format!("{:x}", u32::from(FILTERED_GROUP));
    let carried = listed
        .lines()
        .filter(|line| line.split_whitespace().any(|word| word.to_lowercase().ends_with(&group)))
        .count();
    assert_eq!(carried, usize::from(SOURCES), "{listed}");
}
