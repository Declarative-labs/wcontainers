/// A synchronous exception, with the instruction that raised it still at the program counter.
///
/// TODO: temporary placeholder until every exception is listed: these are the ones the ten instructions raise.
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
    /// An exception no other variant names, kept as ESR_EL1 and FAR_EL1 held it: no behavior simulates one.
    Undecoded {
        syndrome: u64,
        fault_address: u64,
    },
}

/// An abort's [fault status](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D); every page Linux maps is at translation level 3.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaultStatus {
    /// The address lies above the addresses a program can use, which no translation table covers.
    TranslationFaultLevel0,
    TranslationFaultLevel3,
    PermissionFaultLevel3,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessDirection {
    Read,
    Write,
}

/// The [SMTC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=SMTC%2C%20bits%20%5B2%3A0%5D) of a trapped SME instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SmeTrapCode {
    /// SME is turned off where the instruction runs.
    Disabled,
    /// The instruction is legal only in streaming mode.
    NotInStreamingMode,
}
