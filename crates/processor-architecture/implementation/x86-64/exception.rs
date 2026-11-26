/// An exception that ended an instruction, as the frame the chip pushes shows it.
///
/// After a fault the instruction pointer is the instruction's own; after a trap (a breakpoint, a software interrupt)
/// it is the next instruction's.
///
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exception {
    /// #DE.
    DivideError,
    /// #DB other than the trap after a single step, with the DR6 bits it set.
    Debug { status: u64 },
    /// #BP, from INT3 or INT 3.
    Breakpoint,
    /// #OF.
    Overflow,
    /// #BR.
    BoundRange,
    /// #UD: the encoding is undefined, or the chip lacks the instruction, or a behavior leaves it undefined.
    InvalidOpcode,
    /// #NM.
    DeviceNotAvailable,
    /// #TS.
    InvalidTss { error_code: u32 },
    /// #NP.
    SegmentNotPresent { error_code: u32 },
    /// #SS, which a non-canonical address based on RSP or RBP raises with error code 0.
    StackSegmentFault { error_code: u32 },
    /// #GP.
    GeneralProtection { error_code: u32 },
    /// #PF at ring 3: the lowest address the access could not reach, which CR2 holds.
    PageFault {
        access: Access,
        /// Whether the page was present, so that the fault is a protection fault, not a translation fault.
        page_present: bool,
        address: u64,
    },
    /// #MF.
    X87FloatingPoint,
    /// #AC.
    AlignmentCheck,
    /// #MC.
    MachineCheck,
    /// #XM.
    SimdFloatingPoint,
    /// #VE.
    Virtualization,
    /// #CP.
    ControlProtection { error_code: u32 },
    /// INT n through a gate a program may use, such as Linux's 0x80.
    SoftwareInterrupt { vector: u8 },
    /// SYSCALL's entry to the kernel, after it wrote the next instruction's address into RCX and RFLAGS into R11.
    SystemCall,
}

/// What the access that faulted was doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Access {
    Read,
    /// A write, or a read-modify-write.
    Write,
    InstructionFetch,
}

impl Exception {
    /// Whether the chip reports the exception after the instruction, so that the instruction pointer moves past it.
    pub const fn is_trap(self) -> bool {
        matches!(
            self,
            Self::Breakpoint | Self::Overflow | Self::SoftwareInterrupt { .. } | Self::Debug { .. } | Self::SystemCall
        )
    }
}
