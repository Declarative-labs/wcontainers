//! A kernel made through the kernel interface tells its programs that as many processors are
//! online as it was given parallelism, wherever a program asks.

use std::num::NonZeroUsize;

use wcontainers_executor_unicorn::driver::program_exit_code;

/// The processors a program is told are online on a kernel with `parallelism` runners.
fn told(parallelism: usize) -> i32 {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let parallelism = NonZeroUsize::new(parallelism).expect("some parallelism");
    program_exit_code("processors", &[], &out, parallelism)
}

#[test]
fn programs_are_told_the_parallelism_as_the_processors_online() {
    for parallelism in [1, 4, 65] {
        assert_eq!(
            told(parallelism),
            parallelism as i32,
            "parallelism {parallelism}"
        );
    }
}
