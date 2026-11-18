//! Sockets, as Linux's `net/socket.c` and, for local sockets, `net/unix/af_unix.c`: each socket
//! is an inode of its own on sockfs, its file a descriptor of it. Local sockets of every kind
//! (stream, datagram, sequenced packets) carry bytes, descriptors (SCM_RIGHTS) and credentials
//! (SCM_CREDENTIALS) between processes wholly inside the kernel. Internet sockets are in
//! [`inet`].

mod inet;
pub(crate) mod netlink;
mod outside;
mod procnet;
mod resolver;

pub(crate) use inet::Ports;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;

use super::{Args, guest};
use crate::file::{FileRef, OpenFile};
use crate::fs::{Content, Node};
use crate::linux::general::*;
use crate::linux::net::{
    AF_INET, AF_INET6, AF_NETLINK, AF_UNIX, MSG_CMSG_CLOEXEC, MSG_CTRUNC, MSG_DONTWAIT,
    MSG_NOSIGNAL, MSG_OOB, MSG_PEEK, MSG_TRUNC, MSG_WAITALL, SCM_CREDENTIALS, SCM_RIGHTS,
    SHUT_RDWR, SOCK_DGRAM, SOCK_RAW, SOCK_SEQPACKET, SOCK_STREAM, SOL_SOCKET,
};
use crate::process::{Channel, Pid, Tid};
use crate::{Context, Errno, Kernel};

pub(crate) type SocketRef = Rc<RefCell<Socket>>;

/// What `sk_shutdown` holds.
const RCV_SHUTDOWN: u8 = 1;
const SEND_SHUTDOWN: u8 = 2;
const SHUTDOWN_MASK: u8 = 3;

/// `net.core.wmem_default`, `rmem_default` and `wmem_max`, `rmem_max`, the largest settable
/// values (measured).
const DEFAULT_BUFFER: u64 = 212_992;
const MAX_BUFFER: u64 = 4_194_304;
/// `SOCK_MIN_SNDBUF` and `SOCK_MIN_RCVBUF` on ARM64.
const MIN_SNDBUF: u64 = 4608;
const MIN_RCVBUF: u64 = 2304;
/// `net.unix.max_dgram_qlen`: how many datagrams a socket's queue holds from senders it is not
/// connected back to.
const MAX_DGRAM_QUEUE: usize = 10;
/// The most descriptors one message carries (`SCM_MAX_FD`).
const MAX_FDS: usize = 253;
/// `struct sockaddr_un`'s size and its path's offset.
const ADDRESS_SIZE: usize = 110;
const PATH_OFFSET: usize = 2;
/// A `cmsghdr`'s size and alignment on ARM64.
const CMSG_HEADER: u64 = 16;
const CMSG_ALIGN: u64 = 8;

/// The credentials a message carries (`unix_skb_parms`): the sending process, and its real ids
/// or those it named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cred {
    pub pid: Pid,
    pub uid: u32,
    pub gid: u32,
}

/// One buffer in a receive queue (an `sk_buff`): its bytes, how many a stream reader has taken,
/// what it carries, who sent it and what it costs its sender (its true size).
struct Packet {
    data: Vec<u8>,
    consumed: usize,
    files: Vec<FileRef>,
    cred: Cred,
    sender: Weak<RefCell<Socket>>,
    truesize: u64,
    /// Where an internet datagram came from, and what its header and route said.
    source: Option<inet::Endpoint>,
    arrival: Option<inet::Arrival>,
    /// When it came, for a receiver that asked for timestamps when it was sent.
    time: Option<crate::fs::Time>,
    /// The urgent byte of a local stream's `MSG_OOB` that the receiver's mark is at (`oob_skb`).
    urgent: bool,
    /// The real user that sent its descriptors, whose count in flight they add to
    /// (`fpl->user`).
    owner: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// Not connected (`TCP_CLOSE`), or a connection that ended.
    Closed,
    Listening,
    /// Connected (`TCP_ESTABLISHED`).
    Connected,
}

/// One socket.
pub(crate) struct Socket {
    pub family: u32,
    pub kind: u32,
    pub protocol: u32,
    pub state: State,
    /// The other end: a connection's, or a datagram socket's default destination.
    peer: Option<SocketRef>,
    /// The name it is bound to (`sockaddr_un` bytes from the path on), and for a path its node.
    name: Option<Vec<u8>>,
    path: Option<(u64, u64)>,
    queue: VecDeque<Packet>,
    /// For a listener: the connected sockets waiting to be accepted, and how many may wait.
    backlog: VecDeque<SocketRef>,
    max_backlog: u32,
    shutdown: u8,
    /// A pending error (`sk_err`).
    error: i32,
    sndbuf: u64,
    rcvbuf: u64,
    /// The true size of what it sent and its receivers hold (`sk_wmem_alloc`).
    written: u64,
    passcred: bool,
    /// SO_PASSPIDFD, SO_PASSRIGHTS (on unless turned off) and SO_INQ: what its receives carry.
    pass_pidfd: bool,
    pass_rights: bool,
    report_queued: bool,
    /// The credentials of the other end, as at connect or listen (`sk_peer_cred`), and its
    /// supplementary groups.
    peer_cred: Option<Cred>,
    peer_groups: Vec<u32>,
    /// Datagrams dropped for want of room (`sk_drops`).
    drops: u32,
    /// Its SO_COOKIE, given when first asked for; 0 until then.
    cookie: u64,
    /// Released: its last descriptor is gone (`SOCK_DEAD`).
    dead: bool,
    /// Options kept and given back as set.
    options: BTreeMap<(u32, u32), Vec<u8>>,
    /// Where `MSG_PEEK` reads from when set (`SO_PEEK_OFF`).
    peek_offset: Option<u64>,
    /// `SO_RCVTIMEO` and `SO_SNDTIMEO`, in nanoseconds; 0 waits for ever.
    receive_timeout: u64,
    send_timeout: u64,
    /// The effective user that bound it, for SO_REUSEPORT.
    owner_uid: u32,
    /// SO_TIMESTAMP and SO_TIMESTAMPNS (`SOCK_RCVTSTAMP`, `SOCK_RCVTSTAMPNS`, `SOCK_TSTAMP_NEW`).
    stamps: u8,
    /// SO_TIMESTAMPING's flags (`sk_tsflags`), and the key OPT_ID gives the next send's
    /// timestamps (`sk_tskey`).
    tsflags: u32,
    tskey: u32,
    /// Its inode's number and owner, once it has a file (`sock_i_ino`, `sock_i_uid`).
    ino: u64,
    uid: u32,
    /// Its file, whose owner hears of urgent bytes and, with `FASYNC`, of I/O.
    file: Weak<RefCell<OpenFile>>,
    /// Urgent bytes a local stream read passed over unread, which Linux's queued count keeps
    /// (`inq_len` is not lowered when `manage_oob` drops them).
    skipped_urgent: u64,
    /// A send found no room, so room coming back is told (`SOCKWQ_ASYNC_NOSPACE`).
    no_space: bool,
    /// A local stream reader waits for data, so data coming is not told (`SOCKWQ_ASYNC_WAITDATA`).
    waiting_data: bool,
    /// An internet socket's own part.
    inet: Option<alloc::boxed::Box<inet::Inet>>,
    /// A netlink socket's own part.
    netlink: Option<alloc::boxed::Box<netlink::Netlink>>,
}

impl Socket {
    fn new(family: u32, kind: u32, protocol: u32) -> Self {
        Self {
            family,
            kind,
            protocol,
            state: State::Closed,
            peer: None,
            name: None,
            path: None,
            queue: VecDeque::new(),
            backlog: VecDeque::new(),
            max_backlog: if kind == SOCK_DGRAM {
                MAX_DGRAM_QUEUE as u32
            } else {
                0
            },
            shutdown: 0,
            error: 0,
            sndbuf: DEFAULT_BUFFER,
            rcvbuf: DEFAULT_BUFFER,
            written: 0,
            passcred: false,
            pass_pidfd: false,
            pass_rights: true,
            report_queued: false,
            peer_cred: None,
            peer_groups: Vec::new(),
            drops: 0,
            cookie: 0,
            dead: false,
            options: BTreeMap::new(),
            peek_offset: None,
            receive_timeout: 0,
            send_timeout: 0,
            owner_uid: 0,
            stamps: 0,
            tsflags: 0,
            tskey: 0,
            ino: 0,
            uid: 0,
            file: Weak::new(),
            skipped_urgent: 0,
            no_space: false,
            waiting_data: false,
            inet: None,
            netlink: None,
        }
    }

    fn connection(&self) -> bool {
        self.kind == SOCK_STREAM || self.kind == SOCK_SEQPACKET
    }

    /// A local socket, not an internet or netlink one.
    pub(crate) fn local(&self) -> bool {
        self.inet.is_none() && self.netlink.is_none()
    }

    /// A netlink socket, which a message queue's SIGEV_THREAD request names.
    pub(crate) fn is_netlink(&self) -> bool {
        self.netlink.is_some()
    }

    /// A TCP socket, which `mmap` takes (`tcp_mmap`).
    pub(crate) fn tcp(&self) -> bool {
        self.inet
            .as_deref()
            .is_some_and(|inet| inet.protocol == inet::Protocol::Tcp)
    }

    /// Its protocol's name, which its inode's dentry carries (`sk_prot_creator->name`).
    pub(crate) fn protocol_name(&self) -> &'static [u8] {
        match self.family {
            AF_UNIX if self.kind == SOCK_STREAM => b"UNIX-STREAM",
            AF_UNIX => b"UNIX",
            AF_NETLINK => b"NETLINK",
            AF_INET6 if self.kind == SOCK_STREAM => b"TCPv6",
            AF_INET6 => b"UDPv6",
            _ if self.kind == SOCK_STREAM => b"TCP",
            _ => b"UDP",
        }
    }

    /// `unix_may_passcred`: its receives carry who sent.
    fn may_pass_cred(&self) -> bool {
        self.passcred || self.pass_pidfd
    }

    /// SO_OOBINLINE (`SOCK_URGINLINE`).
    fn urgent_inline(&self) -> bool {
        self.options
            .get(&(SOL_SOCKET, crate::linux::net::SO_OOBINLINE))
            .is_some_and(|value| value.iter().any(|byte| *byte != 0))
    }

    /// Whether its queue holds the urgent byte its mark is at (`oob_skb`).
    fn has_urgent(&self) -> bool {
        self.queue.iter().any(|packet| packet.urgent)
    }

    /// `unix_writable`: not listening, and what it sent holds at most a quarter of its buffer.
    fn unix_writable(&self) -> bool {
        self.state != State::Listening && self.written << 2 <= self.sndbuf
    }
}

/// What `sk_wake_async` is told: data or an end came (`SOCK_WAKE_WAITD`), room came back
/// (`SOCK_WAKE_SPACE`), or a connection or error was reported (`SOCK_WAKE_IO`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Wake {
    Data,
    Space,
    Io,
}

/// `sk_wake_async`: the I/O signal of a socket's file with `FASYNC` set, for `reason`. Data is
/// not told while a reader waits for it, and room only after a send found none.
pub(super) fn wake_async(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    socket: &SocketRef,
    how: Wake,
    reason: u32,
) {
    let file = {
        let mut own = socket.borrow_mut();
        match how {
            Wake::Data if own.waiting_data => return,
            Wake::Space if !core::mem::take(&mut own.no_space) => return,
            _ => {}
        }
        own.file.upgrade()
    };
    if let Some(file) = file {
        kernel.send_io_signal(cx, &file, reason);
    }
}

/// `sock_def_readable`: a socket's waiters and its I/O signal hear of data.
pub(super) fn data_ready(kernel: &mut Kernel, cx: &mut Context<'_>, socket: &SocketRef, key: u32) {
    kernel.wake_poll(channel(socket), key);
    wake_async(kernel, cx, socket, Wake::Data, POLL_IN);
}

/// `sock_def_error_report`: an error came, which its waiters and its I/O signal hear of.
pub(super) fn error_report(kernel: &mut Kernel, cx: &mut Context<'_>, socket: &SocketRef) {
    kernel.wake_poll(channel(socket), EPOLLERR);
    wake_async(kernel, cx, socket, Wake::Io, POLL_ERR);
}

/// `unix_write_space` and `sk_stream_write_space`: room came back to a sender, which its waiters
/// and, once it is writable and a send found none, its I/O signal hear of (a stream's only while
/// it may still send).
pub(super) fn write_space(kernel: &mut Kernel, cx: &mut Context<'_>, socket: &SocketRef) {
    kernel.wake_poll(channel(socket), EPOLLOUT | EPOLLWRNORM | EPOLLWRBAND);
    let tell = {
        let own = socket.borrow();
        match own.inet.as_deref() {
            Some(_) => inet::space_to_tell(&own),
            None => own.unix_writable(),
        }
    };
    if tell {
        wake_async(kernel, cx, socket, Wake::Space, POLL_OUT);
    }
}

/// The names local sockets are bound to: abstract names with the kind of socket (Linux hashes
/// both, so each kind has its own), and paths by their inode.
#[derive(Default)]
pub(crate) struct Names {
    abstract_names: BTreeMap<(u32, Vec<u8>), Weak<RefCell<Socket>>>,
    paths: BTreeMap<(u64, u64), Weak<RefCell<Socket>>>,
    /// The ports internet sockets hold.
    inet: Ports,
    /// The port numbers netlink sockets hold.
    netlink: BTreeMap<u32, Weak<RefCell<Socket>>>,
    /// Local sockets whose queues were given descriptors, for counting and collecting them.
    carriers: Vec<Weak<RefCell<Socket>>>,
    /// The last socket cookie given (`sock_gen_cookie`).
    cookies: u64,
}

/// Closes the host connections let go since the last flush.
pub(crate) fn flush_outside(kernel: &mut Kernel, cx: &mut Context<'_>) {
    kernel.sockets.inet.outside.flush(cx.host);
}

/// Hands a network event to the socket or lookup it is for; false if none waits for it.
pub(crate) fn outside_event(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    event: &wcontainers_abi::Event<'_>,
) -> bool {
    outside::event(kernel, cx, event)
}

/// The bytes of a file of `/proc/net`.
pub(crate) fn proc_net(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    file: crate::fs::proc::NetFile,
) -> Vec<u8> {
    procnet::contents(kernel, cx, file)
}

/// `unix_show_fdinfo`: for a local socket, the descriptors in flight to it: those its queue holds
/// while it is connected or takes datagrams, those its connections not yet accepted hold while it
/// listens. Other families show nothing.
pub(crate) fn fdinfo(socket: &SocketRef) -> Option<u64> {
    let own = socket.borrow();
    if !own.local() {
        return None;
    }
    let held = |socket: &Socket| {
        socket
            .queue
            .iter()
            .map(|packet| packet.files.len() as u64)
            .sum::<u64>()
    };
    Some(match own.state {
        _ if own.kind == SOCK_DGRAM => held(&own),
        State::Connected => held(&own),
        State::Listening => own
            .backlog
            .iter()
            .map(|waiting| held(&waiting.borrow()))
            .sum(),
        State::Closed => 0,
    })
}

/// A listener's deferral timer fired ([`inet::deferrals_due`]).
pub(crate) fn deferrals_due(kernel: &mut Kernel, cx: &mut Context<'_>, listener: &SocketRef) {
    inet::deferrals_due(kernel, cx, listener);
}

/// After a restore: connections to the outside read as closed.
pub(crate) fn restored(kernel: &mut Kernel, cx: &mut Context<'_>) {
    outside::restored(kernel, cx);
}

impl Names {
    fn portid_taken(&self, portid: u32) -> bool {
        self.netlink
            .get(&portid)
            .is_some_and(|socket| socket.upgrade().is_some_and(|socket| !socket.borrow().dead))
    }

    fn hold_portid(&mut self, portid: u32, socket: &SocketRef) {
        self.netlink.insert(portid, Rc::downgrade(socket));
    }

    fn let_go_portid(&mut self, portid: u32, socket: &SocketRef) {
        if self
            .netlink
            .get(&portid)
            .is_some_and(|held| core::ptr::eq(held.as_ptr(), Rc::as_ptr(socket)))
        {
            self.netlink.remove(&portid);
        }
    }
}

/// The address waits and epoll use for a socket.
pub(super) fn object(socket: &SocketRef) -> usize {
    Rc::as_ptr(socket) as *const () as usize
}

fn channel(socket: &SocketRef) -> Channel {
    Channel::Object(object(socket))
}

/// The socket behind an open file, if it is one.
pub(crate) fn of(file: &FileRef) -> Option<SocketRef> {
    match &file.borrow().node.borrow().content {
        Content::Socket(socket) => Some(socket.clone()),
        _ => None,
    }
}

/// The socket a descriptor names: EBADF without one, ENOTSOCK if it is no socket.
fn socket_of(kernel: &Kernel, tid: Tid, fd: u64) -> Result<(FileRef, SocketRef), Errno> {
    let file = super::io::file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let socket = of(&file).ok_or(Errno::ENOTSOCK)?;
    Ok((file, socket))
}

/// A new socket's inode and file (`sock_alloc`, `sock_alloc_file`): owned by the caller's
/// effective ids, mode 0777, on sockfs.
fn make_file(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: SocketRef,
    flags: u32,
) -> Result<FileRef, Errno> {
    let now = kernel.now(cx);
    let fs = match &kernel.files.sockfs {
        Some(fs) => fs.clone(),
        None => {
            let fs = kernel.files.numbers.file_system(crate::fs::Kind::Sockfs, 0);
            kernel.files.sockfs = Some(fs.clone());
            fs
        }
    };
    let ino = kernel.files.numbers.ino();
    let credentials = kernel.credentials(tid);
    let (uid, gid) = (credentials.fsuid, credentials.fsgid);
    {
        let mut own = socket.borrow_mut();
        own.ino = ino;
        own.uid = uid;
    }
    let node = crate::fs::new_inode(
        fs,
        ino,
        crate::fs::New {
            mode: S_IFSOCK | 0o777,
            uid,
            gid,
            rdev: 0,
            now,
            content: Content::Socket(socket.clone()),
            parent: None,
            name: Vec::new(),
        },
    )?;
    let link = alloc::format!("socket:[{ino}]").into_bytes();
    let opener = kernel.credentials(tid).serial;
    let file = OpenFile::new(node, O_RDWR | (flags & O_NONBLOCK), link, opener);
    socket.borrow_mut().file = Rc::downgrade(&file);
    Ok(file)
}

fn install(kernel: &mut Kernel, tid: Tid, file: FileRef, cloexec: bool) -> Result<u64, Errno> {
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    Ok(kernel.files_mut(tid).add(file, cloexec, 0, limit)? as u64)
}

/// The type and flags `socket` and `socketpair` take, checked as `__sys_socket` does.
fn kind_and_flags(kind: u64) -> Result<(u32, u32), Errno> {
    let kind = kind as u32;
    let flags = kind & !0xf;
    if flags & !(O_CLOEXEC | O_NONBLOCK) != 0 {
        return Err(Errno::EINVAL);
    }
    Ok((kind & 0xf, flags))
}

/// `unix_create`: the kind of local socket, raw being datagram.
fn unix_kind(kind: u32, protocol: u32) -> Result<u32, Errno> {
    if protocol != 0 && protocol != AF_UNIX {
        return Err(Errno::EPROTONOSUPPORT);
    }
    match kind {
        SOCK_STREAM | SOCK_DGRAM | SOCK_SEQPACKET => Ok(kind),
        SOCK_RAW => Ok(SOCK_DGRAM),
        _ => Err(Errno::ESOCKTNOSUPPORT),
    }
}

