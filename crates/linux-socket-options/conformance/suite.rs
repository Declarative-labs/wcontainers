//! Checks of an implementation of `LinuxSocketOptions` against the kernel it runs on, each in a network namespace of
//! its own (so the checks run as root): every method once with a value Linux takes, and with values and rooms a hostile
//! caller would give.
#![cfg(target_os = "linux")]

use std::ffi::c_int;
use std::mem::size_of;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::os::fd::AsFd;

use linux_raw_sys::net as uapi;
use socket2::{Domain, Protocol, Socket, Type};
use wcontainers_abi::HostError;
use wcontainers_linux_socket_options::{
    GroupFilter, KEY_QUERY_LENGTH, LinuxSocketOptions, SocketStorage, Written,
};
use zerocopy::FromZeros;

/// The name Linux gives the loopback device of every network namespace.
///
/// https://man7.org/linux/man-pages/man7/network_namespaces.7.html
const LOOPBACK_NAME: &str = "lo";

/// Room for the kernel's answer to bringing the loopback up, a netlink error message.
const LOOPBACK_ANSWER_ROOM: usize = 4096;

/// A room larger than any option value read here, which the kernel fills only in part.
const LARGE_ROOM: usize = 4096;

/// A room of one byte, shorter than every structure read here.
const ONE_BYTE: usize = 1;

/// A room of eight bytes, the start of `struct tcp_info`.
const EIGHT_BYTES: usize = 8;

/// The longest IPv4 options, 40 bytes after the 20 of the header.
///
/// https://www.rfc-editor.org/rfc/rfc791#section-3.1
const LONGEST_IPV4_OPTIONS: usize = 40;

/// IPv4's no-operation option, which pads the options to a multiple of four.
///
/// https://www.rfc-editor.org/rfc/rfc791#section-3.1
const IPV4_NO_OPERATION: u8 = 1;

/// IPv4's end-of-options option.
///
/// https://www.rfc-editor.org/rfc/rfc791#section-3.1
const IPV4_END_OF_OPTIONS: u8 = 0;

/// The `SK_MEMINFO_*` counts `SO_MEMINFO` gives, `SK_MEMINFO_VARS`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/sock_diag.h
const MEMORY_COUNTS: usize = libc::SK_MEMINFO_DROPS as usize + 1;

/// The classic BPF instruction that accepts the packet for the first socket of a group, `BPF_RET | BPF_K` with 0.
///
/// https://docs.kernel.org/networking/filter.html
const RETURN_FIRST: libc::sock_filter = libc::sock_filter {
    code: (BPF_RET | BPF_K) as u16,
    jt: u8::MIN,
    jf: u8::MIN,
    k: u32::MIN,
};

/// The classic BPF instruction class that returns, `BPF_RET`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/bpf_common.h
const BPF_RET: u32 = 0x06;

/// The operand source of a constant, `BPF_K`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/bpf_common.h
const BPF_K: u32 = 0x00;

/// The kind of TCP's maximum segment size option.
///
/// https://www.rfc-editor.org/rfc/rfc9293#section-3.2
const TCP_MAXIMUM_SEGMENT_SIZE: u32 = 2;

/// The longest classic BPF program Linux takes, `BPF_MAXINSNS`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/bpf_common.h
const LONGEST_PROGRAM: usize = 4096;

/// The value `SO_DETACH_REUSEPORT_BPF` takes, which Linux does not read.
const DETACH: c_int = 0;

/// An IPv4 multicast group no check joins, the all-hosts group.
///
/// https://www.rfc-editor.org/rfc/rfc1112#section-4
const ALL_HOSTS: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 1);

/// An IPv6 multicast group of the link no check joins.
const LINK_GROUP: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0x1234);

/// A value of a number option Linux takes on every socket here.
const SMALL: c_int = 1;

/// The lowest TCP retransmission timeout Linux takes in microseconds, `TCP_RTO_MIN` of 200 ms.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
const RETRANSMISSION_MINIMUM_MICROSECONDS: c_int = 200_000;

/// The highest TCP retransmission timeout Linux takes in milliseconds, its default of 120 s.
///
/// https://docs.kernel.org/networking/ip-sysctl.html
const RETRANSMISSION_MAXIMUM_MILLISECONDS: c_int = 120_000;

/// The IPv6 checksum offset `IPV6_CHECKSUM` takes to have the kernel compute none.
///
/// https://man7.org/linux/man-pages/man7/ipv6.7.html
const NO_CHECKSUM: c_int = -1;

/// The UDP-Lite checksum coverage of the header alone, eight bytes.
///
/// https://www.rfc-editor.org/rfc/rfc3828#section-3.1
const HEADER_COVERAGE: c_int = 8;

/// A TCP Fast Open key, 16 bytes.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
const FAST_OPEN_KEY: [u8; 16] = [0x5a; 16];

/// A TCP Fast Open queue length a listener takes.
const FAST_OPEN_QUEUE: u32 = 5;

/// `TCP_SAVE_SYN`'s value for saving the headers from the network header.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp.c
const SAVE_FROM_NETWORK_HEADER: c_int = 1;

