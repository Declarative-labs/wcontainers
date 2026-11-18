//! Where things are, in guest-physical memory and in the inside's half of the address space. The monitor and
//! the inside both read these, so each is defined here once.

use wcontainers_abi::PAGE_SIZE;

/// Bits of a guest page offset: the translation granule is the guest's page.
pub const PAGE_SHIFT: u32 = PAGE_SIZE.trailing_zeros();

/// Bytes of one table entry.
pub const ENTRY_BYTES: u64 = size_of::<u64>() as u64;

/// Bits of virtual address one table level resolves.
pub const LEVEL_BITS: u32 = PAGE_SHIFT - ENTRY_BYTES.trailing_zeros();

/// Entries in one table.
pub const ENTRIES: u64 = PAGE_SIZE / ENTRY_BYTES;

/// Levels of translation tables in each half.
pub const LEVELS: u32 = 4;

/// The level whose entries map single pages.
pub const LEAF_LEVEL: u32 = LEVELS - 1;

/// The level whose entries can map a large block (2 MiB with 4 KiB pages).
pub const BLOCK_LEVEL: u32 = LEAF_LEVEL - 1;

/// Width of a virtual address in either half.
pub const VA_BITS: u32 = PAGE_SHIFT + LEVELS * LEVEL_BITS;

/// The bytes one entry at `level` covers.
pub const fn level_span(level: u32) -> u64 {
    1 << (PAGE_SHIFT + (LEAF_LEVEL - level) * LEVEL_BITS)
}

/// The bytes a block entry covers.
pub const BLOCK_SIZE: u64 = level_span(BLOCK_LEVEL);

/// The index of `va`'s entry in its table at `level`.
pub const fn level_index(va: u64, level: u32) -> u64 {
    (va >> (PAGE_SHIFT + (LEAF_LEVEL - level) * LEVEL_BITS)) & (ENTRIES - 1)
}

/// The first address of the inside's half (ARM64: the TTBR1 half).
pub const INSIDE_HALF: u64 = !((1 << VA_BITS) - 1);

/// The regions of the inside's half, one top-level entry each, in this order from [`INSIDE_HALF`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// All of guest-physical memory, at [`INSIDE_HALF`] plus its guest-physical address. Global, EL1 only.
    Linear,
    /// Translated code. Global, execute-only for the guest's level.
    Code,
    /// The running container's kernel heap. Not global: each process of a container links its container's.
    Heap,
    /// The register pages of a process's threads. Not global.
    Threads,
}

impl Region {
    /// The region's first address.
    pub const fn base(self) -> u64 {
        INSIDE_HALF + self as u64 * level_span(0)
    }

    /// The region's top-level index.
    pub const fn index(self) -> u64 {
        level_index(self.base(), 0)
    }
}

/// The entry mirror fills the top half of the inside's half.
pub const MIRROR: u64 = INSIDE_HALF | (1 << (VA_BITS - 1));

/// The inside's address of guest-physical `address`, through the linear map.
pub const fn linear(address: u64) -> u64 {
    Region::Linear.base() + address
}

/// The guest-physical address of the first byte of memory, where the inside's image is loaded.
pub const MEMORY_BASE: u64 = 1 << 30;

/// The guest-physical address of the interrupt controller's distributor, below memory.
pub const GIC_DISTRIBUTOR: u64 = 0x0800_0000;

/// The guest-physical address of the interrupt controller's first redistributor, after the distributor.
pub const GIC_REDISTRIBUTORS: u64 = 0x080A_0000;

/// The guest-physical range below [`MEMORY_BASE`] that holds devices, mapped as device memory.
pub const DEVICES: core::ops::Range<u64> = 0..MEMORY_BASE;

/// The address the inside's image is linked at.
pub const IMAGE: u64 = linear(MEMORY_BASE);

/// The memory attribute index of normal write-back memory in MAIR_EL1.
pub const ATTRIBUTE_NORMAL: u64 = 0;

/// The memory attribute index of device memory in MAIR_EL1.
pub const ATTRIBUTE_DEVICE: u64 = 1;