/// `__sock_create`: a socket of a family and kind, or what Linux answers.
fn create(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    family: u64,
    kind: u32,
    protocol: u64,
) -> Result<Socket, Errno> {
    let family = family as u32;
    if family >= crate::linux::net::AF_MAX {
        return Err(Errno::EAFNOSUPPORT);
    }
    if kind >= crate::linux_extra::SOCK_MAX {
        return Err(Errno::EINVAL);
    }
    match family {
        AF_UNIX => Ok(Socket::new(family, unix_kind(kind, protocol as u32)?, 0)),
        AF_INET | AF_INET6 => inet::create(kernel, cx, family, kind, protocol as u32),
        AF_NETLINK => netlink::create(kernel, cx, kind, protocol as u32),
        _ => Err(kernel.counters.refuse(
            cx.host,
            crate::reason::FAMILY_NOT_BUILT,
            family.into(),
            Errno::EAFNOSUPPORT,
        )),
    }
}

pub(super) fn socket(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [family, kind, protocol, ..]: Args,
) -> Result<u64, Errno> {
    let (kind, flags) = kind_and_flags(kind)?;
    let socket = Rc::new(RefCell::new(create(kernel, cx, family, kind, protocol)?));
    let file = make_file(kernel, cx, tid, socket, flags)?;
    install(kernel, tid, file, flags & O_CLOEXEC != 0)
}

pub(super) fn socketpair(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [family, kind, protocol, pair, ..]: Args,
) -> Result<u64, Errno> {
    let (kind, flags) = kind_and_flags(kind)?;
    // `__sys_socketpair` takes two descriptors and writes their numbers before it makes anything.
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let (a, b) = {
        let files = kernel.files(tid);
        let a = files.lowest_free(0, limit)?;
        (a, files.lowest_free(a + 1, limit)?)
    };
    guest(kernel, cx, tid).write(pair, &a.to_le_bytes())?;
    guest(kernel, cx, tid).write(pair + 4, &b.to_le_bytes())?;
    let made = create(kernel, cx, family, kind, protocol)?;
    if made.family != AF_UNIX {
        // Other families' `socketpair` is `sock_no_socketpair`.
        return Err(Errno::EOPNOTSUPP);
    }
    let (family, kind) = (made.family, made.kind);
    let cred = own_cred(kernel, tid);
    let groups = kernel.credentials(tid).groups.clone();
    let ends = [(); 2].map(|()| Rc::new(RefCell::new(Socket::new(family, kind, 0))));
    for (one, other) in [(0, 1), (1, 0)] {
        let mut socket = ends[one].borrow_mut();
        socket.peer = Some(ends[other].clone());
        socket.peer_cred = Some(cred);
        socket.peer_groups = groups.clone();
        socket.state = State::Connected;
    }
    let first = make_file(kernel, cx, tid, ends[0].clone(), flags)?;
    let second = make_file(kernel, cx, tid, ends[1].clone(), flags)?;
    // The descriptors taken above, which nothing has taken since.
    let cloexec = flags & O_CLOEXEC != 0;
    let first = install(kernel, tid, first, cloexec)?;
    let second = install(kernel, tid, second, cloexec)?;
    debug_assert_eq!((first, second), (a as u64, b as u64));
    Ok(0)
}

/// The caller's credentials as a message or connection carries them: its process and real ids.
fn own_cred(kernel: &Kernel, tid: Tid) -> Cred {
    let credentials = kernel.credentials(tid);
    Cred {
        pid: kernel.threads[&tid].process,
        uid: credentials.uid,
        gid: credentials.gid,
    }
}

/// What a message without credentials shows as them: no process, and the overflow ids.
const NO_CRED: Cred = Cred {
    pid: 0,
    uid: 65534,
    gid: 65534,
};

/// `unix_maybe_add_creds`: a local message carries the credentials its sender named, or its own
/// when either end passes them or the receiver is not yet accepted.
fn sent_cred(
    kernel: &Kernel,
    tid: Tid,
    socket: &SocketRef,
    receiver: &SocketRef,
    named: Option<Cred>,
) -> Cred {
    let carried = socket.borrow().may_pass_cred() || {
        let other = receiver.borrow();
        other.may_pass_cred() || other.file.strong_count() == 0
    };
    match named {
        Some(cred) => cred,
        None if carried => own_cred(kernel, tid),
        None => NO_CRED,
    }
}

/// The caller's credentials as SO_PEERCRED gives them: its process and effective ids.
fn peer_cred(kernel: &Kernel, tid: Tid) -> Cred {
    let credentials = kernel.credentials(tid);
    Cred {
        pid: kernel.threads[&tid].process,
        uid: credentials.euid,
        gid: credentials.egid,
    }
}

/// A local address as `unix_validate_addr` and `unix_mkname_bsd` take it: the bytes from the path
/// on, a path cut at its first zero; an empty one asks for an automatic name.
enum Name {
    Path(Vec<u8>),
    Abstract(Vec<u8>),
    Automatic,
}

fn read_name(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    length: u64,
) -> Result<Name, Errno> {
    let length = length as i32;
    if length < 0 || length as usize > ADDRESS_SIZE {
        return Err(Errno::EINVAL);
    }
    let bytes = guest(kernel, cx, tid).read(address, length as u64)?;
    parse_name(&bytes)
}

/// `unix_validate_addr` and the name a local address gives.
fn parse_name(bytes: &[u8]) -> Result<Name, Errno> {
    if bytes.len() > ADDRESS_SIZE {
        return Err(Errno::EINVAL);
    }
    if bytes.len() < PATH_OFFSET || u16::from_le_bytes([bytes[0], bytes[1]]) as u32 != AF_UNIX {
        return Err(Errno::EINVAL);
    }
    let path = &bytes[PATH_OFFSET..];
    Ok(if path.is_empty() {
        Name::Automatic
    } else if path[0] == 0 {
        Name::Abstract(path.to_vec())
    } else {
        let end = path
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(path.len());
        Name::Path(path[..end].to_vec())
    })
}

pub(super) fn bind(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, address, length, ..]: Args,
) -> Result<u64, Errno> {
    let (file, socket) = socket_of(kernel, tid, fd)?;
    if socket.borrow().inet.is_some() {
        let bytes = address_argument(kernel, cx, tid, address, length)?;
        return inet::bind(kernel, cx, tid, &socket, &bytes);
    }
    if socket.borrow().netlink.is_some() {
        let bytes = address_argument(kernel, cx, tid, address, length)?;
        return netlink::bind(kernel, tid, &socket, &bytes);
    }
    // `unix_bind`: the address is checked before the socket's state.
    let length_ok = (length as i32) >= PATH_OFFSET as i32;
    if !length_ok {
        let name = read_name(kernel, cx, tid, address, length);
        return match name {
            Err(error) => Err(error),
            Ok(_) => Err(Errno::EINVAL),
        };
    }
    let name = read_name(kernel, cx, tid, address, length)?;
    // `unix_autobind` comes first, and is done for a socket that has any name.
    if !matches!(name, Name::Automatic) && socket.borrow().name.is_some() {
        // `unix_bind_bsd` makes the node before it looks at the socket.
        if let Name::Path(path) = &name
            && super::files::lookup_path(kernel, cx, tid, path).is_ok()
        {
            return Err(Errno::EADDRINUSE);
        }
        return Err(Errno::EINVAL);
    }
    match name {
        Name::Automatic => autobind(kernel, cx, &socket),
        Name::Abstract(bytes) => {
            let key = (socket.borrow().kind, bytes.clone());
            if kernel
                .sockets
                .abstract_names
                .get(&key)
                .is_some_and(|other| other.strong_count() > 0)
            {
                return Err(Errno::EADDRINUSE);
            }
            kernel
                .sockets
                .abstract_names
                .insert(key, Rc::downgrade(&socket));
            socket.borrow_mut().name = Some(bytes);
            Ok(0)
        }
        Name::Path(path) => {
            // `unix_bind_bsd`: a socket node made with the socket inode's mode less the umask.
            let umask = kernel.fs(tid).umask;
            let mode = S_IFSOCK | (0o777 & !umask);
            let node = super::files::make_node(kernel, cx, tid, &path, mode).map_err(|error| {
                if error == Errno::EEXIST {
                    Errno::EADDRINUSE
                } else {
                    error
                }
            })?;
            let key = node_key(&node);
            kernel.sockets.paths.insert(key, Rc::downgrade(&socket));
            let mut bound = socket.borrow_mut();
            bound.path = Some(key);
            bound.name = Some(path);
            drop(bound);
            drop(file);
            Ok(0)
        }
    }
}

/// `move_addr_to_kernel`: an address of at most a `sockaddr_storage`.
fn address_argument(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    length: u64,
) -> Result<Vec<u8>, Errno> {
    let length = length as i32;
    if !(0..=128).contains(&length) {
        return Err(Errno::EINVAL);
    }
    if length == 0 {
        return Ok(Vec::new());
    }
    guest(kernel, cx, tid).read(address, length as u64)
}

fn node_key(node: &Node) -> (u64, u64) {
    let inode = node.borrow();
    (inode.fs.device, inode.ino)
}

/// SO_RCVLOWAT: how many bytes a TCP read waits for and its poll wants (`sock_rcvlowat`).
pub(super) fn receive_low_water(own: &Socket) -> u64 {
    own.options
        .get(&(SOL_SOCKET, crate::linux::net::SO_RCVLOWAT))
        .map_or(1, |value| {
            i32::from_le_bytes(value[..4].try_into().expect("4 bytes")).max(1) as u64
        })
}

/// `unix_autobind`: an abstract name of five hex digits not in use, counting on from one drawn at
/// random.
fn autobind(kernel: &mut Kernel, cx: &mut Context<'_>, socket: &SocketRef) -> Result<u64, Errno> {
    if socket.borrow().name.is_some() {
        return Ok(0);
    }
    let mut seed = [0u8; 4];
    cx.host.random(&mut seed);
    let start = u32::from_le_bytes(seed) & 0xfffff;
    let mut number = start;
    let kind = socket.borrow().kind;
    loop {
        number = (number + 1) & 0xfffff;
        let mut name = alloc::vec![0u8];
        name.extend_from_slice(alloc::format!("{number:05x}").as_bytes());
        let key = (kind, name.clone());
        if !kernel
            .sockets
            .abstract_names
            .get(&key)
            .is_some_and(|other| other.strong_count() > 0)
        {
            kernel
                .sockets
                .abstract_names
                .insert(key, Rc::downgrade(socket));
            socket.borrow_mut().name = Some(name);
            return Ok(0);
        }
        if number == start {
            return Err(Errno::ENOSPC);
        }
    }
}

/// The socket bound to a name (`unix_find_other`): a path must be a socket node the caller may
/// write, of the same kind as `kind`.
fn find(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    name: &Name,
    kind: u32,
) -> Result<SocketRef, Errno> {
    let found = match name {
        Name::Automatic => return Err(Errno::EINVAL),
        Name::Abstract(bytes) => {
            return kernel
                .sockets
                .abstract_names
                .get(&(kind, bytes.clone()))
                .and_then(Weak::upgrade)
                .ok_or(Errno::ECONNREFUSED);
        }
        Name::Path(path) => {
            let node = super::files::lookup_path(kernel, cx, tid, path)?;
            let caller = kernel.threads[&tid].process;
            let credentials = kernel.credentials(tid).clone();
            crate::fs::permission(&node.borrow(), &credentials, W_OK, caller)?;
            if node.borrow().file_type() != S_IFSOCK {
                return Err(Errno::ECONNREFUSED);
            }
            let found = kernel
                .sockets
                .paths
                .get(&node_key(&node))
                .and_then(Weak::upgrade)
                .ok_or(Errno::ECONNREFUSED)?;
            if found.borrow().kind == kind {
                // `unix_find_bsd` reads the socket's node.
                super::io::touch_atime(kernel, cx, &node, 0);
            }
            found
        }
    };
    if found.borrow().kind != kind {
        return Err(Errno::EPROTOTYPE);
    }
    Ok(found)
}

pub(super) fn listen(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, backlog, ..]: Args,
) -> Result<u64, Errno> {
    let (_, socket) = socket_of(kernel, tid, fd)?;
    // `__sys_listen`: the backlog is capped at `net.core.somaxconn`.
    let backlog = (backlog as i32).min(4096);
    if socket.borrow().inet.is_some() {
        return inet::listen(kernel, cx, tid, &socket, backlog);
    }
    if socket.borrow().netlink.is_some() {
        return Err(Errno::EOPNOTSUPP);
    }
    let cred = peer_cred(kernel, tid);
    let groups = kernel.credentials(tid).groups.clone();
    let mut listener = socket.borrow_mut();
    if !listener.connection() {
        return Err(Errno::EOPNOTSUPP);
    }
    if listener.name.is_none() {
        return Err(Errno::EINVAL);
    }
    // A named connected socket is left as it is, and the call succeeds (measured on the
    // reference: socket pairs, connected and accepted sockets, also after the peer closed).
    if listener.state == State::Connected {
        return Ok(0);
    }
    // A negative backlog counts as none (`sk_max_ack_backlog` is unsigned, read as an int).
    listener.max_backlog = backlog as u32;
    listener.state = State::Listening;
    listener.peer_cred = Some(cred);
    listener.peer_groups = groups;
    Ok(0)
}

pub(super) fn connect(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, address, length, ..]: Args,
) -> Result<u64, Errno> {
    // `__sys_connect`: the descriptor, the address taken in, then whether it is a socket.
    let file = super::io::file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let bytes = address_argument(kernel, cx, tid, address, length)?;
    let socket = of(&file).ok_or(Errno::ENOTSOCK)?;
    if socket.borrow().inet.is_some() {
        return inet::connect(kernel, cx, tid, &file, &socket, &bytes);
    }
    if socket.borrow().netlink.is_some() {
        return netlink::connect(kernel, cx, tid, &socket, &bytes);
    }
    if (length as i32) < PATH_OFFSET as i32 {
        return Err(Errno::EINVAL);
    }
    let name = read_name(kernel, cx, tid, address, length)?;
    let kind = socket.borrow().kind;
    if kind == SOCK_DGRAM {
        return dgram_connect(kernel, cx, tid, &socket, name);
    }
    if matches!(name, Name::Automatic) {
        return Err(Errno::EINVAL);
    }
    // `unix_stream_connect`: a socket that passes credentials takes a name first; the listener
    // is found and checked before the socket's own state.
    if socket.borrow().may_pass_cred() && socket.borrow().name.is_none() {
        autobind(kernel, cx, &socket)?;
    }
    let other = find(kernel, cx, tid, &name, kind)?;
    if other.borrow().state != State::Listening
        || other.borrow().dead
        || other.borrow().shutdown & RCV_SHUTDOWN != 0
    {
        return Err(Errno::ECONNREFUSED);
    }
    // `unix_recvq_full`: the backlog is full; wait for room unless nonblocking.
    if other.borrow().backlog.len() > other.borrow().max_backlog as usize {
        if file.borrow().flags & O_NONBLOCK != 0 {
            return Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            return Err(Errno::ERESTARTSYS);
        }
        kernel.park(tid, channel(&other));
        return Err(Errno::PARKED);
    }
    match socket.borrow().state {
        State::Connected => return Err(Errno::EISCONN),
        State::Listening => return Err(Errno::EINVAL),
        State::Closed => {}
    }
    // The listener's end of the connection, waiting to be accepted; it takes the listener's name
    // and the connecting process's credentials.
    let embryo = Rc::new(RefCell::new(Socket::new(socket.borrow().family, kind, 0)));
    {
        let mut new = embryo.borrow_mut();
        let listener = other.borrow();
        new.state = State::Connected;
        new.peer = Some(socket.clone());
        new.name = listener.name.clone();
        new.peer_cred = Some(peer_cred(kernel, tid));
        new.peer_groups = kernel.credentials(tid).groups.clone();
        new.passcred = listener.passcred;
        new.pass_pidfd = listener.pass_pidfd;
        new.pass_rights = listener.pass_rights;
    }
    {
        let mut client = socket.borrow_mut();
        client.state = State::Connected;
        client.peer = Some(embryo.clone());
        client.peer_cred = other.borrow().peer_cred;
        client.peer_groups = other.borrow().peer_groups.clone();
    }
    other.borrow_mut().backlog.push_back(embryo);
    data_ready(kernel, cx, &other, EPOLLIN | EPOLLRDNORM);
    Ok(0)
}

/// A datagram socket's connect: its default destination, or none with AF_UNSPEC.
fn dgram_connect(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    name: Name,
) -> Result<u64, Errno> {
    let other = find(kernel, cx, tid, &name, SOCK_DGRAM)?;
    if socket.borrow().may_pass_cred() && socket.borrow().name.is_none() {
        autobind(kernel, cx, socket)?;
    }
    let old = socket.borrow_mut().peer.replace(other.clone());
    socket.borrow_mut().state = State::Connected;
    if let Some(old) = old
        && !Rc::ptr_eq(&old, &other)
    {
        dgram_disconnected(kernel, cx, socket, &old);
    }
    Ok(0)
}

/// `unix_dgram_disconnected`: a datagram socket no longer connected to `other` loses what it
/// held, and if `other` lives and was connected back, `other` is told ECONNRESET.
fn dgram_disconnected(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    socket: &SocketRef,
    other: &SocketRef,
) {
    let queue = core::mem::take(&mut socket.borrow_mut().queue);
    if queue.is_empty() {
        return;
    }
    for packet in queue {
        drop_packet(kernel, cx, packet);
    }
    kernel.wake(channel(socket));
    let back = other
        .borrow()
        .peer
        .as_ref()
        .is_some_and(|back| Rc::ptr_eq(back, socket));
    if !other.borrow().dead && back {
        other.borrow_mut().error = Errno::ECONNRESET.0 as i32;
        kernel.wake(channel(other));
    }
}

pub(super) fn accept(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, address, length, ..]: Args,
) -> Result<u64, Errno> {
    accept4(kernel, cx, tid, [fd, address, length, 0, 0, 0])
}

pub(super) fn accept4(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, address, length, flags, ..]: Args,
) -> Result<u64, Errno> {
    // `__sys_accept4`: the descriptor, the flags, a free descriptor, then whether it is a socket.
    let file = super::io::file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let flags = flags as u32;
    if flags & !(O_CLOEXEC | O_NONBLOCK) != 0 {
        return Err(Errno::EINVAL);
    }
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    kernel.files(tid).lowest_free(0, limit)?;
    let socket = of(&file).ok_or(Errno::ENOTSOCK)?;
    if !socket.borrow().connection() {
        return Err(Errno::EOPNOTSUPP);
    }
    if socket.borrow().state != State::Listening {
        return Err(Errno::EINVAL);
    }
    let Some(embryo) = socket.borrow_mut().backlog.pop_front() else {
        if file.borrow().flags & O_NONBLOCK != 0 {
            return Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            return Err(Errno::ERESTARTSYS);
        }
        kernel.park(tid, channel(&socket));
        return Err(Errno::PARKED);
    };
    // Room in the backlog for one more connecting socket.
    kernel.wake_poll(channel(&socket), EPOLLOUT | EPOLLWRNORM);
    let is_inet = socket.borrow().inet.is_some();
    if is_inet {
        inet::accepted(kernel, cx, &socket);
    }
    let new = make_file(kernel, cx, tid, embryo.clone(), flags)?;
    if address != 0 {
        let bytes = if is_inet {
            inet::peer_bytes(&embryo)
        } else {
            let peer_name = embryo
                .borrow()
                .peer
                .as_ref()
                .and_then(|peer| peer.borrow().name.clone());
            address_bytes(peer_name.as_deref())
        };
        put_address(kernel, cx, tid, address, length, &bytes)?;
    }
    install(kernel, tid, new, flags & O_CLOEXEC != 0)
}

