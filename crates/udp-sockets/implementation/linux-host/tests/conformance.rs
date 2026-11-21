//! The conformance suite on the host implementation, each test in a network namespace of its own thread.
#![cfg(target_os = "linux")]

use std::process::Command;

use wcontainers_udp_sockets_linux_host::LinuxHostUdpSockets;

/// Runs one iproute2 command in this thread's network namespace, which a child process inherits.
fn ip(arguments: &[&str]) {
    let status = Command::new("ip")
        .args(arguments)
        .status()
        .expect("run iproute2's ip");
    assert!(status.success(), "ip {arguments:?} answered {status}");
}

/// A new network namespace for this thread: loopback with multicast, and a dummy interface that loops IPv6
/// multicast back to this host and sends nothing anywhere, both up.
fn isolated() -> LinuxHostUdpSockets {
    // SAFETY: unshare changes only this thread's network namespace.
    assert_eq!(
        unsafe { libc::unshare(libc::CLONE_NEWNET) },
        0,
        "unshare the network namespace (run as root)"
    );
    ip(&["link", "set", "lo", "up", "multicast", "on"]);
    ip(&["link", "add", "multicast0", "up", "type", "dummy"]);
    LinuxHostUdpSockets::new()
}

wcontainers_udp_sockets_conformance::tests!(isolated());
