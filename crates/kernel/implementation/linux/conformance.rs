wcontainers_kernel_conformance::tests!(
    wcontainers_executor_unicorn::driver::program_not_run(
        "hello",
        &["-nostdlib"],
        &std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest"),
    );
    Process: yes(
        wcontainers_executor_unicorn::driver::program_not_run(
        "hello",
        &["-nostdlib"],
        &std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest"),
    )
            .process(wcontainers_executor_unicorn::driver::FIRST_PROCESS)
            .expect("the first process")
    )
);