/// A `sockaddr_un` for a name, as long as Linux gives it (`unix_getname`): the family alone for
/// none, the path with its zero, or the abstract name as bound.
fn address_bytes(name: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = (AF_UNIX as u16).to_le_bytes().to_vec();
    match name {
        None => {}
        Some(name) if name.first() == Some(&0) => bytes.extend_from_slice(name),
        Some(name) => {
            bytes.extend_from_slice(name);
            bytes.push(0);
        }
    }
    bytes
}

/// `move_addr_to_user`: as much of an address as the caller's length holds, then its full length.
fn put_address(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    length: u64,
    bytes: &[u8],
) -> Result<(), Errno> {
    let room: i32 = guest(kernel, cx, tid).read_value(length)?;
    if room < 0 {
        return Err(Errno::EINVAL);
    }
    let copied = (room as usize).min(bytes.len());
    if copied > 0 {
        guest(kernel, cx, tid).write(address, &bytes[..copied])?;
    }
    guest(kernel, cx, tid).write_value(length, &(bytes.len() as i32))
}

pub(super) fn getsockname(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, address, length, ..]: Args,
) -> Result<u64, Errno> {
    let (_, socket) = socket_of(kernel, tid, fd)?;
    let bytes = if socket.borrow().inet.is_some() {
        inet::name(&socket, false)?
    } else if socket.borrow().netlink.is_some() {
        netlink::name(&socket, false)
    } else {
        address_bytes(socket.borrow().name.as_deref())
    };
    put_address(kernel, cx, tid, address, length, &bytes)?;
    Ok(0)
}

pub(super) fn getpeername(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, address, length, ..]: Args,
) -> Result<u64, Errno> {
    let (_, socket) = socket_of(kernel, tid, fd)?;
    if socket.borrow().inet.is_some() || socket.borrow().netlink.is_some() {
        let bytes = if socket.borrow().inet.is_some() {
            inet::name(&socket, true)?
        } else {
            netlink::name(&socket, true)
        };
        put_address(kernel, cx, tid, address, length, &bytes)?;
        return Ok(0);
    }
    let peer = socket.borrow().peer.clone().ok_or(Errno::ENOTCONN)?;
    let bytes = address_bytes(peer.borrow().name.as_deref());
    put_address(kernel, cx, tid, address, length, &bytes)?;
    Ok(0)
}

pub(super) fn shutdown(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, how, ..]: Args,
) -> Result<u64, Errno> {
    let (_, socket) = socket_of(kernel, tid, fd)?;
    let how = how as u32;
    // Netlink has no shutdown at all (`sock_no_shutdown`).
    if socket.borrow().netlink.is_some() {
        return Err(Errno::EOPNOTSUPP);
    }
    if how > SHUT_RDWR {
        return Err(Errno::EINVAL);
    }
    // SHUT_RD, SHUT_WR, SHUT_RDWR as bits.
    let mode = (how + 1) as u8;
    if socket.borrow().inet.is_some() {
        return inet::shutdown(kernel, cx, &socket, mode);
    }
    if socket.borrow().netlink.is_some() {
        return Err(Errno::EOPNOTSUPP);
    }
    let peer = {
        let mut own = socket.borrow_mut();
        own.shutdown |= mode;
        own.peer.clone()
    };
    kernel.wake(channel(&socket));
    if let Some(peer) = peer
        && socket.borrow().connection()
    {
        let mut other = peer.borrow_mut();
        if mode & RCV_SHUTDOWN != 0 {
            other.shutdown |= SEND_SHUTDOWN;
        }
        if mode & SEND_SHUTDOWN != 0 {
            other.shutdown |= RCV_SHUTDOWN;
        }
        drop(other);
        kernel.wake(channel(&peer));
        // What the other end was told, as its I/O signal.
        let told = (u8::from(mode & RCV_SHUTDOWN != 0) * SEND_SHUTDOWN)
            | (u8::from(mode & SEND_SHUTDOWN != 0) * RCV_SHUTDOWN);
        if told == SHUTDOWN_MASK {
            wake_async(kernel, cx, &peer, Wake::Data, POLL_HUP);
        } else if told & RCV_SHUTDOWN != 0 {
            wake_async(kernel, cx, &peer, Wake::Data, POLL_IN);
        }
    }
    Ok(0)
}

/// `unix_poll` and `unix_dgram_poll`.
pub(super) fn poll(socket: &SocketRef) -> u16 {
    if socket.borrow().inet.is_some() {
        return inet::poll(socket);
    }
    if socket.borrow().netlink.is_some() {
        return netlink::poll(socket);
    }
    let own = socket.borrow();
    let mut mask = 0u32;
    if own.error != 0 {
        mask |= EPOLLERR;
        // `unix_dgram_poll` with SO_SELECT_ERR_QUEUE.
        if own.kind != SOCK_STREAM && flag_on(&own, crate::linux::net::SO_SELECT_ERR_QUEUE) {
            mask |= EPOLLPRI;
        }
    }
    if own.shutdown == SHUTDOWN_MASK {
        mask |= EPOLLHUP;
    }
    if own.shutdown & RCV_SHUTDOWN != 0 {
        mask |= EPOLLRDHUP | EPOLLIN | EPOLLRDNORM;
    }
    if !own.queue.is_empty() || (own.state == State::Listening && !own.backlog.is_empty()) {
        mask |= EPOLLIN | EPOLLRDNORM;
    }
    if own.has_urgent() {
        mask |= EPOLLPRI;
    }
    let connection_closed = own.connection() && own.state == State::Closed;
    if (own.kind == SOCK_STREAM || own.kind == SOCK_SEQPACKET) && connection_closed {
        mask |= EPOLLHUP;
    }
    let mut writable = own.unix_writable();
    if writable
        && own.kind == SOCK_DGRAM
        && let Some(other) = &own.peer
    {
        let other = other.borrow();
        let back = other
            .peer
            .as_ref()
            .is_some_and(|back| Rc::ptr_eq(back, socket));
        if !back && other.queue.len() > other.max_backlog as usize {
            writable = false;
        }
    }
    if writable {
        mask |= EPOLLOUT | EPOLLWRNORM | EPOLLWRBAND;
    } else if own.kind != SOCK_STREAM {
        // `unix_dgram_poll` asks to hear when room comes back.
        drop(own);
        socket.borrow_mut().no_space = true;
    }
    mask as u16
}

/// The last descriptor of a socket went (`unix_release_sock`): its name is free, the other end of
/// a connection sees it shut down (and reset if it left bytes unread), waiting connections are
/// refused, and what its queue held is let go.
pub(crate) fn released(kernel: &mut Kernel, cx: &mut Context<'_>, socket: &SocketRef) {
    release(kernel, cx, socket, false);
}

/// As [`released`], for a socket or an embryo a listener held (whose other end is reset).
fn release(kernel: &mut Kernel, cx: &mut Context<'_>, socket: &SocketRef, embryo: bool) {
    if socket.borrow().inet.is_some() {
        return inet::released(kernel, cx, socket);
    }
    if socket.borrow().netlink.is_some() {
        return netlink::released(kernel, socket);
    }
    let (peer, backlog, queue, unread, connection) = {
        let mut own = socket.borrow_mut();
        own.dead = true;
        own.state = State::Closed;
        own.shutdown = SHUTDOWN_MASK;
        // An urgent byte read out of band at the front is not unread.
        let first = usize::from(
            own.queue
                .front()
                .is_some_and(|packet| packet.consumed == packet.data.len()),
        );
        let unread = own.queue.len() > first || embryo;
        (
            own.peer.take(),
            core::mem::take(&mut own.backlog),
            core::mem::take(&mut own.queue),
            unread,
            own.connection(),
        )
    };
    let (kind, name) = (socket.borrow().kind, socket.borrow().name.clone());
    if let Some(name) = name
        && name.first() == Some(&0)
    {
        let key = (kind, name);
        if kernel
            .sockets
            .abstract_names
            .get(&key)
            .is_some_and(|bound| core::ptr::eq(bound.as_ptr(), Rc::as_ptr(socket)))
        {
            kernel.sockets.abstract_names.remove(&key);
        }
    }
    if let Some(path) = socket.borrow().path {
        kernel.sockets.paths.remove(&path);
    }
    if let Some(peer) = peer {
        let back = peer
            .borrow()
            .peer
            .as_ref()
            .is_some_and(|back| Rc::ptr_eq(back, socket));
        // The other end keeps pointing at it: a stream sees it shut down, a datagram socket finds
        // it gone at its next send.
        if connection && back {
            let mut other = peer.borrow_mut();
            other.shutdown = SHUTDOWN_MASK;
            if unread {
                other.error = Errno::ECONNRESET.0 as i32;
            }
        }
        kernel.wake(channel(&peer));
        if connection {
            wake_async(kernel, cx, &peer, Wake::Data, POLL_HUP);
        }
    }
    for embryo in backlog {
        release(kernel, cx, &embryo, true);
    }
    for packet in queue {
        drop_packet(kernel, cx, packet);
    }
    kernel.wake(channel(socket));
}

/// A buffer leaves a queue: its sender's room comes back and its descriptors are let go.
fn drop_packet(kernel: &mut Kernel, cx: &mut Context<'_>, packet: Packet) {
    if let Some(sender) = packet.sender.upgrade() {
        let mut own = sender.borrow_mut();
        own.written = own.written.saturating_sub(packet.truesize);
        drop(own);
        write_space(kernel, cx, &sender);
    }
    kernel.released.extend(packet.files);
}

/// `SKB_TRUESIZE` of a buffer of `head` bytes in its head and `pages` bytes in pages, as
/// `alloc_skb_with_frags` makes it: the head rounded to kmalloc's sizes with the shared info, the
/// `sk_buff` itself, and whole pages.
fn truesize(head: u64, pages: u64) -> u64 {
    const SHARED_INFO: u64 = 320;
    const SK_BUFF: u64 = 256;
    let allocated = (head.next_multiple_of(64) + SHARED_INFO).next_power_of_two();
    allocated + SK_BUFF + pages.next_multiple_of(4096)
}

/// How one stream buffer splits into head and pages (`unix_stream_sendmsg`).
fn stream_piece(size: u64, sndbuf: u64) -> (u64, u64) {
    const MAX_HEAD: u64 = 4096 - 320;
    const FRAGS: u64 = 32768;
    let size = size
        .min((sndbuf >> 1).saturating_sub(64))
        .min(MAX_HEAD + FRAGS);
    let pages = size
        .saturating_sub(MAX_HEAD)
        .next_multiple_of(4096)
        .min(size);
    (size, pages)
}

/// Where a call's bytes are: the caller's buffers, or the kernel's own (a splice or sendfile).
#[derive(Clone, Copy)]
pub(crate) enum Data<'a> {
    Guest(&'a [(u64, u64)]),
    Kernel(&'a RefCell<Vec<u8>>),
}

impl Data<'_> {
    fn size(&self) -> u64 {
        match self {
            Data::Guest(parts) => parts.iter().map(|(_, length)| length).sum(),
            Data::Kernel(bytes) => bytes.borrow().len() as u64,
        }
    }
}

/// A message to send: its bytes, its destination, what it carries and its flags.
struct Outgoing<'a> {
    data: Data<'a>,
    /// The destination as the program gave it (`move_addr_to_kernel`), read by each family.
    address: Option<Vec<u8>>,
    /// An internet socket's control messages as the program gave them, read where its protocol
    /// reads them.
    control: Option<Vec<u8>>,
    files: Vec<FileRef>,
    cred: Option<Cred>,
    flags: u32,
}

/// `____sys_sendmsg` and the families' `sendmsg`: the bytes sent, waiting for room unless
/// nonblocking.
fn send(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    socket: &SocketRef,
    message: Outgoing,
) -> Result<u64, Errno> {
    let total = message.data.size();
    let nonblocking = message.flags & MSG_DONTWAIT != 0 || file.borrow().flags & O_NONBLOCK != 0;
    if message.flags & MSG_OOB != 0
        && socket.borrow().kind != SOCK_STREAM
        && socket.borrow().inet.is_none()
    {
        // `unix_seqpacket_sendmsg` takes a pending error and wants a connection first.
        if socket.borrow().kind == SOCK_SEQPACKET {
            let error = core::mem::take(&mut socket.borrow_mut().error);
            if error != 0 {
                return Err(Errno(error as u32));
            }
            if socket.borrow().state != State::Connected {
                return Err(Errno::ENOTCONN);
            }
        }
        return Err(Errno::EOPNOTSUPP);
    }
    let kind = socket.borrow().kind;
    let is_inet = socket.borrow().inet.is_some();
    if socket.borrow().netlink.is_some() {
        // `netlink_sendmsg` refuses an empty message.
        if total == 0 {
            return Err(Errno::ENODATA);
        }
        let to = message.address.as_deref();
        return netlink::send(kernel, cx, tid, socket, message.data, to, total);
    }
    let urgent = message.flags & MSG_OOB != 0;
    let result = match kind {
        _ if is_inet => inet::send(kernel, cx, tid, socket, &message, total, nonblocking),
        // `unix_stream_sendmsg`: an urgent byte is the last of at least one.
        SOCK_STREAM if urgent && total == 0 => return Err(Errno::EOPNOTSUPP),
        SOCK_STREAM => send_stream(kernel, cx, tid, socket, &message, total, nonblocking),
        _ => send_packet(kernel, cx, tid, socket, &message, total, nonblocking),
    };
    // Only streams send SIGPIPE (`unix_stream_sendmsg`, `sk_stream_error`).
    if result == Err(Errno::EPIPE) && message.flags & MSG_NOSIGNAL == 0 && kind == SOCK_STREAM {
        let process = kernel.process_of(tid);
        let info = crate::signal::sender_info(
            SIGPIPE,
            SI_USER as i32,
            process.pid,
            kernel.credentials(tid).uid,
        );
        kernel.send_to_thread(cx, tid, SIGPIPE, info);
    }
    result
}

/// The peer a connected socket sends to, as `unix_stream_sendmsg` finds it.
fn connected_peer(socket: &SocketRef) -> Result<SocketRef, Errno> {
    let own = socket.borrow();
    if own.shutdown & SEND_SHUTDOWN != 0 {
        return Err(Errno::EPIPE);
    }
    let peer = own.peer.clone().ok_or(Errno::ENOTCONN)?;
    Ok(peer)
}

/// Waits for room or a peer: EAGAIN when nonblocking, else parked on the socket.
fn wait_room(
    kernel: &mut Kernel,
    tid: Tid,
    socket: &SocketRef,
    nonblocking: bool,
    timeout: u64,
) -> Errno {
    if nonblocking {
        return Errno::EAGAIN;
    }
    if timed_out(kernel, tid, timeout) {
        return Errno::EAGAIN;
    }
    if kernel.signal_pending(tid) {
        return Errno::ERESTARTSYS;
    }
    kernel.park(tid, channel(socket));
    Errno::PARKED
}

/// Whether a wait with a socket's timeout (0 for none) has run out, setting its deadline at the
/// first run (`sock_sndtimeo`, `sock_rcvtimeo`).
fn timed_out(kernel: &mut Kernel, tid: Tid, timeout: u64) -> bool {
    if timeout == 0 {
        return false;
    }
    let now = kernel.entry_now;
    let call = &mut kernel.thread_mut(tid).call;
    match call.deadline {
        Some(deadline) => deadline <= now,
        None => {
            call.deadline = Some(now.saturating_add(timeout));
            false
        }
    }
}

fn send_stream(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    total: u64,
    nonblocking: bool,
) -> Result<u64, Errno> {
    if message
        .address
        .as_deref()
        .is_some_and(|bytes| !bytes.is_empty())
    {
        return Err(if socket.borrow().state == State::Connected {
            Errno::EISCONN
        } else {
            Errno::EOPNOTSUPP
        });
    }
    let peer = connected_peer(socket)?;
    // An urgent byte is the last, sent alone after the others (`queue_oob`).
    let urgent = message.flags & MSG_OOB != 0;
    let ordinary = total - u64::from(urgent);
    // A large write goes in pieces across waits; the call's progress is what went.
    let mut sent = kernel.threads[&tid].call.progress;
    let mut files_sent = sent > 0;
    // `unix_stream_sendmsg` sends nothing for nothing, descriptors included (measured).
    while sent < ordinary {
        if peer.borrow().dead || peer.borrow().shutdown & RCV_SHUTDOWN != 0 {
            return if sent > 0 {
                Ok(sent)
            } else {
                Err(Errno::EPIPE)
            };
        }
        let sndbuf = socket.borrow().sndbuf;
        if let Some(error) = wait_send_room(kernel, tid, socket, nonblocking, sent) {
            return error;
        }
        if !files_sent && !message.files.is_empty() && !peer.borrow().pass_rights {
            return Err(Errno::EPERM);
        }
        let (size, pages) = stream_piece(ordinary - sent, sndbuf);
        if !files_sent && !message.files.is_empty() {
            too_many_in_flight(kernel, tid)?;
        }
        // A piece that cannot be copied in ends the send with what went before.
        let data = match gather(kernel, cx, tid, message.data, sent, size) {
            Ok(data) => data,
            Err(_) if sent > 0 => return Ok(sent),
            Err(error) => return Err(error),
        };
        let packet = Packet {
            consumed: 0,
            files: if files_sent {
                Vec::new()
            } else {
                message.files.clone()
            },
            cred: sent_cred(kernel, tid, socket, &peer, message.cred),
            sender: Rc::downgrade(socket),
            truesize: truesize(size - pages, pages),
            data,
            source: None,
            arrival: None,
            time: None,
            urgent: false,
            owner: kernel.credentials(tid).uid,
        };
        if !files_sent && !message.files.is_empty() {
            carries(kernel, &peer);
        }
        files_sent = true;
        socket.borrow_mut().written += packet.truesize;
        peer.borrow_mut().queue.push_back(packet);
        data_ready(kernel, cx, &peer, EPOLLIN | EPOLLRDNORM);
        sent += size;
    }
    if !urgent {
        return Ok(sent);
    }
    // `queue_oob`: room for one byte, then the receiver's mark moves to it.
    if let Some(error) = wait_send_room(kernel, tid, socket, nonblocking, sent) {
        return error;
    }
    let partial = |error: Errno| if sent > 0 { Ok(sent) } else { Err(error) };
    if peer.borrow().dead || peer.borrow().shutdown & RCV_SHUTDOWN != 0 {
        return partial(Errno::EPIPE);
    }
    if !files_sent && !message.files.is_empty() && !peer.borrow().pass_rights {
        return partial(Errno::EPERM);
    }
    if !files_sent && !message.files.is_empty() {
        if let Err(error) = too_many_in_flight(kernel, tid) {
            return partial(error);
        }
        carries(kernel, &peer);
    }
    let data = match gather(kernel, cx, tid, message.data, sent, 1) {
        Ok(data) => data,
        Err(error) => return partial(error),
    };
    let packet = Packet {
        consumed: 0,
        files: if files_sent {
            Vec::new()
        } else {
            message.files.clone()
        },
        cred: sent_cred(kernel, tid, socket, &peer, message.cred),
        sender: Rc::downgrade(socket),
        truesize: truesize(1, 0),
        data,
        source: None,
        arrival: None,
        time: None,
        urgent: true,
        owner: kernel.credentials(tid).uid,
    };
    socket.borrow_mut().written += packet.truesize;
    {
        let mut other = peer.borrow_mut();
        for earlier in other.queue.iter_mut() {
            earlier.urgent = false;
        }
        other.queue.push_back(packet);
    }
    send_urgent(kernel, cx, &peer);
    data_ready(
        kernel,
        cx,
        &peer,
        EPOLLIN | EPOLLPRI | EPOLLRDNORM | EPOLLRDBAND,
    );
    Ok(sent + 1)
}

