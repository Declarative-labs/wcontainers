//! The instructions whose manual documents one behavior for every case, each simulated by its class's skeleton.

use core::convert::Infallible;

use crate::advanced_simd::{
    AdvancedSimdOperation, simulate_advanced_simd, to_advanced_simd_operation,
};
use crate::branch::{BranchOperation, simulate_branch, to_branch_operation};
use crate::cryptography::{
    CryptographyOperation, simulate_cryptography, to_cryptography_operation,
};
use crate::encoding::IMM16;
use crate::floating_point::{
    FloatingPointOperation, simulate_floating_point, to_floating_point_operation,
};
use crate::integer::{ImplicitInputs, IntegerOperation, simulate_integer, to_integer_operation};
use crate::memory_tagging::{TagOperation, simulate_tag_operation, to_tag_operation};
use crate::pointer_authentication::{
    PointerAuthenticationOperation, simulate_pointer_authentication,
    to_pointer_authentication_operation,
};
use crate::scalable::{ScalableOperation, simulate_scalable, to_scalable_operation};
use crate::simd::{VectorOperation, simulate_vector, to_vector_operation};
use crate::{Exception, ExclusiveMonitor, Instruction, Processor};

/// An instruction whose manual leaves no choice, so its documented behavior is its only one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DocumentedInstruction(Instruction<Infallible>);

/// What the skeleton of each class computes for one instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocumentedOperation {
    Integer(IntegerOperation),
    Branch(BranchOperation),
    FloatingPoint(FloatingPointOperation),
    Vector(VectorOperation),
    PointerAuthentication(PointerAuthenticationOperation),
    AdvancedSimd(AdvancedSimdOperation),
    /// An SVE instruction of `scalable`.
    Scalable(ScalableOperation),
    Cryptography(CryptographyOperation),
    MemoryTagging(TagOperation),
    /// A hint or barrier, which changes nothing a single instruction's state shows.
    NoOperation,
    /// Undefined at EL0 whatever its operands.
    Undefined,
    /// SVC: raises the supervisor call its immediate names.
    SupervisorCall,
}

/// The hints and barriers whose effect a single instruction's state does not show, CLREX, which only opens the exclusive
/// monitor, and the instructions undefined at EL0, none for every other instruction.
const fn to_system_operation(instruction: Instruction<Infallible>) -> Option<DocumentedOperation> {
    use Instruction as I;
    Some(match instruction {
        I::Nop
        | I::Yield
        | I::Sev
        | I::Sevl
        | I::Dgh
        | I::Csdb
        | I::Esb
        | I::Psb
        | I::Tsb
        | I::Gcsb
        | I::Clrbhb
        | I::Stshh
        | I::Shuh
        | I::Stcph
        | I::Bti
        | I::Isb
        | I::Dsb
        | I::Dmb
        | I::Sb
        | I::Clrex
        // Linux leaves SCTLR_EL1.nTWE set, so WFE runs at EL0; it waits for an event or completes at once.
        | I::Wfe
        // A hint: no feature CHKFEAT checks is turned on for the program, so it leaves X16 as it is.
        | I::Chkfeat
        // A range prefetch, whose effect a single instruction's state does not show.
        | I::Rprfm => DocumentedOperation::NoOperation,
        I::Svc => DocumentedOperation::SupervisorCall,
        I::Udf | I::Hvc | I::Smc | I::Hlt | I::Dcps1 | I::Dcps2 | I::Dcps3 | I::Eret | I::Drps => {
            DocumentedOperation::Undefined
        }
        _ => return None,
    })
}

