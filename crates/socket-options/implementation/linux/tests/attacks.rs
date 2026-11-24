//! The attacks of the two independent checks of this primitive, kept as its own suite, run as root: safe callers
//! trying to make the kernel touch memory outside what the trait was given. Each read goes into the front of a larger
//! buffer filled with a mark; a changed byte past the front is a write the trait let through.
//!
//! The descriptor swap swaps fd 0 under the calls; `trace_marker` lines bracket the calls for an ftrace of `dev_ioctl`
//! and `tun_chr_ioctl` when one runs.
#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::io::Write;
use std::mem::{offset_of, size_of};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};

use linux_raw_sys::net as uapi;
use nix::sys::socket::{setsockopt, sockopt};
use rustix::io::Errno;
use rustix::net::{AddressFamily, Protocol, SocketFlags, SocketType, ipproto};
use wcontainers_socket_options::{
    AUTHENTICATION_KEY_QUERY, MulticastLevel, PlainOption, QueuedCount, SocketOptions, SourceFilterOptions,
    StorageBytes,
};
use wcontainers_socket_options_linux::LinuxSocketOptions as L;

const MARK: u8 = 0xA5;
const GUARD: usize = 8192;
const COUNTS: [QueuedCount; 3] = [QueuedCount::SendQueue, QueuedCount::Unsent, QueuedCount::AtUrgentMark];
/// Calls made while another thread swaps the descriptor.
const RACED_CALLS: usize = 200_000;

fn marker(text: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open("/sys/kernel/tracing/trace_marker") {
        let _ = file.write_all(text.as_bytes());
    }
}

fn past(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&byte| byte != MARK).count()
}

fn sock(family: AddressFamily, kind: SocketType, protocol: Option<Protocol>) -> Option<OwnedFd> {
    rustix::net::socket(family, kind, protocol).ok()
}

fn raw_protocol(number: u32) -> Option<Protocol> {
    Some(Protocol::from_raw(rustix::net::RawProtocol::new(number)?))
}

fn put(bytes: &mut [u8], at: usize, value: &[u8]) {
    bytes[at..at + value.len()].copy_from_slice(value);
}

/// A loopback connection: the accepted side, the connecting side, the listener.
fn pair(family: AddressFamily, protocol: Protocol) -> Option<(OwnedFd, OwnedFd, OwnedFd)> {
    let loopback = if family == AddressFamily::INET6 {
        IpAddr::V6(Ipv6Addr::LOCALHOST)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };
    let listener = sock(family, SocketType::STREAM, Some(protocol))?;
    rustix::net::bind(&listener, &SocketAddr::new(loopback, 0)).ok()?;
    rustix::net::listen(&listener, 8).ok()?;
    let at = rustix::net::getsockname(&listener).ok()?;
    let connecting = sock(family, SocketType::STREAM, Some(protocol))?;
    rustix::net::connect(&connecting, &at).ok()?;
    let accepted = rustix::net::accept(&listener).ok()?;
    Some((accepted, connecting, listener))
}

fn storage(address: IpAddr) -> StorageBytes {
    let mut storage = [0u8; size_of::<StorageBytes>()];
    match address {
        IpAddr::V4(v4) => {
            storage[..2].copy_from_slice(&(uapi::AF_INET as u16).to_ne_bytes());
            storage[4..8].copy_from_slice(&v4.octets());
        }
        IpAddr::V6(v6) => {
            storage[..2].copy_from_slice(&(uapi::AF_INET6 as u16).to_ne_bytes());
            storage[8..24].copy_from_slice(&v6.octets());
        }
    }
    storage
}

fn add_keys(socket: &OwnedFd, loopback: IpAddr, keys: u8) {
    for id in 1..=keys {
        let mut add = vec![0u8; size_of::<uapi::tcp_ao_add>()];
        add[..size_of::<StorageBytes>()].copy_from_slice(&storage(loopback));
        put(&mut add, offset_of!(uapi::tcp_ao_add, alg_name), b"hmac(sha256)");
        add[offset_of!(uapi::tcp_ao_add, prefix)] = if loopback.is_ipv4() { 32 } else { 128 };
        add[offset_of!(uapi::tcp_ao_add, sndid)] = id;
        add[offset_of!(uapi::tcp_ao_add, rcvid)] = id;
        add[offset_of!(uapi::tcp_ao_add, keylen)] = 16;
        put(&mut add, offset_of!(uapi::tcp_ao_add, key), &[id; 16]);
        L.set_option(socket.as_fd(), PlainOption::TcpAoAddKey, &add).expect("a key is added");
    }
}

