//! The legacy forms that push and pop through RSP, the near CALL and RET, the indirect JMP from memory, LEAVE, XLAT
//! and MOV from a segment register, as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) gives them in 64-bit mode on Intel: a near
//! branch ignores the operand-size prefix, and a stack access through RSP that is not canonical raises #SS(0).

use alloc::vec::Vec;

use iced_x86::Code;

use crate::behavior::StoreAcrossFault;
use crate::forms::Instruction;
use crate::integer::OperandPlace;
use crate::PreciseInstruction;

/// What a stack form does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StackOperation {
    /// PUSH of a register, memory or an immediate, sign-extended.
    Push,
    /// PUSH FS and PUSH GS.
    PushSegment,
    /// POP to a register or memory; to memory the address is formed with RSP already raised.
    Pop,
    /// CALL rel32.
    CallRelative,
    /// CALL r/m64.
    CallIndirect,
    /// JMP m64.
    JumpIndirect,
    /// RET, which then releases the immediate's bytes for RET imm16.
    Return,
    /// LEAVE: RSP from RBP, then RBP popped.
    Leave,
    /// XLAT: AL from the byte at RBX plus AL, unsigned.
    Translate,
    /// MOV r/m, Sreg: the selector, zero-extended into a 32-bit or 64-bit register.
    MoveSegment,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StackForm {
    pub instruction: Instruction,
    pub operation: StackOperation,
    pub place: OperandPlace,
    /// By operand size, smallest first.
    pub codes: &'static [Code],
}

/// What PUSH of a segment register writes with a 64-bit operand size, which the manual leaves to the chip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::stack)
)]
pub enum SegmentPush {
    /// The selector zero-extended to eight bytes.
    ZeroExtended,
    /// The selector's two bytes alone, the six above left as they were, as recent Intel chips do.
    SelectorOnly,
}

/// A stack form's behavior: what a store that faults part-way writes, and what PUSH of a segment register stores.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::stack)
)]
pub struct StackBehavior {
    pub across_fault: StoreAcrossFault,
    pub segment_push: SegmentPush,
}

impl StackForm {
    /// Whether the form pushes, so that its stack access is a store below RSP.
    pub const fn pushes(&self) -> bool {
        matches!(self.operation, StackOperation::Push | StackOperation::PushSegment | StackOperation::CallRelative | StackOperation::CallIndirect)
    }

    /// Whether the form reads the stack at RSP.
    pub const fn pops(&self) -> bool {
        matches!(self.operation, StackOperation::Pop | StackOperation::Return)
    }

    /// Whether the form writes its explicit memory operand.
    pub fn writes_memory(&self) -> bool {
        self.place != OperandPlace::Register && matches!(self.operation, StackOperation::Pop | StackOperation::MoveSegment)
    }

    /// The bytes of the explicit memory operand at the code.
    pub fn to_memory_bytes(code: Code) -> u64 {
        match code {
            Code::Xlat_m8 => 1,
            Code::Mov_r32m16_Sreg | Code::Push_rm16 | Code::Pop_rm16 => 2,
            _ => 8,
        }
    }

    /// The bytes the stack access reaches at the code.
    pub fn to_stack_bytes(code: Code) -> u64 {
        match code {
            Code::Push_rm16 | Code::Pushw_imm8 | Code::Push_imm16 | Code::Pushw_FS | Code::Pushw_GS | Code::Pop_r16
            | Code::Pop_rm16 | Code::Leavew => 2,
            _ => 8,
        }
    }
}

/// The row of a stack form, or `None` for any other form.
pub fn find_stack_form(instruction: Instruction) -> Option<&'static StackForm> {
    STACK_FORMS.iter().find(|form| form.instruction == instruction)
}

const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];
const SEGMENT_PUSH: [SegmentPush; 2] = [SegmentPush::ZeroExtended, SegmentPush::SelectorOnly];

