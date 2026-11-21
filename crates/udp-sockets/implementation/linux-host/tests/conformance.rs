//! The conformance suite on the host implementation, each test in a network namespace of its own thread.
#![cfg(target_os = "linux")]

use wcontainers_udp_sockets_linux_host::LinuxHostUdpSockets;

/// A new network namespace for this thread, holding only its loopback interface, brought up.
fn isolated() -> LinuxHostUdpSockets {
    // SAFETY: unshare changes only this thread's network namespace.
    assert_eq!(
        unsafe { libc::unshare(libc::CLONE_NEWNET) },
        0,
        "unshare the network namespace (run as root)"
    );
    // SAFETY: a socket only for the interface requests, closed below.
    let control = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    assert!(control >= 0, "a socket for interface requests");
    // SAFETY: an all-zero `ifreq` is valid; the name fits it.
    let mut request: libc::ifreq = unsafe { core::mem::zeroed() };
    for (slot, byte) in request.ifr_name.iter_mut().zip(b"lo") {
        *slot = *byte as libc::c_char;
    }
    // SAFETY: the requests read and write the `ifreq` given.
    unsafe {
        assert_eq!(
            libc::ioctl(control, libc::SIOCGIFFLAGS, &mut request),
            0,
            "read the loopback's flags"
        );
        request.ifr_ifru.ifru_flags |= (libc::IFF_UP | libc::IFF_MULTICAST) as libc::c_short;
        assert_eq!(
            libc::ioctl(control, libc::SIOCSIFFLAGS, &request),
            0,
            "bring the loopback up"
        );
        libc::close(control);
    }
    LinuxHostUdpSockets::new()
}

wcontainers_udp_sockets_conformance::tests!(isolated());
