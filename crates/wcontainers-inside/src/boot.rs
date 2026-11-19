//! How a virtual CPU starts: the boot map of the inside's half and the system registers at entry. The monitor
//! applies these through whichever hypervisor it runs on; the floor relies on them.

use aarch64_cpu::registers::{CPACR_EL1, ID_AA64MMFR0_EL1, MAIR_EL1, SPSR_EL1, TCR_EL1};
use aarch64_paging::descriptor::El1Attributes;
use arm_sysregs::el1::registers::SctlrEl1;
use wcontainers_abi::PAGE_SIZE;

use crate::layout::{
    ATTRIBUTE_DEVICE, ATTRIBUTE_NORMAL, DEVICES, ENTRY_BYTES, MEMORY_BASE, Region, VA_BITS,
    level_index, level_span,
};

/// The level of the boot map's blocks (1 GiB each with 4 KiB pages).
const BOOT_BLOCK_LEVEL: u32 = 1;

/// Shift of a descriptor's memory attribute index.
const ATTRIBUTE_INDEX_SHIFT: u32 = El1Attributes::ATTRIBUTE_INDEX_1.bits().trailing_zeros();

/// Bytes the boot map's tables take: the top-level table and one table of blocks under it.
pub const BOOT_MAP_BYTES: u64 = (BOOT_BLOCK_LEVEL as u64 + 1) * PAGE_SIZE;

/// Attributes of the inside's own entries: global, EL1 only, never run by the guest's level.
pub fn inside_attributes(attribute_index: u64) -> El1Attributes {
    El1Attributes::VALID
        | El1Attributes::ACCESSED
        | El1Attributes::INNER_SHAREABLE
        | El1Attributes::UXN
        | El1Attributes::from_bits_retain((attribute_index << ATTRIBUTE_INDEX_SHIFT) as usize)
}

/// Writes the boot map of the inside's half at guest-physical `root`: guest-physical memory below
/// [`MEMORY_BASE`] as device memory and `memory` bytes from it as normal memory, through the linear map, in
/// blocks. `write` stores one entry at a guest-physical address.
pub fn write_boot_map(root: u64, memory: u64, mut write: impl FnMut(u64, u64)) {
    let blocks = root + PAGE_SIZE;
    let table = (El1Attributes::VALID | El1Attributes::TABLE_OR_PAGE).bits() as u64;
    write(root + Region::Linear.index() * ENTRY_BYTES, blocks | table);
    let span = level_span(BOOT_BLOCK_LEVEL);
    let device = inside_attributes(ATTRIBUTE_DEVICE) | El1Attributes::PXN;
    let mut at = DEVICES.start;
    while at < MEMORY_BASE + memory {
        let attributes = if DEVICES.contains(&at) {
            device
        } else {
            inside_attributes(ATTRIBUTE_NORMAL)
        };
        let entry = at | attributes.bits() as u64;
        write(
            blocks + level_index(at, BOOT_BLOCK_LEVEL) * ENTRY_BYTES,
            entry,
        );
        at += span;
    }
}

/// System register values at entry, for every virtual CPU.
#[derive(Clone, Copy, Debug)]
pub struct BootRegisters {
    pub mair: u64,
    pub tcr: u64,
    pub sctlr: u64,
    pub cpacr: u64,
    pub ttbr1: u64,
    /// PSTATE: EL1 on its own stack, every interrupt masked.
    pub pstate: u64,
}

/// The translation granule of both halves, from the guest's page size: the 4 KiB granule, the one
/// aarch64-paging's entries are built for. Another page size stops the build here.
const GRANULE: (TCR_EL1::TG0::Value, TCR_EL1::TG1::Value) =
    if PAGE_SIZE == aarch64_paging::paging::PAGE_SIZE as u64 {
        (TCR_EL1::TG0::Value::KiB_4, TCR_EL1::TG1::Value::KiB_4)
    } else {
        panic!("no translation granule is set up for the guest's page size")
    };

/// The registers for a boot map at `root`, on a CPU whose ID_AA64MMFR0_EL1 is `memory_model_features`, with
/// the guest's stack pointer alignment checked when `sp_alignment_checked` (the guest's `Guest` says).
pub fn boot_registers(
    root: u64,
    memory_model_features: u64,
    sp_alignment_checked: bool,
) -> BootRegisters {
    let physical_range = ID_AA64MMFR0_EL1::PARange.read(memory_model_features);
    let size = u64::from(u64::BITS - VA_BITS);
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
        + TCR_EL1::TBI0::Ignored
        + TCR_EL1::TBI1::Used;
    let mair = MAIR_EL1::Attr0_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc
        + MAIR_EL1::Attr0_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc
        + MAIR_EL1::Attr1_Device::nonGathering_nonReordering_EarlyWriteAck;
    // Linux's EL0 settings: cache maintenance and `dc zva` allowed, unaligned plain accesses allowed, no
    // pointer authentication or branch targets, `wfi` and `wfe` trapped; exception entry and return
    // synchronise context; privileged access to user memory is never blocked.
    let sctlr = SctlrEl1::M
        | SctlrEl1::C
        | SctlrEl1::I
        | SctlrEl1::SA
        | SctlrEl1::UCI
        | SctlrEl1::DZE
        | SctlrEl1::EIS
        | SctlrEl1::EOS
        | SctlrEl1::SPAN
        | SctlrEl1::TSCXT
        | SctlrEl1::NTLSMD
        | SctlrEl1::LSMAOE;
    let sctlr = if sp_alignment_checked {
        sctlr | SctlrEl1::SA0
    } else {
        sctlr
    };
    let cpacr = CPACR_EL1::FPEN::TrapNothing;
    BootRegisters {
        mair: mair.value,
        tcr: tcr.value,
        sctlr: sctlr.bits(),
        cpacr: cpacr.value,
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