fn guarded_get(socket: BorrowedFd<'_>, option: PlainOption, input: &[u8]) -> (Result<usize, Errno>, usize) {
    let mut big = vec![MARK; input.len() + GUARD];
    big[..input.len()].copy_from_slice(input);
    let result = L.get_option(socket, option, &mut big[..input.len()]);
    (result, past(&big[input.len()..]))
}

/// First check, attempt 1: `TCP_AO_GET_KEYS` asking for eight keys with `get_all`, now through the typed method.
#[test]
fn ao_get_keys_stays_in_its_buffer() {
    let socket = sock(AddressFamily::INET, SocketType::STREAM, Some(ipproto::TCP)).expect("a TCP socket");
    add_keys(&socket, IpAddr::V4(Ipv4Addr::LOCALHOST), 3);
    let length = AUTHENTICATION_KEY_QUERY;
    let mut big = vec![MARK; length + GUARD];
    big[..length].fill(0);
    put(&mut big, offset_of!(uapi::tcp_ao_getsockopt, nkeys), &8u32.to_ne_bytes());
    big[offset_of!(uapi::tcp_ao_getsockopt, _bitfield_1)] = 0b100;
    let query: &mut [u8; AUTHENTICATION_KEY_QUERY] = (&mut big[..length]).try_into().expect("one query");
    let result = L.get_authentication_key(socket.as_fd(), query);
    let changed = past(&big[length..]);
    eprintln!("TCP_AO_GET_KEYS (typed) into {length} bytes: {result:?}, {changed} bytes changed past the buffer");
    assert_eq!(changed, 0);
}

/// First check, attempt 2: `MCAST_MSFILTER` is reached only through the typed set, never as a listed option.
#[test]
fn msfilter_is_not_listed() {
    let found = [uapi::SOL_IP, uapi::SOL_IPV6].map(|level| PlainOption::find(level, uapi::MCAST_MSFILTER));
    eprintln!("MCAST_MSFILTER on the list: {found:?}");
    assert_eq!(found, [None, None]);
}

/// First check, attempt 3: every listed option read into its own length, and short lengths for the open ones.
#[test]
fn every_option_stays_in_its_buffer() {
    let kinds = [
        (AddressFamily::INET, SocketType::STREAM, ipproto::TCP),
        (AddressFamily::INET6, SocketType::STREAM, ipproto::TCP),
        (AddressFamily::INET6, SocketType::DGRAM, ipproto::UDP),
        (AddressFamily::INET6, SocketType::DGRAM, ipproto::UDPLITE),
        (AddressFamily::INET, SocketType::RAW, ipproto::ICMP),
        (AddressFamily::INET6, SocketType::RAW, ipproto::ICMPV6),
        (AddressFamily::INET, SocketType::STREAM, ipproto::MPTCP),
    ];
    let (mut bad, mut calls) = (Vec::new(), 0);
    for (family, kind, protocol) in kinds {
        let Some(socket) = sock(family, kind, Some(protocol)) else { continue };
        for &option in PlainOption::ALL {
            let lengths = match option.value().exact_length() {
                Some(length) => vec![length],
                None => vec![0, 1, 4, 8, 64, 1024],
            };
            for length in lengths {
                calls += 1;
                let (result, changed) = guarded_get(socket.as_fd(), option, &vec![0u8; length]);
                if changed != 0 || matches!(result, Ok(written) if written > length) {
                    bad.push(format!("{family:?} {protocol:?} {option:?} {length}: {result:?} past {changed}"));
                }
            }
        }
    }
    eprintln!("{calls} reads, {} outside: {bad:#?}", bad.len());
    assert!(bad.is_empty());
}

/// First check, attempt 4: the multipath read with one-byte elements, each array the front of a marked buffer.
#[test]
fn multipath_arrays_stay_in_their_buffers() {
    let Some((_accepted, socket, _listener)) = pair(AddressFamily::INET, ipproto::MPTCP) else {
        eprintln!("no MPTCP");
        return;
    };
    let mut subflows = vec![MARK; 1 + GUARD];
    let mut information = vec![MARK; 1 + GUARD];
    let mut connection = [0u8; 0];
    let result =
        L.get_multipath_full_information(socket.as_fd(), &mut connection, &mut subflows[..1], &mut information[..1]);
    let changed = past(&subflows[1..]) + past(&information[1..]);
    eprintln!("MPTCP_FULL_INFO with 1-byte elements: {result:?}, {changed} bytes changed past the arrays");
    assert_eq!(changed, 0);
}

