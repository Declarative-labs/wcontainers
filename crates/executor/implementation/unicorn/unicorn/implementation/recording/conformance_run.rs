wcontainers_unicorn_conformance::tests!(
    wcontainers_unicorn_recording::RecordingUnicorn<
        wcontainers_unicorn_direct::DirectUnicorn<wcontainers_unicorn_conformance::Seen>,
    >
);