/// `TCP_QUICKACK`'s value that sends a scheduled acknowledgement and then delays.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp.c
const QUICK_THEN_DELAYING: c_int = 2;

/// A TCP sequence number a repaired socket takes.
const SEQUENCE: u32 = 0x1234_5678;

/// A maximum segment size a repaired connection takes, Ethernet's 1500 bytes less 40 of IPv4 and TCP headers.
const SEGMENT_SIZE: u32 = 1460;

/// A repair option giving the maximum segment size, as `tcp_repair_opt` holds it: the option's kind, then its value.
const MSS_REPAIR: uapi::tcp_repair_opt = uapi::tcp_repair_opt {
    opt_code: TCP_MAXIMUM_SEGMENT_SIZE,
    opt_val: SEGMENT_SIZE,
};

/// Rooms of one `MPTCP_FULL_INFO` read: one subflow, and the start of each structure.
const ONE_SUBFLOW: usize = 1;
const NO_SUBFLOW: usize = 0;
const SUBFLOW_ROOM: usize = 264;
const INFORMATION_ROOM: usize = 256;

/// The error a call gives.
fn errno<T>(result: Result<T, HostError>) -> Option<c_int> {
    match result {
        Err(HostError::Errno(number)) => Some(number),
        _ => None,
    }
}

/// Moves the calling thread into a network namespace of its own with its loopback up.
fn own_network() {
    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET)
        .expect("a network namespace of the check's own, as root");
    bring_loopback_up();
}

/// Brings the calling thread's loopback up through the kernel's routing socket, as `ip link set lo up` does.
fn bring_loopback_up() {
    use netlink_packet_core::{
        NLM_F_ACK, NLM_F_REQUEST, NetlinkHeader, NetlinkMessage, NetlinkPayload,
    };
    use netlink_packet_route::RouteNetlinkMessage;
    use netlink_packet_route::link::{LinkFlags, LinkMessage};
    let mut link = LinkMessage::default();
    link.header.index = nix::net::if_::if_nametoindex(LOOPBACK_NAME).expect("the loopback's index");
    link.header.flags = LinkFlags::Up;
    link.header.change_mask = LinkFlags::Up;
    let mut header = NetlinkHeader::default();
    header.flags = NLM_F_REQUEST | NLM_F_ACK;
    let mut request = NetlinkMessage::new(
        header,
        NetlinkPayload::from(RouteNetlinkMessage::SetLink(link)),
    );
    request.finalize();
    let mut bytes = vec![u8::MIN; request.buffer_len()];
    request.serialize(&mut bytes);
    let socket =
        netlink_sys::Socket::new(netlink_sys::protocols::NETLINK_ROUTE).expect("a routing socket");
    // The kernel's port number and no multicast groups (netlink(7)).
    socket
        .connect(&netlink_sys::SocketAddr::new(u32::MIN, u32::MIN))
        .expect("connect the routing socket to the kernel");
    socket
        .send(&bytes, i32::from(u8::MIN))
        .expect("ask for the loopback up");
    let mut reply = Vec::with_capacity(LOOPBACK_ANSWER_ROOM);
    socket
        .recv(&mut reply, i32::from(u8::MIN))
        .expect("the kernel's answer");
    match NetlinkMessage::<RouteNetlinkMessage>::deserialize(&reply).map(|answer| answer.payload) {
        Ok(NetlinkPayload::Error(error)) if error.code.is_none() => {}
        other => panic!("bring the loopback up: {other:?}"),
    }
}

fn socket(domain: Domain, kind: Type, protocol: Protocol) -> Socket {
    Socket::new(domain, kind, Some(protocol)).expect("a socket")
}

fn udp4() -> Socket {
    socket(Domain::IPV4, Type::DGRAM, Protocol::UDP)
}

fn udp6() -> Socket {
    socket(Domain::IPV6, Type::DGRAM, Protocol::UDP)
}

fn tcp4() -> Socket {
    socket(Domain::IPV4, Type::STREAM, Protocol::TCP)
}

fn tcp6() -> Socket {
    socket(Domain::IPV6, Type::STREAM, Protocol::TCP)
}

fn raw4() -> Socket {
    socket(Domain::IPV4, Type::RAW, Protocol::from(libc::IPPROTO_RAW))
}

/// A raw IPv4 socket of a protocol of its own, which router alerts need (Linux refuses them to `IPPROTO_RAW`).
fn raw4_icmp() -> Socket {
    socket(Domain::IPV4, Type::RAW, Protocol::ICMPV4)
}

fn raw6() -> Socket {
    socket(Domain::IPV6, Type::RAW, Protocol::from(libc::IPPROTO_RAW))
}

fn unix_datagram() -> Socket {
    Socket::new(Domain::UNIX, Type::DGRAM, None).expect("a Unix socket")
}

/// A connected pair of loopback TCP sockets, the listener's end accepted.
fn tcp_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, u16::MIN)).expect("a listener");
    let client =
        TcpStream::connect(listener.local_addr().expect("its address")).expect("a connect");
    let (accepted, _) = listener.accept().expect("an accept");
    (client, accepted)
}