/// `sock_alloc_send_pskb`'s wait for room in the send buffer: None when there is room, else what
/// the send gives back (what went before, EAGAIN, or parked to go on from `sent`).
fn wait_send_room(
    kernel: &mut Kernel,
    tid: Tid,
    socket: &SocketRef,
    nonblocking: bool,
    sent: u64,
) -> Option<Result<u64, Errno>> {
    let (sndbuf, written, timeout) = {
        let own = socket.borrow();
        (own.sndbuf, own.written, own.send_timeout)
    };
    if written < sndbuf {
        return None;
    }
    socket.borrow_mut().no_space = true;
    if sent > 0 && (nonblocking || kernel.signal_pending(tid)) {
        return Some(Ok(sent));
    }
    let error = wait_room(kernel, tid, socket, nonblocking, timeout);
    if error == Errno::PARKED {
        kernel.thread_mut(tid).call.progress = sent;
    }
    Some(Err(error))
}

/// `sk_send_sigurg`: the owner of the receiver's file hears of an urgent byte, and with a chosen
/// I/O signal and `FASYNC` it is told too (`SOCK_WAKE_URG`).
fn send_urgent(kernel: &mut Kernel, cx: &mut Context<'_>, socket: &SocketRef) {
    let Some(file) = socket.borrow().file.upgrade() else {
        return;
    };
    if kernel.send_urgent_signal(cx, &file)
        && file.borrow().owner.is_some_and(|owner| owner.signal != 0)
    {
        kernel.send_io_signal(cx, &file, POLL_PRI);
    }
}

/// The bytes of `data` from `offset`, `length` of them.
fn gather(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    data: Data<'_>,
    offset: u64,
    length: u64,
) -> Result<Vec<u8>, Errno> {
    let parts = match data {
        Data::Guest(parts) => parts,
        Data::Kernel(bytes) => {
            let bytes = bytes.borrow();
            let start = (offset as usize).min(bytes.len());
            let end = (start + length as usize).min(bytes.len());
            return Ok(bytes[start..end].to_vec());
        }
    };
    let mut out = Vec::with_capacity(length as usize);
    let mut skip = offset;
    let mut guest = guest(kernel, cx, tid);
    for &(base, part) in parts {
        if out.len() as u64 == length {
            break;
        }
        if skip >= part {
            skip -= part;
            continue;
        }
        let take = (part - skip).min(length - out.len() as u64);
        out.extend_from_slice(&guest.read(base + skip, take)?);
        skip = 0;
    }
    Ok(out)
}

/// `unix_dgram_sendmsg` (and sequenced packets, whose `sendmsg` it is once connected).
fn send_packet(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    total: u64,
    nonblocking: bool,
) -> Result<u64, Errno> {
    let kind = socket.borrow().kind;
    // `unix_seqpacket_sendmsg`: a pending error first (taken, as `sock_error` does), then
    // connected, and any destination given is passed over.
    if kind == SOCK_SEQPACKET {
        let error = core::mem::take(&mut socket.borrow_mut().error);
        if error != 0 {
            return Err(Errno(error as u32));
        }
        if socket.borrow().state != State::Connected {
            return Err(Errno::ENOTCONN);
        }
    }
    let name = match &message.address {
        Some(bytes) if kind == SOCK_DGRAM && !bytes.is_empty() => Some(parse_name(bytes)?),
        _ => None,
    };
    if socket.borrow().may_pass_cred() && socket.borrow().name.is_none() {
        autobind(kernel, cx, socket)?;
    }
    let (sndbuf, written, timeout) = {
        let own = socket.borrow();
        (own.sndbuf, own.written, own.send_timeout)
    };
    if total > sndbuf.saturating_sub(32) {
        return Err(Errno::EMSGSIZE);
    }
    // `sock_alloc_send_pskb`.
    if socket.borrow().shutdown & SEND_SHUTDOWN != 0 {
        return Err(Errno::EPIPE);
    }
    if written >= sndbuf {
        socket.borrow_mut().no_space = true;
        return Err(wait_room(kernel, tid, socket, nonblocking, timeout));
    }
    // The bytes are taken before the receiver is looked for.
    let data = gather(kernel, cx, tid, message.data, 0, total)?;
    let target = match &name {
        Some(name) => find(kernel, cx, tid, name, SOCK_DGRAM)?,
        None => match socket.borrow().peer.clone() {
            Some(peer) => peer,
            None => return Err(Errno::ENOTCONN),
        },
    };
    if target.borrow().dead {
        // A datagram socket whose peer went is disconnected, by name or not, and what it held
        // is lost.
        let is_peer = socket
            .borrow()
            .peer
            .as_ref()
            .is_some_and(|peer| Rc::ptr_eq(peer, &target));
        if kind == SOCK_DGRAM && is_peer {
            {
                let mut own = socket.borrow_mut();
                own.peer = None;
                own.state = State::Closed;
            }
            dgram_disconnected(kernel, cx, socket, &target);
            return Err(Errno::ECONNREFUSED);
        }
        return Err(if kind == SOCK_SEQPACKET {
            Errno::EPIPE
        } else {
            Errno::ECONNREFUSED
        });
    }
    if target.borrow().shutdown & RCV_SHUTDOWN != 0 {
        return Err(Errno::EPIPE);
    }
    if !message.files.is_empty() && !target.borrow().pass_rights {
        return Err(Errno::EPERM);
    }
    // The receiver's queue is full for a sender it is not connected back to.
    let back = target
        .borrow()
        .peer
        .as_ref()
        .is_some_and(|back| Rc::ptr_eq(back, socket));
    let full = !Rc::ptr_eq(&target, socket)
        && !back
        && target.borrow().queue.len() > target.borrow().max_backlog as usize;
    if written >= sndbuf || full {
        if !full {
            socket.borrow_mut().no_space = true;
        }
        let waits_on = if full { &target } else { socket };
        return Err(wait_room(kernel, tid, waits_on, nonblocking, timeout));
    }
    const MAX_ALLOC: u64 = 16384 - 320;
    let pages = if total > MAX_ALLOC {
        (total - MAX_ALLOC).min(17 * 4096).next_multiple_of(4096)
    } else {
        0
    };
    if !message.files.is_empty() {
        too_many_in_flight(kernel, tid)?;
        carries(kernel, &target);
    }
    let packet = Packet {
        consumed: 0,
        files: message.files.clone(),
        cred: sent_cred(kernel, tid, socket, &target, message.cred),
        sender: Rc::downgrade(socket),
        truesize: truesize(total.saturating_sub(pages), pages),
        data,
        source: None,
        arrival: None,
        time: None,
        urgent: false,
        owner: kernel.credentials(tid).uid,
    };
    let packet = Packet {
        time: stamp(kernel, cx, &target),
        ..packet
    };
    socket.borrow_mut().written += packet.truesize;
    target.borrow_mut().queue.push_back(packet);
    data_ready(kernel, cx, &target, EPOLLIN | EPOLLRDNORM);
    Ok(total)
}

/// Notes that a local socket's queue holds descriptors.
fn carries(kernel: &mut Kernel, socket: &SocketRef) {
    let carriers = &mut kernel.sockets.carriers;
    if !carriers
        .iter()
        .any(|held| core::ptr::eq(held.as_ptr(), Rc::as_ptr(socket)))
    {
        carriers.push(Rc::downgrade(socket));
    }
}

/// Every packet in the queues of sockets that were given descriptors, and in those of their
/// waiting connections, with the socket whose queue it is in.
fn queued_with_files(kernel: &mut Kernel) -> Vec<SocketRef> {
    kernel.sockets.carriers.retain(|held| {
        held.upgrade()
            .is_some_and(|socket| holds_files(&socket.borrow()))
    });
    kernel
        .sockets
        .carriers
        .iter()
        .filter_map(Weak::upgrade)
        .collect()
}

/// Whether a socket's queue, or a waiting connection's, holds descriptors.
fn holds_files(own: &Socket) -> bool {
    own.queue.iter().any(|packet| !packet.files.is_empty())
        || own
            .backlog
            .iter()
            .any(|embryo| holds_files(&embryo.borrow()))
}

/// The descriptors a socket's queue and its waiting connections' queues carry.
fn carried(own: &Socket, each: &mut impl FnMut(&Packet)) {
    for packet in &own.queue {
        each(packet);
    }
    for embryo in &own.backlog {
        carried(&embryo.borrow(), each);
    }
}

/// `too_many_unix_fds`: a user with more descriptors in flight than its limit on open files
/// sends no more, without CAP_SYS_RESOURCE or CAP_SYS_ADMIN.
fn too_many_in_flight(kernel: &mut Kernel, tid: Tid) -> Result<(), Errno> {
    let (user, exempt) = {
        let credentials = kernel.credentials(tid);
        (
            credentials.uid,
            credentials.has(CAP_SYS_RESOURCE) || credentials.has(CAP_SYS_ADMIN),
        )
    };
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let mut in_flight = 0u64;
    for socket in queued_with_files(kernel) {
        carried(&socket.borrow(), &mut |packet| {
            if packet.owner == user {
                in_flight += packet.files.len() as u64;
            }
        });
    }
    if in_flight > limit && !exempt {
        return Err(Errno::ETOOMANYREFS);
    }
    Ok(())
}

/// `unix_gc`: local sockets whose files only the queues of such sockets hold, which no
/// descriptor and no other queue reaches, are let go with everything their queues carry.
/// Returns whether any were.
pub(crate) fn collect(kernel: &mut Kernel, cx: &mut Context<'_>) -> bool {
    let carriers = queued_with_files(kernel);
    if carriers.is_empty() {
        return false;
    }
    // Each local socket's file in flight, with how many queue places hold it.
    let mut held: Vec<(FileRef, usize)> = Vec::new();
    for socket in &carriers {
        carried(&socket.borrow(), &mut |packet| {
            for file in &packet.files {
                if !of(file).is_some_and(|socket| socket.borrow().local()) {
                    continue;
                }
                match held.iter_mut().find(|(seen, _)| Rc::ptr_eq(seen, file)) {
                    Some((_, count)) => *count += 1,
                    None => held.push((file.clone(), 1)),
                }
            }
        });
    }
    // Candidates: files nothing but queues holds (the list above holds one more each).
    let candidates: Vec<(FileRef, SocketRef, usize)> = held
        .into_iter()
        .filter(|(file, count)| Rc::strong_count(file) == count + 1)
        .filter_map(|(file, count)| of(&file).map(|socket| (file, socket, count)))
        .collect();
    let index_of = |file: &FileRef| candidates.iter().position(|(c, _, _)| Rc::ptr_eq(c, file));
    // A candidate held from a queue that is not a candidate's is reached, and so is everything
    // a reached candidate's queue carries.
    let mut from_candidates = alloc::vec![0usize; candidates.len()];
    for (_, socket, _) in &candidates {
        carried(&socket.borrow(), &mut |packet| {
            for file in &packet.files {
                if let Some(index) = index_of(file) {
                    from_candidates[index] += 1;
                }
            }
        });
    }
    let mut reached: Vec<bool> = candidates
        .iter()
        .zip(&from_candidates)
        .map(|((_, _, count), inside)| count > inside)
        .collect();
    let mut work: Vec<usize> = (0..candidates.len()).filter(|&i| reached[i]).collect();
    while let Some(at) = work.pop() {
        let mut found = Vec::new();
        carried(&candidates[at].1.borrow(), &mut |packet| {
            for file in &packet.files {
                if let Some(index) = index_of(file) {
                    found.push(index);
                }
            }
        });
        for index in found {
            if !reached[index] {
                reached[index] = true;
                work.push(index);
            }
        }
    }
    let garbage: Vec<SocketRef> = candidates
        .iter()
        .zip(&reached)
        .filter(|(_, reached)| !**reached)
        .map(|((_, socket, _), _)| socket.clone())
        .collect();
    drop(candidates);
    for socket in &garbage {
        let (queue, backlog) = {
            let mut own = socket.borrow_mut();
            (
                core::mem::take(&mut own.queue),
                core::mem::take(&mut own.backlog),
            )
        };
        for packet in queue {
            drop_packet(kernel, cx, packet);
        }
        for embryo in backlog {
            release(kernel, cx, &embryo, true);
        }
    }
    !garbage.is_empty()
}

/// What a receive gives back besides its bytes.
#[derive(Default)]
struct Received {
    files: Vec<FileRef>,
    cred: Option<Cred>,
    name: Option<Option<Vec<u8>>>,
    /// An internet sender's address, as the receiving socket's family shows it.
    address: Option<Vec<u8>>,
    flags: u32,
    /// The length to report (a datagram's whole length with MSG_TRUNC).
    reported: Option<u64>,
    /// An internet datagram's header and route.
    arrival: Option<inet::Arrival>,
    /// When the bytes came, for SO_TIMESTAMP and SO_TIMESTAMPNS.
    time: Option<crate::fs::Time>,
    /// A queued error's control message (level, type, data), after the others.
    error: Option<(u32, u32, Vec<u8>)>,
}

/// `unix_stream_read_generic` and `__unix_dgram_recvmsg`: bytes into `parts`, waiting for some
/// unless nonblocking.
fn receive(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    socket: &SocketRef,
    parts: Data<'_>,
    flags: u32,
) -> Result<(u64, Received), Errno> {
    let nonblocking = flags & MSG_DONTWAIT != 0 || file.borrow().flags & O_NONBLOCK != 0;
    let kind = socket.borrow().kind;
    if socket.borrow().inet.is_some() {
        return inet::receive(kernel, cx, tid, socket, parts, flags, nonblocking);
    }
    if socket.borrow().netlink.is_some() {
        return netlink::receive(kernel, cx, tid, socket, parts, flags, nonblocking);
    }
    let size = parts.size();
    if kind == SOCK_STREAM {
        if socket.borrow().state != State::Connected && socket.borrow().queue.is_empty() {
            return Err(Errno::EINVAL);
        }
        if flags & MSG_OOB != 0 {
            return receive_urgent(kernel, cx, tid, socket, parts, flags);
        }
        socket.borrow_mut().waiting_data = false;
        let result = receive_stream(kernel, cx, tid, socket, parts, size, flags, nonblocking);
        if result.as_ref().err() == Some(&Errno::PARKED) {
            socket.borrow_mut().waiting_data = true;
        }
        return result;
    }
    // `unix_seqpacket_recvmsg` wants a connection before the datagram read refuses urgent data.
    if flags & MSG_OOB != 0 {
        if kind == SOCK_SEQPACKET && socket.borrow().state != State::Connected {
            return Err(Errno::ENOTCONN);
        }
        return Err(Errno::EOPNOTSUPP);
    }
    // `__skb_try_recv_datagram`: a pending error comes before any datagram.
    let error = core::mem::take(&mut socket.borrow_mut().error);
    if error != 0 {
        return Err(Errno(error as u32));
    }
    let packet_ready = !socket.borrow().queue.is_empty();
    if !packet_ready {
        let own = socket.borrow();
        // A datagram socket shut for reading ends only a wait; a nonblocking read of it finds
        // nothing (`__unix_dgram_recvmsg` turns EAGAIN into the end for sequenced packets only).
        if kind == SOCK_DGRAM && own.shutdown & RCV_SHUTDOWN != 0 && nonblocking {
            return Err(Errno::EAGAIN);
        }
        if own.shutdown & RCV_SHUTDOWN != 0
            || (kind == SOCK_SEQPACKET
                && own.state == State::Closed
                && own.peer.is_none()
                && own.dead)
        {
            return Ok((0, Received::default()));
        }
        if kind == SOCK_SEQPACKET && own.state != State::Connected {
            return Err(Errno::ENOTCONN);
        }
        let timeout = own.receive_timeout;
        drop(own);
        return Err(wait_room(kernel, tid, socket, nonblocking, timeout));
    }
    let peek = flags & MSG_PEEK != 0;
    // `sk_peek_offset`: a peek starts at SO_PEEK_OFF, past whole datagrams
    // (`__skb_try_recv_from_queue`).
    let found = {
        let own = socket.borrow();
        let mut skip = if peek {
            own.peek_offset.unwrap_or(0) as usize
        } else {
            0
        };
        let mut found = None;
        for (index, packet) in own.queue.iter().enumerate() {
            let length = packet.data.len();
            if skip >= length && (length > 0 || skip > 0) {
                skip -= length;
                continue;
            }
            found = Some((index, skip));
            break;
        }
        found
    };
    let Some((index, skip)) = found else {
        let timeout = socket.borrow().receive_timeout;
        return Err(wait_room(kernel, tid, socket, nonblocking, timeout));
    };
    let (data, files, cred, sender, time) = {
        let own = socket.borrow();
        let packet = &own.queue[index];
        (
            packet.data[skip..].to_vec(),
            packet.files.clone(),
            packet.cred,
            packet.sender.upgrade(),
            packet.time,
        )
    };
    let copied = (data.len() as u64).min(size);
    if peek && let Some(offset) = &mut socket.borrow_mut().peek_offset {
        *offset += copied;
    }
    if let Err(error) = scatter(kernel, cx, tid, parts, &data[..copied as usize]) {
        // `__unix_dgram_recvmsg` has taken the message off the queue and frees it.
        if !peek {
            let packet = socket.borrow_mut().queue.pop_front().expect("a packet");
            drop_packet(kernel, cx, packet);
        }
        return Err(error);
    }
    let mut received = Received {
        files,
        time,
        cred: Some(cred),
        name: Some(sender.and_then(|sender| sender.borrow().name.clone())),
        ..Received::default()
    };
    if (data.len() as u64) > size {
        received.flags |= MSG_TRUNC;
    }
    if flags & MSG_TRUNC != 0 {
        received.reported = Some(data.len() as u64);
    }
    if !peek {
        let packet = socket.borrow_mut().queue.pop_front().expect("a packet");
        if let Some(offset) = &mut socket.borrow_mut().peek_offset {
            *offset = offset.saturating_sub(packet.data.len() as u64);
        }
        let Packet {
            sender, truesize, ..
        } = packet;
        if let Some(sender) = sender.upgrade() {
            let mut own = sender.borrow_mut();
            own.written = own.written.saturating_sub(truesize);
            drop(own);
            write_space(kernel, cx, &sender);
        }
        // Room in a full queue for the senders waiting on it.
        kernel.wake_poll(channel(socket), EPOLLOUT | EPOLLWRNORM | EPOLLWRBAND);
    }
    Ok((copied, received))
}