/// Both rules of a store that faults part-way for a form that stores where a state can make it fault part-way, and
/// both of PUSH of a segment register. Beside an explicit memory operand the stack lies on a page of its own, so a
/// push there never straddles one; its rule is the register form's.
pub(crate) fn list_stack_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_stack_form(instruction) else {
        return Vec::new();
    };
    let stores = form.pushes() && form.place == OperandPlace::Register || form.writes_memory();
    let across: &[StoreAcrossFault] = if stores { &ACROSS_FAULT } else { &ACROSS_FAULT[..1] };
    let segment: &[SegmentPush] = if form.operation == StackOperation::PushSegment { &SEGMENT_PUSH } else { &SEGMENT_PUSH[..1] };
    let mut behaviors = Vec::new();
    for &across_fault in across {
        for &segment_push in segment {
            // A selector's two bytes alone never straddle a page in a state, so both rules of a fault are one there.
            if segment_push == SegmentPush::SelectorOnly && across_fault != ACROSS_FAULT[0] {
                continue;
            }
            behaviors.push(PreciseInstruction::Stack { instruction, behavior: StackBehavior { across_fault, segment_push } });
        }
    }
    behaviors
}

const fn stack(instruction: Instruction, operation: StackOperation, place: OperandPlace, codes: &'static [Code]) -> StackForm {
    StackForm { instruction, operation, place, codes }
}

use OperandPlace::{Memory, Register};
use StackOperation::{
    CallIndirect, CallRelative, JumpIndirect, Leave, MoveSegment, Pop, Push, PushSegment, Return, Translate,
};

/// Every stack form modeled.
pub static STACK_FORMS: &[StackForm] = &[
    stack(Instruction::PushGprvFfr6, Push, Register, &[Code::Push_rm16, Code::Push_rm64]),
    stack(Instruction::PushMemv, Push, Memory, &[Code::Push_rm16, Code::Push_rm64]),
    stack(Instruction::PushImmb, Push, Register, &[Code::Pushw_imm8, Code::Pushq_imm8]),
    stack(Instruction::PushImmz, Push, Register, &[Code::Push_imm16, Code::Pushq_imm32]),
    stack(Instruction::PushFs, PushSegment, Register, &[Code::Pushw_FS, Code::Pushq_FS]),
    stack(Instruction::PushGs, PushSegment, Register, &[Code::Pushw_GS, Code::Pushq_GS]),
    stack(Instruction::PopGprv58, Pop, Register, &[Code::Pop_r16, Code::Pop_r64]),
    stack(Instruction::PopGprv8f, Pop, Register, &[Code::Pop_rm16, Code::Pop_rm64]),
    stack(Instruction::PopMemv, Pop, Memory, &[Code::Pop_rm16, Code::Pop_rm64]),
    stack(Instruction::CallNearRelbrd, CallRelative, Register, &[Code::Call_rel32_64]),
    stack(Instruction::CallNearGprv, CallIndirect, Register, &[Code::Call_rm64]),
    stack(Instruction::CallNearMemv, CallIndirect, Memory, &[Code::Call_rm64]),
    stack(Instruction::JmpMemv, JumpIndirect, Memory, &[Code::Jmp_rm64]),
    stack(Instruction::RetNear, Return, Register, &[Code::Retnq]),
    stack(Instruction::RetNearImmw, Return, Register, &[Code::Retnq_imm16]),
    stack(Instruction::Leave, Leave, Register, &[Code::Leavew, Code::Leaveq]),
    stack(Instruction::Xlat, Translate, Memory, &[Code::Xlat_m8]),
    stack(Instruction::MovGprvSeg, MoveSegment, Register, &[Code::Mov_rm16_Sreg, Code::Mov_r32m16_Sreg, Code::Mov_r64m16_Sreg]),
    stack(Instruction::MovMemwSeg, MoveSegment, Memory, &[Code::Mov_r32m16_Sreg]),
];
