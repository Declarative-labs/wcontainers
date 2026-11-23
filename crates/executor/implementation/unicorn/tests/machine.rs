//! The machine a container is started with: the memory size the guest is told and the width of
//! its address space, given apart from the executor's address space.

use wcontainers_abi::{KIB, PAGE_SIZE};
use wcontainers_executor_unicorn::busybox::{run_ours_prepared, try_ours_prepared};
use wcontainers_executor_unicorn::driver::Failure;
use wcontainers_executor_unicorn::standin::SPACE_32_BIT;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel_linux::ARM64_ADDRESS_SPACE_BYTES;

/// The first number on the line of `text` that starts with `label`.
fn number_after(text: &str, label: &str) -> u64 {
    text.lines()
        .find_map(|line| line.strip_prefix(label))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("no {label:?} in {text:?}"))
}

/// The end of the highest mapping `/proc/self/maps` lists.
fn highest_end(maps: &str) -> u64 {
    maps.lines()
        .map(|line| {
            let range = line.split_whitespace().next().expect("a range");
            let (_, end) = range.split_once('-').expect("a range");
            u64::from_str_radix(end, 16).expect("an address")
        })
        .max()
        .expect("some mappings")
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
        assert!(
            highest_end(&maps) <= width.min(SPACE_32_BIT),
            "{width:#x}: {maps}"
        );
        let narrowed = ours.warnings.iter().any(|warning| warning.reason == reason);
        assert_eq!(narrowed, width > ARM64_ADDRESS_SPACE_BYTES, "{width:#x}");
    }
}

/// A width that is not a whole number of pages gives the whole pages below it, counted and warned
/// of with the width asked for.
#[test]
fn a_width_of_part_pages_is_cut_to_whole_ones() {
    let reason = wcontainers_kernel_linux::reason::ADDRESS_SPACE_NARROWED;
    let width = SPACE_32_BIT / 2 + PAGE_SIZE / 3;
    let ours = run_ours_prepared(&["cat", "/proc/self/maps"], |prepared| {
        prepared.address_space_bytes = width
    });
    let maps = String::from_utf8_lossy(&ours.outcome.stdout);
    assert_eq!(ours.outcome.exit_code, 0, "{maps}");
    assert!(highest_end(&maps) <= SPACE_32_BIT / 2, "{maps}");
    let warned: Vec<u64> = ours
        .warnings
        .iter()
        .filter(|warning| warning.reason == reason)
        .map(|warning| warning.detail)
        .collect();
    assert_eq!(warned, [width]);
}

/// A width too small for the first program and its stack fails the start as out of memory, before
/// the program runs.
#[test]
fn a_width_too_small_for_the_program_fails_the_start() {
    let width = SPACE_32_BIT >> 12;
    let failed = try_ours_prepared(&["true"], |prepared| prepared.address_space_bytes = width);
    assert!(
        matches!(failed, Err(Failure::Start(Errno::ENOMEM))),
        "{:?}",
        failed.map(|run| run.outcome)
    );
}