#[allow(
    clippy::too_many_arguments,
    reason = "a read names where its bytes go and how"
)]
fn receive_stream(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    parts: Data<'_>,
    size: u64,
    flags: u32,
    nonblocking: bool,
) -> Result<(u64, Received), Errno> {
    let peek = flags & MSG_PEEK != 0;
    let is_inet = socket.borrow().inet.is_some();
    // SO_RCVLOWAT, kept for every socket, is how much a TCP read waits for.
    let low = if is_inet {
        receive_low_water(&socket.borrow())
    } else {
        1
    };
    // `sock_rcvlowat`: at least one byte, so an empty read of an empty queue waits too.
    let target = if flags & MSG_WAITALL != 0 {
        size
    } else {
        low.min(size)
    }
    .max(1);
    // A TCP read with MSG_TRUNC throws the bytes away.
    let discard = is_inet && flags & MSG_TRUNC != 0;
    let mut copied = kernel.threads[&tid].call.progress;
    let mut received = Received::default();
    let check_creds = socket.borrow().may_pass_cred();
    let mut skip = if peek {
        socket.borrow().peek_offset.unwrap_or(0)
    } else {
        0
    } as usize;
    let mut index = 0;
    // A local stream's peek goes on to each next buffer even once it has all it wants
    // (`unix_stream_read_generic`'s `goto again`), where it stops at the urgent byte or at a
    // buffer of other credentials or with descriptors.
    let walk = peek && !is_inet;
    loop {
        if copied >= size && size > 0 && !walk {
            break;
        }
        if socket.borrow().queue.get(index).is_some() {
            let managed = if is_inet {
                pass_tcp_urgent(kernel, cx, socket, index, peek, copied)
            } else {
                manage_urgent(kernel, cx, socket, index, peek, copied)
            };
            match managed {
                Some(at) => index = at,
                None if copied > 0 => break,
                None => index = socket.borrow().queue.len(),
            }
        }
        let next = {
            let own = socket.borrow();
            let mut found = None;
            while let Some(packet) = own.queue.get(index) {
                let left = packet.data.len() - packet.consumed;
                if skip >= left {
                    skip -= left;
                    index += 1;
                    continue;
                }
                found = Some((
                    packet.data[packet.consumed + skip..].to_vec(),
                    packet.files.clone(),
                    packet.cred,
                    packet.sender.upgrade(),
                    packet.time,
                ));
                break;
            }
            found
        };
        let Some((data, files, cred, sender, time)) = next else {
            if copied >= target {
                break;
            }
            let own = socket.borrow();
            // TCP: the other end's FIN comes before a pending error (`SOCK_DONE`).
            if own.inet.as_deref().is_some_and(|inet| inet.done) {
                break;
            }
            if own.error != 0 {
                let error = own.error;
                drop(own);
                socket.borrow_mut().error = 0;
                if copied > 0 {
                    break;
                }
                return Err(Errno(error as u32));
            }
            if own.shutdown & RCV_SHUTDOWN != 0 {
                break;
            }
            let timeout = own.receive_timeout;
            drop(own);
            if copied > 0 && nonblocking {
                break;
            }
            let error = wait_room(kernel, tid, socket, nonblocking, timeout);
            if error == Errno::PARKED {
                kernel.thread_mut(tid).call.progress = copied;
                return Err(error);
            }
            if copied > 0 {
                break;
            }
            return Err(error);
        };
        // Bytes from different writers' credentials are never joined.
        if check_creds {
            match received.cred {
                Some(first) if first != cred => break,
                None => received.cred = Some(cred),
                _ => {}
            }
        }
        if received.name.is_none() {
            received.name = Some(sender.and_then(|sender| sender.borrow().name.clone()));
        }
        // TCP reports the time of the last buffer read (`tcp_recv_timestamp`).
        if time.is_some() {
            received.time = time;
        }
        let chunk = (data.len() as u64).min(size - copied);
        if !discard && scatter_at(kernel, cx, tid, parts, copied, &data[..chunk as usize]).is_err()
        {
            if copied == 0 {
                return Err(Errno::EFAULT);
            }
            break;
        }
        copied += chunk;
        if !peek {
            let mut own = socket.borrow_mut();
            let packet = own.queue.get_mut(index).expect("the packet");
            packet.consumed += chunk as usize;
            let had_files = !packet.files.is_empty();
            if had_files {
                received.files = core::mem::take(&mut packet.files);
            }
            if let Some(offset) = &mut own.peek_offset {
                *offset = offset.saturating_sub(chunk);
            }
            let packet = &own.queue[index];
            if packet.consumed < packet.data.len() {
                break;
            }
            let packet = own.queue.remove(index).expect("the packet");
            drop(own);
            let Packet {
                sender, truesize, ..
            } = packet;
            if let Some(sender) = sender.upgrade() {
                let mut own = sender.borrow_mut();
                own.written = own.written.saturating_sub(truesize);
                drop(own);
                write_space(kernel, cx, &sender);
            }
            if had_files {
                break;
            }
        } else {
            if !files.is_empty() {
                received.files = files;
            }
            if let Some(offset) = &mut socket.borrow_mut().peek_offset {
                *offset += chunk;
            }
            if !received.files.is_empty() {
                break;
            }
            skip = 0;
            index += 1;
            if walk {
                if index >= socket.borrow().queue.len() {
                    break;
                }
                continue;
            }
        }
        if size == 0 {
            break;
        }
    }
    Ok((copied, received))
}

/// `manage_oob`: before a local stream read takes the buffer at `index`, an emptied one (an urgent
/// byte read out of band) is passed or let go, and at the urgent byte the read stops if it has
/// bytes, or else passes it (letting it go unless SO_OOBINLINE). The buffer to read, or None.
fn manage_urgent(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    socket: &SocketRef,
    mut index: usize,
    peek: bool,
    copied: u64,
) -> Option<usize> {
    let at = |socket: &SocketRef, index: usize| {
        let own = socket.borrow();
        own.queue
            .get(index)
            .map(|packet| (packet.consumed == packet.data.len(), packet.urgent))
    };
    let (empty, urgent) = at(socket, index)?;
    if !empty && !urgent {
        return Some(index);
    }
    if empty {
        if copied > 0 && (urgent || !socket.borrow().has_urgent()) {
            return None;
        }
        if peek {
            index += 1;
        } else {
            let packet = socket.borrow_mut().queue.remove(index).expect("the packet");
            drop_packet(kernel, cx, packet);
        }
        if !at(socket, index)?.1 {
            return Some(index);
        }
    }
    if copied > 0 {
        return None;
    }
    let inline = socket.borrow().urgent_inline();
    if !peek {
        let mut own = socket.borrow_mut();
        own.queue[index].urgent = false;
        if !inline {
            let packet = own.queue.remove(index).expect("the urgent byte");
            own.skipped_urgent += (packet.data.len() - packet.consumed) as u64;
            let any = !own.queue.is_empty();
            drop(own);
            drop_packet(kernel, cx, packet);
            return any.then_some(0);
        }
        Some(index)
    } else if !inline {
        at(socket, index + 1).map(|_| index + 1)
    } else {
        Some(index)
    }
}

/// `tcp_recvmsg` at TCP's urgent byte (read out of band or not): a read with bytes stops there;
/// one without reads it as an ordinary byte with SO_OOBINLINE, else passes over it, letting it
/// go unless peeking (the urgent data ends). The buffer to read, or None.
fn pass_tcp_urgent(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    socket: &SocketRef,
    mut index: usize,
    peek: bool,
    copied: u64,
) -> Option<usize> {
    let (urgent, unread) = {
        let own = socket.borrow();
        let packet = own.queue.get(index)?;
        (packet.urgent, packet.consumed == 0)
    };
    if !urgent {
        return Some(index);
    }
    if copied > 0 {
        return None;
    }
    if unread && socket.borrow().urgent_inline() {
        return Some(index);
    }
    if peek {
        index += 1;
    } else {
        let packet = socket
            .borrow_mut()
            .queue
            .remove(index)
            .expect("the urgent byte");
        drop_packet(kernel, cx, packet);
    }
    socket.borrow().queue.get(index).map(|_| index)
}

/// `unix_stream_recv_urg`: the urgent byte the mark is at, out of band. Taken, it stays in the
/// queue emptied, and an emptied buffer just before it goes.
fn receive_urgent(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    parts: Data<'_>,
    flags: u32,
) -> Result<(u64, Received), Errno> {
    let (byte, emptied) = {
        let mut own = socket.borrow_mut();
        let at = own
            .queue
            .iter()
            .position(|packet| packet.urgent)
            .filter(|_| !own.urgent_inline())
            .ok_or(Errno::EINVAL)?;
        let packet = &mut own.queue[at];
        let byte = packet.data[packet.consumed];
        let mut emptied = None;
        if flags & MSG_PEEK == 0 {
            packet.urgent = false;
            packet.consumed += 1;
            if at > 0 && own.queue[at - 1].consumed == own.queue[at - 1].data.len() {
                emptied = own.queue.remove(at - 1);
            }
        }
        (byte, emptied)
    };
    let room = parts.size().min(1) as usize;
    let copy = scatter(kernel, cx, tid, parts, &[byte][..room]);
    if let Some(packet) = emptied {
        drop_packet(kernel, cx, packet);
    }
    copy.map_err(|_| Errno::EFAULT)?;
    let received = Received {
        flags: MSG_OOB,
        ..Received::default()
    };
    Ok((1, received))
}

fn scatter(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    parts: Data<'_>,
    bytes: &[u8],
) -> Result<(), Errno> {
    scatter_at(kernel, cx, tid, parts, 0, bytes)
}

/// Stores bytes at `offset` into a reader's buffers, across them.
fn scatter_at(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    parts: Data<'_>,
    offset: u64,
    bytes: &[u8],
) -> Result<(), Errno> {
    if bytes.is_empty() {
        return Ok(());
    }
    match parts {
        Data::Guest(parts) => super::signalfd::store(kernel, cx, tid, parts, offset, bytes),
        Data::Kernel(into) => {
            let mut into = into.borrow_mut();
            let start = offset as usize;
            into[start..start + bytes.len()].copy_from_slice(bytes);
            Ok(())
        }
    }
}

/// read and readv on a socket: a receive with no flags.
pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let socket = of(file).expect("a socket");
    // `sock_read_iter`: a read of nothing is nothing, as System V's; one that may not wait is
    // MSG_DONTWAIT.
    if parts.iter().all(|(_, length)| *length == 0) {
        return Ok(0);
    }
    let flags = dont_wait(kernel, tid);
    let (copied, received) = receive(kernel, cx, tid, file, &socket, Data::Guest(parts), flags)?;
    // Descriptors that came with the bytes are let go (`scm_destroy`).
    kernel.released.extend(received.files);
    Ok(copied)
}

/// MSG_DONTWAIT for a call that may not wait (RWF_NOWAIT), else no flags.
fn dont_wait(kernel: &Kernel, tid: Tid) -> u32 {
    if kernel.threads[&tid].call.nowait {
        MSG_DONTWAIT
    } else {
        0
    }
}

/// Sends bytes the kernel holds on a socket with `flags` and no address, as a splice or
/// sendfile into it does (`splice_to_socket`).
pub(crate) fn send_bytes(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    bytes: &RefCell<Vec<u8>>,
    flags: u32,
) -> Result<u64, Errno> {
    let socket = of(file).expect("a socket");
    let message = Outgoing {
        data: Data::Kernel(bytes),
        address: None,
        control: None,
        files: Vec::new(),
        cred: None,
        flags,
    };
    send(kernel, cx, tid, file, &socket, message)
}

/// Receives up to `size` bytes from a socket into the kernel, as a splice from it does
/// (`sock_splice_read`); descriptors that came with them are let go.
pub(crate) fn receive_bytes(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    size: u64,
    flags: u32,
) -> Result<Vec<u8>, Errno> {
    let socket = of(file).expect("a socket");
    let into = RefCell::new(alloc::vec![0; size as usize]);
    let (copied, received) = receive(kernel, cx, tid, file, &socket, Data::Kernel(&into), flags)?;
    kernel.released.extend(received.files);
    let mut bytes = into.into_inner();
    bytes.truncate(copied as usize);
    Ok(bytes)
}

/// write and writev on a socket: a send with no flags, or MSG_DONTWAIT for one that may not wait
/// (`sock_write_iter`).
pub(super) fn write(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let socket = of(file).expect("a socket");
    let message = Outgoing {
        data: Data::Guest(parts),
        address: None,
        control: None,
        files: Vec::new(),
        cred: None,
        flags: dont_wait(kernel, tid),
    };
    send(kernel, cx, tid, file, &socket, message)
}

pub(super) fn sendto(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, length, flags, address, address_length]: Args,
) -> Result<u64, Errno> {
    // `import_ubuf` comes before the descriptor.
    let length = length.min(super::io::MAX_RW_COUNT);
    if !crate::user::access_ok(buffer, length) {
        return Err(Errno::EFAULT);
    }
    let (file, socket) = socket_of(kernel, tid, fd)?;
    let address = if address == 0 {
        None
    } else {
        Some(address_argument(kernel, cx, tid, address, address_length)?)
    };
    let buffers = [(buffer, length)];
    let message = Outgoing {
        data: Data::Guest(&buffers),
        address,
        control: None,
        files: Vec::new(),
        cred: None,
        flags: flags as u32,
    };
    send(kernel, cx, tid, &file, &socket, message)
}

pub(super) fn recvfrom(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, length, flags, address, address_length]: Args,
) -> Result<u64, Errno> {
    // `import_ubuf` comes before the descriptor.
    let length = length.min(super::io::MAX_RW_COUNT);
    if !crate::user::access_ok(buffer, length) {
        return Err(Errno::EFAULT);
    }
    let (file, socket) = socket_of(kernel, tid, fd)?;
    let flags = flags as u32;
    let (copied, received) = receive(
        kernel,
        cx,
        tid,
        &file,
        &socket,
        Data::Guest(&[(buffer, length)]),
        flags,
    )?;
    kernel.released.extend(received.files);
    if address != 0 {
        // A sender with no name gives a length of 0 (`unix_copy_addr`).
        let bytes = received
            .address
            .or_else(|| {
                received
                    .name
                    .flatten()
                    .map(|name| address_bytes(Some(&name)))
            })
            .unwrap_or_default();
        put_address(kernel, cx, tid, address, address_length, &bytes)?;
    }
    Ok(received.reported.unwrap_or(copied))
}

/// A `msghdr` as `copy_msghdr_from_user` reads it, with its vector.
struct Header {
    name: u64,
    name_length: i32,
    /// A send's destination, taken in before the vectors.
    address: Option<Vec<u8>>,
    parts: Vec<(u64, u64)>,
    control: u64,
    control_length: u64,
}

/// `copy_msghdr_from_user`: the header, a send's destination (its length cut to 128), then the
/// vectors.
fn read_header(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    sending: bool,
) -> Result<Header, Errno> {
    let bytes = guest(kernel, cx, tid).read(address, 56)?;
    let long = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
    let name_length = i32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
    if name_length < 0 {
        return Err(Errno::EINVAL);
    }
    let name_length = name_length.min(128);
    let address = if sending && long(0) != 0 && name_length != 0 {
        Some(guest(kernel, cx, tid).read(long(0), name_length as u64)?)
    } else {
        None
    };
    let (vector, count) = (long(16), long(24));
    if count > super::io::MAX_SEGMENTS {
        return Err(Errno::EMSGSIZE);
    }
    let parts = super::io::segments(&mut guest(kernel, cx, tid), vector, count)?;
    Ok(Header {
        name: long(0),
        name_length,
        address,
        parts,
        control: long(32),
        control_length: long(40),
    })
}

/// The control messages of a send (`__scm_send`): descriptors (only on local sockets) and
/// credentials.
fn read_control(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    header: &Header,
    local: bool,
) -> Result<(Vec<FileRef>, Option<Cred>), Errno> {
    let mut files = Vec::new();
    let mut cred = None;
    if header.control_length == 0 {
        return Ok((files, cred));
    }
    if header.control_length > i32::MAX as u64 {
        return Err(Errno::ENOBUFS);
    }
    let control = guest(kernel, cx, tid).read(header.control, header.control_length)?;
    let mut at = 0usize;
    while at + CMSG_HEADER as usize <= control.len() {
        let length = u64::from_le_bytes(control[at..at + 8].try_into().expect("8 bytes"));
        let level = u32::from_le_bytes(control[at + 8..at + 12].try_into().expect("4 bytes"));
        let kind = u32::from_le_bytes(control[at + 12..at + 16].try_into().expect("4 bytes"));
        if length < CMSG_HEADER || at as u64 + length > control.len() as u64 {
            return Err(Errno::EINVAL);
        }
        let data = &control[at + CMSG_HEADER as usize..at + length as usize];
        if level == SOL_SOCKET {
            match kind {
                SCM_RIGHTS => {
                    if !local {
                        return Err(Errno::EINVAL);
                    }
                    let count = data.len() / 4;
                    if count == 0 || files.len() + count > MAX_FDS {
                        return Err(Errno::EINVAL);
                    }
                    for chunk in data.as_chunks::<4>().0 {
                        let fd = i32::from_le_bytes(*chunk);
                        let file = kernel.call_file(tid, fd)?;
                        if file.borrow().is_path() {
                            return Err(Errno::EBADF);
                        }
                        files.push(file);
                    }
                }
                SCM_CREDENTIALS => {
                    if data.len() != 12 {
                        return Err(Errno::EINVAL);
                    }
                    let word = |at: usize| {
                        u32::from_le_bytes(data[at..at + 4].try_into().expect("4 bytes"))
                    };
                    let asked = Cred {
                        pid: word(0) as i32,
                        uid: word(4),
                        gid: word(8),
                    };
                    check_cred(kernel, tid, asked)?;
                    cred = Some(asked);
                }
                _ => return Err(Errno::EINVAL),
            }
        }
        at += length.next_multiple_of(CMSG_ALIGN) as usize;
    }
    Ok((files, cred))
}

/// `scm_check_creds`: a process may name only itself, and ids it holds unless it has the powers.
fn check_cred(kernel: &Kernel, tid: Tid, asked: Cred) -> Result<(), Errno> {
    let credentials = kernel.credentials(tid);
    let own_pid = kernel.threads[&tid].process;
    let pid_ok = asked.pid == own_pid || credentials.has(CAP_SYS_ADMIN);
    let uid_ok = [credentials.uid, credentials.euid, credentials.suid].contains(&asked.uid)
        || credentials.has(CAP_SETUID);
    let gid_ok = [credentials.gid, credentials.egid, credentials.sgid].contains(&asked.gid)
        || credentials.has(CAP_SETGID);
    if pid_ok && uid_ok && gid_ok {
        Ok(())
    } else {
        Err(Errno::EPERM)
    }
}

pub(super) fn sendmsg(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, header, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (file, socket) = socket_of(kernel, tid, fd)?;
    let header = read_header(kernel, cx, tid, header, true)?;
    // `____sys_sendmsg` takes the control messages in; an internet protocol reads them itself.
    if header.control_length > i32::MAX as u64 {
        return Err(Errno::ENOBUFS);
    }
    // `unix_seqpacket_sendmsg` wants a connection before it reads the control messages.
    if socket.borrow().local()
        && socket.borrow().kind == SOCK_SEQPACKET
        && socket.borrow().state != State::Connected
    {
        return Err(Errno::ENOTCONN);
    }
    // `netlink_sendmsg` refuses urgent and empty messages before it reads the control messages
    // `____sys_sendmsg` took in.
    if socket.borrow().netlink.is_some() {
        guest(kernel, cx, tid).read(header.control, header.control_length)?;
        if flags as u32 & MSG_OOB != 0 {
            return Err(Errno::EOPNOTSUPP);
        }
        if header.parts.iter().all(|&(_, length)| length == 0) {
            return Err(Errno::ENODATA);
        }
    }
    let (files, cred, control) = if socket.borrow().inet.is_some() {
        let bytes = guest(kernel, cx, tid).read(header.control, header.control_length)?;
        (
            Vec::new(),
            None,
            Some(bytes).filter(|bytes| !bytes.is_empty()),
        )
    } else {
        let local = socket.borrow().local();
        let (files, cred) = read_control(kernel, cx, tid, &header, local)?;
        (files, cred, None)
    };
    let message = Outgoing {
        data: Data::Guest(&header.parts),
        address: header.address,
        control,
        files,
        cred,
        flags: flags as u32,
    };
    send(kernel, cx, tid, &file, &socket, message)
}

