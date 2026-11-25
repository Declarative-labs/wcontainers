//! x86-64 as Linux runs a 64-bit program at ring 3: IF set, IOPL 0, CR0.AM set, UMIP and FSGSBASE on, no XSAVE
//! state beyond SSE yet.
//!
//! A program's addresses are [`ADDRESSABLE`](ProcessorArchitecture::ADDRESSABLE), the lower canonical half. Every
//! canonical address above it is the kernel's: present to the chip and refused to ring 3, so an access there is a
//! page fault on a present page, as on a kernel that maps its whole half. A state gives permissions and bytes only
//! inside the lower half.
//!
//! One state type serves every x86-64 chip. State that only some chips have (AVX-512, APX, AMX) is a field of every
//! state once it is modeled, sized for the largest chip; a chip without the feature is a chip whose instructions
//! that use it have the behavior that raises #UD, so its source never changes those fields. The privileged forms are
//! already in the list of forms, where they fault at ring 3, so there is no privilege level type: a ring 0 level
//! would add state and behaviors, not instructions.

#![no_std]

extern crate alloc;

use alloc::vec::{self, Vec};
use core::ops::Range;

use iced_x86::Code;
use wcontainers_processor_architecture::ProcessorArchitecture;

mod behavior;
pub mod branch;
mod cpuid;
mod cpuid_recording;
mod exception;
mod flags;
pub mod float;
// Every form of the list, generated from Intel XED by the `generate_forms` example.
#[path = "forms/instruction.rs"]
mod forms;
pub mod integer;
pub mod memory;
pub mod planted;
mod simulation;
pub mod vector;
#[cfg(test)]
mod tests;

pub use behavior::{
    BitScanFlags, CarryBeyondWidth, CpuidReading, DoubleShiftFill, ExchangeMismatch, FlagRule, IntegerBehavior, OverflowAfterShift,
    ScanOfZero, SixteenBitSwap, StoreAcrossFault, UndefinedAuxiliaryCarry, UndefinedFlags, UndefinedResultFlags,
    UndefinedStatusFlags,
};
pub use cpuid::{CpuidLeaf, CpuidTable, XEON_PLATINUM_8488C_UNDER_KVM};
pub use exception::{Access, Exception};
pub use flags::Flags;
pub use forms::{INSTRUCTIONS, Instruction, XED_VERSION, list_instructions as list_forms};

/// The bits of a linear address under 4-level paging; LA57 is not modeled.
pub const VIRTUAL_ADDRESS_BITS: u32 = 48;
/// RAX, RCX, RDX, RBX, RSP, RBP, RSI, RDI and R8 to R15.
pub const GENERAL_PURPOSE_REGISTERS: usize = 16;
/// XMM0 to XMM15.
pub const VECTOR_REGISTERS: usize = 16;
/// The number of RSP among the general registers.
pub const STACK_POINTER: usize = 4;
/// What [MXCSR](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) holds after reset and in a new Linux program: every exception masked, round to nearest.
pub const MXCSR_DEFAULT: u32 = 0x1F80;

/// The forms of the first slice that are not legacy integer forms of [`integer::INTEGER_FORMS`].
///
/// TODO: temporary placeholder until every form is modeled: `INSTRUCTIONS` lists all 8,881.
pub const FIRST_SLICE: [Instruction; 4] = [
    Instruction::PushGprv50,
    Instruction::Ud2,
    Instruction::Int3,
    Instruction::Cpuid,
];

/// The forms the manuals give #GP(0) at ring 3 whatever their operands, and the iced-x86 `Code` of each operand
/// size, 32 bits first: SWAPGS and SYSRET for CPL other than 0, SYSEXIT for CPL other than 0 (Linux sets
/// IA32_SYSENTER_CS, so that check comes second), CLI and STI for CPL above IOPL, which Linux leaves 0; CLTS, HLT,
/// INVD, WBINVD, RDMSR and WRMSR for CPL other than 0, and RDPMC for CPL other than 0 with CR4.PCE clear, as
/// Linux leaves it for a program that has not mapped a performance counter.
pub const PRIVILEGED_FORMS: [(Instruction, &[Code]); 13] = [
    (Instruction::Swapgs, &[Code::Swapgs]),
    (Instruction::Sysexit, &[Code::Sysexitd, Code::Sysexitq]),
    (Instruction::Sysret, &[Code::Sysretd]),
    (Instruction::Sysret64, &[Code::Sysretq]),
    (Instruction::Cli, &[Code::Cli]),
    (Instruction::Sti, &[Code::Sti]),
    (Instruction::Clts, &[Code::Clts]),
    (Instruction::Hlt, &[Code::Hlt]),
    (Instruction::Invd, &[Code::Invd]),
    (Instruction::Wbinvd, &[Code::Wbinvd]),
    (Instruction::Rdmsr, &[Code::Rdmsr]),
    (Instruction::Wrmsr, &[Code::Wrmsr]),
    (Instruction::Rdpmc, &[Code::Rdpmc]),
];