/// First check, attempt 5: the queue counts on descriptors that are not sockets.
#[test]
fn queue_counts_refuse_what_is_not_a_socket() {
    let terminal = std::fs::OpenOptions::new().read(true).write(true).open("/dev/ptmx").expect("a terminal");
    let null = std::fs::File::open("/dev/null").expect("/dev/null");
    let file = std::fs::File::open("/proc/self/exe").expect("a file");
    let (pipe, _writer) = std::io::pipe().expect("a pipe");
    let mut taken = Vec::new();
    for (name, descriptor) in
        [("ptmx", terminal.as_fd()), ("null", null.as_fd()), ("file", file.as_fd()), ("pipe", pipe.as_fd())]
    {
        for count in COUNTS {
            let result = L.count_queued(descriptor, count);
            eprintln!("{name} {count:?}: {result:?}");
            if result.is_ok() {
                taken.push(format!("{name} {count:?}"));
            }
        }
    }
    assert!(taken.is_empty(), "a count was read from a descriptor that is not a socket: {taken:?}");
}

/// Second check (b): a raw socket made with TCP's or MPTCP's number reports that protocol; the check now also reads
/// the socket's type, so it is not taken for TCP and its counts are refused before any call.
#[test]
fn a_raw_socket_named_tcp_is_not_taken_for_tcp() {
    let control = pair(AddressFamily::INET, ipproto::TCP).expect("a TCP connection");
    let kinds: Vec<(&str, Option<OwnedFd>)> = vec![
        ("TCP-control", Some(control.1)),
        ("RAWv4-proto6", sock(AddressFamily::INET, SocketType::RAW, Some(ipproto::TCP))),
        ("RAWv6-proto6", sock(AddressFamily::INET6, SocketType::RAW, Some(ipproto::TCP))),
        ("RAWv4-proto262", sock(AddressFamily::INET, SocketType::RAW, raw_protocol(262))),
        ("RAWv6-proto262", sock(AddressFamily::INET6, SocketType::RAW, raw_protocol(262))),
    ];
    let mut reached = Vec::new();
    for (name, descriptor) in kinds {
        let Some(descriptor) = descriptor else {
            eprintln!("{name}: cannot make");
            continue;
        };
        let protocol = rustix::net::sockopt::socket_protocol(&descriptor);
        for count in COUNTS {
            marker(&format!("BEGIN {name} {count:?}\n"));
            let result = L.count_queued(descriptor.as_fd(), count);
            marker(&format!("END {name} {count:?}\n"));
            eprintln!("{name} (SO_PROTOCOL {protocol:?}) {count:?}: {result:?}");
            if name != "TCP-control" && result != Err(Errno::NOTTY) {
                reached.push(format!("{name} {count:?}: {result:?}"));
            }
        }
    }
    assert!(reached.is_empty(), "a raw socket was counted as TCP: {reached:?}");
}