fn storage_of(address: SocketAddr) -> SocketStorage {
    let mut storage = SocketStorage::new_zeroed();
    let mut put =
        |at: usize, bytes: &[u8]| storage.bytes[at..at + bytes.len()].copy_from_slice(bytes);
    match address {
        SocketAddr::V4(v4) => {
            put(
                core::mem::offset_of!(libc::sockaddr_in, sin_family),
                &(libc::AF_INET as u16).to_ne_bytes(),
            );
            put(
                core::mem::offset_of!(libc::sockaddr_in, sin_port),
                &v4.port().to_be_bytes(),
            );
            put(
                core::mem::offset_of!(libc::sockaddr_in, sin_addr),
                &v4.ip().octets(),
            );
        }
        SocketAddr::V6(v6) => {
            put(
                core::mem::offset_of!(libc::sockaddr_in6, sin6_family),
                &(libc::AF_INET6 as u16).to_ne_bytes(),
            );
            put(
                core::mem::offset_of!(libc::sockaddr_in6, sin6_port),
                &v6.port().to_be_bytes(),
            );
            put(
                core::mem::offset_of!(libc::sockaddr_in6, sin6_addr),
                &v6.ip().octets(),
            );
        }
    }
    storage
}

fn kernel_storage(address: SocketAddr) -> uapi::__kernel_sockaddr_storage {
    let storage = storage_of(address);
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

fn written<const ROOM: usize>(result: Result<Written<ROOM>, HostError>) -> Written<ROOM> {
    result.expect("a read Linux answers")
}

/// Socket-level options take their values on the sockets they belong to.
pub fn socket_level_options_take_their_values(options: &impl LinuxSocketOptions) {
    own_network();
    let udp = udp4();
    let fd = udp.as_fd();
    options.set_so_rcvmark(fd, true).expect("SO_RCVMARK on");
    options
        .set_so_rcvpriority(fd, true)
        .expect("SO_RCVPRIORITY on");
    options
        .set_so_max_pacing_rate(fd, u64::MAX)
        .expect("no pacing");
    options
        .set_so_rcvlowat(fd, SMALL)
        .expect("a low water mark");
    assert_eq!(
        options.get_so_rcvlowat(fd).expect("the low water mark"),
        SMALL
    );
    options
        .set_so_buf_lock(fd, uapi::SOCK_SNDBUF_LOCK)
        .expect("the send buffer locked");
    // Rehashing is TCP's: a UDP socket refuses it.
    assert_eq!(
        errno(options.set_so_txrehash(fd, uapi::SOCK_TXREHASH_ENABLED as c_int)),
        Some(libc::EOPNOTSUPP)
    );
    options
        .set_so_wifi_status(fd, true)
        .expect("SO_WIFI_STATUS on");
    options.set_so_no_check(fd, true).expect("SO_NO_CHECK on");
    options
        .set_so_timestamping(
            fd,
            &uapi::so_timestamping {
                flags: uapi::SOF_TIMESTAMPING_RX_SOFTWARE as c_int,
                bind_phc: c_int::default(),
            },
        )
        .expect("receive timestamps");
    let memory = written(options.get_so_meminfo::<LARGE_ROOM>(fd));
    assert_eq!(memory.length, MEMORY_COUNTS * size_of::<u32>());
    assert!(
        memory.bytes[memory.length..]
            .iter()
            .all(|byte| *byte == u8::MIN)
    );
    let tcp = tcp4();
    options
        .set_so_peek_off(tcp.as_fd(), SMALL)
        .expect("a peek offset");
    options
        .set_so_cnx_advice(tcp.as_fd(), true)
        .expect("negative advice");
}

/// A read into a room shorter than the value gives the room's worth, and one into a longer room what the kernel has.
pub fn reads_fit_their_rooms(options: &impl LinuxSocketOptions) {
    own_network();
    let (client, _accepted) = tcp_pair();
    let fd = client.as_fd();
    let long = written(options.get_tcp_info::<LARGE_ROOM>(fd));
    assert!(long.length > EIGHT_BYTES && long.length < LARGE_ROOM);
    assert!(
        long.bytes[long.length..]
            .iter()
            .all(|byte| *byte == u8::MIN)
    );
    let short = written(options.get_tcp_info::<EIGHT_BYTES>(fd));
    assert_eq!(short.length, EIGHT_BYTES);
    assert_eq!(short.bytes[..], long.bytes[..EIGHT_BYTES]);
    let one = written(options.get_so_meminfo::<ONE_BYTE>(udp4().as_fd()));
    assert_eq!(one.length, ONE_BYTE);
    let empty = written(options.get_tcp_cc_info::<LARGE_ROOM>(fd));
    assert!(empty.length <= LARGE_ROOM);
}

/// A classic BPF program steers a reuse-port group, and programs Linux cannot take are refused.
pub fn reuse_port_programs_attach_and_detach(options: &impl LinuxSocketOptions) {
    own_network();
    let udp = udp4();
    udp.set_reuse_port(true).expect("port sharing");
    udp.bind(&SocketAddr::from((Ipv4Addr::LOCALHOST, u16::MIN)).into())
        .expect("a binding");
    let fd = udp.as_fd();
    options
        .set_so_attach_reuseport_cbpf(fd, &[RETURN_FIRST])
        .expect("a program of one instruction");
    options
        .set_so_detach_reuseport_bpf(fd, DETACH)
        .expect("the program detached");
    assert_eq!(
        errno(options.set_so_attach_reuseport_cbpf(fd, &[])),
        Some(libc::EINVAL)
    );
    let too_long = vec![RETURN_FIRST; LONGEST_PROGRAM + 1];
    assert_eq!(
        errno(options.set_so_attach_reuseport_cbpf(fd, &too_long)),
        Some(libc::EINVAL)
    );
    let past_the_count = vec![RETURN_FIRST; usize::from(u16::MAX) + 1];
    assert_eq!(
        errno(options.set_so_attach_reuseport_cbpf(fd, &past_the_count)),
        Some(libc::EINVAL)
    );
}

/// IPv4 options take their values, and byte strings Linux cannot take are refused.
pub fn ipv4_options_take_their_values(options: &impl LinuxSocketOptions) {
    own_network();
    let udp = udp4();
    let fd = udp.as_fd();
    options.set_ip_options(fd, &[]).expect("no options");
    options
        .set_ip_options(
            fd,
            &[
                IPV4_NO_OPERATION,
                IPV4_NO_OPERATION,
                IPV4_NO_OPERATION,
                IPV4_END_OF_OPTIONS,
            ],
        )
        .expect("padding options");
    assert_eq!(
        errno(options.set_ip_options(fd, &[IPV4_NO_OPERATION; LONGEST_IPV4_OPTIONS + 1])),
        Some(libc::EINVAL)
    );
    options
        .set_ip_unicast_if(fd, u32::MIN)
        .expect("no interface");
    let range = u32::from(u16::MAX) << u16::BITS | u32::from(u16::MAX / 2);
    options
        .set_ip_local_port_range(fd, range)
        .expect("a port range");
    options.set_ip_recvopts(fd, true).expect("IP_RECVOPTS on");
    options
        .set_ip_recvfragsize(fd, true)
        .expect("IP_RECVFRAGSIZE on");
    // Linux refuses router alerts to a raw socket that sends whole packets, which receives none.
    assert_eq!(
        errno(options.set_ip_router_alert(raw4().as_fd(), true)),
        Some(libc::EINVAL)
    );
    let raw = raw4_icmp();
    options
        .set_ip_router_alert(raw.as_fd(), true)
        .expect("router alerts on a raw socket");
    options
        .set_ip_nodefrag(raw.as_fd(), true)
        .expect("IP_NODEFRAG on a raw socket");
    // Router alerts are raw sockets' alone.
    assert_eq!(
        errno(options.set_ip_router_alert(fd, true)),
        Some(libc::EINVAL)
    );
    let (client, _accepted) = tcp_pair();
    options
        .set_ip_minttl(client.as_fd(), SMALL)
        .expect("a minimum TTL");
    let records = written(options.get_ip_pktoptions::<LARGE_ROOM>(client.as_fd()));
    assert!(records.length < LARGE_ROOM);
    assert!(options.get_ip_pktoptions::<LARGE_ROOM>(fd).is_err());
}

/// Multicast memberships are joined and left at both IP levels, and a filter is written only for a group joined.
pub fn multicast_memberships_join_and_leave(options: &impl LinuxSocketOptions) {
    own_network();
    let index = nix::net::if_::if_nametoindex(LOOPBACK_NAME).expect("the loopback's index");
    let udp = udp4();
    let fd = udp.as_fd();
    let group = SocketAddr::from((ALL_HOSTS, u16::MIN));
    let request = uapi::group_req {
        gr_interface: index,
        gr_group: kernel_storage(group),
    };
    let filter = GroupFilter {
        interface: index,
        group: storage_of(group),
        mode: uapi::MCAST_INCLUDE,
        sources: &[storage_of(SocketAddr::from((
            Ipv4Addr::LOCALHOST,
            u16::MIN,
        )))],
    };
    assert!(options.set_ip_mcast_msfilter(fd, &filter).is_err());
    let joined = options.set_ip_mcast_join_group(fd, &request);
    if joined.is_ok() {
        options
            .set_ip_mcast_msfilter(fd, &filter)
            .expect("a filter of a group joined");
        options
            .set_ip_mcast_leave_group(fd, &request)
            .expect("the group left");
    }
    assert_eq!(
        errno(options.set_ip_mcast_leave_group(fd, &request)),
        Some(libc::EADDRNOTAVAIL)
    );
    let udp = udp6();
    let fd = udp.as_fd();
    let group = SocketAddr::from((LINK_GROUP, u16::MIN));
    let request = uapi::group_req {
        gr_interface: index,
        gr_group: kernel_storage(group),
    };
    let filter = GroupFilter {
        interface: index,
        group: storage_of(group),
        mode: uapi::MCAST_EXCLUDE,
        sources: &[],
    };
    assert!(options.set_ipv6_mcast_msfilter(fd, &filter).is_err());
    if options.set_ipv6_mcast_join_group(fd, &request).is_ok() {
        options
            .set_ipv6_mcast_msfilter(fd, &filter)
            .expect("a filter of a group joined");
        options
            .set_ipv6_mcast_leave_group(fd, &request)
            .expect("the group left");
    }
    assert!(options.set_ipv6_mcast_leave_group(fd, &request).is_err());
}

/// IPv6 options take their values, extension headers Linux cannot parse are refused, and anycast leaves only what joined.
pub fn ipv6_options_take_their_values(options: &impl LinuxSocketOptions) {
    own_network();
    let udp = udp6();
    let fd = udp.as_fd();
    // An empty header of each kind removes it; one whose length field claims more than given is refused.
    options
        .set_ipv6_hopopts(fd, &[])
        .expect("no hop-by-hop options");
    options
        .set_ipv6_rthdrdstopts(fd, &[])
        .expect("no routing destination options");
    options.set_ipv6_rthdr(fd, &[]).expect("no routing header");
    options
        .set_ipv6_dstopts(fd, &[])
        .expect("no destination options");
    // Next header, a length of one 8-byte unit past the first, then six bytes of padding: 8 bytes given of 16 claimed.
    let claims_more = [u8::MIN, 1, 1, 4, u8::MIN, u8::MIN, u8::MIN, u8::MIN];
    assert_eq!(
        errno(options.set_ipv6_hopopts(fd, &claims_more)),
        Some(libc::EINVAL)
    );
    assert_eq!(
        errno(options.set_ipv6_dstopts(fd, &claims_more)),
        Some(libc::EINVAL)
    );
    options
        .set_ipv6_flowinfo_send(fd, true)
        .expect("flow information sent");
    options
        .set_ipv6_autoflowlabel(fd, false)
        .expect("no automatic flow label");
    options
        .set_ipv6_addr_preferences(
            fd,
            uapi::IPV6_PREFER_SRC_PUBLIC | uapi::IPV6_PREFER_SRC_HOME,
        )
        .expect("source preferences");
    options
        .set_ipv6_mtu(fd, c_int::from(u16::MAX))
        .expect("a fragment size");
    options
        .set_ipv6_unicast_if(fd, u32::MIN)
        .expect("no interface");
    options
        .set_ipv6_recvhopopts(fd, true)
        .expect("IPV6_RECVHOPOPTS on");
    options
        .set_ipv6_recvdstopts(fd, true)
        .expect("IPV6_RECVDSTOPTS on");
    options
        .set_ipv6_recvrthdr(fd, true)
        .expect("IPV6_RECVRTHDR on");
    options
        .set_ipv6_flowinfo(fd, true)
        .expect("IPV6_FLOWINFO on");
    options
        .set_ipv6_recvfragsize(fd, true)
        .expect("IPV6_RECVFRAGSIZE on");
    let lease = uapi::in6_flowlabel_req {
        flr_dst: uapi::in6_addr {
            in6_u: uapi::in6_addr__bindgen_ty_1 {
                u6_addr8: Ipv6Addr::LOCALHOST.octets(),
            },
        },
        flr_label: u32::MIN,
        flr_action: uapi::IPV6_FL_A_GET as u8,
        flr_share: uapi::IPV6_FL_S_ANY as u8,
        flr_flags: u16::MIN,
        flr_expires: u16::MIN,
        flr_linger: u16::MIN,
        __flr_pad: u32::MIN,
    };
    // Label zero names no label to lease.
    assert!(options.set_ipv6_flowlabel_mgr(fd, &lease).is_err());
    let anycast = uapi::ipv6_mreq {
        ipv6mr_multiaddr: uapi::in6_addr {
            in6_u: uapi::in6_addr__bindgen_ty_1 {
                u6_addr8: Ipv6Addr::LOCALHOST.octets(),
            },
        },
        ipv6mr_ifindex: c_int::default(),
    };
    assert!(options.set_ipv6_leave_anycast(fd, &anycast).is_err());
    assert!(options.set_ipv6_join_anycast(fd, &anycast).is_err());
    let raw = raw6();
    let rfd = raw.as_fd();
    options
        .set_ipv6_router_alert(rfd, c_int::default())
        .expect("router alerts of value zero");
    options
        .set_ipv6_router_alert_isolate(rfd, true)
        .expect("router alerts of the namespace");
    options
        .set_ipv6_checksum(rfd, NO_CHECKSUM)
        .expect("no checksum");
    assert!(options.set_ipv6_checksum(fd, NO_CHECKSUM).is_err());
    let tcp = tcp6();
    options
        .set_ipv6_minhopcount(tcp.as_fd(), SMALL)
        .expect("a minimum hop count");
    let records = written(options.get_ipv6_2292pktoptions::<LARGE_ROOM>(tcp.as_fd()));
    assert!(records.length < LARGE_ROOM);
}

/// TCP's number options take their values and give them back where Linux reads them.
pub fn tcp_number_options_take_their_values(options: &impl LinuxSocketOptions) {
    own_network();
    let (client, _accepted) = tcp_pair();
    let fd = client.as_fd();
    options
        .set_tcp_window_clamp(fd, c_int::from(u16::MAX))
        .expect("a clamp");
    assert!(options.get_tcp_window_clamp(fd).expect("the clamp") > c_int::default());
    let fresh = tcp4();
    let ffd = fresh.as_fd();
    options.set_tcp_syncnt(ffd, SMALL).expect("SYN retries");
    options
        .set_tcp_defer_accept(ffd, SMALL)
        .expect("a deferred accept");
    options
        .set_tcp_linger2(ffd, SMALL)
        .expect("a FIN-WAIT-2 time");
    options
        .set_tcp_rto_min_us(ffd, RETRANSMISSION_MINIMUM_MICROSECONDS)
        .expect("a lowest retransmission timeout");
    options
        .set_tcp_rto_max_ms(ffd, RETRANSMISSION_MAXIMUM_MILLISECONDS)
        .expect("a highest retransmission timeout");
    options
        .set_tcp_delack_max_us(ffd, RETRANSMISSION_MINIMUM_MICROSECONDS)
        .expect("a longest delayed acknowledgement");
    options
        .set_tcp_tx_delay(ffd, SMALL)
        .expect("a transmit delay");
    options
        .set_tcp_fastopen_no_cookie(ffd, true)
        .expect("Fast Open without a cookie");
    options.set_tcp_inq(ffd, true).expect("TCP_INQ on");
    options
        .set_tcp_quickack(fd, QUICK_THEN_DELAYING)
        .expect("quick then delaying");
    assert!(options.get_tcp_timestamp(fd).is_ok());
    assert_eq!(
        errno(options.set_tcp_syncnt(udp4().as_fd(), SMALL)),
        Some(libc::ENOPROTOOPT)
    );
}

/// A listener takes Fast Open's queue and keys and keeps a SYN's headers, read only into a room that holds them.
pub fn listeners_keep_what_they_are_asked_to(options: &impl LinuxSocketOptions) {
    own_network();
    let listener = tcp4();
    let lfd = listener.as_fd();
    options
        .set_tcp_fastopen(lfd, FAST_OPEN_QUEUE)
        .expect("a Fast Open queue");
    options
        .set_tcp_fastopen_key(lfd, &FAST_OPEN_KEY)
        .expect("a Fast Open key");
    assert_eq!(
        errno(options.set_tcp_fastopen_key(lfd, &FAST_OPEN_KEY[..ONE_BYTE])),
        Some(libc::EINVAL)
    );
    options
        .set_tcp_save_syn(lfd, SAVE_FROM_NETWORK_HEADER)
        .expect("SYNs saved");
    listener
        .bind(&SocketAddr::from((Ipv4Addr::LOCALHOST, u16::MIN)).into())
        .expect("a binding");
    listener.listen(c_int::from(u8::MAX)).expect("listening");
    let address = listener
        .local_addr()
        .expect("its address")
        .as_socket()
        .expect("an IP address");
    let _client = TcpStream::connect(address).expect("a connect");
    let (accepted, _) = listener.accept().expect("an accept");
    assert_eq!(
        errno(options.get_tcp_saved_syn::<ONE_BYTE>(accepted.as_fd())),
        Some(libc::EINVAL)
    );
    let saved = written(options.get_tcp_saved_syn::<LARGE_ROOM>(accepted.as_fd()));
    assert!(saved.length > ONE_BYTE);
}

/// A socket in repair mode takes and gives back its sequence numbers, window and options.
pub fn repair_mode_takes_and_gives_its_state(options: &impl LinuxSocketOptions) {
    own_network();
    let (client, _accepted) = tcp_pair();
    let fd = client.as_fd();
    nix::sys::socket::setsockopt(&fd, nix::sys::socket::sockopt::TcpRepair, &u32::from(true))
        .expect("repair mode, as root");
    options
        .set_tcp_repair_queue(fd, uapi::TCP_SEND_QUEUE as c_int)
        .expect("the send queue chosen");
    assert_eq!(
        options.get_tcp_repair_queue(fd).expect("the queue chosen"),
        uapi::TCP_SEND_QUEUE as c_int
    );
    assert!(options.get_tcp_queue_seq(fd).is_ok());
    let window =
        written(options.get_tcp_repair_window::<{ size_of::<uapi::tcp_repair_window>() }>(fd));
    assert_eq!(window.length, size_of::<uapi::tcp_repair_window>());
    let field = |at: usize| {
        u32::from_ne_bytes(
            window.bytes[at..at + size_of::<u32>()]
                .try_into()
                .expect("four bytes"),
        )
    };
    let kept = uapi::tcp_repair_window {
        snd_wl1: field(core::mem::offset_of!(uapi::tcp_repair_window, snd_wl1)),
        snd_wnd: field(core::mem::offset_of!(uapi::tcp_repair_window, snd_wnd)),
        max_window: field(core::mem::offset_of!(uapi::tcp_repair_window, max_window)),
        rcv_wnd: field(core::mem::offset_of!(uapi::tcp_repair_window, rcv_wnd)),
        rcv_wup: field(core::mem::offset_of!(uapi::tcp_repair_window, rcv_wup)),
    };
    options
        .set_tcp_repair_window(fd, &kept)
        .expect("the window put back");
    // The timestamp clock is written only in repair mode, and read at any time.
    options
        .set_tcp_timestamp(fd, SEQUENCE)
        .expect("a timestamp clock");
    assert_eq!(
        errno(options.get_tcp_repair_window::<ONE_BYTE>(fd)),
        Some(libc::EINVAL)
    );
    let fresh = tcp4();
    let ffd = fresh.as_fd();
    nix::sys::socket::setsockopt(&ffd, nix::sys::socket::sockopt::TcpRepair, &u32::from(true))
        .expect("repair mode, as root");
    options
        .set_tcp_repair_queue(ffd, uapi::TCP_SEND_QUEUE as c_int)
        .expect("the send queue");
    options
        .set_tcp_queue_seq(ffd, SEQUENCE)
        .expect("a sequence number before connecting");
    assert_eq!(
        options.get_tcp_queue_seq(ffd).expect("the sequence number"),
        SEQUENCE
    );
    assert!(options.set_tcp_repair_options(ffd, &[MSS_REPAIR]).is_err());
    assert!(options.set_tcp_repair_options(fd, &[MSS_REPAIR]).is_ok());
}

/// TCP's signature and authentication keys are refused when they name no key, and a query asking past its room is refused
/// before the kernel sees it.
pub fn tcp_keys_are_named_or_refused(options: &impl LinuxSocketOptions) {
    own_network();
    let tcp = tcp4();
    let fd = tcp.as_fd();
    let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, u16::MIN));
    let removal = uapi::tcp_md5sig {
        tcpm_addr: kernel_storage(peer),
        tcpm_flags: u8::MIN,
        tcpm_prefixlen: u8::MIN,
        tcpm_keylen: u16::MIN,
        tcpm_ifindex: c_int::default(),
        tcpm_key: [u8::MIN; 80],
    };
    assert_eq!(
        errno(options.set_tcp_md5sig_ext(fd, &removal)),
        Some(libc::ENOENT)
    );
    let addition = uapi::tcp_ao_add {
        addr: kernel_storage(peer),
        alg_name: [libc::c_char::default(); 64],
        ifindex: c_int::default(),
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        reserved2: u16::MIN,
        prefix: u8::MIN,
        sndid: u8::MIN,
        rcvid: u8::MIN,
        maclen: u8::MIN,
        keyflags: u8::MIN,
        keylen: u8::MIN,
        key: [u8::MIN; 80],
    };
    assert!(options.set_tcp_ao_add_key(fd, &addition).is_err());
    let deletion = uapi::tcp_ao_del {
        addr: kernel_storage(peer),
        ifindex: c_int::default(),
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        reserved2: u16::MIN,
        prefix: u8::MIN,
        sndid: u8::MIN,
        rcvid: u8::MIN,
        current_key: u8::MIN,
        rnext: u8::MIN,
        keyflags: u8::MIN,
    };
    assert!(options.set_tcp_ao_del_key(fd, &deletion).is_err());
    let information = uapi::tcp_ao_info_opt {
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
    };
    assert_eq!(
        errno(options.get_tcp_ao_info::<{ size_of::<uapi::tcp_ao_info_opt>() }>(fd)),
        Some(libc::ENOENT)
    );
    // Writing the information with nothing asked of it gives the socket the option's state, which reads back whole.
    options
        .set_tcp_ao_info(fd, &information)
        .expect("the option's state made");
    let made = options
        .get_tcp_ao_info::<{ size_of::<uapi::tcp_ao_info_opt>() }>(fd)
        .expect("the option's state");
    assert_eq!(made.length, size_of::<uapi::tcp_ao_info_opt>());
    let repair = uapi::tcp_ao_repair {
        snt_isn: u32::MIN,
        rcv_isn: u32::MIN,
        snd_sne: u32::MIN,
        rcv_sne: u32::MIN,
    };
    assert!(options.set_tcp_ao_repair(fd, &repair).is_err());
    assert!(
        options
            .get_tcp_ao_repair::<{ size_of::<uapi::tcp_ao_repair>() }>(fd)
            .is_err()
    );
    let at = core::mem::offset_of!(uapi::tcp_ao_getsockopt, nkeys);
    let mut query = [u8::MIN; KEY_QUERY_LENGTH];
    query[at..at + size_of::<u32>()].copy_from_slice(&1_u32.to_ne_bytes());
    assert!(options.get_tcp_ao_get_keys(fd, &query).is_err());
    // Two answers asked for in the room of one: the kernel would write the second past the room.
    query[at..at + size_of::<u32>()].copy_from_slice(&2_u32.to_ne_bytes());
    assert_eq!(
        errno(options.get_tcp_ao_get_keys(fd, &query)),
        Some(libc::EINVAL)
    );
    query[at..at + size_of::<u32>()].copy_from_slice(&u32::MAX.to_ne_bytes());
    assert_eq!(
        errno(options.get_tcp_ao_get_keys(fd, &query)),
        Some(libc::EINVAL)
    );
}

