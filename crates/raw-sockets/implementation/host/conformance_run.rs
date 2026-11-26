#![cfg(target_os = "linux")]

wcontainers_raw_sockets_conformance::tests!(
    wcontainers_raw_sockets_host::HostRawSockets::new().expect("the host's readiness"),
    wcontainers_linux_socket_options_direct::DirectLinuxSocketOptions
);