/// Second check (d): the descriptor swapped after the check. One thread swaps fd 0 between a TCP socket and something
/// else with the safe `dup2_stdin`; this one calls the trait on fd 0. The check cannot stop it; the scratch does:
/// whatever the call reaches, the kernel's copy from the four bytes it is given fails with `EFAULT` past them.
#[test]
fn a_descriptor_swapped_after_the_check_is_contained() {
    let stdin = std::io::stdin();
    let saved = rustix::io::dup(&stdin).expect("stdin is kept");
    let tcp = pair(AddressFamily::INET, ipproto::TCP).expect("a TCP connection");
    let (pipe, _writer) = std::io::pipe().expect("a pipe");
    let tun = std::fs::OpenOptions::new().read(true).write(true).open("/dev/net/tun").ok();
    let (unix, _other) =
        rustix::net::socketpair(AddressFamily::UNIX, SocketType::STREAM, SocketFlags::empty(), None)
            .expect("a Unix socket pair");
    let udp = sock(AddressFamily::INET, SocketType::DGRAM, Some(ipproto::UDP)).expect("a UDP socket");
    let terminal = std::fs::OpenOptions::new().read(true).write(true).open("/dev/ptmx").expect("a terminal");
    // The case, the other descriptor, the count or none for a read of `TCP_INFO`, and an answer that would mean the
    // kernel read past the four bytes it was given.
    let mut cases: Vec<(&str, BorrowedFd<'_>, Option<QueuedCount>, Option<Errno>)> = vec![
        ("SendQueue: TCP <-> pipe", pipe.as_fd(), Some(QueuedCount::SendQueue), None),
        ("TcpInfo get: TCP <-> unix socket", unix.as_fd(), None, None),
        ("Unsent: TCP <-> UDP", udp.as_fd(), Some(QueuedCount::Unsent), None),
        ("AtUrgentMark: TCP <-> UDP", udp.as_fd(), Some(QueuedCount::AtUrgentMark), None),
        ("SendQueue: TCP <-> terminal", terminal.as_fd(), Some(QueuedCount::SendQueue), None),
    ];
    if let Some(tun) = &tun {
        // tun answers EBADFD only after it has copied a 40-byte `ifreq` from the argument (drivers/net/tun.c).
        cases.push(("Unsent: TCP <-> /dev/net/tun", tun.as_fd(), Some(QueuedCount::Unsent), Some(Errno::BADFD)));
    }
    let mut leaked = Vec::new();
    for (name, other, count, overrun) in cases {
        let stop = AtomicBool::new(false);
        let mut tally: BTreeMap<String, usize> = BTreeMap::new();
        let mut information = vec![MARK; 1024 + GUARD];
        marker(&format!("BEGIN race {name}\n"));
        std::thread::scope(|scope| {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let _ = rustix::stdio::dup2_stdin(&tcp.1);
                    let _ = rustix::stdio::dup2_stdin(other);
                }
            });
            for _ in 0..RACED_CALLS {
                let result = match count {
                    Some(count) => L.count_queued(stdin.as_fd(), count).map(drop),
                    None => L.get_option(stdin.as_fd(), PlainOption::TcpInfo, &mut information[..1024]).map(drop),
                };
                if overrun.is_some() && result == Err(overrun.unwrap_or(Errno::IO)) {
                    leaked.push(name);
                }
                *tally.entry(format!("{result:?}")).or_default() += 1;
            }
            stop.store(true, Ordering::Relaxed);
        });
        marker(&format!("END race {name}\n"));
        let changed = past(&information[1024..]);
        eprintln!("race {name}: {tally:?}; {changed} bytes changed past the information read");
        assert_eq!(changed, 0);
    }
    rustix::stdio::dup2_stdin(&saved).expect("stdin is put back");
    leaked.dedup();
    assert!(leaked.is_empty(), "the kernel read past the argument in {leaked:?}");
}

