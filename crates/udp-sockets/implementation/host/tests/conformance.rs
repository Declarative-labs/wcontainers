//! The conformance suite on the host implementation, each test in a network namespace of its own thread.
#![cfg(target_os = "linux")]

use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{ByteCount, IpSocketSetting, IpVersion};
use wcontainers_kernel::UserId;
use wcontainers_udp_sockets::{UdpProtocol, UdpSocketSetting, UdpSockets};
use wcontainers_udp_sockets_linux_host::{
    LinuxHostUdpSockets, RECEIVE_BUFFER_CUT_BY_THE_HOST, SEND_BUFFER_CUT_BY_THE_HOST,
};

/// How many networks the tests of one run can take, all made before any test opens a socket.
const NETWORKS: usize = 64;

/// The network the suite asks for, as iproute2's `ip -batch` commands: loopback with multicast and a route with
/// an MTU of 1500 bytes to 127.0.0.2 and 2001:db8::2; a dummy interface that sends nothing anywhere, with
/// 192.0.2.3/24, the IPv4 multicast route and 198.51.100.0/24 through a gateway on it; user 1000's datagrams
/// to anything not local prohibited; a dummy interface with 203.0.113.1/24 and 2001:db8:1::1/64, from which
/// Linux makes a temporary address.
const NETWORK: &str = "\
link set lo up multicast on
route add local 127.0.0.2 dev lo table local mtu 1500
address add 2001:db8::2/128 dev lo
link add multicast0 up type dummy
address add 192.0.2.3/24 dev multicast0
route add 224.0.0.0/4 dev multicast0
route add 198.51.100.0/24 via 192.0.2.2 dev multicast0
rule add uidrange 1000-1000 prohibit
link add scheduled0 up type dummy
address add 203.0.113.1/24 dev scheduled0
address add 2001:db8:1::1/64 dev scheduled0 mngtmpaddr
";

/// A qdisc that paces each socket's datagrams to its rate and holds each until its transmit time, and one that
/// keeps a strict schedule on the international atomic clock and so misses every time it holds.
const QUEUES: &str = "\
qdisc add dev lo root fq
qdisc add dev scheduled0 root etf clockid CLOCK_TAI delta 0
";

const NARROW_V6: &str = "2001:db8::2";

/// How often and how long apart the network's maker looks for what Linux adds to an IPv6 address after it.
const LOCAL_ROUTE_TRIES: u32 = 100;
const LOCAL_ROUTE_WAIT: Duration = Duration::from_millis(10);

/// Runs one iproute2 program with commands on its standard input, in this thread's network namespace.
fn iproute2(program: &str, commands: &str) {
    let mut child = Command::new(program)
        .args(["-batch", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("run iproute2");
    child
        .stdin
        .take()
        .expect("its input")
        .write_all(commands.as_bytes())
        .expect("give iproute2 its commands");
    let status = child.wait().expect("wait for iproute2");
    assert!(status.success(), "{program} {commands} answered {status}");
}

/// A new network namespace made as the suite asks, held open by the descriptor returned.
fn network() -> OwnedFd {
    // SAFETY: unshare changes only this thread's network namespace.
    assert_eq!(
        unsafe { libc::unshare(libc::CLONE_NEWNET) },
        0,
        "unshare the network namespace (run as root)"
    );
    // Temporary addresses made and preferred on every interface made from here on.
    std::fs::write("/proc/sys/net/ipv6/conf/default/use_tempaddr", "2")
        .expect("prefer temporary addresses");
    iproute2("ip", NETWORK);
    // Linux adds an IPv6 address's local route a moment after the address, from a work queue.
    let mut tries = 0;
    while !Command::new("ip")
        .args(["-6", "route", "del", "local", NARROW_V6, "table", "local"])
        .stderr(Stdio::null())
        .status()
        .expect("run iproute2")
        .success()
    {
        tries += 1;
        assert!(tries < LOCAL_ROUTE_TRIES, "the local route of {NARROW_V6}");
        std::thread::sleep(LOCAL_ROUTE_WAIT);
    }
    iproute2(
        "ip",
        &format!("route add local {NARROW_V6} dev lo table local mtu 1500\n"),
    );
    iproute2("tc", QUEUES);
    // Linux makes the temporary address a moment after the public one, and it is tentative at first.
    let mut tries = 0;
    while Command::new("ip")
        .args([
            "-6",
            "-o",
            "address",
            "show",
            "dev",
            "scheduled0",
            "temporary",
            "-tentative",
        ])
        .output()
        .expect("run iproute2")
        .stdout
        .is_empty()
    {
        tries += 1;
        assert!(
            tries < LOCAL_ROUTE_TRIES,
            "the temporary address of scheduled0"
        );
        std::thread::sleep(LOCAL_ROUTE_WAIT);
    }
    // This thread's namespace is the one /proc/sys/net shows it.
    for interface in ["all", "lo"] {
        std::fs::write(
            format!("/proc/sys/net/ipv6/conf/{interface}/seg6_enabled"),
            "1",
        )
        .expect("take segment routing headers");
    }
    File::open("/proc/thread-self/ns/net")
        .expect("this thread's network namespace")
        .into()
}

/// The networks not yet taken. They are all made at the first test's start, while every other test waits here
/// before opening a socket: a child process holds every descriptor of its parent until it starts its program,
/// which would keep a socket another test closed alive.
static NETWORKS_LEFT: LazyLock<Mutex<Vec<OwnedFd>>> = LazyLock::new(|| {
    let makers: Vec<_> = (0..NETWORKS).map(|_| std::thread::spawn(network)).collect();
    Mutex::new(
        makers
            .into_iter()
            .map(|maker| maker.join().expect("make a network"))
            .collect(),
    )
});

/// Moves this thread into a network of its own, made as the suite asks.
fn isolated() -> LinuxHostUdpSockets {
    let network = NETWORKS_LEFT
        .lock()
        .expect("the networks left")
        .pop()
        .expect("a network left for this test (raise NETWORKS)");
    // SAFETY: setns changes only this thread's network namespace.
    assert_eq!(
        unsafe { libc::setns(network.as_raw_fd(), libc::CLONE_NEWNET) },
        0,
        "enter the network"
    );
    LinuxHostUdpSockets::new()
}

wcontainers_udp_sockets_conformance::tests!(isolated());

/// A buffer size the host cuts to its own limit is taken, and counted under its reason.
#[test]
fn a_buffer_the_host_cuts_is_counted() {
    let sockets = isolated();
    let socket = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, UserId(u32::MIN))
        .unwrap();
    // The largest size a host's socket option takes.
    let beyond_any_limit = ByteCount(usize::try_from(libc::c_int::MAX).unwrap());
    for setting in [
        IpSocketSetting::SendBuffer(beyond_any_limit),
        IpSocketSetting::ReceiveBuffer(beyond_any_limit),
    ] {
        sockets
            .configure(&socket, UdpSocketSetting::Ip(setting))
            .unwrap();
    }
    let counted: Vec<_> = sockets
        .counters()
        .into_iter()
        .filter(|reading| reading.count == 1)
        .map(|reading| reading.reason)
        .collect();
    assert_eq!(
        counted,
        [RECEIVE_BUFFER_CUT_BY_THE_HOST, SEND_BUFFER_CUT_BY_THE_HOST]
    );
}
