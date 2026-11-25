//! The boot map of the handler's half and the system registers at the handler's entry.

use aarch64_cpu::registers::{CPACR_EL1, ID_AA64MMFR0_EL1, MAIR_EL1, SPSR_EL1, TCR_EL1};
use aarch64_paging::descriptor::El1Attributes;
use arm_sysregs::el1::registers::SctlrEl1;
use wcontainers_abi::PAGE_SIZE;

use crate::guest_layout::{
    ATTRIBUTE_DEVICE, ATTRIBUTE_NORMAL, DEVICES, ENTRY_BYTES, HANDLER_HALF, MEMORY_BASE, TOP_LEVEL,
    level_index, level_span,
};

/// The level of the boot map's blocks (1 GiB each with 4 KiB pages).
const BOOT_BLOCK_LEVEL: u32 = 1;

/// Shift of a descriptor's memory attribute index.
const ATTRIBUTE_INDEX_SHIFT: u32 = El1Attributes::ATTRIBUTE_INDEX_1.bits().trailing_zeros();

/// Bytes the boot map's tables take: the top-level table and one table of blocks under it.
pub(crate) const BOOT_MAP_BYTES: u64 = (BOOT_BLOCK_LEVEL as u64 + 1) * PAGE_SIZE;

/// Returns the attributes of the handler's own entries: global, EL1 only, never run by EL0.
pub(crate) fn to_handler_attributes(attribute_index: u64) -> El1Attributes {
    El1Attributes::VALID
        | El1Attributes::ACCESSED
        | El1Attributes::INNER_SHAREABLE
        | El1Attributes::UXN
        | El1Attributes::from_bits_retain((attribute_index << ATTRIBUTE_INDEX_SHIFT) as usize)
}

/// Writes the boot map of the handler's half at guest-physical `root`: guest-physical memory below
/// [`MEMORY_BASE`] as device memory and `memory` bytes from it as normal memory, through the linear map, in blocks.
pub(crate) fn write_boot_map(root: u64, memory: u64, mut write: impl FnMut(u64, u64)) {
    let blocks = root + PAGE_SIZE;
    let table = (El1Attributes::VALID | El1Attributes::TABLE_OR_PAGE).bits() as u64;
    write(
        root + level_index(HANDLER_HALF, TOP_LEVEL) * ENTRY_BYTES,
        blocks | table,
    );
    let span = level_span(BOOT_BLOCK_LEVEL);
    let device = to_handler_attributes(ATTRIBUTE_DEVICE) | El1Attributes::PXN;
    let mut at = DEVICES.start;
    while at < MEMORY_BASE + memory {
        let attributes = if DEVICES.contains(&at) {
            device
        } else {
            to_handler_attributes(ATTRIBUTE_NORMAL)
        };
        write(
            blocks + level_index(at, BOOT_BLOCK_LEVEL) * ENTRY_BYTES,
            at | attributes.bits() as u64,
        );
        at += span;
    }
}

/// System register values at the handler's entry.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BootRegisters {
    pub(crate) mair: u64,
    pub(crate) tcr: u64,
    pub(crate) sctlr: u64,
    pub(crate) cpacr: u64,
    pub ttbr1: u64,
    /// PSTATE: EL1 on its own stack, every interrupt masked.
    pub(crate) pstate: u64,
}

/// The translation granule of both halves, from the guest's page size: the 4 KiB granule, the one aarch64-paging's
/// entries are built for. Another page size stops the build here.
const GRANULE: (TCR_EL1::TG0::Value, TCR_EL1::TG1::Value) =
    if PAGE_SIZE == aarch64_paging::paging::PAGE_SIZE as u64 {
        (TCR_EL1::TG0::Value::KiB_4, TCR_EL1::TG1::Value::KiB_4)
    } else {
        panic!("no translation granule is set up for the guest's page size")
    };

/// Returns the registers for a boot map at `root` on a CPU whose ID_AA64MMFR0_EL1 is `memory_model_features`, with
/// EL0's stack pointer alignment checked, as Linux has it.
pub(crate) fn to_boot_registers(root: u64, memory_model_features: u64) -> BootRegisters {
    let physical_range = ID_AA64MMFR0_EL1::PARange.read(memory_model_features);
    let size = u64::from(u64::BITS - crate::guest_layout::VA_BITS);
    // The top byte of an EL0 address is part of it (TBI0 clear), so a tagged address lies past the addresses a program
    // can use and faults at level 0, as the architecture's model has it; Linux sets TBI0 instead.
    let tcr = TCR_EL1::T0SZ.val(size)
        + TCR_EL1::T1SZ.val(size)
        + TCR_EL1::TG0.val(GRANULE.0 as u64)
        + TCR_EL1::TG1.val(GRANULE.1 as u64)
        + TCR_EL1::SH0::Inner
        + TCR_EL1::SH1::Inner
        + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
        + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
        + TCR_EL1::IRGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
        + TCR_EL1::ORGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
        + TCR_EL1::IPS.val(physical_range)
        + TCR_EL1::AS::ASID16Bits
        + TCR_EL1::A1::TTBR0
        + TCR_EL1::TBI0::Used
        + TCR_EL1::TBI1::Used;
    let mair = MAIR_EL1::Attr0_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc
        + MAIR_EL1::Attr0_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc
        + MAIR_EL1::Attr1_Device::nonGathering_nonReordering_EarlyWriteAck;
    // EL0 as the native back end's inside presents it: cache maintenance and `dc zva` allowed, unaligned plain
    // accesses allowed, the stack pointer's alignment checked, no pointer authentication or branch targets; `wfi`,
    // `wfe` and CTR_EL0 trap. Exception entry and return synchronize context.
    let sctlr = SctlrEl1::M
        | SctlrEl1::C
        | SctlrEl1::I
        | SctlrEl1::SA
        | SctlrEl1::SA0
        | SctlrEl1::UCI
        | SctlrEl1::DZE
        | SctlrEl1::EIS
        | SctlrEl1::EOS
        | SctlrEl1::SPAN
        | SctlrEl1::TSCXT
        | SctlrEl1::NTLSMD
        | SctlrEl1::LSMAOE;
    BootRegisters {
        mair: mair.value,
        tcr: tcr.value,
        sctlr: sctlr.bits(),
        cpacr: CPACR_EL1::FPEN::TrapNothing.value,
        ttbr1: root,
        pstate: (SPSR_EL1::M::EL1h
            + SPSR_EL1::D::Masked
            + SPSR_EL1::A::Masked
            + SPSR_EL1::I::Masked
            + SPSR_EL1::F::Masked)
            .value,
    }
}

// The devices take exactly the first block.
const _: () = assert!(level_span(BOOT_BLOCK_LEVEL) == MEMORY_BASE);
