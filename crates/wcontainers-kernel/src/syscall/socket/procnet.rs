//! The files of `/proc/net` that describe the network and its sockets, in Linux's formats:
//! `dev_seq_show`, `if6_seq_show`, `rt_cache_seq_show`'s `fib_route_seq_show`, `tcp4_seq_show`,
//! `tcp6_seq_show`, `udp4_seq_show`, `udp6_seq_show` and `unix_seq_show`. Kernel pointers show
//! as a number unique to each socket, as Linux's hashed `%pK` does.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use super::inet::{Endpoint, Protocol, v4_of};
use super::{SocketRef, State};
use crate::fs::proc::NetFile;
use crate::linux::net::{AF_INET, AF_INET6, AF_UNIX};
use crate::linux_extra::{
    TCP_CLOSE, TCP_ESTABLISHED, TCP_LISTEN, TCP_SYN_RECV, TCP_SYN_SENT, TCP_TIME_WAIT,
};
use crate::{Context, Kernel, network};

/// `TMPSZ`: the width IPv4 TCP lines are padded to, less one.
const TCP_WIDTH: usize = 149;
/// The width IPv4 UDP lines and the route table's are padded to.
const UDP_WIDTH: usize = 127;
/// The UDP hash table's buckets, which number the lines of `/proc/net/udp` (a made-up size; Linux's
/// scales with memory and mixes in a random value per network).
const UDP_BUCKETS: u16 = 1 << 14;
/// `__SO_ACCEPTCON`, the flag a listening local socket shows.
const SO_ACCEPTCON: u32 = 1 << 16;
/// Linux's `socket_state`.
const SS_UNCONNECTED: u32 = 1;
const SS_CONNECTING: u32 = 2;
const SS_CONNECTED: u32 = 3;
const SS_DISCONNECTING: u32 = 4;

/// The bytes of one file of `/proc/net`.
pub(crate) fn contents(kernel: &mut Kernel, cx: &mut Context<'_>, file: NetFile) -> Vec<u8> {
    let text = match file {
        NetFile::Dev => dev(kernel, cx),
        NetFile::IfInet6 => String::from("00000000000000000000000000000001 01 80 10 80       lo\n"),
        NetFile::Route => route(),
        NetFile::Tcp => tcp(kernel, AF_INET),
        NetFile::Tcp6 => tcp(kernel, AF_INET6),
        NetFile::Udp => udp(kernel, AF_INET),
        NetFile::Udp6 => udp(kernel, AF_INET6),
        NetFile::Unix => return unix(kernel),
    };
    text.into_bytes()
}

fn pad(out: &mut String, start: usize, width: usize) {
    while out.len() - start < width {
        out.push(' ');
    }
    out.push('\n');
}

fn dev(kernel: &mut Kernel, cx: &mut Context<'_>) -> String {
    kernel
        .counters
        .count(cx.host, crate::reason::INTERFACE_COUNTERS_NOT_KEPT, 0);
    let mut out = String::from(
        "Inter-|   Receive                                                |  Transmit\n \
         face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets \
         errs drop fifo colls carrier compressed\n",
    );
    for name in [network::LOOPBACK_NAME, network::ETHERNET_NAME] {
        let _ = write!(out, "{name:>6}:");
        let _ = writeln!(
            out,
            "{:8} {:7} {:4} {:4} {:4} {:5} {:10} {:9} {:8} {:7} {:4} {:4} {:4} {:5} {:7} {:10}",
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        );
    }
    out
}

/// An IPv4 address as Linux prints a `__be32` with `%08X`.
fn hex4(v4: [u8; 4]) -> u32 {
    u32::from_le_bytes(v4)
}

fn route() -> String {
    let mut out = String::new();
    out.push_str(
        "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT",
    );
    pad(&mut out, 0, UDP_WIDTH);
    let mask = u32::MAX >> (32 - network::PREFIX);
    let network = u32::from_be_bytes(network::ADDRESS) & !(u32::MAX >> network::PREFIX);
    let routes = [
        // The default route, through the gateway (RTF_UP | RTF_GATEWAY).
        (0, hex4(network::GATEWAY), 0x0003, 0),
        // eth0's own network (RTF_UP).
        (
            hex4(network.to_be_bytes()),
            0,
            0x0001,
            hex4(mask.to_le_bytes()),
        ),
    ];
    for (destination, gateway, flags, mask) in routes {
        let start = out.len();
        let _ = write!(
            out,
            "{}\t{destination:08X}\t{gateway:08X}\t{flags:04X}\t0\t0\t0\t{mask:08X}\t0\t0\t0",
            network::ETHERNET_NAME
        );
        pad(&mut out, start, UDP_WIDTH);
    }
    out
}