/// A multipath socket gives its information, its subflows only into the rooms asked for.
pub fn multipath_information_fits_its_rooms(options: &impl LinuxSocketOptions) {
    own_network();
    let Ok(multipath) = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::MPTCP)) else {
        // A kernel without MPTCP refuses the socket; the TCP socket's refusal is still checked.
        assert!(
            options
                .get_mptcp_info::<LARGE_ROOM>(tcp4().as_fd())
                .is_err()
        );
        return;
    };
    let fd = multipath.as_fd();
    let _ = options.get_mptcp_info::<LARGE_ROOM>(fd);
    let none = options.get_mptcp_full_info::<SUBFLOW_ROOM, INFORMATION_ROOM>(fd, NO_SUBFLOW);
    let one = options.get_mptcp_full_info::<SUBFLOW_ROOM, INFORMATION_ROOM>(fd, ONE_SUBFLOW);
    let short = options.get_mptcp_full_info::<ONE_BYTE, ONE_BYTE>(fd, ONE_SUBFLOW);
    assert_eq!(none.is_ok(), one.is_ok());
    assert_eq!(one.is_ok(), short.is_ok());
    assert!(
        options
            .get_mptcp_info::<LARGE_ROOM>(tcp4().as_fd())
            .is_err()
    );
    assert!(
        options
            .get_mptcp_full_info::<SUBFLOW_ROOM, INFORMATION_ROOM>(tcp4().as_fd(), ONE_SUBFLOW)
            .is_err()
    );
}

