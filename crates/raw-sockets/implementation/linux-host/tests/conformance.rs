//! The conformance suite on Linux's raw sockets.
#![cfg(target_os = "linux")]

wcontainers_raw_sockets_conformance::tests!(
    wcontainers_raw_sockets_linux_host::LinuxHostRawSockets::new()
);