/// The operation of each modeled instruction whose manual leaves no choice, none for every other instruction.
pub const fn to_documented_operation(
    instruction: Instruction<Infallible>,
) -> Option<DocumentedOperation> {
    if let Some(operation) = to_integer_operation(instruction) {
        return Some(DocumentedOperation::Integer(operation));
    }
    if let Some(operation) = to_branch_operation(instruction) {
        return Some(DocumentedOperation::Branch(operation));
    }
    if let Some(operation) = to_floating_point_operation(instruction) {
        return Some(DocumentedOperation::FloatingPoint(operation));
    }
    if let Some(operation) = to_vector_operation(instruction) {
        return Some(DocumentedOperation::Vector(operation));
    }
    if let Some(operation) = to_pointer_authentication_operation(instruction) {
        return Some(DocumentedOperation::PointerAuthentication(operation));
    }
    if let Some(operation) = to_advanced_simd_operation(instruction) {
        return Some(DocumentedOperation::AdvancedSimd(operation));
    }
    if let Some(operation) = to_scalable_operation(instruction) {
        return Some(DocumentedOperation::Scalable(operation));
    }
    if let Some(operation) = to_cryptography_operation(instruction) {
        return Some(DocumentedOperation::Cryptography(operation));
    }
    if let Some(operation) = to_tag_operation(instruction) {
        return Some(DocumentedOperation::MemoryTagging(operation));
    }
    to_system_operation(instruction)
}

/// What the instruction reads beyond the operands of its syntax; nothing for an instruction that is not documented.
pub const fn to_implicit_inputs(instruction: Instruction<Infallible>) -> ImplicitInputs {
    match to_documented_operation(instruction) {
        Some(DocumentedOperation::Integer(operation)) => operation.to_implicit_inputs(),
        Some(DocumentedOperation::Branch(operation)) => ImplicitInputs {
            condition_flags: operation.reads_condition_flags(),
            destination: false,
            program_counter: operation.links(),
            floating_point_control: false,
        },
        Some(DocumentedOperation::FloatingPoint(operation)) => ImplicitInputs {
            condition_flags: operation.reads_condition_flags(),
            destination: false,
            program_counter: false,
            floating_point_control: operation.reads_control(),
        },
        // XPACI and XPACD strip the register they write.
        Some(DocumentedOperation::PointerAuthentication(operation)) => ImplicitInputs {
            condition_flags: false,
            destination: matches!(operation, PointerAuthenticationOperation::Strip),
            program_counter: false,
            floating_point_control: false,
        },
        Some(DocumentedOperation::AdvancedSimd(operation)) => ImplicitInputs {
            condition_flags: false,
            destination: false,
            program_counter: false,
            floating_point_control: operation.reads_control(),
        },
        Some(
            DocumentedOperation::Vector(_)
            | DocumentedOperation::Scalable(_)
            | DocumentedOperation::Cryptography(_)
            | DocumentedOperation::MemoryTagging(_)
            | DocumentedOperation::NoOperation
            | DocumentedOperation::Undefined
            | DocumentedOperation::SupervisorCall,
        )
        | None => ImplicitInputs {
            condition_flags: false,
            destination: false,
            program_counter: false,
            floating_point_control: false,
        },
    }
}

/// Whether the instruction may write NZCV, whatever it reads; false for an instruction that is not documented.
pub const fn writes_condition_flags(instruction: Instruction<Infallible>) -> bool {
    use crate::floating_point::FloatingPointOperation as F;
    use crate::integer::FlagSetting::Sets;
    use IntegerOperation as I;
    matches!(
        to_documented_operation(instruction),
        Some(DocumentedOperation::Integer(
            I::Add(Sets)
                | I::Subtract(Sets)
                | I::AddWithCarry(Sets)
                | I::SubtractWithCarry(Sets)
                | I::And(Sets)
                | I::AndNot(Sets)
                | I::ConditionalCompare(_)
                | I::RotateMaskInsertFlags
                | I::SetFlags { .. }
                | I::InvertCarry
                | I::ConvertFlagsToExternalFormat
                | I::ConvertFlagsFromExternalFormat
        )) | Some(DocumentedOperation::FloatingPoint(
            F::Compare { .. } | F::ConditionalCompare { .. } | F::ConvertToJavaScriptInteger
        )) | Some(DocumentedOperation::MemoryTagging(
            TagOperation::SubtractPointer { sets_flags: true }
        ))
    )
}

