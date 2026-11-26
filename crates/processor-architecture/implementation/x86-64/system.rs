//! PUSHF, POPF and ENTER, as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! (vol. 2) gives them in 64-bit mode at ring 3 under IOPL 0, where every access to the stack goes through SS.
//!
//! PUSHF pushes RFLAGS with IF and the reserved bit 1 set, as Linux runs a program, and RF and VM clear. POPF changes
//! only the flags a program at ring 3 may change: IF, IOPL, VIF and VIP stay, RF and VM are cleared, and the 16-bit
//! form leaves the flags above bit 15 as they were. TF, which POPF may set too, is not in the state.

use alloc::vec::Vec;

use iced_x86::Code;

use crate::PreciseInstruction;
use crate::behavior::StoreAcrossFault;
use crate::forms::Instruction;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemOperation {
    PushFlags,
    PopFlags,
    /// ENTER: RBP pushed, the frame pointers of the outer levels copied, the frame pointer pushed, RBP set to the new
    /// frame and RSP lowered by the frame's size.
    Enter,
    /// SYSCALL: RCX and R11 written, then the kernel entered, which ends the program's instruction as an exception does.
    SystemCall,
    /// JCXZ: the jump taken where CX is zero. It needs the 16-bit address size, which 64-bit mode does not have (0x67
    /// makes 0xE3 JECXZ there), so no byte string encodes it and no state runs it.
    JumpIfCountZero,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemForm {
    pub instruction: Instruction,
    pub operation: SystemOperation,
    /// By operand size, 16 bits first.
    pub codes: &'static [Code],
}

/// What ENTER checks of the stack below its frame, which the manual describes as a page fault "if a write using the
/// final value of the stack pointer (within the current stack segment) would cause a page fault".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameProbe {
    /// Nothing below the frame pointer's push.
    Unchecked,
    /// A write of an operand at the final RSP.
    OperandAtFinalPointer,
}

/// A form's behavior: what a store that faults part-way writes, and for ENTER what it checks below the frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemBehavior {
    pub across_fault: StoreAcrossFault,
    pub probe: FrameProbe,
}

/// The row of a form of this family, or `None` for any other form.
pub fn find_system_form(instruction: Instruction) -> Option<&'static SystemForm> {
    SYSTEM_FORMS.iter().find(|form| form.instruction == instruction)
}

const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];
const PROBES: [FrameProbe; 2] = [FrameProbe::Unchecked, FrameProbe::OperandAtFinalPointer];

/// Both rules of a store that faults part-way for the forms that push, and both of ENTER's check below its frame.
pub(crate) fn list_system_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_system_form(instruction) else {
        return Vec::new();
    };
    let stores = matches!(form.operation, SystemOperation::PushFlags | SystemOperation::Enter);
    let across: &[StoreAcrossFault] = if stores { &ACROSS_FAULT } else { &ACROSS_FAULT[..1] };
    let probes: &[FrameProbe] = if form.operation == SystemOperation::Enter { &PROBES } else { &PROBES[..1] };
    across
        .iter()
        .flat_map(|&across_fault| {
            probes.iter().map(move |&probe| PreciseInstruction::System { instruction, behavior: SystemBehavior { across_fault, probe } })
        })
        .collect()
}

const fn system(instruction: Instruction, operation: SystemOperation, codes: &'static [Code]) -> SystemForm {
    SystemForm { instruction, operation, codes }
}

/// Every form of this family modeled.
pub static SYSTEM_FORMS: &[SystemForm] = &[
    system(Instruction::Pushf, SystemOperation::PushFlags, &[Code::Pushfw]),
    system(Instruction::Pushfq, SystemOperation::PushFlags, &[Code::Pushfq]),
    system(Instruction::Popf, SystemOperation::PopFlags, &[Code::Popfw]),
    system(Instruction::Popfq, SystemOperation::PopFlags, &[Code::Popfq]),
    system(Instruction::EnterImmwImmb, SystemOperation::Enter, &[Code::Enterw_imm16_imm8, Code::Enterq_imm16_imm8]),
    system(Instruction::JcxzRelbrb, SystemOperation::JumpIfCountZero, &[Code::Jcxz_rel8_16]),
    system(Instruction::Syscall, SystemOperation::SystemCall, &[Code::Syscall]),
];