/// An endpoint as the file of `family` shows it: IPv4 as one word, IPv6 as four, each a host
/// order word of the address's bytes.
fn endpoint(out: &mut String, family: u32, end: &Endpoint) {
    if family == AF_INET {
        let _ = write!(out, "{:08X}", hex4(v4_of(&end.address).unwrap_or([0; 4])));
    } else {
        for word in end.address.chunks(4) {
            let _ = write!(out, "{:08X}", u32::from_le_bytes(word.try_into().unwrap()));
        }
    }
    let _ = write!(out, ":{:04X}", end.port);
}

/// The live sockets holding ports of a protocol, each once, in port order.
fn holders(kernel: &Kernel, protocol: Protocol) -> Vec<SocketRef> {
    let mut seen = BTreeSet::new();
    kernel
        .sockets
        .inet
        .bound
        .iter()
        .filter(|((held, _), _)| *held == protocol)
        .flat_map(|(_, sockets)| sockets.iter().filter_map(alloc::rc::Weak::upgrade))
        .filter(|socket| !socket.borrow().dead && seen.insert(socket.as_ptr() as usize))
        .collect()
}

/// A number unique to a socket while it lives, where Linux prints its hashed address.
fn pointer(socket: &SocketRef) -> u64 {
    socket.as_ptr() as usize as u64
}

/// The unread bytes of a socket's queue.
fn unread(socket: &SocketRef) -> u64 {
    socket
        .borrow()
        .queue
        .iter()
        .map(|packet| (packet.data.len() - packet.consumed) as u64)
        .sum()
}

fn tcp(kernel: &Kernel, family: u32) -> String {
    let mut out = String::new();
    if family == AF_INET {
        out.push_str(
            "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  \
             timeout inode",
        );
        pad(&mut out, 0, TCP_WIDTH);
    } else {
        out.push_str(
            "  sl  local_address                         remote_address                        \
             st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n",
        );
    }
    let sockets: Vec<SocketRef> = holders(kernel, Protocol::Tcp)
        .into_iter()
        .filter(|socket| socket.borrow().family == family)
        .collect();
    // Listeners first, as Linux walks its listening hash before its established one.
    let (listening, others): (Vec<_>, Vec<_>) = sockets
        .into_iter()
        .partition(|socket| socket.borrow().state == State::Listening);
    let mut number = 0;
    let mut line = |out: &mut String, text: &dyn Fn(&mut String)| {
        let start = out.len();
        let _ = write!(out, "{number:4}: ");
        text(out);
        if family == AF_INET {
            pad(out, start, TCP_WIDTH);
        } else {
            out.push('\n');
        }
        number += 1;
    };
    for socket in listening.iter().chain(&others) {
        let own = socket.borrow();
        let inet = own.inet.as_deref().unwrap();
        // A connection held back by TCP_DEFER_ACCEPT is still a request (`get_openreq4`), owned
        // by its listener's user.
        if let Some((left, fires)) = super::inet::deferred_request(&own, kernel.entry_now) {
            let (local, remote) = (inet.local, inet.remote.unwrap_or_default());
            let uid = inet
                .deferral
                .as_ref()
                .and_then(|(listener, ..)| listener.upgrade())
                .map_or(0, |listener| listener.borrow().uid);
            drop(own);
            line(&mut out, &|out: &mut String| {
                endpoint(out, family, &local);
                out.push(' ');
                endpoint(out, family, &remote);
                let _ = write!(
                    out,
                    " {TCP_SYN_RECV:02X} {:08X}:{:08X} 01:{:08X} {fires:08X} {uid:5} {:8} 0 0 {:016x}",
                    0,
                    0,
                    left.div_ceil(10_000_000),
                    0,
                    pointer(socket)
                );
            });
            continue;
        }
        let (state, rx, tail) = if own.state == State::Listening {
            (
                TCP_LISTEN,
                own.backlog.len() as u64,
                String::from("100 0 0 10 0"),
            )
        } else if inet.connecting.is_some() {
            (TCP_SYN_SENT, 0, String::from("100 0 0 10 -1"))
        } else if own.state == State::Connected {
            let state = super::inet::tcp_state(&own);
            // A FIN that came counts as one byte until the program reads the end.
            let fin = u64::from(inet.done && !inet.fin_read);
            let ato = if inet.received || inet.done { 4 } else { 0 };
            let quick = if inet.received { 30 } else { 0 } | u32::from(inet.pingpong);
            (
                state,
                unread(socket) + fin,
                alloc::format!("20 {ato} {quick} 10 -1"),
            )
        } else {
            continue;
        };
        let local = inet.local;
        let remote = inet.remote.unwrap_or_default();
        let refs = if own.ino != 0 { 2 } else { 1 };
        let (uid, ino) = (own.uid, own.ino);
        drop(own);
        line(&mut out, &|out: &mut String| {
            endpoint(out, family, &local);
            out.push(' ');
            endpoint(out, family, &remote);
            let _ = write!(
                out,
                " {state:02X} {:08X}:{:08X} 00:00000000 00000000 {uid:5} {:8} {ino} {refs} {:016x} {tail}",
                0,
                rx as u32,
                0,
                pointer(socket)
            );
        });
    }
    let now = kernel.entry_now;
    for held in &kernel.sockets.inet.time_wait {
        if held.family != family || held.until <= now {
            continue;
        }
        let left = (held.until - now) / 10_000_000;
        line(&mut out, &|out: &mut String| {
            endpoint(out, family, &held.local);
            out.push(' ');
            endpoint(out, family, &held.remote);
            let _ = write!(
                out,
                " {TCP_TIME_WAIT:02X} {:08X}:{:08X} 03:{left:08X} 00000000 {:5} {:8} 0 3 {:016x}",
                0, 0, 0, 0, 0
            );
        });
    }
    out
}

