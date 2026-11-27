use vstd::prelude::*;

verus! {

/// A synchronous exception, with the instruction that raised it still at the program counter, except a supervisor
/// call, which leaves the program counter at the instruction after it, as ELR_EL1 holds it.
///
/// TODO: temporary placeholder until every exception is listed: these are the ones the modeled instructions raise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exception {
    /// The encoding is unallocated, or a behavior leaves it undefined.
    UndefinedInstruction,
    ProgramCounterAlignmentFault {
        address: u64,
    },
    StackPointerAlignmentFault,
    /// The instruction's own fetch faulted.
    InstructionAbort {
        status: FaultStatus,
        address: u64,
    },
    DataAbort {
        status: FaultStatus,
        access: AccessDirection,
        address: u64,
    },
    Breakpoint {
        comment: u16,
    },
    SmeTrap(SmeTrapCode),
    SupervisorCall {
        immediate: u16,
    },
    /// A WFI or WFE that EL1 traps, as [SCTLR_EL1.nTWI](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/SCTLR-EL1--System-Control-Register--EL1-#:~:text=nTWI%2C%20bit%20%5B16%5D) or nTWE clear makes it.
    WaitTrapped(WaitInstruction),
    /// An MRS, MSR or system instruction EL1 traps, with the fields the [syndrome](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=Op0%2C%20bits%20%5B21%3A20%5D) gives it.
    SystemAccessTrapped(SystemAccess),
    /// An exception no other variant names, kept as ESR_EL1 and FAR_EL1 held it: no behavior simulates one.
    Undecoded {
        syndrome: u64,
        fault_address: u64,
    },
}

impl Exception {
    /// Whether the exception is taken after its instruction, so that it returns to the next one.
    pub const fn is_taken_after_instruction(self) -> (taken: bool)
        ensures
            taken == self is SupervisorCall,
    {
        matches!(self, Self::SupervisorCall { .. })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WaitInstruction {
    WaitForInterrupt,
    WaitForEvent,
}

/// The encoding fields of a system register access or system instruction, and whether it reads (MRS, SYSL).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemAccess {
    pub op0: u8,
    pub op1: u8,
    pub crn: u8,
    pub crm: u8,
    pub op2: u8,
    pub rt: u8,
    pub direction: AccessDirection,
}

/// An abort's [fault status](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D); every page Linux maps is at translation level 3.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaultStatus {
    /// An access outside the lower half's 48 bits of addresses: the upper half, which [E0PD1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TCR-EL1--Translation-Control-Register--EL1-#:~:text=E0PD1%2C%20bit%20%5B56%5D) keeps from EL0 as Linux sets it, or a lower address above them.
    TranslationFaultLevel0,
    TranslationFaultLevel3,
    PermissionFaultLevel3,
    /// The access's address is not aligned as the instruction requires.
    AlignmentFault,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum AccessDirection {
    Read,
    Write,
}

/// The [SMTC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=SMTC%2C%20bits%20%5B2%3A0%5D) of a trapped SME instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64)
)]
pub enum SmeTrapCode {
    /// SME is turned off where the instruction runs.
    Disabled,
    /// The instruction is legal only in streaming mode.
    NotInStreamingMode,
}

} // verus!
