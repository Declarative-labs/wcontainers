wcontainers_unicorn_conformance::tests!(
    wcontainers_unicorn_hostile::HostileUnicorn<
        wcontainers_unicorn_conformance::Seen,
        wcontainers_unicorn_direct::DirectUnicorn<wcontainers_unicorn_conformance::Seen>,
    >
);
