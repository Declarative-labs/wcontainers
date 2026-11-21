//! The conformance suite on the host implementation, each test in a network namespace of its own thread.
#![cfg(target_os = "linux")]

use std::process::Command;

use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{ByteCount, IpSocketSetting, IpVersion};
use wcontainers_kernel::UserId;
use wcontainers_udp_sockets::{UdpProtocol, UdpSocketSetting, UdpSockets};
use wcontainers_udp_sockets_linux_host::{
    LinuxHostUdpSockets, RECEIVE_BUFFER_CUT_BY_THE_HOST, SEND_BUFFER_CUT_BY_THE_HOST,
};

/// Runs one iproute2 command in this thread's network namespace, which a child process inherits.
fn ip(arguments: &[&str]) {
    let status = Command::new("ip")
        .args(arguments)
        .status()
        .expect("run iproute2's ip");
    assert!(status.success(), "ip {arguments:?} answered {status}");
}

/// A new network namespace for this thread, the network the suite asks for: loopback with multicast, segment
/// routing headers taken and a qdisc that paces; a dummy interface that sends nothing anywhere, for IPv6
/// multicast and 192.0.2.0/24; user 1000's datagrams to anything not local prohibited.
fn isolated() -> LinuxHostUdpSockets {
    // SAFETY: unshare changes only this thread's network namespace.
    assert_eq!(
        unsafe { libc::unshare(libc::CLONE_NEWNET) },
        0,
        "unshare the network namespace (run as root)"
    );
    ip(&["link", "set", "lo", "up", "multicast", "on"]);
    ip(&["link", "add", "multicast0", "up", "type", "dummy"]);
    ip(&["route", "add", "192.0.2.0/24", "dev", "multicast0"]);
    ip(&["rule", "add", "uidrange", "1000-1000", "prohibit"]);
    let status = Command::new("tc")
        .args(["qdisc", "add", "dev", "lo", "root", "fq"])
        .status()
        .expect("run iproute2's tc");
    assert!(status.success(), "tc answered {status}");
    // This thread's namespace is the one /proc/sys/net shows it.
    for interface in ["all", "lo"] {
        std::fs::write(
            format!("/proc/sys/net/ipv6/conf/{interface}/seg6_enabled"),
            "1",
        )
        .expect("take segment routing headers");
    }
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
