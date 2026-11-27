//! The forms that load a segment register or change CS: MOV to a segment register, POP FS and POP GS, LFS, LGS and
//! LSS, the far CALL and JMP through memory, the far RET, and IRET, as the
//! [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (vol. 2 and vol. 3,
//! chapter 5) gives them at ring 3 in 64-bit mode, against Linux's descriptor table ([`crate::descriptor`]).
//!
//! A data segment register takes a null selector, or one naming a data segment or a readable code segment whose DPL
//! is 3: Linux's user segments and its per-CPU segment; SS takes only 0x2B. Any other selector raises #GP with the
//! selector's index and table indicator as the error code, or #GP(0) for a null SS. Loading FS or GS makes its base
//! the descriptor's, 0 for every one of Linux's, or 0 for a null selector. CS takes Linux's 64-bit code segment,
//! staying in 64-bit mode, or its 32-bit one, entering compatibility mode, with RPL 3; a far return or IRET to an
//! RPL below 3 raises #GP. A target in 64-bit mode must be canonical and one in compatibility mode below 4 GiB, or the
//! form raises #GP(0).

use alloc::vec::Vec;

use iced_x86::Code;

use crate::PreciseInstruction;
use crate::behavior::StoreAcrossFault;
use crate::forms::Instruction;
use crate::stack::SegmentPush;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SegmentOperation {
    /// MOV to a segment register from a register's low 16 bits or memory.
    Move,
    /// POP FS and POP GS.
    Pop,
    /// LFS, LGS and LSS: an offset into the register, then the selector after it into the segment register.
    LoadFar,
    /// The far CALL through memory: CS and the return address pushed, then the far pointer taken.
    CallFar,
    /// The far JMP through memory.
    JumpFar,
    /// The far RET: the return address and CS popped, then the immediate's bytes released.
    ReturnFar,
    /// IRET: the return address, CS, RFLAGS, RSP and SS popped.
    InterruptReturn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SegmentForm {
    pub instruction: Instruction,
    pub operation: SegmentOperation,
    pub in_memory: bool,
    /// By operand size, smallest first.
    pub codes: &'static [Code],
}

/// A form's behavior: what a push that faults part-way writes, and what the far CALL writes of CS's slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::segment)
)]
pub struct SegmentBehavior {
    pub across_fault: StoreAcrossFault,
    pub segment_push: SegmentPush,
}

/// What a selector names in Linux's table, as a segment register can take it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SegmentKind {
    Null,
    /// The user data segment, the only one SS takes.
    Data,
    /// The per-CPU segment: data, read-only.
    ReadOnlyData,
    /// The 32-bit user code segment.
    CompatibilityCode,
    /// The 64-bit user code segment.
    LongCode,
    /// Anything ring 3 cannot reach: a kernel segment, a system descriptor, an empty entry, past the table, the LDT.
    Unreachable,
}

/// What the selector names.
pub fn classify_selector(selector: u16) -> SegmentKind {
    const TABLE_INDICATOR: u16 = 1 << 2;
    if selector & !3 == 0 {
        return SegmentKind::Null;
    }
    if selector & TABLE_INDICATOR != 0 {
        return SegmentKind::Unreachable;
    }
    match selector >> 3 {
        4 => SegmentKind::CompatibilityCode,
        5 => SegmentKind::Data,
        6 => SegmentKind::LongCode,
        15 => SegmentKind::ReadOnlyData,
        _ => SegmentKind::Unreachable,
    }
}

pub fn find_segment_form(instruction: Instruction) -> Option<&'static SegmentForm> {
    SEGMENT_FORMS.iter().find(|form| form.instruction == instruction)
}

const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];

/// For the far CALL both rules of a push that faults part-way and both of CS's slot; else one behavior.
pub(crate) fn list_segment_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_segment_form(instruction) else {
        return Vec::new();
    };
    let first = SegmentBehavior { across_fault: ACROSS_FAULT[0], segment_push: SegmentPush::ZeroExtended };
    if form.operation != SegmentOperation::CallFar {
        return alloc::vec![PreciseInstruction::Segment { instruction, behavior: first }];
    }
    let mut behaviors = Vec::new();
    for across_fault in ACROSS_FAULT {
        for segment_push in [SegmentPush::ZeroExtended, SegmentPush::SelectorOnly] {
            behaviors.push(PreciseInstruction::Segment { instruction, behavior: SegmentBehavior { across_fault, segment_push } });
        }
    }
    behaviors
}

const fn segment(instruction: Instruction, operation: SegmentOperation, in_memory: bool, codes: &'static [Code]) -> SegmentForm {
    SegmentForm { instruction, operation, in_memory, codes }
}

use SegmentOperation::{CallFar, InterruptReturn, JumpFar, LoadFar, Move, Pop, ReturnFar};

pub static SEGMENT_FORMS: &[SegmentForm] = &[
    segment(Instruction::MovSegGpr16, Move, false, &[Code::Mov_Sreg_r32m16, Code::Mov_Sreg_r64m16]),
    segment(Instruction::MovSegMemw, Move, true, &[Code::Mov_Sreg_r32m16]),
    segment(Instruction::PopFs, Pop, false, &[Code::Popw_FS, Code::Popq_FS]),
    segment(Instruction::PopGs, Pop, false, &[Code::Popw_GS, Code::Popq_GS]),
    segment(Instruction::LfsGprvMemp2, LoadFar, true, &[Code::Lfs_r16_m1616, Code::Lfs_r32_m1632, Code::Lfs_r64_m1664]),
    segment(Instruction::LgsGprvMemp2, LoadFar, true, &[Code::Lgs_r16_m1616, Code::Lgs_r32_m1632, Code::Lgs_r64_m1664]),
    segment(Instruction::LssGprvMemp2, LoadFar, true, &[Code::Lss_r16_m1616, Code::Lss_r32_m1632, Code::Lss_r64_m1664]),
    segment(Instruction::CallFarMemp2, CallFar, true, &[Code::Call_m1616, Code::Call_m1632, Code::Call_m1664]),
    segment(Instruction::JmpFarMemp2, JumpFar, true, &[Code::Jmp_m1616, Code::Jmp_m1632, Code::Jmp_m1664]),
    segment(Instruction::RetFar, ReturnFar, false, &[Code::Retfw, Code::Retfd, Code::Retfq]),
    segment(Instruction::RetFarImmw, ReturnFar, false, &[Code::Retfw_imm16, Code::Retfd_imm16, Code::Retfq_imm16]),
    segment(Instruction::Iret, InterruptReturn, false, &[Code::Iretw]),
    segment(Instruction::Iretd, InterruptReturn, false, &[Code::Iretd]),
    segment(Instruction::Iretq, InterruptReturn, false, &[Code::Iretq]),
];
