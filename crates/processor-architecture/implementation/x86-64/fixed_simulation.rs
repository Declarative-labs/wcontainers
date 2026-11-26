//! The fixed forms' outcomes; only the cache line flushes and PTWRITE reach their memory operand.

use iced_x86::{Code, Instruction as Decoded};

use super::{Memory, check_access, locate};
use crate::fixed::{FixedOutcome, find_fixed_form};
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Flags, Instruction, Processor};

/// The vectors whose gates Linux lets ring 3 reach with INT n, besides its system call's.
const BREAKPOINT_VECTOR: u8 = 3;
const OVERFLOW_VECTOR: u8 = 4;
/// An IDT vector's error code: its index in the table, with the bit that names the IDT.
const INTERRUPT_TABLE_BIT: u32 = 2;
const GATE_BYTES: u32 = 8;

pub(super) fn execute_fixed(
    instruction: Instruction,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    let form = find_fixed_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no fixed form"));
    match form.outcome {
        FixedOutcome::GeneralProtection if is_active(Bug::PrivilegedOperandFormsInvalidOpcode) => Err(Exception::InvalidOpcode),
        FixedOutcome::GeneralProtection if is_active(Bug::SupervisorSaveChecksOperandFirst) && is_supervisor_save(decoded.code()) => {
            check_access(processor.flags, memory, locate(processor, decoded), 1, Access::Read)?;
            Err(Exception::GeneralProtection { error_code: 0 })
        }
        FixedOutcome::GeneralProtection if is_active(Bug::TransactionEndCompletes) && instruction == Instruction::Xend => Ok(()),
        FixedOutcome::GeneralProtection
            if is_active(Bug::TimeStampCounterUnrestricted) && matches!(instruction, Instruction::Rdtsc | Instruction::Rdtscp) =>
        {
            Ok(())
        }
        FixedOutcome::GeneralProtection => Err(Exception::GeneralProtection { error_code: 0 }),
        FixedOutcome::InvalidOpcode => Err(Exception::InvalidOpcode),
        FixedOutcome::SoftwareInterrupt if decoded.code() == Code::Int1 => Err(Exception::Debug { status: 0 }),
        FixedOutcome::SoftwareInterrupt => match decoded.immediate8() {
            BREAKPOINT_VECTOR => Err(Exception::Breakpoint),
            OVERFLOW_VECTOR if !is_active(Bug::InterruptOverflowGeneralProtection) => Err(Exception::Overflow),
            vector => {
                let table_bit = if is_active(Bug::InterruptErrorCodeWithoutTableBit) { 0 } else { INTERRUPT_TABLE_BIT };
                Err(Exception::GeneralProtection { error_code: u32::from(vector) * GATE_BYTES + table_bit })
            }
        },
        FixedOutcome::NoOperation if is_active(Bug::NoOperationReadsItsOperand) && decoded.op_count() > 0 => {
            let reads = (0..decoded.op_count()).any(|index| decoded.op_kind(index) == iced_x86::OpKind::Memory);
            if reads {
                check_access(processor.flags, memory, locate(processor, decoded), 1, Access::Read)?;
            }
            Ok(())
        }
        FixedOutcome::NoOperation | FixedOutcome::WritesUndecided => Ok(()),
        FixedOutcome::WritesRandom if is_active(Bug::RandomLeavesStatusFlags) => Ok(()),
        FixedOutcome::WritesRandom => {
            let carry = if is_active(Bug::RandomClearsCarry) { Flags::empty() } else { Flags::CARRY };
            processor.flags = processor.flags.difference(Flags::STATUS).union(carry);
            Ok(())
        }
        FixedOutcome::OutsideTransaction => {
            let zero = if is_active(Bug::TransactionTestClearsZero) { Flags::empty() } else { Flags::ZERO };
            processor.flags = processor.flags.difference(Flags::STATUS).union(zero);
            Ok(())
        }
        FixedOutcome::TouchesByte => {
            let access = if is_active(Bug::FlushChecksWrite) { Access::Write } else { Access::Read };
            check_access(processor.flags, memory, locate(processor, decoded), 1, access)
        }
        FixedOutcome::SystemEnter if is_active(Bug::SystemEnterAdvances) => Err(Exception::SystemCall),
        FixedOutcome::SystemEnter => Err(Exception::SystemEnter),
        FixedOutcome::ReadsOperand => {
            let width = decoded.memory_size().size() as u64;
            check_access(processor.flags, memory, locate(processor, decoded), width, Access::Read)
        }
    }
}

/// Whether the code is XSAVES's or XRSTORS's.
fn is_supervisor_save(code: Code) -> bool {
    matches!(code, Code::Xsaves_mem | Code::Xsaves64_mem | Code::Xrstors_mem | Code::Xrstors64_mem)
}