impl DocumentedInstruction {
    /// None for an instruction that is not modeled or whose manual leaves a choice.
    pub const fn new(instruction: Instruction<Infallible>) -> Option<Self> {
        match (
            to_documented_operation(instruction),
            instruction.to_record(),
        ) {
            (Some(_), Some(_)) => Some(Self(instruction)),
            _ => None,
        }
    }

    pub const fn instruction(self) -> Instruction<Infallible> {
        self.0
    }

    /// Applies the instruction encoded in the word and returns where the next instruction is when it branches;
    /// changes nothing when it returns an exception.
    pub(crate) fn simulate<LevelSpecific>(
        self,
        processor: &mut Processor<LevelSpecific>,
        word: u32,
    ) -> Result<Option<u64>, Exception> {
        let (Some(operation), Some(record)) = (to_documented_operation(self.0), self.0.to_record())
        else {
            unreachable!("`new` makes only listed instructions with an operation");
        };
        match operation {
            DocumentedOperation::Integer(operation) => {
                simulate_integer(processor, word, record.operands, operation).map(|()| None)
            }
            DocumentedOperation::Branch(operation) => {
                simulate_branch(processor, word, record.operands, operation)
            }
            DocumentedOperation::FloatingPoint(operation) => {
                simulate_floating_point(processor, word, record.operands, operation).map(|()| None)
            }
            DocumentedOperation::Vector(operation) => {
                simulate_vector(processor, word, record.operands, operation).map(|()| None)
            }
            DocumentedOperation::PointerAuthentication(operation) => {
                simulate_pointer_authentication(processor, word, operation).map(|()| None)
            }
            DocumentedOperation::AdvancedSimd(operation) => {
                simulate_advanced_simd(processor, word, operation).map(|()| None)
            }
            DocumentedOperation::Scalable(operation) => {
                simulate_scalable(processor, word, operation).map(|()| None)
            }
            DocumentedOperation::Cryptography(operation) => {
                simulate_cryptography(processor, word, operation).map(|()| None)
            }
            DocumentedOperation::MemoryTagging(operation) => {
                simulate_tag_operation(processor, word, operation).map(|()| None)
            }
            DocumentedOperation::NoOperation if crate::planted::is(257) => {
                Err(Exception::UndefinedInstruction)
            }
            DocumentedOperation::NoOperation if crate::planted::is(260) => {
                Err(Exception::WaitTrapped(crate::WaitInstruction::WaitForEvent))
            }
            DocumentedOperation::NoOperation => {
                const X16: usize = 16;
                if self.0 == Instruction::Clrex {
                    processor.exclusive_monitor = ExclusiveMonitor::Open;
                }
                if crate::planted::is(32) && self.0 == Instruction::Chkfeat {
                    processor.general_purpose_registers[X16] &= !1;
                }
                if crate::planted::is(59) && self.0 == Instruction::Rprfm {
                    let base = crate::registers::read_register(
                        processor,
                        crate::encoding::RN.read(word),
                        crate::operand::RegisterThirtyOne::StackPointer,
                    );
                    if crate::branch::is_outside_lower_addresses(base) {
                        return Err(Exception::DataAbort {
                            status: crate::FaultStatus::TranslationFaultLevel0,
                            access: crate::AccessDirection::Read,
                            address: base,
                        });
                    }
                }
                Ok(None)
            }
            DocumentedOperation::Undefined if crate::planted::is(254) => {
                Err(Exception::SupervisorCall {
                    immediate: IMM16.read(word) as u16,
                })
            }
            DocumentedOperation::Undefined
                if crate::planted::is(255) || crate::planted::is(258) =>
            {
                Ok(None)
            }
            DocumentedOperation::Undefined if crate::planted::is(432) => {
                Err(Exception::Breakpoint {
                    comment: IMM16.read(word) as u16,
                })
            }
            DocumentedOperation::Undefined => Err(Exception::UndefinedInstruction),
            DocumentedOperation::SupervisorCall => Err(Exception::SupervisorCall {
                immediate: IMM16.read(word) as u16,
            }),
        }
    }
}
