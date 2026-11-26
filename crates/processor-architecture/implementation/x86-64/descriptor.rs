//! LAR, LSL, VERR and VERW, which answer at ring 3 from the descriptor table Linux gives a 64-bit program, as the
//! [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (vol. 2) gives them.
//!
//! Linux's table has 16 entries. A program at ring 3 reaches four: the 32-bit and the 64-bit user code segments
//! (0x23 and 0x33), the user data segment (0x2B), and the per-CPU segment (0x7B), a read-only expand-down data
//! segment whose limit holds the CPU's number, which the state does not decide. The kernel's segments and the task
//! state segment have DPL 0, Linux leaves the LDT and the TLS entries empty in a program that never set them, and
//! no LDT is loaded, so every other selector fails: ZF is cleared and the destination kept. Each form changes no flag
//! but ZF. The answers are as a host program reads them (`gdt.c`, on cores 3 and 50).

use iced_x86::Code;

use crate::forms::Instruction;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DescriptorOperation {
    /// LAR: the access rights, bits 8 to 23 of the descriptor's high doubleword.
    AccessRights,
    /// LSL: the segment limit in bytes.
    SegmentLimit,
    /// VERR: whether the segment can be read.
    VerifyRead,
    /// VERW: whether the segment can be written.
    VerifyWrite,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DescriptorForm {
    pub instruction: Instruction,
    pub operation: DescriptorOperation,
    pub in_memory: bool,
    /// By operand size, 16 bits first.
    pub codes: &'static [Code],
}

/// What a selector reaches at ring 3.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Descriptor {
    pub access_rights: u32,
    /// None for the per-CPU segment, whose limit the state does not decide.
    pub limit: Option<u32>,
    pub readable: bool,
    pub writable: bool,
}

/// The selector of the per-CPU segment, whatever its RPL.
pub const CPU_SEGMENT_INDEX: u16 = 15;

/// The descriptor the selector reaches from ring 3, or none where LAR fails.
pub fn find_descriptor(selector: u16) -> Option<Descriptor> {
    const TABLE_INDICATOR: u16 = 1 << 2;
    let code = |access_rights| Descriptor { access_rights, limit: Some(u32::MAX), readable: true, writable: false };
    if selector & TABLE_INDICATOR != 0 {
        return None;
    }
    match selector >> 3 {
        4 => Some(code(0x00CF_FB00)),
        5 => Some(Descriptor { access_rights: 0x00CF_F300, limit: Some(u32::MAX), readable: true, writable: true }),
        6 => Some(code(0x00AF_FB00)),
        CPU_SEGMENT_INDEX => Some(Descriptor { access_rights: 0x0040_F500, limit: None, readable: true, writable: false }),
        _ => None,
    }
}

pub fn find_descriptor_form(instruction: Instruction) -> Option<&'static DescriptorForm> {
    DESCRIPTOR_FORMS.iter().find(|form| form.instruction == instruction)
}

const fn descriptor(instruction: Instruction, operation: DescriptorOperation, in_memory: bool, codes: &'static [Code]) -> DescriptorForm {
    DescriptorForm { instruction, operation, in_memory, codes }
}

use DescriptorOperation::{AccessRights, SegmentLimit, VerifyRead, VerifyWrite};

const LAR: &[Code] = &[Code::Lar_r16_rm16, Code::Lar_r32_r32m16, Code::Lar_r64_r64m16];
const LSL: &[Code] = &[Code::Lsl_r16_rm16, Code::Lsl_r32_r32m16, Code::Lsl_r64_r64m16];

/// Every form of this family modeled.
pub static DESCRIPTOR_FORMS: &[DescriptorForm] = &[
    descriptor(Instruction::LarGprvGprv, AccessRights, false, LAR),
    descriptor(Instruction::LarGprvMemw, AccessRights, true, LAR),
    descriptor(Instruction::LslGprvGprz, SegmentLimit, false, LSL),
    descriptor(Instruction::LslGprvMemw, SegmentLimit, true, LSL),
    descriptor(Instruction::VerrGpr16, VerifyRead, false, &[Code::Verr_rm16]),
    descriptor(Instruction::VerrMemw, VerifyRead, true, &[Code::Verr_rm16]),
    descriptor(Instruction::VerwGpr16, VerifyWrite, false, &[Code::Verw_rm16]),
    descriptor(Instruction::VerwMemw, VerifyWrite, true, &[Code::Verw_rm16]),
];