/// A 64-bit program at ring 3.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct X86_64;

impl ProcessorArchitecture for X86_64 {
    type ProcessorState = Processor;
    type PagePermission = PagePermission;
    type Instruction = Instruction;
    type PreciseInstruction = PreciseInstruction;
    type Instructions = vec::IntoIter<Instruction>;
    type KnownBehaviors = vec::IntoIter<PreciseInstruction>;

    /// The lower canonical half, which Linux gives a program.
    const ADDRESSABLE: Range<u64> = 0..1 << (VIRTUAL_ADDRESS_BITS - 1);
    const DEFAULT_PERMISSION: PagePermission = PagePermission::Unmapped;

    /// Lists the modeled forms, a subset of [`INSTRUCTIONS`].
    fn list_instructions() -> vec::IntoIter<Instruction> {
        let modeled: Vec<Instruction> = FIRST_SLICE
            .into_iter()
            .chain(PRIVILEGED_FORMS.map(|(instruction, _)| instruction))
            .chain(integer::INTEGER_FORMS.iter().map(|form| form.instruction))
            .chain(vector::VECTOR_FORMS.iter().map(|form| form.instruction))
            .chain(float::FLOAT_FORMS.iter().map(|form| form.instruction))
            .chain(branch::BRANCH_FORMS.iter().map(|form| form.instruction))
            .collect();
        modeled.into_iter()
    }

    fn list_known_behaviors(instruction: Instruction) -> vec::IntoIter<PreciseInstruction> {
        behavior::list_behaviors(instruction)
    }
}

/// The registers a program at ring 3 reads and writes, and the exception its last instruction raised.
///
/// TODO: temporary placeholder until the rest of the state is designed: the x87 stack with its control, status and
/// tag words and MMX; YMM and ZMM beyond 128 bits, XMM16 to XMM31 and the opmask registers; APX's R16 to R31; AMX's
/// tile configuration and tiles; PKRU; CET's shadow stack pointer; the segment selectors; the trap flag. The chip
/// source checks that an instruction leaves the x87 state and the selectors as they were.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Processor {
    /// In the order of their encodings, RSP at [`STACK_POINTER`].
    pub general_purpose_registers: [u64; GENERAL_PURPOSE_REGISTERS],
    /// Canonical: no instruction leaves a program at a non-canonical address on Intel.
    pub instruction_pointer: u64,
    /// Only the bits [`Flags`] names; a state with any other bit set is not a state of this architecture.
    pub flags: Flags,
    /// Canonical, as WRFSBASE requires.
    pub fs_base: u64,
    /// Canonical, as WRGSBASE requires.
    pub gs_base: u64,
    /// No reserved bit set, as LDMXCSR requires.
    pub mxcsr: u32,
    /// The low 128 bits of each vector register.
    pub vector_registers: [u128; VECTOR_REGISTERS],
    /// None when the last instruction completed.
    pub exception: Option<Exception>,
}

/// Every register zero, the flags clear, MXCSR at its default and no exception.
pub const ZERO_PROCESSOR: Processor = Processor {
    general_purpose_registers: [0; GENERAL_PURPOSE_REGISTERS],
    instruction_pointer: 0,
    flags: Flags::empty(),
    fs_base: 0,
    gs_base: 0,
    mxcsr: MXCSR_DEFAULT,
    vector_registers: [0; VECTOR_REGISTERS],
    exception: None,
};

/// What the running program may do with a page; memory that was never given a permission is unmapped.
///
/// A page a program maps with no access is not present to the chip, as Linux maps `PROT_NONE`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PagePermission {
    Unmapped,
    Mapped {
        data_access: DataAccess,
        instruction_fetch: InstructionFetch,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataAccess {
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InstructionFetch {
    Forbidden,
    Allowed,
}

/// An instruction form with one behavior for every case the manuals leave open or a chip does its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreciseInstruction {
    PushGprv50 { across_fault: StoreAcrossFault },
    Ud2,
    Int3,
    Cpuid(CpuidReading),
    /// A form of [`vector::VECTOR_FORMS`], whose results the manuals define whole.
    Vector(Instruction),
    /// A form of [`float::FLOAT_FORMS`], whose results IEEE 754 and the manuals define whole.
    Float(Instruction),
    /// A form of [`PRIVILEGED_FORMS`].
    Privileged(Instruction),
    /// A form of [`branch::BRANCH_FORMS`].
    Branch(Instruction),
    /// A form of [`integer::INTEGER_FORMS`].
    Integer {
        instruction: Instruction,
        behavior: IntegerBehavior,
    },
}