/// Second check (d): every kind of descriptor this host can make, against every method.
#[test]
fn every_descriptor_kind_against_every_method() {
    use AddressFamily as F;
    use SocketType as T;
    let (pipe_read, pipe_write) = std::io::pipe().expect("a pipe");
    let mut kinds: Vec<(String, OwnedFd)> = Vec::new();
    let mut add = |name: &str, descriptor: Option<OwnedFd>| match descriptor {
        Some(descriptor) => kinds.push((name.to_string(), descriptor)),
        None => eprintln!("{name}: cannot make"),
    };
    add("file", std::fs::File::open("/proc/self/exe").ok().map(OwnedFd::from));
    add("/dev/null", std::fs::File::open("/dev/null").ok().map(OwnedFd::from));
    add("pipe-read", Some(OwnedFd::from(pipe_read)));
    add("pipe-write", Some(OwnedFd::from(pipe_write)));
    add("pty-master", rustix::pty::openpt(rustix::pty::OpenptFlags::RDWR).ok());
    add("eventfd", rustix::event::eventfd(0, rustix::event::EventfdFlags::empty()).ok());
    add("epoll", rustix::event::epoll::create(rustix::event::epoll::CreateFlags::empty()).ok());
    add("memfd", rustix::fs::memfd_create("x", rustix::fs::MemfdFlags::empty()).ok());
    add(
        "/dev/net/tun",
        std::fs::OpenOptions::new().read(true).write(true).open("/dev/net/tun").ok().map(OwnedFd::from),
    );
    add("unix-stream", sock(F::UNIX, T::STREAM, None));
    add("unix-dgram", sock(F::UNIX, T::DGRAM, None));
    add("unix-seqpacket", sock(F::UNIX, T::SEQPACKET, None));
    add("netlink", sock(F::NETLINK, T::RAW, None));
    add("packet-raw", sock(F::PACKET, T::RAW, None));
    add("packet-dgram", sock(F::PACKET, T::DGRAM, None));
    add("alg", sock(F::from_raw(38), T::SEQPACKET, None));
    add("vsock", sock(F::VSOCK, T::STREAM, None));
    add("inet-sctp", sock(F::INET, T::STREAM, raw_protocol(132)));
    add("inet-l2tp", sock(F::INET, T::DGRAM, raw_protocol(115)));
    add("inet-ping", sock(F::INET, T::DGRAM, Some(ipproto::ICMP)));
    add("inet-raw-udp", sock(F::INET, T::RAW, Some(ipproto::UDP)));
    add("inet-raw-tcp", sock(F::INET, T::RAW, Some(ipproto::TCP)));
    add("inet6-raw-tcp", sock(F::INET6, T::RAW, Some(ipproto::TCP)));
    add("inet6-udplite", sock(F::INET6, T::DGRAM, Some(ipproto::UDPLITE)));
    let refused = |result: &Result<(), Errno>| {
        matches!(result, Err(error) if *error == Errno::NOTSOCK || *error == Errno::NOPROTOOPT)
    };
    let mut reached = Vec::new();
    for (name, descriptor) in &kinds {
        let descriptor = descriptor.as_fd();
        let mut results: Vec<(String, Result<(), Errno>)> = Vec::new();
        results.push(("set IpMinttl".into(), L.set_option(descriptor, PlainOption::IpMinttl, &64i32.to_ne_bytes())));
        results.push(("get IpMinttl".into(), L.get_option(descriptor, PlainOption::IpMinttl, &mut [0; 4]).map(drop)));
        let mut buffer = vec![MARK; 256 + GUARD];
        let result = L.get_option(descriptor, PlainOption::TcpInfo, &mut buffer[..256]).map(drop);
        assert_eq!(past(&buffer[256..]), 0, "{name} TcpInfo wrote past");
        results.push(("get TcpInfo".into(), result));
        for count in COUNTS {
            results.push((format!("count {count:?}"), L.count_queued(descriptor, count).map(drop)));
        }
        let mut query = [0u8; AUTHENTICATION_KEY_QUERY];
        results.push(("auth key".into(), L.get_authentication_key(descriptor, &mut query)));
        let filter = SourceFilterOptions {
            level: MulticastLevel::Ipv4,
            interface: 1,
            group: storage(IpAddr::V4(Ipv4Addr::new(232, 9, 9, 9))),
            mode: uapi::MCAST_INCLUDE,
            sources: &[],
        };
        results.push(("source filter".into(), L.set_source_filter(descriptor, &filter)));
        let (mut subflows, mut information, mut connection) = ([0u8; 8], [0u8; 8], [0u8; 8]);
        let result =
            L.get_multipath_full_information(descriptor, &mut connection, &mut subflows[..], &mut information[..]);
        results.push(("multipath".into(), result.map(drop)));
        let not_refused: Vec<_> = results.iter().filter(|(_, result)| !refused(result)).collect();
        if not_refused.is_empty() {
            eprintln!("{name}: all {} refused ({:?})", results.len(), results[0].1);
        } else {
            eprintln!("{name}: NOT refused: {not_refused:?}");
            reached.push(name.clone());
        }
    }
    eprintln!("kinds tried {}, kinds that reached a call: {reached:?}", kinds.len());
}

/// Second check (c): the key read with every single bit of the query set, alone and with `get_all`, and several
/// `nkeys`, on a socket holding five keys; the query is the front of a marked buffer.
#[test]
fn the_key_read_stays_in_its_query() {
    let length = AUTHENTICATION_KEY_QUERY;
    let (mut bad, mut calls) = (Vec::new(), 0);
    for (family, loopback) in
        [(AddressFamily::INET, IpAddr::V4(Ipv4Addr::LOCALHOST)), (AddressFamily::INET6, IpAddr::V6(Ipv6Addr::LOCALHOST))]
    {
        let socket = sock(family, SocketType::STREAM, Some(ipproto::TCP)).expect("a TCP socket");
        add_keys(&socket, loopback, 5);
        let flags = offset_of!(uapi::tcp_ao_getsockopt, _bitfield_1);
        let nkeys = offset_of!(uapi::tcp_ao_getsockopt, nkeys);
        for get_all in [false, true] {
            for bit in 0..=length * 8 {
                for asked in [0u32, 1, 2, 8, u32::MAX] {
                    let mut buffer = vec![MARK; length + GUARD];
                    buffer[..length].fill(0);
                    if get_all {
                        buffer[flags] |= 0b100;
                    }
                    if bit < length * 8 {
                        buffer[bit / 8] ^= 1 << (bit % 8);
                    }
                    put(&mut buffer, nkeys, &asked.to_ne_bytes());
                    let query: &mut [u8; AUTHENTICATION_KEY_QUERY] =
                        (&mut buffer[..length]).try_into().expect("one query");
                    let result = L.get_authentication_key(socket.as_fd(), query);
                    calls += 1;
                    let changed = past(&buffer[length..]);
                    if changed > 0 {
                        bad.push(format!("{family:?} get_all={get_all} bit {bit} nkeys {asked}: {result:?}, {changed} past"));
                    }
                }
            }
        }
    }
    eprintln!("auth key reads {calls}, outside {}: {:?}", bad.len(), bad.iter().take(3).collect::<Vec<_>>());
    assert!(bad.is_empty());
}

