//! The probe of every row of `options.table` on the running kernel, which measures the table's kernel column.
//!
//! Each call's value sits between canary bytes, a get's room is filled with them, and every byte is compared after the
//! call. A set the table marks `reads` must leave its value as it was; one marked `writes-back` must be seen writing
//! into it; a get marked `within` must write nothing past the length it is told and answer no longer one; `past` must be
//! seen writing past it; `through` must be seen writing into the rooms its value names and nowhere else. No call may
//! write before its value. Runs as root, in a network namespace of its own, as the conformance suite does.

use std::ffi::c_int;
use std::fmt;
use std::mem::{offset_of, size_of};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use linux_raw_sys::net as uapi;
use socket2::{Domain, Protocol, Socket, Type};
use wcontainers_abi::HostError;
use wcontainers_linux_socket_options::KEY_QUERY_LENGTH;
use wcontainers_linux_socket_options_conformance::{own_network, storage_of};
use zerocopy::{FromZeros, IntoBytes};

use crate::generated::ROWS;
use crate::{
    FullInformationHeader, full_info_into, get_into, get_within, group_filter_bytes, ioctl_within,
    length_of, set_bytes_at, set_value,
};

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
    pub(crate) level: Option<u32>,
    pub(crate) option: u32,
    pub(crate) value: &'static str,
    pub(crate) kernel: Kernel,
}

/// The canary bytes on each side of a value, enough to hold a second `TCP_AO_GET_KEYS` answer.
const GUARD: usize = 1024;

/// The two canary patterns, so that a byte the kernel writes equal to one is seen through the other.
const CANARIES: [u8; 2] = [0xa5, 0x5a];

/// The rooms a number option is read into: none, one byte, the number, and twice it.
const NUMBER_ROOMS: [usize; 4] = [0, 1, size_of::<u32>(), size_of::<u64>()];

/// The rooms an option of varying length is read into: none, one byte, eight, and more than any value.
const ROOMS: [usize; 4] = [0, 1, 8, LARGE_ROOM];

/// A room larger than any option value read here.
const LARGE_ROOM: usize = 4096;

/// The rooms of options whose value is one structure, which Linux reads only into a room of its length or more.
const AO_INFORMATION_ROOMS: [usize; 5] = [0, 1, 8, size_of::<uapi::tcp_ao_info_opt>(), LARGE_ROOM];
const AO_REPAIR_ROOMS: [usize; 5] = [0, 1, 8, size_of::<uapi::tcp_ao_repair>(), LARGE_ROOM];
const REPAIR_WINDOW_ROOMS: [usize; 5] = [0, 1, 8, size_of::<uapi::tcp_repair_window>(), LARGE_ROOM];

/// The forwarding setting of the namespace's loopback, which lets it join any anycast address, not only those its
/// prefixes hold.
///
/// https://docs.kernel.org/networking/ip-sysctl.html
const LOOPBACK_FORWARDING: &str = "/proc/sys/net/ipv6/conf/lo/forwarding";

/// A room of `MPTCP_FULL_INFO` shorter than each structure it holds, and one longer.
const SHORT_ENTRY: usize = 8;
const LONG_ENTRY: usize = 512;

/// The subflow counts `MPTCP_FULL_INFO` is told: none and one.
const SUBFLOW_COUNTS: [usize; 2] = [0, 1];

/// A value of a number or flag option that every socket here takes.
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

/// The program lengths the reuse-port option is given: none, one instruction, and a few.
const PROGRAM_LENGTHS: [usize; 3] = [0, 1, 4];

/// An IPv4 multicast group of the organization's own scope.
///
/// https://www.rfc-editor.org/rfc/rfc2365#section-6.2
const GROUP_V4: Ipv4Addr = Ipv4Addr::new(239, 1, 2, 3);

/// An IPv6 multicast group of the link.
const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0x1234);

/// An IPv6 address of the documentation prefix, joined as anycast.
///
/// https://www.rfc-editor.org/rfc/rfc3849
const ANYCAST: Ipv6Addr = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);

/// A flow label below the kernel's reserved half, as it reads one in network order.
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

/// A TCP sequence number and maximum segment size a repaired socket takes.
const SEQUENCE: u32 = 0x1234_5678;
const SEGMENT_SIZE: u32 = 1460;

/// The kind of TCP's maximum segment size option.
///
/// https://www.rfc-editor.org/rfc/rfc9293#section-3.2
const MAXIMUM_SEGMENT_SIZE: u32 = 2;

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

