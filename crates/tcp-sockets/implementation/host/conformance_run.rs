wcontainers_tcp_sockets_conformance::tests!(
    wcontainers_tcp_sockets_host::HostTcpSockets::new()
        .expect("the host's readiness mechanism starts")
);
