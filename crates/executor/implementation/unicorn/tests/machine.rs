//! The machine a container is started with: the memory size the guest is told and the width of
//! its address space, given apart from the executor's address space.

use wcontainers_abi::KIB;
use wcontainers_executor_unicorn::busybox::run_ours_prepared;
use wcontainers_executor_unicorn::standin::SPACE_32_BIT;
use wcontainers_kernel_linux::ARM64_ADDRESS_SPACE_BYTES;

/// The first number on the line of `text` that starts with `label`.
fn number_after(text: &str, label: &str) -> u64 {
    text.lines()
        .find_map(|line| line.strip_prefix(label))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("no {label:?} in {text:?}"))
}

/// `/proc/meminfo` and `sysinfo` (which `free` reads) tell the size given, not the executor's.
#[test]
fn the_memory_size_given_is_the_one_told() {
    let given = SPACE_32_BIT / 4 * 3;
    let ours = run_ours_prepared(&["sh", "-c", "cat /proc/meminfo; free"], |prepared| {
        prepared.memory_bytes = Some(given)
    });
    let shown = String::from_utf8_lossy(&ours.outcome.stdout);
    assert_eq!(number_after(&shown, "MemTotal:") * KIB, given, "{shown}");
    assert_eq!(number_after(&shown, "Mem:") * KIB, given, "{shown}");
}

/// Every mapping of a process lies under its address space's end, whatever the width given; a
/// width past the reference's is cut to it, counted and warned of.
#[test]
fn placements_stay_under_each_address_width() {
    let reason = wcontainers_kernel_linux::reason::ADDRESS_SPACE_NARROWED;
    for width in [
        SPACE_32_BIT / 4,
        SPACE_32_BIT / 2,
        ARM64_ADDRESS_SPACE_BYTES << 4,
    ] {
        let ours = run_ours_prepared(&["cat", "/proc/self/maps"], |prepared| {
            prepared.address_space_bytes = width
        });
        let maps = String::from_utf8_lossy(&ours.outcome.stdout);
        let end = maps
            .lines()
            .map(|line| {
                let range = line.split_whitespace().next().expect("a range");
                let (_, end) = range.split_once('-').expect("a range");
                u64::from_str_radix(end, 16).expect("an address")
            })
            .max()
            .expect("some mappings");
        assert!(end <= width.min(SPACE_32_BIT), "{width:#x}: {maps}");
        let narrowed = ours.warnings.iter().any(|warning| warning.reason == reason);
        assert_eq!(narrowed, width > ARM64_ADDRESS_SPACE_BYTES, "{width:#x}");
    }
}