/// Reads `MPTCP_FULL_INFO` into arrays of `SUBFLOW`- and `INFORMATION`-byte elements, each the front of a marked
/// buffer, and gives how many bytes changed past them.
fn full<const SUBFLOW: usize, const INFORMATION: usize, const CONNECTION: usize>(
    socket: BorrowedFd<'_>,
    subflows: usize,
    information: usize,
) -> usize {
    let mut subflow_array = vec![[MARK; SUBFLOW]; subflows + 1 + GUARD / (SUBFLOW + 1)];
    let mut information_array = vec![[MARK; INFORMATION]; information + 1 + GUARD / (INFORMATION + 1)];
    let mut connection = [MARK; CONNECTION];
    let result = L.get_multipath_full_information(
        socket,
        &mut connection,
        &mut subflow_array[..subflows],
        &mut information_array[..information],
    );
    let changed = past(subflow_array[subflows..].as_flattened()) + past(information_array[information..].as_flattened());
    eprintln!(
        "MPTCP_FULL_INFO subflow {SUBFLOW}B x{subflows} info {INFORMATION}B x{information} conn {CONNECTION}: {result:?}, \
         {changed} past the arrays"
    );
    changed
}

/// Second check (c): the multipath read with odd element sizes, zero-sized elements, empty and unequal arrays.
#[test]
fn the_multipath_read_stays_in_its_arrays() {
    let Some((accepted, connecting, _listener)) = pair(AddressFamily::INET, ipproto::MPTCP) else {
        eprintln!("no MPTCP");
        return;
    };
    let mut total = 0;
    for socket in [accepted.as_fd(), connecting.as_fd()] {
        total += full::<1, 1, 1>(socket, 1, 1);
        total += full::<3, 3, 0>(socket, 2, 2);
        total += full::<0, 300, 4096>(socket, 1, 1);
        total += full::<300, 0, 7>(socket, 1, 1);
        total += full::<4096, 4096, 300>(socket, 1, 1);
        total += full::<1, 1, 1>(socket, 0, 5);
        total += full::<1, 1, 1>(socket, 5, 0);
        total += full::<0, 0, 0>(socket, 0, 0);
        let mut zero_sized = [[0u8; 0]; usize::MAX];
        let mut information = vec![[MARK; 300]; 2 + GUARD / 300];
        let mut connection = [MARK; 8];
        let result =
            L.get_multipath_full_information(socket, &mut connection, &mut zero_sized[..], &mut information[..1]);
        let changed = past(information[1..].as_flattened());
        eprintln!("MPTCP_FULL_INFO zero-sized subflows x usize::MAX, info 300B x1: {result:?}, {changed} past");
        total += changed;
    }
    assert_eq!(total, 0);
}