pub(super) fn recvmsg(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, address, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (file, socket) = socket_of(kernel, tid, fd)?;
    let header = read_header(kernel, cx, tid, address, false)?;
    let flags = flags as u32;
    let result = receive(
        kernel,
        cx,
        tid,
        &file,
        &socket,
        Data::Guest(&header.parts),
        flags,
    );
    let (copied, received) = match result {
        // `unix_stream_read_generic` hands its credentials over even when its loop read nothing
        // and failed (no bytes, or a pending error): empty ones, as nothing was examined.
        Err(error)
            if !matches!(
                error,
                Errno::PARKED | Errno::EINVAL | Errno::EFAULT | Errno::ERESTARTSYS
            ) && socket.borrow().local()
                && socket.borrow().kind == SOCK_STREAM
                && socket.borrow().passcred
                && flags & MSG_OOB == 0 =>
        {
            let mut control = Vec::new();
            let mut ignored = 0;
            let room = header.control_length as usize;
            put_control(
                &mut control,
                room,
                SOL_SOCKET,
                SCM_CREDENTIALS,
                &[0; 12],
                &mut ignored,
            );
            if !control.is_empty() {
                guest(kernel, cx, tid).write(header.control, &control)?;
            }
            return Err(error);
        }
        result => result?,
    };
    // `____sys_recvmsg` starts the flags given back from MSG_CMSG_CLOEXEC.
    let mut out_flags = received.flags | (flags & MSG_CMSG_CLOEXEC);
    // The sender's name.
    let mut name_length = 0i32;
    let sender = received.address.clone().or_else(|| {
        received
            .name
            .clone()
            .flatten()
            .map(|name| address_bytes(Some(&name)))
    });
    if header.name != 0
        && let Some(bytes) = sender
    {
        let room = (header.name_length.max(0) as usize).min(bytes.len());
        guest(kernel, cx, tid).write(header.name, &bytes[..room])?;
        name_length = bytes.len() as i32;
    }
    // Control messages: the time the bytes came, an internet datagram's header and route,
    // credentials when asked for, then descriptors.
    let mut control = Vec::new();
    let room = header.control_length as usize;
    let stamps = socket.borrow().stamps;
    // A datagram that came before any time was asked for is stamped as it is read
    // (`__sock_recv_timestamp`); TCP gives none for such bytes, and local streams none at all.
    let datagram = socket.borrow().kind != SOCK_STREAM && socket.borrow().netlink.is_none();
    let time = match received.time {
        None if datagram && stamps & STAMP != 0 => Some(kernel.now(cx)),
        time => time,
    };
    if let Some(time) = time.filter(|_| stamps & STAMP != 0) {
        // `__sock_recv_timestamp`: microseconds or nanoseconds; the old and new forms share
        // ARM64's layout.
        let ns = stamps & STAMP_NS != 0;
        use crate::linux::net::{
            SO_TIMESTAMP_NEW, SO_TIMESTAMP_OLD, SO_TIMESTAMPNS_NEW, SO_TIMESTAMPNS_OLD,
        };
        let kind = match (ns, stamps & STAMP_NEW != 0) {
            (false, false) => SO_TIMESTAMP_OLD,
            (true, false) => SO_TIMESTAMPNS_OLD,
            (false, true) => SO_TIMESTAMP_NEW,
            (true, true) => SO_TIMESTAMPNS_NEW,
        };
        let part = if ns {
            i64::from(time.nanoseconds)
        } else {
            i64::from(time.nanoseconds / 1000)
        };
        let mut data = time.seconds.to_le_bytes().to_vec();
        data.extend_from_slice(&part.to_le_bytes());
        put_control(&mut control, room, SOL_SOCKET, kind, &data, &mut out_flags);
    }
    // `__sock_recv_timestamp` and `tcp_recv_timestamp`: software timestamping gives the time
    // with two empty ones after it (the old and hardware places); local sockets give it only
    // beside SO_TIMESTAMP, and local streams never.
    use crate::linux::net::{
        SOF_TIMESTAMPING_OPT_RX_FILTER, SOF_TIMESTAMPING_RX_SOFTWARE, SOF_TIMESTAMPING_SOFTWARE,
    };
    let tsflags = socket.borrow().tsflags;
    let local = socket.borrow().family == AF_UNIX;
    let reported = tsflags & tsflag(SOF_TIMESTAMPING_SOFTWARE) != 0
        && (tsflags & tsflag(SOF_TIMESTAMPING_RX_SOFTWARE) != 0
            || received.flags & crate::linux::net::MSG_ERRQUEUE != 0
            || tsflags & tsflag(SOF_TIMESTAMPING_OPT_RX_FILTER) == 0)
        && (!local || (stamps & STAMP != 0 && datagram));
    if let Some(time) = time.filter(|_| reported) {
        use crate::linux::net::{SO_TIMESTAMPING_NEW, SO_TIMESTAMPING_OLD};
        let kind = if stamps & STAMP_NEW != 0 {
            SO_TIMESTAMPING_NEW
        } else {
            SO_TIMESTAMPING_OLD
        };
        let mut data = time.seconds.to_le_bytes().to_vec();
        data.extend_from_slice(&i64::from(time.nanoseconds).to_le_bytes());
        data.resize(48, 0);
        put_control(&mut control, room, SOL_SOCKET, kind, &data, &mut out_flags);
    }
    // `sock_recv_drops`: the drops before a datagram, when asked for as it came and now.
    if let Some(arrival) = received.arrival.filter(|arrival| arrival.drops != 0)
        && flag_on(&socket.borrow(), crate::linux::net::SO_RXQ_OVFL)
    {
        put_control(
            &mut control,
            room,
            SOL_SOCKET,
            crate::linux::net::SO_RXQ_OVFL,
            &arrival.drops.to_le_bytes(),
            &mut out_flags,
        );
    }
    // `sock_recv_mark` and `sock_recv_priority` for a datagram (an error read gives neither).
    if let Some(arrival) = received.arrival.filter(|arrival| !arrival.error) {
        use crate::linux::net::{SO_MARK, SO_PRIORITY, SO_RCVMARK, SO_RCVPRIORITY};
        let own = socket.borrow();
        let on = |name: u32| {
            own.options
                .get(&(SOL_SOCKET, name))
                .is_some_and(|value| value.iter().any(|&byte| byte != 0))
        };
        let (mark, priority) = (on(SO_RCVMARK), on(SO_RCVPRIORITY));
        drop(own);
        if mark {
            put_control(
                &mut control,
                room,
                SOL_SOCKET,
                SO_MARK,
                &arrival.mark.to_le_bytes(),
                &mut out_flags,
            );
        }
        if priority {
            put_control(
                &mut control,
                room,
                SOL_SOCKET,
                SO_PRIORITY,
                &arrival.priority.to_le_bytes(),
                &mut out_flags,
            );
        }
    }
    // `udp_cmsg_recv`: a datagram kept whole for UDP_GRO tells its segment size.
    let receives_whole = socket
        .borrow()
        .options
        .get(&(
            crate::linux::net::IPPROTO_UDP as u32,
            crate::linux_extra::UDP_GRO,
        ))
        .is_some_and(|value| value.iter().any(|&byte| byte != 0));
    if let Some(arrival) = received
        .arrival
        .filter(|arrival| arrival.segment > 0 && receives_whole)
    {
        let data = i32::from(arrival.segment).to_le_bytes();
        let level = crate::linux::net::IPPROTO_UDP as u32;
        put_control(
            &mut control,
            room,
            level,
            crate::linux_extra::UDP_GRO,
            &data,
            &mut out_flags,
        );
    }
    if let Some(arrival) = &received.arrival {
        for (level, kind, data) in inet::arrival_messages(kernel, cx, &socket, arrival) {
            put_control(&mut control, room, level, kind, &data, &mut out_flags);
        }
    }
    if let Some((level, kind, data)) = &received.error {
        put_control(&mut control, room, *level, *kind, data, &mut out_flags);
    }
    // A datagram read that found none (its end) gives no control messages
    // (`__unix_dgram_recvmsg`); a stream read that examined nothing has empty credentials.
    let local_control = socket.borrow().family == AF_UNIX
        && (received.cred.is_some() || socket.borrow().kind == SOCK_STREAM);
    let passcred = socket.borrow().passcred && local_control;
    if passcred {
        let cred = received.cred.unwrap_or(Cred {
            pid: 0,
            uid: 0,
            gid: 0,
        });
        let mut data = Vec::new();
        data.extend_from_slice(&(cred.pid as u32).to_le_bytes());
        data.extend_from_slice(&cred.uid.to_le_bytes());
        data.extend_from_slice(&cred.gid.to_le_bytes());
        put_control(
            &mut control,
            room,
            SOL_SOCKET,
            SCM_CREDENTIALS,
            &data,
            &mut out_flags,
        );
    }
    if !received.files.is_empty() {
        let space = room.saturating_sub(control.len());
        let fits = space.saturating_sub(CMSG_HEADER as usize) / 4;
        let count = fits.min(received.files.len());
        if count < received.files.len() {
            out_flags |= MSG_CTRUNC;
        }
        let mut data = Vec::new();
        let cloexec = flags & MSG_CMSG_CLOEXEC != 0;
        let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
        let mut files = received.files.into_iter();
        for file in files.by_ref().take(count) {
            match kernel.files_mut(tid).add(file, cloexec, 0, limit) {
                Ok(fd) => data.extend_from_slice(&fd.to_le_bytes()),
                Err(_) => {
                    out_flags |= MSG_CTRUNC;
                    break;
                }
            }
        }
        kernel.released.extend(files);
        if !data.is_empty() {
            let length = CMSG_HEADER as usize + data.len();
            control.extend_from_slice(&(length as u64).to_le_bytes());
            control.extend_from_slice(&SOL_SOCKET.to_le_bytes());
            control.extend_from_slice(&SCM_RIGHTS.to_le_bytes());
            control.extend_from_slice(&data);
            // `scm_detach_fds` counts the padding too, within the room.
            let padded = control
                .len()
                .next_multiple_of(CMSG_ALIGN as usize)
                .min(room);
            control.resize(padded, 0);
        }
    }
    // `scm_pidfd_recv`, after the descriptors: a process descriptor (close on exec) for the
    // sender, made only when its whole message fits; too little room is MSG_CTRUNC whether or
    // not the message carries a sender.
    let sender = received.cred.map(|cred| cred.pid).filter(|pid| *pid != 0);
    if socket.borrow().pass_pidfd && local_control {
        if room.saturating_sub(control.len()) < CMSG_HEADER as usize + 4 {
            out_flags |= MSG_CTRUNC;
        } else if let Some(pid) = sender {
            let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
            let fd = super::pidfd::make(kernel, cx, tid, pid, 0)
                .and_then(|file| kernel.files_mut(tid).add(file, true, 0, limit))
                .unwrap_or_else(|error| -(error.0 as i32));
            let data = fd.to_le_bytes();
            put_control(
                &mut control,
                room,
                SOL_SOCKET,
                crate::linux_extra::SCM_PIDFD,
                &data,
                &mut out_flags,
            );
        }
    }
    // SO_INQ: what is left to read, once the read went well.
    if socket.borrow().report_queued {
        let left = queued(&socket.borrow()) as i32;
        put_control(
            &mut control,
            room,
            SOL_SOCKET,
            crate::linux::net::SCM_INQ,
            &left.to_le_bytes(),
            &mut out_flags,
        );
    }
    if !control.is_empty() {
        guest(kernel, cx, tid).write(header.control, &control)?;
    }
    // `msg_namelen`, `msg_controllen` and `msg_flags` go back.
    guest(kernel, cx, tid).write_value(address + 8, &name_length)?;
    guest(kernel, cx, tid).write_value(address + 40, &(control.len() as u64))?;
    guest(kernel, cx, tid).write_value(address + 48, &out_flags)?;
    Ok(received.reported.unwrap_or(copied))
}

/// What a stream holds unread, as FIONREAD and SCM_INQ tell it (`unix_inq_len`, `tcp_inq`):
/// with TCP's urgent byte ahead and not inline, what comes before it.
fn queued(own: &Socket) -> u64 {
    let tcp_mark = own.inet.is_some() && !own.urgent_inline();
    own.queue
        .iter()
        .take_while(|packet| !(tcp_mark && packet.urgent))
        .map(|packet| (packet.data.len() - packet.consumed) as u64)
        .sum::<u64>()
        + own.skipped_urgent
}

/// The bits of [`Socket::stamps`].
const STAMP: u8 = 1;
const STAMP_NS: u8 = 2;
const STAMP_NEW: u8 = 4;

/// The time a packet comes, if its receiver asked for timestamps (Linux stamps buffers as they
/// arrive while some socket wants them).
fn stamp(kernel: &Kernel, cx: &mut Context<'_>, receiver: &SocketRef) -> Option<crate::fs::Time> {
    use crate::linux::net::SOF_TIMESTAMPING_RX_SOFTWARE;
    let own = receiver.borrow();
    // Internet packets are stamped as they come for receive timestamping too; local ones only
    // for SO_TIMESTAMP (`unix_dgram_sendmsg`).
    let asked = own.stamps & STAMP != 0
        || (own.family != AF_UNIX && own.tsflags & tsflag(SOF_TIMESTAMPING_RX_SOFTWARE) != 0);
    asked.then(|| kernel.now(cx))
}

/// One SO_TIMESTAMPING flag as a number.
fn tsflag(flag: crate::linux::net::_bindgen_ty_10) -> u32 {
    flag as u32
}

/// `put_cmsg`: a control message padded to the next one; one that does not fit whole is cut to
/// the room left, with MSG_CTRUNC, and none at all when not even its header fits.
fn put_control(
    control: &mut Vec<u8>,
    room: usize,
    level: u32,
    kind: u32,
    data: &[u8],
    flags: &mut u32,
) {
    let left = room.saturating_sub(control.len());
    if left < CMSG_HEADER as usize {
        *flags |= MSG_CTRUNC;
        return;
    }
    let mut length = CMSG_HEADER as usize + data.len();
    if length > left {
        *flags |= MSG_CTRUNC;
        length = left;
    }
    control.extend_from_slice(&(length as u64).to_le_bytes());
    control.extend_from_slice(&level.to_le_bytes());
    control.extend_from_slice(&kind.to_le_bytes());
    control.extend_from_slice(&data[..length - CMSG_HEADER as usize]);
    let padded = control
        .len()
        .next_multiple_of(CMSG_ALIGN as usize)
        .min(room);
    control.resize(padded, 0);
}