/// UDP's and UDP-Lite's checksum options take their values only on their own sockets.
pub fn udp_checksum_options_take_their_values(options: &impl LinuxSocketOptions) {
    own_network();
    let udp = udp6();
    options
        .set_udp_no_check6_rx(udp.as_fd(), true)
        .expect("zero checksums taken");
    options
        .set_udp_no_check6_tx(udp.as_fd(), true)
        .expect("zero checksums sent");
    let lite = socket(
        Domain::IPV4,
        Type::DGRAM,
        Protocol::from(libc::IPPROTO_UDPLITE),
    );
    options
        .set_udplite_recv_cscov(lite.as_fd(), HEADER_COVERAGE)
        .expect("a receive coverage");
    options
        .set_udplite_send_cscov(lite.as_fd(), HEADER_COVERAGE)
        .expect("a send coverage");
    assert!(
        options
            .set_udplite_send_cscov(udp.as_fd(), HEADER_COVERAGE)
            .is_err()
    );
    assert!(options.set_udp_no_check6_rx(tcp4().as_fd(), true).is_err());
}

/// The queue requests count what a connection holds, and a socket of another kind refuses them.
pub fn queue_requests_count_a_connection(options: &impl LinuxSocketOptions) {
    own_network();
    let (client, _accepted) = tcp_pair();
    let fd = client.as_fd();
    assert_eq!(
        options.get_siocoutq(fd).expect("the unacknowledged bytes"),
        c_int::default()
    );
    assert_eq!(
        options.get_siocoutqnsd(fd).expect("the unsent bytes"),
        c_int::default()
    );
    assert_eq!(
        options.get_siocatmark(fd).expect("the urgent mark"),
        c_int::default()
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, u16::MIN)).expect("a listener");
    assert_eq!(
        errno(options.get_siocoutq(listener.as_fd())),
        Some(libc::EINVAL)
    );
    assert!(options.get_siocoutqnsd(unix_datagram().as_fd()).is_err());
}

