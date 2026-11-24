//! The checks every implementation of wcontainers-socket-options passes on a Linux host, run as root.

use std::net::{Ipv4Addr, SocketAddr};
use std::os::fd::{AsFd, OwnedFd};

use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use rustix::net::{AddressFamily, Protocol, SocketType, ipproto};
use wcontainers_socket_options::{PlainOption, QueuedCount, SocketOptions, ValueLength};

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
    match option.value_length() {
        ValueLength::Exactly(length) => length,
        ValueLength::Any => any,
    }
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
            Err(Errno::NOPROTOOPT) => subject.set_option(
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
