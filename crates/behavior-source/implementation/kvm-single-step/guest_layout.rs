//! Where the single step machine's guest puts things, in guest-physical memory and in the handler's half of the
//! address space, as the native back end's inside does; the build script reads the handler's constants from here.

use wcontainers_abi::PAGE_SIZE;

/// Bytes of one table entry.
pub const ENTRY_BYTES: u64 = size_of::<u64>() as u64;

/// Bits of virtual address one table level resolves.
pub const LEVEL_BITS: u32 = PAGE_SIZE.trailing_zeros() - ENTRY_BYTES.trailing_zeros();

/// Entries in one table.
pub const ENTRIES: u64 = PAGE_SIZE / ENTRY_BYTES;

/// Levels of translation tables in each half.
pub const LEVELS: u32 = 4;

/// The level of the top-level table.
pub const TOP_LEVEL: u32 = 0;

/// The level whose entries map single pages.
pub const LEAF_LEVEL: u32 = LEVELS - 1;

/// Width of a virtual address in either half.
pub const VA_BITS: u32 = PAGE_SIZE.trailing_zeros() + LEVELS * LEVEL_BITS;

/// Returns the bytes one entry at the level covers.
pub const fn level_span(level: u32) -> u64 {
    1 << (PAGE_SIZE.trailing_zeros() + (LEAF_LEVEL - level) * LEVEL_BITS)
}

/// Returns the index of the address's entry in its table at the level.
pub const fn level_index(address: u64, level: u32) -> u64 {
    (address >> (PAGE_SIZE.trailing_zeros() + (LEAF_LEVEL - level) * LEVEL_BITS)) & (ENTRIES - 1)
}

/// The first address of the handler's half (ARM64: the TTBR1 half), which maps all of guest-physical memory.
pub const HANDLER_HALF: u64 = !((1 << VA_BITS) - 1);

/// Returns the handler's address of a guest-physical address, through the linear map at the start of its half.
pub const fn linear(address: u64) -> u64 {
    HANDLER_HALF + address
}

/// The guest-physical address of the first byte of memory.
pub const MEMORY_BASE: u64 = 1 << 30;

/// The guest-physical range below [`MEMORY_BASE`] that holds devices, mapped as device memory.
pub const DEVICES: core::ops::Range<u64> = 0..MEMORY_BASE;

/// The guest-physical address of the interrupt controller's distributor, below memory.
pub const GIC_DISTRIBUTOR: u64 = 0x0800_0000;

/// The guest-physical address of the interrupt controller's first redistributor, after the distributor.
pub const GIC_REDISTRIBUTORS: u64 = 0x080A_0000;

/// The memory attribute index of normal write-back memory in MAIR_EL1.
pub const ATTRIBUTE_NORMAL: u64 = 0;

/// The memory attribute index of device memory in MAIR_EL1.
pub const ATTRIBUTE_DEVICE: u64 = 1;

/// Bytes of the exception vector table, whose base VBAR_EL1 needs aligned to its size.
pub const VECTOR_TABLE_BYTES: u64 = 2048;

/// Vectors in the table: four kinds of exception from each of four sources.
pub const VECTORS: u64 = 16;

/// Bytes of one vector's entry.
pub const VECTOR_BYTES: u64 = VECTOR_TABLE_BYTES / VECTORS;

/// The bit of an SMCCC function number where its owner starts.
const SMCCC_OWNER_SHIFT: u32 = 24;

/// SMCCC's owner of OEM service calls, which no hypervisor defines a call of.
const SMCCC_OWNER_OEM: u32 = 3;

/// The doorbell: an `hvc #0` with this SMCCC function number in w0, which KVM forwards to the machine's filter.
pub const DOORBELL_FUNCTION: u32 = SMCCC_OWNER_OEM << SMCCC_OWNER_SHIFT;