#[macro_export]
macro_rules! tests {
    ($options:expr) => {
        #[test]
        fn socket_level_options_take_their_values() {
            $crate::socket_level_options_take_their_values(&$options);
        }

        #[test]
        fn reads_fit_their_rooms() {
            $crate::reads_fit_their_rooms(&$options);
        }

        #[test]
        fn reuse_port_programs_attach_and_detach() {
            $crate::reuse_port_programs_attach_and_detach(&$options);
        }

        #[test]
        fn ipv4_options_take_their_values() {
            $crate::ipv4_options_take_their_values(&$options);
        }

        #[test]
        fn multicast_memberships_join_and_leave() {
            $crate::multicast_memberships_join_and_leave(&$options);
        }

        #[test]
        fn ipv6_options_take_their_values() {
            $crate::ipv6_options_take_their_values(&$options);
        }

        #[test]
        fn tcp_number_options_take_their_values() {
            $crate::tcp_number_options_take_their_values(&$options);
        }

        #[test]
        fn listeners_keep_what_they_are_asked_to() {
            $crate::listeners_keep_what_they_are_asked_to(&$options);
        }

        #[test]
        fn repair_mode_takes_and_gives_its_state() {
            $crate::repair_mode_takes_and_gives_its_state(&$options);
        }

        #[test]
        fn tcp_keys_are_named_or_refused() {
            $crate::tcp_keys_are_named_or_refused(&$options);
        }

        #[test]
        fn multipath_information_fits_its_rooms() {
            $crate::multipath_information_fits_its_rooms(&$options);
        }

        #[test]
        fn udp_checksum_options_take_their_values() {
            $crate::udp_checksum_options_take_their_values(&$options);
        }

        #[test]
        fn queue_requests_count_a_connection() {
            $crate::queue_requests_count_a_connection(&$options);
        }
    };
}
