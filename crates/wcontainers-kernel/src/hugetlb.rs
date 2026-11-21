//! Huge pages as the made-up machine offers them: the sizes the reference's ARM64 kernel has with
//! 4 KiB pages, none of them ever reserved (a container cannot write `nr_hugepages`). Every request
//! for huge page memory finds no pages, as on such a machine.

use crate::Errno;

/// Bytes in a KiB, the unit `/sys/kernel/mm/hugepages` names sizes in.
pub(crate) use wcontainers_abi::KIB;

/// The huge page sizes offered, in KiB, with the size each demotes to.
pub(crate) const SIZES: [(u64, Option<u64>); 4] = [
    (64, None),
    (2048, Some(64)),
    (32768, Some(2048)),
    (1048576, Some(32768)),
];

/// The size a request that names none takes (`default_hstate`: a PMD's on ARM64), in KiB.
pub(crate) const DEFAULT_SIZE: u64 = SIZES[1].0;

/// The page shift (log2 of the size in bytes) of the huge pages a request's size bits name, as
/// `hstate_sizelog` and `get_hstate_idx` find them: 0 for the default size; ENODEV when the machine
/// offers no such size.
pub(crate) fn page_shift(size_log: u32) -> Result<u32, Errno> {
    let default_shift = (DEFAULT_SIZE * KIB).trailing_zeros();
    let shift = if size_log == 0 {
        default_shift
    } else {
        size_log
    };
    SIZES
        .iter()
        .any(|(size, _)| (size * KIB).trailing_zeros() == shift)
        .then_some(shift)
        .ok_or(Errno::ENODEV)
}