/// Second check (c): the source filter set with no, many and too many sources, a wrong level and a wrong mode.
#[test]
fn the_source_filter_set_with_odd_inputs() {
    let v4 = sock(AddressFamily::INET, SocketType::DGRAM, Some(ipproto::UDP)).expect("a UDP socket");
    let v6 = sock(AddressFamily::INET6, SocketType::DGRAM, Some(ipproto::UDP)).expect("a UDP socket");
    let group = storage(IpAddr::V4(Ipv4Addr::new(232, 7, 7, 7)));
    let mut join = vec![0u8; size_of::<uapi::group_req>()];
    put(&mut join, 0, &1u32.to_ne_bytes());
    put(&mut join, offset_of!(uapi::group_req, gr_group), &group);
    let _ = L.set_option(v4.as_fd(), PlainOption::IpMcastJoinGroup, &join);
    let sources: Vec<StorageBytes> =
        (0..70_000u32).map(|index| storage(IpAddr::V4(Ipv4Addr::from(0x0a00_0000 | index)))).collect();
    for (name, socket, level, mode, count) in [
        ("v4 0 sources", &v4, MulticastLevel::Ipv4, uapi::MCAST_INCLUDE, 0),
        ("v4 64 sources", &v4, MulticastLevel::Ipv4, uapi::MCAST_INCLUDE, 64),
        ("v4 70000 sources", &v4, MulticastLevel::Ipv4, uapi::MCAST_INCLUDE, 70_000),
        ("v4 mode 77", &v4, MulticastLevel::Ipv4, 77, 6),
        ("v6 level on v4 socket", &v4, MulticastLevel::Ipv6, uapi::MCAST_INCLUDE, 6),
        ("v4 level on v6 socket", &v6, MulticastLevel::Ipv4, uapi::MCAST_INCLUDE, 6),
    ] {
        let filter =
            SourceFilterOptions { level, interface: 1, group, mode, sources: &sources[..count] };
        eprintln!("source filter {name}: {:?}", L.set_source_filter(socket.as_fd(), &filter));
    }
}

/// Reads the option at every length 0..=1024 with five backgrounds into the front of a marked buffer.
fn sweep(name: &str, socket: BorrowedFd<'_>, option: PlainOption) -> (usize, Vec<String>) {
    let mut decoy = vec![MARK; GUARD];
    let address = decoy.as_mut_ptr().expose_provenance() as u64;
    let fill = |word: &[u8]| word.iter().copied().cycle().take(1024).collect::<Vec<u8>>();
    let backgrounds = [
        vec![0u8; 1024],
        vec![0xffu8; 1024],
        fill(&2u32.to_ne_bytes()),
        fill(&65536u32.to_ne_bytes()),
        fill(&address.to_ne_bytes()),
    ];
    let (mut calls, mut bad, mut answered) = (0, Vec::new(), 0);
    for length in 0..=1024usize {
        for (index, background) in backgrounds.iter().enumerate() {
            let mut buffer = vec![MARK; length + GUARD];
            buffer[..length].copy_from_slice(&background[..length]);
            let result = L.get_option(socket, option, &mut buffer[..length]);
            calls += 1;
            if result.is_ok() {
                answered += 1;
            }
            let (changed, followed) = (past(&buffer[length..]), past(&decoy));
            if changed > 0 || followed > 0 || matches!(result, Ok(written) if written > length) {
                bad.push(format!("{name} {option:?} len {length} bg {index}: {result:?} {changed} past, {followed} decoy"));
                decoy.fill(MARK);
            }
        }
    }
    eprintln!("{name} {option:?}: {calls} reads ({answered} Ok), {} outside", bad.len());
    (calls, bad)
}