pub(super) fn setsockopt(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, level, name, value, length, _]: Args,
) -> Result<u64, Errno> {
    let (_, socket) = socket_of(kernel, tid, fd)?;
    let (level, name, length) = (level as u32, name as u32, length as i32);
    if length < 0 {
        return Err(Errno::EINVAL);
    }
    if socket.borrow().inet.is_some() {
        // The longest value an internet socket reads is `struct group_req`'s.
        let bytes = guest(kernel, cx, tid).read(value, (length as u64).min(136))?;
        if let Some(answer) = inet::set_option(kernel, cx, tid, &socket, level, name, &bytes) {
            return answer;
        }
    } else if socket.borrow().netlink.is_some() && level != SOL_SOCKET {
        // `netlink_setsockopt` refuses other levels before reading the value.
        if level != SOL_NETLINK {
            return Err(Errno::ENOPROTOOPT);
        }
        let bytes = guest(kernel, cx, tid).read(value, (length as u64).min(64))?;
        return netlink::set_option(&socket, level, name, &bytes)
            .unwrap_or(Err(Errno::ENOPROTOOPT));
    } else if level != SOL_SOCKET {
        return Err(Errno::EOPNOTSUPP);
    }
    use crate::linux::net::*;
    let bytes = guest(kernel, cx, tid).read(value, (length as u64).min(64))?;
    // `sk_setsockopt`: every option but a device name is at least an int.
    if name != SO_BINDTODEVICE && bytes.len() < 4 {
        return Err(Errno::EINVAL);
    }
    let int = || -> Result<i32, Errno> {
        if bytes.len() < 4 {
            return Err(Errno::EINVAL);
        }
        Ok(i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")))
    };
    let mut own = socket.borrow_mut();
    match name {
        SO_SNDBUF | SO_RCVBUF => {
            let wanted = (int()? as u32 as u64).min(MAX_BUFFER) * 2;
            let lock = if name == SO_SNDBUF {
                SOCK_SNDBUF_LOCK
            } else {
                SOCK_RCVBUF_LOCK
            };
            let locks = buffer_locks(&own) | lock;
            own.options
                .insert((level, SO_BUF_LOCK), locks.to_le_bytes().to_vec());
            if name == SO_SNDBUF {
                own.sndbuf = wanted.max(MIN_SNDBUF);
                drop(own);
                kernel.wake_poll(channel(&socket), EPOLLOUT | EPOLLWRNORM | EPOLLWRBAND);
            } else {
                own.rcvbuf = wanted.max(MIN_RCVBUF);
            }
        }
        SO_SNDBUFFORCE | SO_RCVBUFFORCE => return Err(Errno::EPERM),
        // `sock_setbindtodevice` (a name of at most 15 bytes; none lets go) and
        // `sock_bindtoindex_locked` (an index the program names, kept whether or not it exists).
        SO_BINDTODEVICE | SO_BINDTOIFINDEX => {
            let index = if name == SO_BINDTOIFINDEX {
                int()?
            } else {
                let device = &bytes[..bytes.len().min(15)];
                let device = &device[..device.iter().position(|&b| b == 0).unwrap_or(device.len())];
                if device.is_empty() {
                    0
                } else {
                    crate::network::index_of(device).ok_or(Errno::ENODEV)? as i32
                }
            };
            if bound_device(&own) != 0 && !kernel.credentials(tid).has(CAP_NET_RAW) {
                return Err(Errno::EPERM);
            }
            if index < 0 {
                return Err(Errno::EINVAL);
            }
            own.options
                .insert((level, SO_BINDTOIFINDEX), index.to_le_bytes().to_vec());
        }
        // `sk_setsockopt`: a mark wants CAP_NET_RAW or CAP_NET_ADMIN, as does a priority past
        // the ones anyone may choose (`sk_set_prio_allowed`).
        SO_MARK | SO_PRIORITY => {
            let value = int()?;
            let credentials = kernel.credentials(tid);
            let powerful = credentials.has(CAP_NET_RAW) || credentials.has(CAP_NET_ADMIN);
            if !(powerful || (name == SO_PRIORITY && (0..=6).contains(&value))) {
                return Err(Errno::EPERM);
            }
            own.options
                .insert((level, name), value.to_le_bytes().to_vec());
        }
        // The transmit time each send may name is kept for nothing: no queue discipline holds
        // packets here. Clocks other than CLOCK_MONOTONIC want CAP_NET_ADMIN.
        SO_TXTIME => {
            if bytes.len() != 8 {
                return Err(Errno::EINVAL);
            }
            let clock = int()?;
            let flags = u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes"));
            if flags & !(txtime_flags::SOF_TXTIME_FLAGS_MASK as u32) != 0 {
                return Err(Errno::EINVAL);
            }
            if clock != CLOCK_MONOTONIC as i32 && !kernel.credentials(tid).has(CAP_NET_ADMIN) {
                return Err(Errno::EPERM);
            }
            if ![CLOCK_REALTIME, CLOCK_MONOTONIC, CLOCK_TAI].contains(&(clock as u32)) {
                return Err(Errno::EINVAL);
            }
            own.options.insert((level, name), bytes[..8].to_vec());
        }
        // `sk_may_scm_recv` and `sk_is_unix`.
        SO_PASSCRED | SO_PASSSEC if own.inet.is_some() => return Err(Errno::EOPNOTSUPP),
        SO_PASSPIDFD | SO_PASSRIGHTS if own.family != AF_UNIX => return Err(Errno::EOPNOTSUPP),
        SO_PASSCRED => own.passcred = int()? != 0,
        SO_PASSPIDFD => own.pass_pidfd = int()? != 0,
        SO_PASSRIGHTS => own.pass_rights = int()? != 0,
        // `unix_setsockopt`, which only local streams reach: 0 or 1.
        SO_INQ if own.family == AF_UNIX && own.kind == SOCK_STREAM => {
            if bytes.len() != 4 {
                return Err(Errno::EINVAL);
            }
            own.report_queued = match int()? {
                0 => false,
                1 => true,
                _ => return Err(Errno::EINVAL),
            };
        }
        SO_INQ => return Err(Errno::ENOPROTOOPT),
        SO_PEEK_OFF => {
            let offset = int()?;
            own.peek_offset = (offset >= 0).then_some(offset as u64);
        }
        SO_RCVTIMEO_OLD | SO_RCVTIMEO_NEW | SO_SNDTIMEO_OLD | SO_SNDTIMEO_NEW => {
            if bytes.len() < 16 {
                return Err(Errno::EINVAL);
            }
            let seconds = i64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"));
            let micros = i64::from_le_bytes(bytes[8..16].try_into().expect("8 bytes"));
            if !(0..1_000_000).contains(&micros) {
                return Err(Errno::EDOM);
            }
            let nanoseconds = if seconds < 0 {
                0
            } else {
                (seconds as u64)
                    .saturating_mul(1_000_000_000)
                    .saturating_add(micros as u64 * 1000)
            };
            if matches!(name, SO_RCVTIMEO_OLD | SO_RCVTIMEO_NEW) {
                own.receive_timeout = nanoseconds;
            } else {
                own.send_timeout = nanoseconds;
            }
            own.options.insert((level, name), bytes[..16].to_vec());
        }
        // `sock_setsockopt`: zero copy sends are for internet sockets only.
        SO_ZEROCOPY if own.inet.is_none() => return Err(Errno::EOPNOTSUPP),
        // `sock_set_timestamp`.
        SO_TIMESTAMP_OLD | SO_TIMESTAMPNS_OLD | SO_TIMESTAMP_NEW | SO_TIMESTAMPNS_NEW => {
            let new = if matches!(name, SO_TIMESTAMP_NEW | SO_TIMESTAMPNS_NEW) {
                STAMP_NEW
            } else {
                0
            };
            let ns = matches!(name, SO_TIMESTAMPNS_OLD | SO_TIMESTAMPNS_NEW);
            own.stamps = match (int()? != 0, ns) {
                (false, _) => own.stamps & STAMP_NEW,
                (true, false) => STAMP | new,
                (true, true) => STAMP | STAMP_NS | new,
            };
        }
        // `sock_set_timestamping`: an int, or `struct so_timestamping` naming a clock to bind.
        SO_TIMESTAMPING_OLD | SO_TIMESTAMPING_NEW => {
            let flags = int()? as u32;
            if flags & !tsflag(SOF_TIMESTAMPING_MASK) != 0 {
                return Err(Errno::EINVAL);
            }
            let id = tsflag(SOF_TIMESTAMPING_OPT_ID);
            if flags & tsflag(SOF_TIMESTAMPING_OPT_ID_TCP) != 0 && flags & id == 0 {
                return Err(Errno::EINVAL);
            }
            let tcp = own.inet.is_some() && own.kind == SOCK_STREAM;
            if flags & id != 0 && own.tsflags & id == 0 && tcp && own.state != State::Connected {
                return Err(Errno::EINVAL);
            }
            let tsonly = flags & tsflag(SOF_TIMESTAMPING_OPT_TSONLY) != 0;
            if flags & tsflag(SOF_TIMESTAMPING_OPT_STATS) != 0 && !tsonly {
                return Err(Errno::EINVAL);
            }
            // No device here has a hardware clock to bind to.
            if flags & tsflag(SOF_TIMESTAMPING_BIND_PHC) != 0 {
                return Err(Errno::EOPNOTSUPP);
            }
            // Software send timestamps are built for UDP with OPT_TSONLY (local and netlink
            // sockets make none, as on Linux, and hardware ones never come on these
            // interfaces); TCP's and copies of the packets are not: answered as a Linux without
            // the option.
            let software_sends = tsflag(SOF_TIMESTAMPING_TX_SOFTWARE)
                | tsflag(SOF_TIMESTAMPING_TX_SCHED)
                | tsflag(SOF_TIMESTAMPING_TX_ACK)
                | tsflag(SOF_TIMESTAMPING_TX_COMPLETION);
            let udp = own.inet.is_some() && own.kind == SOCK_DGRAM;
            let built = own.inet.is_none()
                || (udp && tsonly && flags & tsflag(SOF_TIMESTAMPING_TX_COMPLETION) == 0);
            if flags & software_sends != 0 && !built {
                drop(own);
                return Err(kernel.counters.refuse(
                    cx.host,
                    crate::reason::OPTION_NOT_BUILT,
                    (u64::from(level) << 32) | u64::from(name),
                    Errno::ENOPROTOOPT,
                ));
            }
            if flags & id != 0 && own.tsflags & id == 0 {
                own.tskey = 0;
            }
            own.tsflags = flags;
            own.stamps = if name == SO_TIMESTAMPING_NEW {
                own.stamps | STAMP_NEW
            } else {
                own.stamps & !STAMP_NEW
            };
        }
        // Kept as given: busy polling's microseconds (never negative) and the CPU asked for.
        SO_BUSY_POLL | SO_INCOMING_CPU => {
            let value = int()?;
            if name == SO_BUSY_POLL && value < 0 {
                return Err(Errno::EINVAL);
            }
            own.options
                .insert((level, name), value.to_le_bytes().to_vec());
        }
        // A negative low water mark is the most there is; TCP's is capped (`tcp_set_rcvlowat`).
        SO_RCVLOWAT => {
            let value = int()?;
            let value = if value < 0 { i32::MAX } else { value };
            if own.inet.is_some() && own.kind == SOCK_STREAM {
                drop(own);
                inet::set_receive_low_water(kernel, cx, &socket, value);
                return Ok(0);
            }
            own.options
                .insert((level, name), value.max(1).to_le_bytes().to_vec());
        }
        SO_REUSEADDR | SO_REUSEPORT | SO_KEEPALIVE | SO_BROADCAST | SO_OOBINLINE | SO_DONTROUTE
        | SO_PASSSEC | SO_ZEROCOPY | SO_RCVMARK | SO_RCVPRIORITY => {
            let value = int()?;
            own.options
                .insert((level, name), i32::from(value != 0).to_le_bytes().to_vec());
        }
        SO_LINGER => {
            if bytes.len() < 8 {
                return Err(Errno::EINVAL);
            }
            own.options.insert((level, name), bytes[..8].to_vec());
        }
        // `sk_setsockopt`: debugging wants CAP_NET_ADMIN to turn on, as do busy polls.
        SO_DEBUG | SO_PREFER_BUSY_POLL => {
            let on = int()? != 0;
            if on && !kernel.credentials(tid).has(CAP_NET_ADMIN) {
                return Err(if name == SO_DEBUG {
                    Errno::EACCES
                } else {
                    Errno::EPERM
                });
            }
            own.options
                .insert((level, name), i32::from(on).to_le_bytes().to_vec());
        }
        // A filter once locked stays locked.
        SO_LOCK_FILTER => {
            let on = int()? != 0;
            if !on && flag_on(&own, SO_LOCK_FILTER) {
                return Err(Errno::EPERM);
            }
            own.options
                .insert((level, name), i32::from(on).to_le_bytes().to_vec());
        }
        // Kept and read back; nothing here acts on them: checksums, frame check sequences and
        // wireless status are for devices this machine does not have.
        SO_NO_CHECK | SO_WIFI_STATUS | SO_NOFCS | SO_RXQ_OVFL | SO_SELECT_ERR_QUEUE => {
            let on = i32::from(int()? != 0);
            own.options.insert((level, name), on.to_le_bytes().to_vec());
        }
        SO_BSDCOMPAT => {
            int()?;
        }
        // `sk_detach_filter`: none is attached here.
        SO_DETACH_FILTER => {
            return Err(if flag_on(&own, SO_LOCK_FILTER) {
                Errno::EPERM
            } else {
                Errno::ENOENT
            });
        }
        // The rate TCP paces its sends at (`sk_max_pacing_rate`): kept, and counted when it is
        // not unlimited, as nothing here paces.
        SO_MAX_PACING_RATE => {
            let value = int()? as u32;
            let rate = if bytes.len() >= 8 {
                u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"))
            } else if value == u32::MAX {
                u64::MAX
            } else {
                u64::from(value)
            };
            own.options
                .insert((level, name), rate.to_le_bytes().to_vec());
            drop(own);
            if rate != u64::MAX {
                kernel.counters.count(
                    cx.host,
                    crate::reason::OPTION_NOT_ACTED_ON,
                    (u64::from(level) << 32) | u64::from(name),
                );
            }
        }
        SO_BUF_LOCK => {
            let locks = int()? as u32;
            if locks & !SOCK_BUF_LOCK_MASK != 0 {
                return Err(Errno::EINVAL);
            }
            own.options
                .insert((level, name), locks.to_le_bytes().to_vec());
            if let Some(inet) = own.inet.as_deref_mut() {
                inet.sndbuf_set = locks & SOCK_SNDBUF_LOCK != 0;
                inet.rcvbuf_set = locks & SOCK_RCVBUF_LOCK != 0;
            }
        }
        // `sk_setsockopt`: TCP alone rehashes its flow on a retransmit.
        SO_TXREHASH => {
            let value = int()?;
            if !own
                .inet
                .as_deref()
                .is_some_and(|inet| inet.protocol == inet::Protocol::Tcp)
            {
                return Err(Errno::EOPNOTSUPP);
            }
            if !(-1..=1).contains(&value) {
                return Err(Errno::EINVAL);
            }
            let kept = if value == -1 { 1 } else { value };
            own.options
                .insert((level, name), kept.to_le_bytes().to_vec());
        }
        SO_SNDLOWAT | SO_PEERCRED | SO_TYPE | SO_ERROR | SO_ACCEPTCONN | SO_DOMAIN
        | SO_PROTOCOL | SO_PEERSEC | SO_PEERNAME | SO_PEERPIDFD | SO_BPF_EXTENSIONS
        | SO_INCOMING_NAPI_ID | SO_COOKIE | SO_PEERGROUPS | SO_MEMINFO | SO_NETNS_COOKIE => {
            return Err(Errno::ENOPROTOOPT);
        }
        _ => {
            drop(own);
            return Err(kernel.counters.refuse(
                cx.host,
                crate::reason::OPTION_NOT_BUILT,
                (u64::from(level) << 32) | u64::from(name),
                Errno::ENOPROTOOPT,
            ));
        }
    }
    Ok(0)
}

/// SO_PEERPIDFD: a new process descriptor (close on exec) for the process the other end was made
/// by, which may have been reaped since (`PIDFD_STALE`), its number cut to the room given.
fn peer_pidfd(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    value: u64,
    length: u64,
    room: i32,
) -> Result<u64, Errno> {
    let pid = socket
        .borrow()
        .peer_cred
        .map(|cred| cred.pid)
        .filter(|pid| *pid != 0)
        .ok_or(Errno::ENODATA)?;
    let file = super::pidfd::make(kernel, cx, tid, pid, 0)?;
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel.files(tid).lowest_free(0, limit)?;
    let room = (room as usize).min(4);
    guest(kernel, cx, tid).write(value, &fd.to_le_bytes()[..room])?;
    guest(kernel, cx, tid).write_value(length, &(room as i32))?;
    kernel.files_mut(tid).add(file, true, 0, limit)?;
    Ok(0)
}

/// Whether a socket-level flag kept in its options is on.
fn flag_on(own: &Socket, name: u32) -> bool {
    own.options
        .get(&(crate::linux::net::SOL_SOCKET, name))
        .is_some_and(|value| value.iter().any(|&byte| byte != 0))
}

/// SO_BUF_LOCK: which of its buffer sizes the program set (`sk_userlocks`); an internet
/// socket keeps them in its own part.
fn buffer_locks(own: &Socket) -> u32 {
    use crate::linux::net::{SOCK_RCVBUF_LOCK, SOCK_SNDBUF_LOCK};
    if let Some(inet) = own.inet.as_deref() {
        return (u32::from(inet.sndbuf_set) * SOCK_SNDBUF_LOCK)
            | (u32::from(inet.rcvbuf_set) * SOCK_RCVBUF_LOCK);
    }
    own.options
        .get(&(
            crate::linux::net::SOL_SOCKET,
            crate::linux::net::SO_BUF_LOCK,
        ))
        .map_or(0, |bytes| {
            u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes"))
        })
}

/// The interface a socket is bound to (SO_BINDTODEVICE), 0 for none.
fn bound_device(own: &Socket) -> i32 {
    own.options
        .get(&(
            crate::linux::net::SOL_SOCKET,
            crate::linux::net::SO_BINDTOIFINDEX,
        ))
        .map_or(0, |bytes| {
            i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes"))
        })
}

pub(super) fn getsockopt(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, level, name, value, length, _]: Args,
) -> Result<u64, Errno> {
    let (_, socket) = socket_of(kernel, tid, fd)?;
    let (level, name) = (level as u32, name as u32);
    // `netlink_getsockopt` refuses other levels before reading the length.
    if socket.borrow().netlink.is_some() && level != SOL_SOCKET && level != SOL_NETLINK {
        return Err(Errno::ENOPROTOOPT);
    }
    let room: i32 = guest(kernel, cx, tid).read_value(length)?;
    // A local socket has no options past its own level, whatever the length (`do_sock_getsockopt`).
    if socket.borrow().local() && level != SOL_SOCKET {
        return Err(Errno::EOPNOTSUPP);
    }
    if socket.borrow().inet.is_some() {
        inet::level_known(&socket, level)?;
    }
    // IPv6 and UDP options take the length as unsigned (`min_t(unsigned int, ...)`), so a
    // negative one is as long as can be; the socket level, IPv4 and TCP refuse it.
    let unsigned = socket.borrow().inet.is_some()
        && matches!(
            level,
            crate::linux::net::SOL_IPV6 | crate::linux::net::SOL_UDP
        );
    let room = if room < 0 && unsigned { i32::MAX } else { room };
    if room < 0 {
        return Err(Errno::EINVAL);
    }
    if socket.borrow().inet.is_some() {
        if let Some(answer) = inet::get_option(kernel, cx, &socket, level, name, room) {
            let bytes = answer?;
            // `ip_getsockopt` gives a small value in one byte to a buffer shorter than an int.
            let small = level == crate::linux::net::SOL_IP
                && bytes.len() == 4
                && (room as usize) < 4
                && room > 0
                && (0..=255).contains(&i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")));
            let copied = if small {
                1
            } else {
                (room as usize).min(bytes.len())
            };
            // The protocols write the length first (`do_ip_getsockopt`, `do_tcp_getsockopt`,
            // `do_ipv6_getsockopt`); the socket level writes the value first.
            if level == SOL_SOCKET {
                guest(kernel, cx, tid).write(value, &bytes[..copied])?;
                guest(kernel, cx, tid).write_value(length, &(copied as i32))?;
            } else {
                guest(kernel, cx, tid).write_value(length, &(copied as i32))?;
                guest(kernel, cx, tid).write(value, &bytes[..copied])?;
            }
            return Ok(0);
        }
    } else if socket.borrow().netlink.is_some() && level != SOL_SOCKET {
        let bytes = netlink::get_option(&socket, level, name).unwrap_or(Err(Errno::ENOPROTOOPT))?;
        if room < 4 {
            return Err(Errno::EINVAL);
        }
        guest(kernel, cx, tid).write(value, &bytes)?;
        guest(kernel, cx, tid).write_value(length, &(bytes.len() as i32))?;
        return Ok(0);
    } else if level != SOL_SOCKET {
        return Err(Errno::EOPNOTSUPP);
    }
    use crate::linux::net::*;
    if name == SO_PEERPIDFD {
        return peer_pidfd(kernel, cx, tid, &socket, value, length, room);
    }
    // `sock_getbindtodevice`: none bound reads as nothing; a name wants room for any name.
    if name == SO_BINDTODEVICE {
        let index = bound_device(&socket.borrow());
        let mut bytes = Vec::new();
        if index != 0 {
            if room < 16 {
                return Err(Errno::EINVAL);
            }
            let device = crate::network::name_of(index as u32).ok_or(Errno::ENODEV)?;
            bytes.extend_from_slice(device.as_bytes());
            bytes.push(0);
        }
        guest(kernel, cx, tid).write(value, &bytes)?;
        guest(kernel, cx, tid).write_value(length, &(bytes.len() as i32))?;
        return Ok(0);
    }
    let int = |value: i32| value.to_le_bytes().to_vec();
    let bytes = {
        let mut own = socket.borrow_mut();
        match name {
            SO_BINDTOIFINDEX => int(bound_device(&own)),
            SO_TYPE => int(own.kind as i32),
            SO_DOMAIN => int(own.family as i32),
            SO_PROTOCOL => int(own.protocol as i32),
            SO_ERROR => {
                let error = core::mem::take(&mut own.error);
                int(error)
            }
            SO_ACCEPTCONN => int(i32::from(own.state == State::Listening)),
            SO_SNDBUF => int(own.sndbuf as i32),
            SO_RCVBUF => int(own.rcvbuf as i32),
            SO_PASSCRED | SO_PASSSEC if own.inet.is_some() => return Err(Errno::EOPNOTSUPP),
            SO_PASSPIDFD | SO_PASSRIGHTS if own.family != AF_UNIX => {
                return Err(Errno::EOPNOTSUPP);
            }
            SO_PASSCRED => int(i32::from(own.passcred)),
            SO_PASSPIDFD => int(i32::from(own.pass_pidfd)),
            SO_PASSRIGHTS => int(i32::from(own.pass_rights)),
            SO_INQ => return Err(Errno::ENOPROTOOPT),
            SO_PEEK_OFF => int(own.peek_offset.map_or(-1, |offset| offset as i32)),
            SO_SNDLOWAT => int(1),
            SO_RCVLOWAT => own
                .options
                .get(&(level, name))
                .cloned()
                .unwrap_or_else(|| int(1)),
            SO_PEERCRED => {
                let cred = own.peer_cred.unwrap_or(Cred {
                    pid: 0,
                    uid: u32::MAX,
                    gid: u32::MAX,
                });
                let mut bytes = (cred.pid as u32).to_le_bytes().to_vec();
                bytes.extend_from_slice(&cred.uid.to_le_bytes());
                bytes.extend_from_slice(&cred.gid.to_le_bytes());
                bytes
            }
            SO_LINGER | SO_TXTIME => own
                .options
                .get(&(level, name))
                .cloned()
                .unwrap_or_else(|| alloc::vec![0; 8]),
            SO_RCVTIMEO_OLD | SO_RCVTIMEO_NEW | SO_SNDTIMEO_OLD | SO_SNDTIMEO_NEW => {
                let nanoseconds = if matches!(name, SO_RCVTIMEO_OLD | SO_RCVTIMEO_NEW) {
                    own.receive_timeout
                } else {
                    own.send_timeout
                };
                let mut bytes = (nanoseconds / 1_000_000_000).to_le_bytes().to_vec();
                bytes.extend_from_slice(&(nanoseconds % 1_000_000_000 / 1000).to_le_bytes());
                bytes
            }
            SO_REUSEADDR | SO_REUSEPORT | SO_KEEPALIVE | SO_BROADCAST | SO_OOBINLINE
            | SO_DONTROUTE | SO_PASSSEC | SO_PRIORITY | SO_ZEROCOPY | SO_BUSY_POLL
            | SO_INCOMING_CPU | SO_MARK | SO_RCVMARK | SO_RCVPRIORITY => own
                .options
                .get(&(level, name))
                .cloned()
                .unwrap_or_else(|| int(if name == SO_INCOMING_CPU { -1 } else { 0 })),
            // `sk_getsockopt`'s reading of the flags.
            SO_TIMESTAMP_OLD | SO_TIMESTAMPNS_OLD | SO_TIMESTAMP_NEW | SO_TIMESTAMPNS_NEW => {
                let flags = own.stamps;
                let (stamp, ns, new) = (
                    flags & STAMP != 0,
                    flags & STAMP_NS != 0,
                    flags & STAMP_NEW != 0,
                );
                int(i32::from(match name {
                    SO_TIMESTAMP_OLD => stamp && !new && !ns,
                    SO_TIMESTAMPNS_OLD => ns && !new,
                    SO_TIMESTAMP_NEW => stamp && new,
                    _ => ns && new,
                }))
            }
            // `struct so_timestamping`: the flags and no bound clock.
            SO_TIMESTAMPING_OLD | SO_TIMESTAMPING_NEW => {
                let mut bytes = own.tsflags.to_le_bytes().to_vec();
                bytes.extend_from_slice(&0u32.to_le_bytes());
                bytes
            }
            SO_PEERNAME => {
                let peer = own.peer.clone().ok_or(Errno::ENOTCONN)?;
                drop(own);
                let bytes = address_bytes(peer.borrow().name.as_deref());
                if (room as usize) < bytes.len() {
                    return Err(Errno::EINVAL);
                }
                bytes
            }
            SO_PEERSEC => return Err(Errno::ENOPROTOOPT),
            SO_DEBUG | SO_NO_CHECK | SO_WIFI_STATUS | SO_NOFCS | SO_RXQ_OVFL
            | SO_SELECT_ERR_QUEUE | SO_LOCK_FILTER | SO_PREFER_BUSY_POLL => {
                int(i32::from(flag_on(&own, name)))
            }
            // Nothing reserved, no busy polling device, and no filter attached.
            SO_BSDCOMPAT | SO_INCOMING_NAPI_ID | SO_RESERVE_MEM => int(0),
            SO_ATTACH_FILTER => Vec::new(),
            // `bpf_tell_extensions`: SKF_AD_MAX.
            SO_BPF_EXTENSIONS => int(64),
            // A room shorter than a long reads an int, at most `~0U`.
            SO_MAX_PACING_RATE => {
                let rate = own.options.get(&(level, name)).map_or(u64::MAX, |bytes| {
                    u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"))
                });
                if room < 8 {
                    int(rate.min(u64::from(u32::MAX)) as u32 as i32)
                } else {
                    rate.to_le_bytes().to_vec()
                }
            }
            SO_COOKIE => {
                if room < 8 {
                    return Err(Errno::EINVAL);
                }
                if own.cookie == 0 {
                    drop(own);
                    kernel.sockets.cookies += 1;
                    socket.borrow_mut().cookie = kernel.sockets.cookies;
                    own = socket.borrow_mut();
                }
                own.cookie.to_le_bytes().to_vec()
            }
            // The one network namespace's cookie.
            SO_NETNS_COOKIE => {
                if room != 8 {
                    return Err(Errno::EINVAL);
                }
                1u64.to_le_bytes().to_vec()
            }
            // The other end's supplementary groups; with too little room, the room wanted.
            SO_PEERGROUPS => {
                if own.peer_cred.is_none() {
                    return Err(Errno::ENODATA);
                }
                let wanted = own.peer_groups.len() * 4;
                if (room as usize) < wanted {
                    drop(own);
                    guest(kernel, cx, tid).write_value(length, &(wanted as i32))?;
                    return Err(Errno::ERANGE);
                }
                let bytes: Vec<u8> = own
                    .peer_groups
                    .iter()
                    .flat_map(|group| group.to_le_bytes())
                    .collect();
                drop(own);
                guest(kernel, cx, tid).write(value, &bytes)?;
                guest(kernel, cx, tid).write_value(length, &(bytes.len() as i32))?;
                return Ok(0);
            }
            SO_BUF_LOCK => int(buffer_locks(&own) as i32),
            SO_TXREHASH => {
                if !own
                    .inet
                    .as_deref()
                    .is_some_and(|inet| inet.protocol == inet::Protocol::Tcp)
                {
                    return Err(Errno::EOPNOTSUPP);
                }
                own.options
                    .get(&(level, name))
                    .cloned()
                    .unwrap_or_else(|| int(1))
            }
            _ => {
                drop(own);
                return Err(kernel.counters.refuse(
                    cx.host,
                    crate::reason::OPTION_NOT_BUILT,
                    (u64::from(level) << 32) | u64::from(name),
                    Errno::ENOPROTOOPT,
                ));
            }
        }
    };
    let copied = (room as usize).min(bytes.len());
    guest(kernel, cx, tid).write(value, &bytes[..copied])?;
    guest(kernel, cx, tid).write_value(length, &(copied as i32))?;
    Ok(0)
}

/// `sock_ioctl` and the families' `ioctl`: the bytes waiting and unsent, the owner, and the
/// questions about interfaces (`dev_ioctl`, `devinet_ioctl`).
pub(super) fn ioctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    socket: &SocketRef,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    use crate::linux::ioctl::*;
    let put_int = |kernel: &mut Kernel, cx: &mut Context<'_>, value: i32| {
        guest(kernel, cx, tid)
            .write_value(argument, &value)
            .map(|()| 0)
    };
    let (family, kind, listening) = {
        let own = socket.borrow();
        (own.family, own.kind, own.state == State::Listening)
    };
    match request {
        FIOSETOWN | SIOCSPGRP => {
            let who: i32 = guest(kernel, cx, tid).read_value(argument)?;
            return super::io::set_owner_number(kernel, tid, file, who);
        }
        FIOGETOWN | SIOCGPGRP => {
            let owner = super::io::get_owner(kernel, file);
            return put_int(kernel, cx, owner);
        }
        SIOCGIFCONF => return interface_list(kernel, cx, tid, argument),
        _ => {}
    }
    if family == AF_UNIX || family == AF_INET || family == AF_INET6 {
        match request {
            FIONREAD | TIOCOUTQ if listening => return Err(Errno::EINVAL),
            FIONREAD => {
                let own = socket.borrow();
                let amount = if kind == SOCK_DGRAM {
                    own.queue.front().map_or(0, |packet| packet.data.len())
                } else {
                    queued(&own) as usize
                };
                drop(own);
                return put_int(kernel, cx, amount as i32);
            }
            TIOCOUTQ => {
                let own = socket.borrow();
                let amount = match &own.inet {
                    Some(_) if kind == SOCK_STREAM => {
                        let window = own.peer.as_ref().map_or(0, |peer| peer.borrow().rcvbuf);
                        own.written.saturating_sub(window)
                    }
                    Some(_) => 0,
                    None => own.written,
                };
                drop(own);
                return put_int(kernel, cx, amount as i32);
            }
            // `unix_ioctl`: at the urgent byte, or at an emptied buffer with none past it or the
            // urgent byte next. `tcp_ioctl`: the urgent byte, read or not, is next.
            SIOCATMARK => {
                let own = socket.borrow();
                let mut queue = own.queue.iter();
                let mark = if family == AF_UNIX {
                    queue.next().is_some_and(|first| {
                        first.urgent
                            || (first.consumed == first.data.len()
                                && (!own.has_urgent()
                                    || queue.next().is_some_and(|next| next.urgent)))
                    })
                } else {
                    queue
                        .find(|packet| packet.urgent || packet.consumed < packet.data.len())
                        .is_some_and(|packet| packet.urgent)
                };
                drop(own);
                return put_int(kernel, cx, i32::from(mark));
            }
            _ => {}
        }
    }
    if family == AF_INET || family == AF_INET6 {
        match request {
            SIOCGSTAMP
            | crate::linux::net::SIOCGSTAMPNS_OLD
            | SIOCGSTAMP_NEW
            | SIOCGSTAMPNS_NEW => {
                return Err(kernel.counters.refuse(
                    cx.host,
                    crate::reason::IOCTL_NOT_BUILT,
                    request.into(),
                    Errno::ENOENT,
                ));
            }
            SIOCGIFADDR | SIOCGIFBRDADDR | SIOCGIFNETMASK | SIOCGIFDSTADDR if family == AF_INET => {
                return interface_address(kernel, cx, tid, request, argument);
            }
            SIOCGIFPFLAGS if family == AF_INET => return Err(Errno::EINVAL),
            _ => {}
        }
    }
    match request {
        SIOCGIFNAME | SIOCGIFFLAGS | SIOCGIFMETRIC | SIOCGIFMTU | SIOCGIFHWADDR | SIOCGIFSLAVE
        | SIOCGIFMAP | SIOCGIFINDEX | SIOCGIFTXQLEN => {
            interface_question(kernel, cx, tid, request, argument)
        }
        // Requests that change the network need CAP_NET_ADMIN, which Docker does not give.
        SIOCSIFFLAGS | SIOCSIFMETRIC | SIOCSIFMTU | SIOCSIFNAME | SIOCSIFHWADDR | SIOCSIFSLAVE
        | SIOCADDMULTI | SIOCDELMULTI | SIOCSIFHWBROADCAST | SIOCSIFMAP | SIOCSIFTXQLEN
        | SIOCSIFADDR | SIOCSIFBRDADDR | SIOCSIFNETMASK | SIOCSIFDSTADDR | SIOCSIFPFLAGS
        | SIOCADDRT | SIOCDELRT | SIOCDARP | SIOCSARP | SIOCSIFLINK | SIOCDIFADDR
            if !kernel.credentials(tid).has(CAP_NET_ADMIN) =>
        {
            Err(Errno::EPERM)
        }
        SIOCSIFFLAGS | SIOCSIFMETRIC | SIOCSIFMTU | SIOCSIFNAME | SIOCSIFHWADDR | SIOCSIFSLAVE
        | SIOCADDMULTI | SIOCDELMULTI | SIOCSIFHWBROADCAST | SIOCSIFMAP | SIOCSIFTXQLEN
        | SIOCSIFADDR | SIOCSIFBRDADDR | SIOCSIFNETMASK | SIOCSIFDSTADDR | SIOCSIFPFLAGS
        | SIOCADDRT | SIOCDELRT | SIOCDARP | SIOCSARP | SIOCSIFLINK | SIOCDIFADDR => {
            Err(kernel.counters.refuse(
                cx.host,
                crate::reason::IOCTL_NOT_BUILT,
                request.into(),
                Errno::EOPNOTSUPP,
            ))
        }
        _ => Err(Errno::ENOTTY),
    }
}

