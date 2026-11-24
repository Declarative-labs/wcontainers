#![cfg(target_os = "linux")]

wcontainers_icmp_sockets_conformance::tests!(
    wcontainers_icmp_sockets_host::HostIcmpSockets::new().expect("the host's readiness")
);
