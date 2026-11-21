//! The conformance suite on the host implementation, each test in a network namespace of its own thread.
#![cfg(target_os = "linux")]

use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use std::net::{Ipv4Addr, SocketAddr};

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options};
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BindAddress, BroadcastPermission, ByteCount, BytesPerSecond, IpHeaderFields, IpSendFields,
    IpSocketSetting, IpVersion, Ipv4HeaderFields, Ipv4RouteSelectors, LocalBinding, LocalPort,
    NeighbourConfirmation, PacketMark, PacketPriority, PathMtuDiscovery, Port, Report, RouteScope,
    SendError, SourceAddressCheck, TransmitReportKey, TransmitReportRequest, TransportPorts,
    UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_udp_sockets::{
    UdpChecksum, UdpDatagram, UdpProtocol, UdpReceived, UdpSocketSetting, UdpSockets,
};
use wcontainers_udp_sockets_host::{
    HostUdpSocket, HostUdpSockets, RECEIVE_BUFFER_CUT_BY_THE_HOST, SEND_BUFFER_CUT_BY_THE_HOST,
};

/// How many networks the tests of one run can take, all made before any test opens a socket.
const NETWORKS: usize = 80;

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
fn isolated() -> HostUdpSockets {
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
    HostUdpSockets::new().expect("make the host's UDP sockets")
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

/// A socket bound to IPv4 loopback on a port the kernel chooses, and its address.
fn on_loopback(sockets: &HostUdpSockets) -> (HostUdpSocket, SocketAddr) {
    let socket = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, UserId(u32::MIN))
        .unwrap();
    let address = sockets
        .bind(
            &socket,
            LocalBinding {
                address: BindAddress::V4(Ipv4Addr::LOCALHOST),
                port: LocalPort::ChosenByKernel(Port(u16::MIN)),
            },
        )
        .unwrap();
    (socket, address)
}

/// The header of a datagram a socket on IPv4 loopback sends to its own port.
fn to_itself(options: &Ipv4Options, address: SocketAddr) -> IpHeaderFields<'_> {
    IpHeaderFields::V4(Ipv4HeaderFields {
        route: Ipv4RouteSelectors {
            destination: Ipv4Addr::LOCALHOST,
            source: None,
            source_address_check: SourceAddressCheck::Required,
            interface: None,
            dscp: IpDscp::ZERO,
            ecn: IpEcn::NotEct,
            mark: PacketMark(u32::MIN),
            scope: RouteScope::Universe,
            broadcast: BroadcastPermission::Refused,
            options,
            upper_layer: UpperLayer {
                protocol: IpNumber::UDP,
                selector: Some(UpperLayerSelector::Ports(TransportPorts {
                    source: Port(address.port()),
                    destination: Port(address.port()),
                })),
            },
        },
        time_to_live: None,
        path_mtu_discovery: PathMtuDiscovery::Want,
    })
}

fn reports(key: TransmitReportKey, software: Report) -> IpSendFields {
    IpSendFields {
        neighbour: NeighbourConfirmation::Unconfirmed,
        priority: PacketPriority(u32::MIN),
        transmit_time: None,
        transmit_reports: TransmitReportRequest {
            key,
            scheduled: Report::NotRequested,
            software,
            hardware: Report::NotRequested,
            acknowledgement: Report::NotRequested,
            completion: Report::NotRequested,
            wireless_status: Report::NotRequested,
            packet: Report::NotRequested,
        },
    }
}

fn computed(payload: &[u8]) -> UdpDatagram<'_> {
    UdpDatagram {
        checksum: UdpChecksum::Computed,
        segment_size: None,
        payload,
    }
}

/// A send's report comes before a datagram that arrived after it: this implementation reads the host's error
/// queue first while a send's reports may wait there.
#[test]
fn a_report_comes_before_a_datagram_that_arrived_after_it() {
    let sockets = isolated();
    let (socket, address) = on_loopback(&sockets);
    let options = Ipv4Options::new();
    let key = TransmitReportKey(u32::from(u16::MAX));
    // Over loopback the report is queued as the datagram leaves, before it arrives back on the same socket.
    sockets
        .send_to(
            &socket,
            to_itself(&options, address),
            reports(key, Report::Requested),
            computed(b"to itself"),
        )
        .unwrap();
    let mut into = vec![u8::MIN; usize::from(u16::MAX)];
    match sockets.receive(&socket, &mut into) {
        Ok(UdpReceived::TransmitReport(report)) => assert_eq!(report.key, key),
        other => panic!("the report first, not {other:?}"),
    }
    assert!(matches!(
        sockets.receive(&socket, &mut into),
        Ok(UdpReceived::Packet(_))
    ));
}

/// The send buffer size the full buffer check gives, of which Linux keeps twice.
const SEND_BUFFER: usize = 16384;

/// The payload of each paced datagram, and the most memory Linux keeps one in: its payload, headers and buffer.
const PACED_LENGTH: usize = 1000;
const PACED_DATAGRAM_ROOM: usize = 4 * PACED_LENGTH;

/// A full send buffer counts unsent the memory Linux's `SIOCOUTQ` gives a UDP socket: at least the twice the size
/// given that Linux keeps, and less than one datagram more.
#[test]
fn a_full_send_buffer_counts_the_memory_linux_counts() {
    let sockets = isolated();
    let (socket, address) = on_loopback(&sockets);
    for setting in [
        IpSocketSetting::SendBuffer(ByteCount(SEND_BUFFER)),
        // A quarter of a second for each datagram after the first, so that the buffer fills.
        IpSocketSetting::MaximumPacingRate(Some(BytesPerSecond(4 * PACED_LENGTH as u64))),
    ] {
        sockets
            .configure(&socket, UdpSocketSetting::Ip(setting))
            .unwrap();
    }
    let options = Ipv4Options::new();
    let payload = [u8::MAX; PACED_LENGTH];
    let sent = (0..=SEND_BUFFER)
        .take_while(|_| {
            match sockets.send_to(
                &socket,
                to_itself(&options, address),
                reports(TransmitReportKey(u32::MIN), Report::NotRequested),
                computed(&payload),
            ) {
                Ok(()) => true,
                Err(SendError::Host(HostError::WouldBlock)) => false,
                Err(error) => panic!("send: {error:?}"),
            }
        })
        .count();
    let unsent = sockets.count_unsent_bytes(&socket).unwrap().0;
    let kept = SEND_BUFFER * 2;
    assert!(
        (kept..kept + PACED_DATAGRAM_ROOM).contains(&unsent),
        "{unsent} bytes unsent after {sent} datagrams"
    );
}