/// The interface an `ifreq` names, by name with any `:label` cut off.
fn named_interface(request: &[u8]) -> Option<&'static netlink::Interface> {
    let mut name = &request[..15];
    if let Some(end) = name.iter().position(|&byte| byte == 0 || byte == b':') {
        name = &name[..end];
    }
    netlink::INTERFACES
        .iter()
        .find(|interface| interface.name.as_bytes() == name)
}

/// `dev_ioctl`'s questions about one interface, answered in the caller's `ifreq`.
fn interface_question(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    use crate::linux::ioctl::*;
    use crate::linux::net::net_device_flags::*;
    let mut ifreq = guest(kernel, cx, tid).read(argument, 40)?;
    let int = |ifreq: &mut Vec<u8>, value: i32| ifreq[16..20].copy_from_slice(&value.to_le_bytes());
    if request == SIOCGIFNAME {
        let index = i32::from_le_bytes(ifreq[16..20].try_into().expect("4 bytes"));
        let interface = netlink::INTERFACES
            .iter()
            .find(|interface| interface.index as i32 == index)
            .ok_or(Errno::ENODEV)?;
        ifreq[..16].fill(0);
        ifreq[..interface.name.len()].copy_from_slice(interface.name.as_bytes());
        guest(kernel, cx, tid).write(argument, &ifreq)?;
        return Ok(0);
    }
    let interface = named_interface(&ifreq).ok_or(Errno::ENODEV)?;
    match request {
        SIOCGIFFLAGS => {
            let hidden = IFF_PROMISC as u32 | IFF_ALLMULTI as u32 | IFF_LOWER_UP as u32;
            let flags = (interface.flags & !hidden) as u16;
            ifreq[16..18].copy_from_slice(&flags.to_le_bytes());
        }
        SIOCGIFMETRIC => int(&mut ifreq, 0),
        SIOCGIFMTU => int(&mut ifreq, interface.mtu as i32),
        SIOCGIFINDEX => int(&mut ifreq, interface.index as i32),
        SIOCGIFTXQLEN => int(&mut ifreq, interface.queue_length as i32),
        SIOCGIFHWADDR => {
            ifreq[16..32].fill(0);
            ifreq[16..18].copy_from_slice(&(interface.kind as u16).to_le_bytes());
            ifreq[18..24].copy_from_slice(&interface.hardware);
        }
        SIOCGIFMAP => ifreq[16..40].fill(0),
        _ => return Err(Errno::EINVAL),
    }
    guest(kernel, cx, tid).write(argument, &ifreq)?;
    Ok(0)
}

/// A `sockaddr_in` of an IPv4 address, in an `ifreq` from offset 16.
fn put_v4(ifreq: &mut [u8], address: [u8; 4]) {
    ifreq[16..32].fill(0);
    ifreq[16..18].copy_from_slice(&(AF_INET as u16).to_le_bytes());
    ifreq[20..24].copy_from_slice(&address);
}

/// `devinet_ioctl`'s questions about an interface's IPv4 address.
fn interface_address(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    use crate::linux::ioctl::*;
    let mut ifreq = guest(kernel, cx, tid).read(argument, 40)?;
    let interface = named_interface(&ifreq).ok_or(Errno::ENODEV)?;
    // An alias label names an address the interface does not have.
    if ifreq[..16].contains(&b':') {
        return Err(Errno::EADDRNOTAVAIL);
    }
    let (address, prefix, broadcast) = interface.v4;
    let mask = (u32::MAX << (32 - u32::from(prefix))).to_be_bytes();
    let value = match request {
        SIOCGIFADDR | SIOCGIFDSTADDR => address,
        SIOCGIFBRDADDR => broadcast,
        _ => mask,
    };
    put_v4(&mut ifreq, value);
    guest(kernel, cx, tid).write(argument, &ifreq)?;
    Ok(0)
}

/// `dev_ifconf`: an `ifreq` for each interface's IPv4 address, as many whole ones as fit, or with
/// no buffer the room they need.
fn interface_list(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    argument: u64,
) -> Result<u64, Errno> {
    let header = guest(kernel, cx, tid).read(argument, 16)?;
    let room = i32::from_le_bytes(header[..4].try_into().expect("4 bytes"));
    let buffer = u64::from_le_bytes(header[8..16].try_into().expect("8 bytes"));
    let mut list = Vec::new();
    for interface in &netlink::INTERFACES {
        let mut ifreq = alloc::vec![0u8; 40];
        ifreq[..interface.name.len()].copy_from_slice(interface.name.as_bytes());
        put_v4(&mut ifreq, interface.v4.0);
        list.push(ifreq);
    }
    let used = if buffer == 0 {
        list.len() * 40
    } else {
        let fits = (room.max(0) as usize / 40).min(list.len());
        let bytes: Vec<u8> = list[..fits].concat();
        guest(kernel, cx, tid).write(buffer, &bytes)?;
        bytes.len()
    };
    guest(kernel, cx, tid).write_value(argument, &(used as i32))?;
    Ok(0)
}

/// The size of a `struct mmsghdr`, and where its `msg_len` is.
const MMSGHDR: u64 = 64;
const MSG_LEN: u64 = 56;

/// `__sys_sendmmsg`: each message sent as sendmsg sends it (waiting as it waits), its length
/// stored after it. An error after the first ends the call with those sent.
pub(super) fn sendmmsg(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, flags, ..]: Args,
) -> Result<u64, Errno> {
    let count = (count as u32).min(UIO_MAXIOV) as u64;
    socket_of(kernel, tid, fd)?;
    let mut sent = kernel.threads[&tid].call.messages;
    while sent < count {
        let header = vector + sent * MMSGHDR;
        match sendmsg(kernel, cx, tid, [fd, header, flags, 0, 0, 0]) {
            Ok(length) => {
                guest(kernel, cx, tid).write_value(header + MSG_LEN, &(length as u32))?;
                sent += 1;
                next_message(kernel, tid, sent);
            }
            Err(error) if sent == 0 || error == Errno::PARKED => return Err(error),
            Err(_) => break,
        }
    }
    Ok(sent)
}

/// One message of sendmmsg or recvmmsg is done: the next starts afresh, with its own wait.
fn next_message(kernel: &mut Kernel, tid: Tid, done: u64) {
    let call = &mut kernel.thread_mut(tid).call;
    call.messages = done;
    call.progress = 0;
    call.deadline = None;
}

/// `do_recvmmsg`: messages received as recvmsg receives them (each waiting as it waits), their
/// lengths stored after them. With MSG_WAITFORONE only the first waits. A timeout is checked
/// only after each message, as Linux checks it, and what is left of it is stored.
pub(super) fn recvmmsg(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, flags, timeout, _]: Args,
) -> Result<u64, Errno> {
    let count = (count as u32).min(UIO_MAXIOV) as u64;
    let (_, socket) = socket_of(kernel, tid, fd)?;
    let flags = flags as u32;
    let end = if timeout != 0 {
        let value: __kernel_timespec = guest(kernel, cx, tid).read_value(timeout)?;
        if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
            return Err(Errno::EINVAL);
        }
        let length = (value.tv_sec as u64)
            .saturating_mul(1_000_000_000)
            .saturating_add(value.tv_nsec as u64);
        Some(kernel.settle_once(tid, kernel.entry_now.saturating_add(length)))
    } else {
        None
    };
    let mut received = kernel.threads[&tid].call.messages;
    while received < count {
        let header = vector + received * MMSGHDR;
        let wait_for_one = flags & crate::linux_extra::MSG_WAITFORONE != 0;
        let call_flags = if received > 0 && wait_for_one {
            (flags & !crate::linux_extra::MSG_WAITFORONE) | MSG_DONTWAIT
        } else {
            flags & !crate::linux_extra::MSG_WAITFORONE
        };
        match recvmsg(
            kernel,
            cx,
            tid,
            [fd, header, u64::from(call_flags), 0, 0, 0],
        ) {
            Ok(length) => {
                guest(kernel, cx, tid).write_value(header + MSG_LEN, &(length as u32))?;
                received += 1;
                next_message(kernel, tid, received);
            }
            Err(error) if received == 0 || error == Errno::PARKED => return Err(error),
            Err(Errno::EAGAIN) => break,
            Err(error) => {
                // The error is kept for the next call (`do_recvmmsg`).
                socket.borrow_mut().error = error.0 as i32;
                break;
            }
        }
        if let Some(end) = end
            && kernel.entry_now >= end
        {
            break;
        }
    }
    if let Some(end) = end {
        let left = end.saturating_sub(kernel.entry_now);
        let value = __kernel_timespec {
            tv_sec: (left / 1_000_000_000) as i64,
            tv_nsec: (left % 1_000_000_000) as i64,
        };
        guest(kernel, cx, tid).write_value(timeout, &value)?;
    }
    Ok(received)
}
