//! Single programs from busybox in the reference image run and match the reference in output, exit
//! status and the files they leave behind.

use wcontainers_kernel_test::busybox::compare;

#[test]
fn echo() {
    compare(&["echo", "hello", "world"]);
}