fn udp(kernel: &Kernel, family: u32) -> String {
    let mut out = String::new();
    if family == AF_INET {
        out.push_str(
            "   sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  \
             timeout inode ref pointer drops",
        );
        pad(&mut out, 0, UDP_WIDTH);
    } else {
        out.push_str(
            "  sl  local_address                         remote_address                        \
             st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode ref pointer drops\n",
        );
    }
    let mut sockets: Vec<SocketRef> = holders(kernel, Protocol::Udp)
        .into_iter()
        .filter(|socket| socket.borrow().family == family)
        .collect();
    sockets.sort_by_key(|socket| {
        let own = socket.borrow();
        own.inet.as_deref().unwrap().local.port % UDP_BUCKETS
    });
    for socket in &sockets {
        let own = socket.borrow();
        let inet = own.inet.as_deref().unwrap();
        let state = if own.state == State::Connected {
            TCP_ESTABLISHED
        } else {
            TCP_CLOSE
        };
        let held: u64 = own.queue.iter().map(|packet| packet.truesize).sum();
        let start = out.len();
        let _ = write!(out, "{:5}: ", inet.local.port % UDP_BUCKETS);
        endpoint(&mut out, family, &inet.local);
        out.push(' ');
        endpoint(&mut out, family, &inet.remote.unwrap_or_default());
        let _ = write!(
            out,
            " {state:02X} {:08X}:{:08X} 00:00000000 00000000 {:5} {:8} {} 2 {:016x} {}",
            own.written as u32,
            held as u32,
            own.uid,
            0,
            own.ino,
            pointer(socket),
            own.drops
        );
        if family == AF_INET {
            pad(&mut out, start, UDP_WIDTH);
        } else {
            out.push('\n');
        }
    }
    out
}

/// Every local socket: those open in some process, each followed by the connections waiting in
/// its backlog.
fn local_sockets(kernel: &Kernel) -> Vec<SocketRef> {
    let mut seen = BTreeSet::new();
    let mut open = Vec::new();
    for thread in kernel.threads.values() {
        for (_, descriptor) in thread.files.borrow().iter() {
            let file = descriptor.file.borrow();
            let node = file.node.borrow();
            if let crate::fs::Content::Socket(socket) = &node.content
                && socket.borrow().family == AF_UNIX
                && seen.insert(socket.as_ptr() as usize)
            {
                open.push(socket.clone());
            }
        }
    }
    open.sort_by_key(|socket| socket.borrow().ino);
    let mut all = Vec::new();
    for socket in open {
        let waiting: Vec<SocketRef> = socket.borrow().backlog.iter().cloned().collect();
        all.push(socket);
        all.extend(waiting);
    }
    all
}

fn unix(kernel: &Kernel) -> Vec<u8> {
    let mut out = b"Num       RefCount Protocol Flags    Type St Inode Path\n".to_vec();
    let sockets = local_sockets(kernel);
    for socket in &sockets {
        // One reference of its own and one of the table, and one for each socket whose other
        // end it is.
        let peers = sockets
            .iter()
            .filter(|other| {
                other
                    .borrow()
                    .peer
                    .as_ref()
                    .is_some_and(|peer| alloc::rc::Rc::ptr_eq(peer, socket))
            })
            .count();
        let own = socket.borrow();
        let flags = if own.state == State::Listening {
            SO_ACCEPTCON
        } else {
            0
        };
        let connected = own.state == State::Connected;
        let state = match (own.ino != 0, connected) {
            (true, true) => SS_CONNECTED,
            (true, false) => SS_UNCONNECTED,
            (false, true) => SS_CONNECTING,
            (false, false) => SS_DISCONNECTING,
        };
        let line = alloc::format!(
            "{:016x}: {:08X} {:08X} {flags:08X} {:04X} {state:02X} {:5}",
            pointer(socket),
            2 + peers,
            0,
            own.kind,
            own.ino
        );
        out.extend_from_slice(line.as_bytes());
        if let Some(name) = &own.name {
            out.push(b' ');
            // An abstract name's leading zero and the zeros in it show as '@'.
            out.extend(name.iter().map(|&byte| if byte == 0 { b'@' } else { byte }));
        }
        out.push(b'\n');
    }
    out
}
