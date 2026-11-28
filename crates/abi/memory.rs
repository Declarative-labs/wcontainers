//! Guest memory as both sides see it: pages and what each allows.

use bitflags::bitflags;
use vstd::prelude::*;

/// The page size the guest is told. Permissions are kept per page of this size.
pub const PAGE_SIZE: u64 = 4096;
/// log2 of [`PAGE_SIZE`], and the bits of an offset within a page.
pub const PAGE_SHIFT: u32 = PAGE_SIZE.trailing_zeros();
pub const PAGE_MASK: u64 = PAGE_SIZE - 1;
/// Bytes in a KiB.
pub const KIB: u64 = 1 << 10;
/// Bytes of a 64-bit word: an address, a general register.
pub const WORD: u64 = size_of::<u64>() as u64;

/// Nothing is ever placed below this address, so a null pointer always faults.
pub const LOWEST_ADDRESS: u64 = 64 * 1024;

verus! {

bitflags! {
    /// What a page allows. Empty means any access faults.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Permissions: u8 {
        const READ = 1;
        const WRITE = 2;
        const EXECUTE = 4;
    }
}

} // verus!

/// The addresses a process may use: from [`LOWEST_ADDRESS`] up to, not including, `end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressSpace {
    /// The host's cap. 4 GiB on a host with 32 bit memories.
    pub end: u64,
}