/// What a row is called with.
enum Input {
    /// The bytes of each value a set gives, made from the prepared socket.
    Values(fn(BorrowedFd<'_>) -> Vec<Vec<u8>>),
    /// A classic BPF program of each of `PROGRAM_LENGTHS`, through a `struct sock_fprog` the probe writes.
    Program,
    /// The rooms a get reads into.
    Rooms(&'static [usize]),
    /// The queries a get answers in place, each in a room of its own length.
    Queries(fn() -> Vec<Vec<u8>>),
    /// `MPTCP_FULL_INFO`'s header and rooms.
    FullInformation,
    /// An ioctl request that writes one int.
    Ioctl,
}

/// One way of calling a row: the socket, made afresh for every call, and the input.
struct Case {
    prepare: fn() -> Prepared,
    input: Input,
}

/// What the calls of one row did to the memory around and inside their values.
#[derive(Default)]
struct Seen {
    calls: usize,
    succeeded: usize,
    /// Calls after which a byte of the value or room differed from what was given.
    inside: usize,
    /// Calls after which a byte past the value or room differed.
    past: usize,
    /// Calls after which a byte before the value differed.
    before: usize,
    /// Calls whose answered length was below the length told.
    shorter: usize,
    /// Calls whose answered length was above the length told.
    longer: usize,
    /// Calls that wrote into a room the value named.
    through: usize,
    errors: Vec<c_int>,
}

impl Seen {
    fn record<T>(&mut self, result: &Result<T, HostError>) {
        self.calls += 1;
        match result {
            Ok(_) => self.succeeded += 1,
            Err(HostError::Errno(number)) => {
                if !self.errors.contains(number) {
                    self.errors.push(*number);
                }
            }
            Err(_) => {}
        }
    }

    /// Gives what the calls did that the row's kernel column does not allow, if anything.
    fn against(&self, kernel: Kernel) -> Option<String> {
        let fault = if self.before > 0 {
            "wrote before its value"
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
                Kernel::Reads | Kernel::WritesBack if self.past > 0 => "wrote past its value",
                Kernel::WritesBack if self.inside == 0 => "was never seen writing into its value",
                Kernel::Within if self.past > 0 => "wrote past the length it was told",
                Kernel::Within if self.longer > 0 => "answered a length longer than it was told",
                Kernel::Past if self.past == 0 => {
                    "was never seen writing past the length it was told"
                }
                Kernel::Through if self.past > 0 => "wrote past its header or rooms",
                Kernel::Through if self.through == 0 => "was never seen writing into its rooms",
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
            "calls {} ok {} inside {} past {} before {} shorter {} longer {} through {} errors {:?}",
            self.calls,
            self.succeeded,
            self.inside,
            self.past,
            self.before,
            self.shorter,
            self.longer,
            self.through,
            self.errors
        )
    }
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

/// Puts a TCP socket in repair mode, as root.
fn repaired(prepared: Prepared) -> Prepared {
    set_value(
        prepared.socket.as_fd(),
        uapi::SOL_TCP,
        uapi::TCP_REPAIR,
        &ON,
    )
    .expect("repair mode, as root");
    prepared
}

fn repair_connected() -> Prepared {
    repaired(tcp4_pair())
}

/// A TCP socket in repair mode, not connected, with its send queue chosen.
fn send_queue_chosen() -> Prepared {
    let prepared = repaired(tcp4());
    set_value(
        prepared.socket.as_fd(),
        uapi::SOL_TCP,
        uapi::TCP_REPAIR_QUEUE,
        &(uapi::TCP_SEND_QUEUE as c_int),
    )
    .expect("the send queue chosen");
    prepared
}

/// A connection accepted by a listener that saved its SYN.
fn saved_syn() -> Prepared {
    let listener = new_socket(Domain::IPV4, Type::STREAM, Protocol::TCP);
    set_value(listener.as_fd(), uapi::SOL_TCP, uapi::TCP_SAVE_SYN, &ON).expect("SYNs saved");
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
    crate::set_program(
        prepared.socket.as_fd(),
        uapi::SOL_SOCKET,
        uapi::SO_ATTACH_REUSEPORT_CBPF,
        &[RETURN_FIRST],
    )
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

/// The bytes of a `struct group_req` for the group on loopback.
fn group_request(group: SocketAddr) -> Vec<u8> {
    let mut bytes = vec![0; size_of::<uapi::group_req>()];
    put(
        &mut bytes,
        offset_of!(uapi::group_req, gr_interface),
        &loopback_index().to_ne_bytes(),
    );
    put(
        &mut bytes,
        offset_of!(uapi::group_req, gr_group),
        storage_of(group).as_bytes(),
    );
    bytes
}

fn group_v4() -> SocketAddr {
    SocketAddr::from((GROUP_V4, 0))
}

fn group_v6() -> SocketAddr {
    SocketAddr::from((GROUP_V6, 0))
}

fn joined(prepared: Prepared, level: u32, group: SocketAddr) -> Prepared {
    set_value(
        prepared.socket.as_fd(),
        level,
        uapi::MCAST_JOIN_GROUP,
        group_request(group).as_slice(),
    )
    .expect("the group joined");
    prepared
}

fn group_v4_joined() -> Prepared {
    joined(udp4(), uapi::SOL_IP, group_v4())
}

fn group_v6_joined() -> Prepared {
    joined(udp6(), uapi::SOL_IPV6, group_v6())
}

/// The bytes of a source filter of the group on loopback, through the implementation's own writing.
fn group_filter(group: SocketAddr, mode: u32, sources: &[SocketAddr]) -> Vec<u8> {
    let sources: Vec<_> = sources.iter().map(|source| storage_of(*source)).collect();
    group_filter_bytes(&wcontainers_linux_socket_options::GroupFilter {
        interface: loopback_index(),
        group: storage_of(group),
        mode,
        sources: &sources,
    })
    .expect("a filter's bytes")
}

/// The bytes of a `struct ipv6_mreq` for the address on loopback.
fn anycast_request() -> Vec<u8> {
    let mut bytes = vec![0; size_of::<uapi::ipv6_mreq>()];
    put(
        &mut bytes,
        offset_of!(uapi::ipv6_mreq, ipv6mr_multiaddr),
        &ANYCAST.octets(),
    );
    put(
        &mut bytes,
        offset_of!(uapi::ipv6_mreq, ipv6mr_ifindex),
        &loopback_index().cast_signed().to_ne_bytes(),
    );
    bytes
}

fn anycast_joined() -> Prepared {
    let prepared = udp6();
    set_value(
        prepared.socket.as_fd(),
        uapi::SOL_IPV6,
        uapi::IPV6_JOIN_ANYCAST,
        anycast_request().as_slice(),
    )
    .expect("the anycast address joined");
    prepared
}

/// The bytes of a `struct in6_flowlabel_req` to loopback.
fn flow_label_request(label: u32, action: u32, share: u32, flags: u32) -> Vec<u8> {
    let mut bytes = vec![0; size_of::<uapi::in6_flowlabel_req>()];
    put(
        &mut bytes,
        offset_of!(uapi::in6_flowlabel_req, flr_dst),
        &Ipv6Addr::LOCALHOST.octets(),
    );
    put(
        &mut bytes,
        offset_of!(uapi::in6_flowlabel_req, flr_label),
        &label.to_be_bytes(),
    );
    put(
        &mut bytes,
        offset_of!(uapi::in6_flowlabel_req, flr_action),
        &[action as u8],
    );
    put(
        &mut bytes,
        offset_of!(uapi::in6_flowlabel_req, flr_share),
        &[share as u8],
    );
    put(
        &mut bytes,
        offset_of!(uapi::in6_flowlabel_req, flr_flags),
        &(flags as u16).to_ne_bytes(),
    );
    bytes
}

fn loopback_v4() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
}

/// The bytes of a `struct tcp_md5sig` for loopback, with the key given or none.
fn md5_key(key: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; size_of::<uapi::tcp_md5sig>()];
    put(
        &mut bytes,
        offset_of!(uapi::tcp_md5sig, tcpm_addr),
        storage_of(loopback_v4()).as_bytes(),
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_md5sig, tcpm_keylen),
        &u16::try_from(key.len()).expect("a short key").to_ne_bytes(),
    );
    put(&mut bytes, offset_of!(uapi::tcp_md5sig, tcpm_key), key);
    bytes
}

/// The bytes of a `struct tcp_ao_add` of a key for loopback with the identifier given both ways.
fn ao_key(identifier: u8) -> Vec<u8> {
    let mut bytes = vec![0; size_of::<uapi::tcp_ao_add>()];
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_add, addr),
        storage_of(loopback_v4()).as_bytes(),
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_add, alg_name),
        AO_ALGORITHM,
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_add, prefix),
        &[ONE_ADDRESS_V4],
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_add, sndid),
        &[identifier],
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_add, rcvid),
        &[identifier],
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_add, keylen),
        &[SECRET.len() as u8],
    );
    put(&mut bytes, offset_of!(uapi::tcp_ao_add, key), &SECRET);
    bytes
}

