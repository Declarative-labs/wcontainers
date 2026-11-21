//! Opening device nodes compared with real ARM64 Linux, and the device not built counted.

use std::path::Path;

use wcontainers_executor_unicorn::assert_same_output;
use wcontainers_kernel_linux::reason;

#[test]
fn device_opens_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "devopen", &[]);
}

#[test]
fn tunnel_device_is_counted() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_executor_unicorn::guest::build("devopen", &[], &out);
    let (ours, theirs) = wcontainers_executor_unicorn::run_both(&program, &["tun"]);
    assert_eq!(
        String::from_utf8_lossy(&theirs.stdout),
        "char 10:200: mknod 0, open ok\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stdout),
        "char 10:200: mknod 0, open No such device\n"
    );
    assert!(
        ours.counters
            .iter()
            .any(|counter| counter.reason == reason::DEVICE_NOT_BUILT)
    );
}
