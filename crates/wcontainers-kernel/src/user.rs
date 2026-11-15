//! Checks on guest addresses that Linux makes before touching user memory.

/// One past the highest user address on ARM64 Linux with 48 bit virtual addresses (`TASK_SIZE_MAX`).
const ADDRESS_LIMIT: u64 = 1 << 48;

/// Linux's `access_ok`: the whole range lies below the user address limit. Whether it is mapped is
/// found out only when it is copied.
pub(crate) fn access_ok(address: u64, length: u64) -> bool {
    length <= ADDRESS_LIMIT && address <= ADDRESS_LIMIT - length
}