/// The bytes of a `struct tcp_ao_del` of the key with the identifier given both ways.
fn ao_deletion(identifier: u8) -> Vec<u8> {
    let mut bytes = vec![0; size_of::<uapi::tcp_ao_del>()];
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_del, addr),
        storage_of(loopback_v4()).as_bytes(),
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_del, prefix),
        &[ONE_ADDRESS_V4],
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_del, sndid),
        &[identifier],
    );
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_del, rcvid),
        &[identifier],
    );
    bytes
}

/// Gives the socket the TCP-AO keys of `KEY_IDENTIFIERS`.
fn keyed(prepared: Prepared) -> Prepared {
    for identifier in KEY_IDENTIFIERS {
        set_value(
            prepared.socket.as_fd(),
            uapi::SOL_TCP,
            uapi::TCP_AO_ADD_KEY,
            ao_key(identifier).as_slice(),
        )
        .expect("a TCP-AO key");
    }
    prepared
}

fn ao_keyed() -> Prepared {
    keyed(tcp4())
}

/// A TCP socket whose TCP-AO state is made, with no key.
fn ao_informed() -> Prepared {
    let prepared = tcp4();
    set_value(
        prepared.socket.as_fd(),
        uapi::SOL_TCP,
        uapi::TCP_AO_INFO,
        vec![0; size_of::<uapi::tcp_ao_info_opt>()].as_slice(),
    )
    .expect("the TCP-AO state");
    prepared
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

/// A `TCP_AO_GET_KEYS` query for every key, asking for `keys` answers.
fn every_key_query(keys: u32) -> Vec<u8> {
    let mut bytes = vec![0; KEY_QUERY_LENGTH];
    put(
        &mut bytes,
        offset_of!(uapi::tcp_ao_getsockopt, nkeys),
        &keys.to_ne_bytes(),
    );
    bytes[offset_of!(uapi::tcp_ao_getsockopt, _bitfield_1)] = GET_ALL;
    bytes
}

fn int(value: c_int) -> Vec<u8> {
    value.to_ne_bytes().to_vec()
}

fn word(value: u32) -> Vec<u8> {
    value.to_ne_bytes().to_vec()
}

fn flags(_: BorrowedFd<'_>) -> Vec<Vec<u8>> {
    vec![int(ON), int(OFF)]
}

fn small(_: BorrowedFd<'_>) -> Vec<Vec<u8>> {
    vec![int(ON)]
}

fn zero_word(_: BorrowedFd<'_>) -> Vec<Vec<u8>> {
    vec![word(0)]
}

fn set(prepare: fn() -> Prepared, values: fn(BorrowedFd<'_>) -> Vec<Vec<u8>>) -> Vec<Case> {
    vec![Case {
        prepare,
        input: Input::Values(values),
    }]
}

fn get(prepare: fn() -> Prepared, rooms: &'static [usize]) -> Vec<Case> {
    vec![Case {
        prepare,
        input: Input::Rooms(rooms),
    }]
}

/// The way each row is called, by method.
fn cases() -> Vec<(&'static str, Vec<Case>)> {
    vec![
        ("set_so_rcvmark", set(udp4, flags)),
        ("set_so_rcvpriority", set(udp4, flags)),
        (
            "set_so_attach_reuseport_cbpf",
            vec![Case {
                prepare: reuse_port_group,
                input: Input::Program,
            }],
        ),
        (
            "set_so_detach_reuseport_bpf",
            set(reuse_port_program, |_| vec![int(OFF)]),
        ),
        (
            "set_so_max_pacing_rate",
            set(udp4, |_| {
                vec![u64::MAX.to_ne_bytes().to_vec(), word(u32::MAX), vec![0; 8]]
            }),
        ),
        ("set_so_rcvlowat", set(udp4, small)),
        ("get_so_rcvlowat", get(udp4, &NUMBER_ROOMS)),
        (
            "set_so_buf_lock",
            set(udp4, |_| vec![word(uapi::SOCK_SNDBUF_LOCK)]),
        ),
        (
            "set_so_txrehash",
            set(tcp4, |_| vec![int(uapi::SOCK_TXREHASH_ENABLED as c_int)]),
        ),
        ("set_so_cnx_advice", set(tcp4_pair, |_| vec![int(ON)])),
        ("set_so_wifi_status", set(udp4, flags)),
        ("set_so_peek_off", set(tcp4, small)),
        ("get_so_meminfo", get(udp4, &ROOMS)),
        ("set_so_no_check", set(udp4, flags)),
        (
            "set_so_timestamping",
            set(udp4, |_| {
                let mut bytes = vec![0; size_of::<uapi::so_timestamping>()];
                put(
                    &mut bytes,
                    offset_of!(uapi::so_timestamping, flags),
                    &(uapi::SOF_TIMESTAMPING_RX_SOFTWARE as c_int).to_ne_bytes(),
                );
                vec![bytes]
            }),
        ),
        (
            "set_ip_options",
            set(udp4, |_| vec![Vec::new(), PADDING_OPTIONS.to_vec()]),
        ),
        ("set_ip_unicast_if", set(udp4, zero_word)),
        (
            "set_ip_local_port_range",
            set(udp4, |_| {
                vec![word(
                    u32::from(u16::MAX) << u16::BITS | u32::from(u16::MAX / 2),
                )]
            }),
        ),
        ("set_ip_router_alert", set(raw4, flags)),
        ("set_ip_nodefrag", set(raw4, flags)),
        ("set_ip_recvopts", set(udp4, flags)),
        ("set_ip_recvfragsize", set(udp4, flags)),
        ("set_ip_minttl", set(tcp4, small)),
        ("get_ip_pktoptions", get(tcp4_pair, &ROOMS)),
        (
            "set_ip_mcast_join_group",
            set(udp4, |_| vec![group_request(group_v4())]),
        ),
        (
            "set_ip_mcast_leave_group",
            set(group_v4_joined, |_| vec![group_request(group_v4())]),
        ),
        (
            "set_ip_mcast_msfilter",
            set(group_v4_joined, |_| {
                vec![group_filter(
                    group_v4(),
                    uapi::MCAST_INCLUDE,
                    &[loopback_v4()],
                )]
            }),
        ),
        (
            "set_ipv6_hopopts",
            set(udp6, |_| vec![Vec::new(), PADDING_HEADER.to_vec()]),
        ),
        (
            "set_ipv6_rthdrdstopts",
            set(udp6, |_| vec![Vec::new(), PADDING_HEADER.to_vec()]),
        ),
        ("set_ipv6_rthdr", set(udp6, |_| vec![Vec::new()])),
        (
            "set_ipv6_dstopts",
            set(udp6, |_| vec![Vec::new(), PADDING_HEADER.to_vec()]),
        ),
        ("set_ipv6_flowinfo_send", set(udp6, flags)),
        ("set_ipv6_autoflowlabel", set(udp6, flags)),
        (
            "set_ipv6_flowlabel_mgr",
            set(udp6, |_| {
                vec![
                    flow_label_request(
                        0,
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
                    flow_label_request(FLOW_LABEL, uapi::IPV6_FL_A_PUT, 0, 0),
                    flow_label_request(0, uapi::IPV6_FL_A_GET, 0, uapi::IPV6_FL_F_REFLECT),
                ]
            }),
        ),
        (
            "set_ipv6_addr_preferences",
            set(udp6, |_| {
                vec![word(
                    uapi::IPV6_PREFER_SRC_PUBLIC | uapi::IPV6_PREFER_SRC_HOME,
                )]
            }),
        ),
        (
            "set_ipv6_join_anycast",
            set(udp6, |_| vec![anycast_request()]),
        ),
        (
            "set_ipv6_leave_anycast",
            set(anycast_joined, |_| vec![anycast_request()]),
        ),
        (
            "set_ipv6_mtu",
            set(udp6, |_| vec![int(c_int::from(u16::MAX))]),
        ),
        ("set_ipv6_unicast_if", set(udp6, zero_word)),
        ("set_ipv6_router_alert", set(raw6, |_| vec![int(OFF)])),
        ("set_ipv6_router_alert_isolate", set(raw6, flags)),
        ("set_ipv6_checksum", set(raw6, |_| vec![int(-1)])),
        ("set_ipv6_recvhopopts", set(udp6, flags)),
        ("set_ipv6_recvdstopts", set(udp6, flags)),
        ("set_ipv6_recvrthdr", set(udp6, flags)),
        ("set_ipv6_flowinfo", set(udp6, flags)),
        ("set_ipv6_recvfragsize", set(udp6, flags)),
        ("set_ipv6_minhopcount", set(tcp6, small)),
        ("get_ipv6_2292pktoptions", get(tcp6_pair, &ROOMS)),
        (
            "set_ipv6_mcast_join_group",
            set(udp6, |_| vec![group_request(group_v6())]),
        ),
        (
            "set_ipv6_mcast_leave_group",
            set(group_v6_joined, |_| vec![group_request(group_v6())]),
        ),
        (
            "set_ipv6_mcast_msfilter",
            set(group_v6_joined, |_| {
                vec![group_filter(group_v6(), uapi::MCAST_EXCLUDE, &[])]
            }),
        ),
        (
            "set_tcp_md5sig_ext",
            set(tcp4, |_| vec![md5_key(&SECRET), md5_key(&[])]),
        ),
        (
            "set_tcp_ao_add_key",
            set(tcp4, |_| vec![ao_key(KEY_IDENTIFIERS[0])]),
        ),
        (
            "get_tcp_ao_get_keys",
            vec![Case {
                prepare: ao_keyed,
                input: Input::Queries(|| {
                    vec![every_key_query(1), every_key_query(2), every_key_query(0)]
                }),
            }],
        ),
        (
            "set_tcp_ao_del_key",
            set(ao_keyed, |_| vec![ao_deletion(KEY_IDENTIFIERS[1])]),
        ),
        (
            "set_tcp_ao_info",
            set(tcp4, |_| vec![vec![0; size_of::<uapi::tcp_ao_info_opt>()]]),
        ),
        ("get_tcp_ao_info", get(ao_informed, &AO_INFORMATION_ROOMS)),
        (
            "set_tcp_ao_repair",
            set(ao_repaired, |_| {
                vec![vec![0; size_of::<uapi::tcp_ao_repair>()]]
            }),
        ),
        ("get_tcp_ao_repair", get(ao_repaired, &AO_REPAIR_ROOMS)),
        (
            "set_tcp_window_clamp",
            set(tcp4_pair, |_| vec![int(c_int::from(u16::MAX))]),
        ),
        ("get_tcp_window_clamp", get(tcp4_pair, &NUMBER_ROOMS)),
        ("set_tcp_syncnt", set(tcp4, small)),
        ("set_tcp_defer_accept", set(tcp4, small)),
        ("set_tcp_linger2", set(tcp4, small)),
        ("set_tcp_rto_min_us", set(tcp4, |_| vec![int(200_000)])),
        ("set_tcp_rto_max_ms", set(tcp4, |_| vec![int(120_000)])),
        ("set_tcp_delack_max_us", set(tcp4, |_| vec![int(200_000)])),
        ("set_tcp_tx_delay", set(tcp4, small)),
        ("set_tcp_fastopen_no_cookie", set(tcp4, flags)),
        ("set_tcp_inq", set(tcp4, flags)),
        (
            "set_tcp_queue_seq",
            set(send_queue_chosen, |_| vec![word(SEQUENCE)]),
        ),
        ("get_tcp_queue_seq", get(send_queue_chosen, &NUMBER_ROOMS)),
        ("get_tcp_saved_syn", get(saved_syn, &ROOMS)),
        ("set_tcp_fastopen", set(tcp4, |_| vec![word(5)])),
        (
            "set_tcp_fastopen_key",
            set(tcp4, |_| vec![SECRET.to_vec(), [SECRET, SECRET].concat()]),
        ),
        ("set_tcp_save_syn", set(tcp4, small)),
        (
            "set_tcp_repair_queue",
            set(repair_connected, |_| {
                vec![int(uapi::TCP_SEND_QUEUE as c_int)]
            }),
        ),
        (
            "get_tcp_repair_queue",
            get(send_queue_chosen, &NUMBER_ROOMS),
        ),
        (
            "set_tcp_timestamp",
            set(repair_connected, |_| vec![word(SEQUENCE)]),
        ),
        ("get_tcp_timestamp", get(tcp4_pair, &NUMBER_ROOMS)),
        (
            "set_tcp_quickack",
            set(tcp4_pair, |_| vec![int(2), int(ON)]),
        ),
        ("get_tcp_info", get(tcp4_pair, &ROOMS)),
        ("get_tcp_cc_info", get(tcp4_pair, &ROOMS)),
        (
            "set_tcp_repair_window",
            set(repair_connected, |socket| {
                let mut window = vec![0; size_of::<uapi::tcp_repair_window>()];
                get_into(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_WINDOW, &mut window)
                    .expect("the window");
                vec![window]
            }),
        ),
        (
            "get_tcp_repair_window",
            get(repair_connected, &REPAIR_WINDOW_ROOMS),
        ),
        (
            "set_tcp_repair_options",
            set(repair_connected, |_| {
                vec![[word(MAXIMUM_SEGMENT_SIZE), word(SEGMENT_SIZE)].concat()]
            }),
        ),
        ("get_mptcp_info", get(mptcp_pair, &ROOMS)),
        (
            "get_mptcp_full_info",
            vec![Case {
                prepare: mptcp_pair,
                input: Input::FullInformation,
            }],
        ),
        ("set_udp_no_check6_rx", set(udp6, flags)),
        ("set_udp_no_check6_tx", set(udp6, flags)),
        ("set_udplite_recv_cscov", set(udplite4, |_| vec![int(8)])),
        ("set_udplite_send_cscov", set(udplite4, |_| vec![int(8)])),
        (
            "get_siocoutq",
            vec![Case {
                prepare: tcp4_pair,
                input: Input::Ioctl,
            }],
        ),
        (
            "get_siocoutqnsd",
            vec![Case {
                prepare: tcp4_pair,
                input: Input::Ioctl,
            }],
        ),
        (
            "get_siocatmark",
            vec![Case {
                prepare: tcp4_pair,
                input: Input::Ioctl,
            }],
        ),
    ]
}

/// Gives `bytes` with `GUARD` canary bytes on each side.
fn guarded(canary: u8, bytes: &[u8]) -> Vec<u8> {
    let mut buffer = vec![canary; GUARD];
    buffer.extend_from_slice(bytes);
    buffer.extend(std::iter::repeat_n(canary, GUARD));
    buffer
}

/// Records how the guarded buffer differs from what it held: before the value, inside it, past it.
fn compare(seen: &mut Seen, canary: u8, buffer: &[u8], given: &[u8]) {
    let (before, rest) = buffer.split_at(GUARD);
    let (inside, past) = rest.split_at(given.len());
    seen.before += usize::from(before.iter().any(|byte| *byte != canary));
    seen.inside += usize::from(inside != given);
    seen.past += usize::from(past.iter().any(|byte| *byte != canary));
}

fn level_of(row: &Row) -> u32 {
    row.level.expect("an option has a level")
}

/// Sets the row's option to `value` from between canaries.
fn set_once(seen: &mut Seen, row: &Row, socket: BorrowedFd<'_>, value: &[u8]) {
    for canary in CANARIES {
        let mut buffer = guarded(canary, value);
        let length = length_of(value.len()).expect("a value's length");
        let at = buffer[GUARD..].as_mut_ptr().cast_const().cast();
        let result = set_bytes_at(socket, level_of(row), row.option, at, length);
        seen.record(&result);
        compare(seen, canary, &buffer, value);
    }
}

/// Attaches a program of `count` instructions, its header and its instructions each between canaries.
fn program_once(seen: &mut Seen, row: &Row, socket: BorrowedFd<'_>, count: usize) {
    let mut instruction = [0; size_of::<libc::sock_filter>()];
    put(
        &mut instruction,
        offset_of!(libc::sock_filter, code),
        &RETURN_FIRST.code.to_ne_bytes(),
    );
    let instructions = instruction.repeat(count);
    for canary in CANARIES {
        let code = guarded(canary, &instructions);
        let mut header = [0; size_of::<libc::sock_fprog>()];
        put(
            &mut header,
            offset_of!(libc::sock_fprog, len),
            &u16::try_from(count).expect("a short program").to_ne_bytes(),
        );
        put(
            &mut header,
            offset_of!(libc::sock_fprog, filter),
            &code[GUARD..].as_ptr().expose_provenance().to_ne_bytes(),
        );
        let mut buffer = guarded(canary, &header);
        let length = length_of(header.len()).expect("a header's length");
        let at = buffer[GUARD..].as_mut_ptr().cast_const().cast();
        let result = set_bytes_at(socket, level_of(row), row.option, at, length);
        seen.record(&result);
        compare(seen, canary, &buffer, &header);
        compare(seen, canary, &code, &instructions);
    }
}

/// Reads the row's option into a room of `given`'s length holding `given`, between canaries.
fn get_once(seen: &mut Seen, row: &Row, socket: BorrowedFd<'_>, canary: u8, given: &[u8]) {
    let mut buffer = guarded(canary, given);
    let told = given.len();
    let result = get_within(
        socket,
        level_of(row),
        row.option,
        &mut buffer[GUARD..],
        told,
    );
    seen.record(&result);
    if let Ok(answered) = result {
        let answered = usize::try_from(answered).expect("a length");
        seen.shorter += usize::from(answered < told);
        seen.longer += usize::from(answered > told);
    }
    compare(seen, canary, &buffer, given);
}

/// Makes the row's ioctl request into an int's room between canaries.
fn ioctl_once(seen: &mut Seen, row: &Row, socket: BorrowedFd<'_>) {
    for canary in CANARIES {
        let given = [canary; size_of::<c_int>()];
        let mut buffer = guarded(canary, &given);
        let result = ioctl_within(socket, row.option, &mut buffer[GUARD..]);
        seen.record(&result);
        compare(seen, canary, &buffer, &given);
    }
}

/// `MPTCP_FULL_INFO`'s header between canaries.
#[repr(C)]
struct GuardedHeader {
    before: [u8; GUARD],
    header: FullInformationHeader,
    after: [u8; GUARD],
}

/// Reads `MPTCP_FULL_INFO` told `count` entries, each run of rooms with a canary entry on each side.
fn full_information_once<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize>(
    seen: &mut Seen,
    row: &Row,
    socket: BorrowedFd<'_>,
    count: usize,
) {
    for canary in CANARIES {
        let mut header = GuardedHeader {
            before: [canary; GUARD],
            header: FullInformationHeader::new_zeroed(),
            after: [canary; GUARD],
        };
        let mut subflows = vec![[canary; SUBFLOW_ROOM]; count + 2];
        let mut information = vec![[canary; INFORMATION_ROOM]; count + 2];
        let result = full_info_into(
            socket,
            level_of(row),
            row.option,
            &mut header.header,
            &mut subflows[1..],
            &mut information[1..],
            count,
        );
        seen.record(&result);
        let untouched = |bytes: &[u8]| bytes.iter().all(|byte| *byte == canary);
        seen.before += usize::from(
            !untouched(&header.before) || !untouched(&subflows[0]) || !untouched(&information[0]),
        );
        seen.past += usize::from(
            !untouched(&header.after)
                || !untouched(&subflows[count + 1])
                || !untouched(&information[count + 1]),
        );
        seen.through += usize::from(
            subflows[1..=count]
                .iter()
                .map(|room| &room[..])
                .chain(information[1..=count].iter().map(|room| &room[..]))
                .any(|room| !untouched(room)),
        );
    }
}

/// Calls the row as the case says, a fresh socket for each call, and gives what the calls did.
fn calls(row: &Row, case: &Case) -> Seen {
    let mut seen = Seen::default();
    match &case.input {
        Input::Values(values) => {
            let count = values((case.prepare)().socket.as_fd()).len();
            for index in 0..count {
                let prepared = (case.prepare)();
                let socket = prepared.socket.as_fd();
                let value = values(socket).swap_remove(index);
                set_once(&mut seen, row, socket, &value);
            }
        }
        Input::Program => {
            for count in PROGRAM_LENGTHS {
                let prepared = (case.prepare)();
                program_once(&mut seen, row, prepared.socket.as_fd(), count);
            }
        }
        Input::Rooms(rooms) => {
            for room in rooms.iter().copied() {
                for fill in CANARIES.into_iter().chain([0]) {
                    let prepared = (case.prepare)();
                    let canary = if fill == 0 { CANARIES[0] } else { fill };
                    get_once(
                        &mut seen,
                        row,
                        prepared.socket.as_fd(),
                        canary,
                        &vec![fill; room],
                    );
                }
            }
        }
        Input::Queries(queries) => {
            for query in queries() {
                for canary in CANARIES {
                    let prepared = (case.prepare)();
                    get_once(&mut seen, row, prepared.socket.as_fd(), canary, &query);
                }
            }
        }
        Input::FullInformation => {
            for count in SUBFLOW_COUNTS {
                let prepared = (case.prepare)();
                let socket = prepared.socket.as_fd();
                full_information_once::<SHORT_ENTRY, SHORT_ENTRY>(&mut seen, row, socket, count);
                full_information_once::<LONG_ENTRY, LONG_ENTRY>(&mut seen, row, socket, count);
            }
        }
        Input::Ioctl => {
            let prepared = (case.prepare)();
            ioctl_once(&mut seen, row, prepared.socket.as_fd());
        }
    }
    seen
}

fn add(total: &mut Seen, more: Seen) {
    total.calls += more.calls;
    total.succeeded += more.succeeded;
    total.inside += more.inside;
    total.past += more.past;
    total.before += more.before;
    total.shorter += more.shorter;
    total.longer += more.longer;
    total.through += more.through;
    for error in more.errors {
        if !total.errors.contains(&error) {
            total.errors.push(error);
        }
    }
}

#[test]
fn every_row_is_probed_and_the_kernel_does_what_its_column_says() {
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
            add(&mut seen, calls(row, case));
        }
        println!("PROBE {} {:?} {} {seen}", row.method, row.kernel, row.value);
        if let Some(fault) = seen.against(row.kernel) {
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