/// Second check (b): the open-length options in states the suite's kinds do not set up.
#[test]
fn open_options_in_states_the_suite_does_not_make() {
    let (mut all, mut calls) = (Vec::new(), 0);
    let mut tally = |(made, bad): (usize, Vec<String>)| {
        calls += made;
        all.extend(bad);
    };
    // IP_PKTOPTIONS with the packet information, TTL and TOS flags on a connected TCP socket.
    let (accepted, connecting, _listener) = pair(AddressFamily::INET, ipproto::TCP).expect("a TCP connection");
    setsockopt(&accepted, sockopt::Ipv4PacketInfo, &true).expect("IP_PKTINFO");
    setsockopt(&accepted, sockopt::Ipv4RecvTtl, &true).expect("IP_RECVTTL");
    socket2::SockRef::from(&accepted).set_recv_tos_v4(true).expect("IP_RECVTOS");
    let _ = L.set_option(accepted.as_fd(), PlainOption::IpRecvopts, &1i32.to_ne_bytes());
    let _ = L.set_option(connecting.as_fd(), PlainOption::IpOptions, &[1, 1, 1, 1, 7, 7, 4, 0, 0, 0, 0, 0]);
    let _ = rustix::net::send(&connecting, b"hello", rustix::net::SendFlags::empty());
    tally(sweep("tcp4 with receive flags", accepted.as_fd(), PlainOption::IpPktoptions));
    tally(sweep("tcp4 with IP options", connecting.as_fd(), PlainOption::IpOptions));
    // IPV6_2292PKTOPTIONS with the receive flags on, after data with extension headers arrived.
    let (accepted, connecting, _listener) = pair(AddressFamily::INET6, ipproto::TCP).expect("a TCP connection");
    setsockopt(&accepted, sockopt::Ipv6RecvPacketInfo, &true).expect("IPV6_RECVPKTINFO");
    let receiving = socket2::SockRef::from(&accepted);
    receiving.set_recv_hoplimit_v6(true).expect("IPV6_RECVHOPLIMIT");
    receiving.set_recv_tclass_v6(true).expect("IPV6_RECVTCLASS");
    for option in
        [PlainOption::Ipv6Recvhopopts, PlainOption::Ipv6Recvdstopts, PlainOption::Ipv6Recvrthdr, PlainOption::Ipv6Flowinfo]
    {
        let _ = L.set_option(accepted.as_fd(), option, &1i32.to_ne_bytes());
    }
    let header = [0u8, 1, 1, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    for option in [PlainOption::Ipv6Hopopts, PlainOption::Ipv6Dstopts, PlainOption::Ipv6Rthdrdstopts] {
        let _ = L.set_option(connecting.as_fd(), option, &header);
    }
    for _ in 0..4 {
        let _ = rustix::net::send(&connecting, b"hello", rustix::net::SendFlags::empty());
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let mut sink = [0u8; 64];
    let _ = rustix::net::recv(&accepted, &mut sink, rustix::net::RecvFlags::DONTWAIT);
    for option in [
        PlainOption::Ipv62292pktoptions,
        PlainOption::Ipv6Hopopts,
        PlainOption::Ipv6Dstopts,
        PlainOption::Ipv6Rthdrdstopts,
        PlainOption::Ipv6Rthdr,
    ] {
        tally(sweep("tcp6 receiver", accepted.as_fd(), option));
        tally(sweep("tcp6 sender", connecting.as_fd(), option));
    }
    // TCP_FASTOPEN_KEY holding two keys, on a listener.
    let listener = sock(AddressFamily::INET, SocketType::STREAM, Some(ipproto::TCP)).expect("a TCP socket");
    eprintln!("two fast open keys: {:?}", L.set_option(listener.as_fd(), PlainOption::TcpFastopenKey, &[7u8; 32]));
    tally(sweep("listener with 2 keys", listener.as_fd(), PlainOption::TcpFastopenKey));
    // TCP_CC_INFO under each congestion control that has information.
    for congestion in ["bbr", "dctcp", "vegas", "westwood", "cubic", "reno"] {
        let (accepted, _connecting, _listener) = pair(AddressFamily::INET, ipproto::TCP).expect("a TCP connection");
        if let Err(error) = rustix::net::sockopt::set_tcp_congestion(&accepted, congestion) {
            eprintln!("congestion {congestion}: {error:?}");
            continue;
        }
        tally(sweep(&format!("tcp4 {congestion}"), accepted.as_fd(), PlainOption::TcpCcInfo));
    }
    eprintln!("open options in extra states: {calls} reads, {} outside: {:?}", all.len(), all.iter().take(5).collect::<Vec<_>>());
    assert!(all.is_empty());
}

/// Second check (b): every listed option on raw sockets named TCP and UDP.
#[test]
fn every_option_on_raw_sockets_named_tcp() {
    let (mut all, mut calls) = (Vec::new(), 0);
    for (name, socket) in [
        ("raw4-tcp", sock(AddressFamily::INET, SocketType::RAW, Some(ipproto::TCP))),
        ("raw6-tcp", sock(AddressFamily::INET6, SocketType::RAW, Some(ipproto::TCP))),
        ("raw4-udp", sock(AddressFamily::INET, SocketType::RAW, Some(ipproto::UDP))),
    ] {
        let Some(socket) = socket else { continue };
        for &option in PlainOption::ALL {
            let length = option.value().exact_length().unwrap_or(1024);
            for background in [0u8, 0xff] {
                let mut buffer = vec![MARK; length + GUARD];
                buffer[..length].fill(background);
                let result = L.get_option(socket.as_fd(), option, &mut buffer[..length]);
                calls += 1;
                let changed = past(&buffer[length..]);
                if changed > 0 {
                    all.push(format!("{name} {option:?}: {result:?} {changed} past"));
                }
            }
        }
    }
    eprintln!("raw sockets named TCP and UDP: {calls} reads, {} outside {all:?}", all.len());
    assert!(all.is_empty());
}
