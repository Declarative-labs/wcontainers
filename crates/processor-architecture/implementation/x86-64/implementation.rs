//! x86-64 as Linux runs a 64-bit program at ring 3: IF set, IOPL 0, CR0.AM set, UMIP, FSGSBASE, OSXSAVE and PKE
//! on, and XCR0 enabling x87, SSE and AVX state, which the state holds.
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
use core::{iter, slice};

use iced_x86::Code;
use wcontainers_processor_architecture::ProcessorArchitecture;

pub mod avx;
mod behavior;
pub mod branch;
mod cpuid;
mod cpuid_recording;
mod exception;
pub mod extension;
pub mod fixed;
mod flags;
pub mod float;
// Every form of the list, generated from Intel XED by the `generate_forms` example.
#[path = "forms/instruction.rs"]
mod forms;
pub mod integer;
pub mod memory;
pub mod mmx;
pub mod planted;
mod simulation;
pub mod sse;
pub mod stack;
pub mod vector;
pub mod x87;
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
pub use forms::{
    CPUID_REQUIREMENTS, CpuidRegister, CpuidRequirement, INSTRUCTIONS, Instruction, SAMPLE_ENCODINGS, XED_VERSION,
    list_instructions as list_forms,
};

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
/// Linux leaves it for a program that has not mapped a performance counter; XSETBV for CPL other than 0.
pub const PRIVILEGED_FORMS: [(Instruction, &[Code]); 14] = [
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
    (Instruction::Xsetbv, &[Code::Xsetbv]),
];

/// The CPUID features every x86-64 chip has: long mode, and the x87, CMOV, MMX, FXSAVE, SSE and SSE2 that AMD's
/// [AMD64 Architecture Programmer's Manual](https://docs.amd.com/v/u/en-US/24592_3.24) vol. 3 appendix D and the
/// [x86-64 psABI](https://gitlab.com/x86-psABIs/x86-64-ABI) baseline give it.
const BASELINE_FEATURES: [CpuidRequirement; 7] = [
    forms::REQUIRES_INTEL64,
    forms::REQUIRES_FPU,
    forms::REQUIRES_CMOV,
    forms::REQUIRES_MMX,
    forms::REQUIRES_FXSAVE,
    forms::REQUIRES_SSE,
    forms::REQUIRES_SSE2,
];

/// The forms that take input from outside the processor and memory, the bytes a device gives: VIA's XSTORE.
pub const FORBIDDEN_FORMS: [Instruction; 2] = [Instruction::Xstore, Instruction::RepXstore];

/// Returns whether a chip may lack the form: it needs a CPUID feature beyond [`BASELINE_FEATURES`].
pub fn is_optional(instruction: Instruction) -> bool {
    let alternatives = CPUID_REQUIREMENTS[instruction as usize];
    !alternatives.is_empty()
        && !alternatives
            .iter()
            .any(|requirements| requirements.iter().all(|requirement| BASELINE_FEATURES.contains(requirement)))
}

/// Returns the modeled forms, in the order the earlier lists gave them.
pub fn list_modeled_forms() -> Vec<Instruction> {
    FIRST_SLICE
        .into_iter()
        .chain(PRIVILEGED_FORMS.map(|(instruction, _)| instruction))
        .chain(integer::INTEGER_FORMS.iter().map(|form| form.instruction))
        .chain(vector::VECTOR_FORMS.iter().map(|form| form.instruction))
        .chain(float::FLOAT_FORMS.iter().map(|form| form.instruction))
        .chain(branch::BRANCH_FORMS.iter().map(|form| form.instruction))
        .chain(vector::VEX_FORMS.iter().map(|form| form.instruction))
        .chain(sse::SSE_FORMS.iter().map(|form| form.instruction))
        .chain(avx::AVX_FORMS.iter().map(|form| form.instruction))
        .chain(extension::EXTENSION_FORMS.iter().map(|form| form.instruction))
        .chain(fixed::FIXED_FORMS.iter().map(|form| form.instruction))
        .chain(mmx::MMX_FORMS.iter().map(|form| form.instruction))
        .chain(x87::X87_FORMS.iter().map(|form| form.instruction))
        .chain(stack::STACK_FORMS.iter().map(|form| form.instruction))
        .collect()
}

/// A 64-bit program at ring 3.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct X86_64;

impl ProcessorArchitecture for X86_64 {
    type ProcessorState = Processor;
    type ProcessorStateExpectation = ProcessorExpectation;
    type PagePermission = PagePermission;
    type Instruction = Instruction;
    type PreciseInstruction = PreciseInstruction;
    type Instructions = iter::Copied<slice::Iter<'static, Instruction>>;
    type KnownBehaviors = vec::IntoIter<PreciseInstruction>;

    /// The lower canonical half, which Linux gives a program.
    const ADDRESSABLE: Range<u64> = 0..1 << (VIRTUAL_ADDRESS_BITS - 1);
    const DEFAULT_PERMISSION: PagePermission = PagePermission::Unmapped;

    /// Lists every form of [`INSTRUCTIONS`].
    fn list_instructions() -> iter::Copied<slice::Iter<'static, Instruction>> {
        INSTRUCTIONS.iter().copied()
    }

    fn list_known_behaviors(instruction: Instruction) -> vec::IntoIter<PreciseInstruction> {
        behavior::list_behaviors(instruction)
    }

    fn meets_expectations(observed: &Processor, expectation: &ProcessorExpectation) -> bool {
        let registers_meet = observed
            .general_purpose_registers
            .iter()
            .zip(&expectation.general_purpose_registers)
            .all(|(value, expected)| match expected {
                RegisterValueExpectation::Deterministic(expected) => value == expected,
                RegisterValueExpectation::NonDeterministic => true,
            });
        registers_meet
            && observed.instruction_pointer == expectation.instruction_pointer
            && observed.flags == expectation.flags
            && observed.fs_base == expectation.fs_base
            && observed.gs_base == expectation.gs_base
            && observed.mxcsr == expectation.mxcsr
            && observed.vector_registers == expectation.vector_registers
            && observed.upper_vector_halves == expectation.upper_vector_halves
            && observed.x87 == expectation.x87
            && observed.exception == expectation.exception
    }
}

/// The registers a program at ring 3 reads and writes, and the exception its last instruction raised.
///
/// TODO: temporary placeholder until the rest of the state is designed: ZMM beyond 256 bits, XMM16 to XMM31 and the opmask registers; APX's R16 to R31; AMX's tile
/// configuration and tiles; PKRU; CET's shadow stack pointer; the segment selectors; the trap flag. The chip source
/// checks that an instruction leaves the selectors as they were.
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
    /// Bits 128 to 255 of each vector register, the upper halves of YMM0 to YMM15, which XCR0's AVX bit enables.
    pub upper_vector_halves: [u128; VECTOR_REGISTERS],
    pub x87: X87,
    /// None when the last instruction completed.
    pub exception: Option<Exception>,
}

/// The x87 registers MMX shares, FXSAVE64's view of the x87 state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct X87 {
    /// The physical registers R0 to R7, which MMX names MM0 to MM7, each 80 bits in the low bits.
    pub registers: [u128; X87_REGISTERS],
    /// FCW.
    pub control: u16,
    /// FSW, the top of the stack in bits 11 to 13.
    pub status: u16,
    /// FXSAVE's abridged tag word: bit i set where physical register i is not empty.
    pub tags: u8,
    /// The opcode, instruction pointer and data pointer of the last x87 instruction that was not a control
    /// instruction, as FXSAVE64 saves them; a program reads them with FNSTENV.
    pub opcode: u16,
    pub instruction_pointer: u64,
    pub data_pointer: u64,
}

/// R0 to R7.
pub const X87_REGISTERS: usize = 8;

/// The x87 state after FNINIT, as Linux starts a program: every exception masked, round to nearest, 64-bit
/// precision, every register empty.
pub const X87_INITIAL: X87 = X87 {
    registers: [0; X87_REGISTERS],
    control: 0x037F,
    status: 0,
    tags: 0,
    opcode: 0,
    instruction_pointer: 0,
    data_pointer: 0,
};

/// What a behavior expects one register to hold after an instruction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegisterValueExpectation {
    Deterministic(u64),
    /// The state before does not decide it, as for RDTSC, RDTSCP, RDRAND and RDSEED, so it is not compared.
    NonDeterministic,
}

/// What a behavior expects the processor to hold after an instruction: [`Processor`] with each general register's
/// value an expectation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProcessorExpectation {
    pub general_purpose_registers: [RegisterValueExpectation; GENERAL_PURPOSE_REGISTERS],
    pub instruction_pointer: u64,
    pub flags: Flags,
    pub fs_base: u64,
    pub gs_base: u64,
    pub mxcsr: u32,
    pub vector_registers: [u128; VECTOR_REGISTERS],
    pub upper_vector_halves: [u128; VECTOR_REGISTERS],
    pub x87: X87,
    pub exception: Option<Exception>,
}

impl Processor {
    /// Returns the expectation that the processor holds exactly these values.
    pub fn to_expectation(&self) -> ProcessorExpectation {
        ProcessorExpectation {
            general_purpose_registers: self.general_purpose_registers.map(RegisterValueExpectation::Deterministic),
            instruction_pointer: self.instruction_pointer,
            flags: self.flags,
            fs_base: self.fs_base,
            gs_base: self.gs_base,
            mxcsr: self.mxcsr,
            vector_registers: self.vector_registers,
            upper_vector_halves: self.upper_vector_halves,
            x87: self.x87,
            exception: self.exception,
        }
    }
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
    upper_vector_halves: [0; VECTOR_REGISTERS],
    x87: X87_INITIAL,
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
    /// A form of [`sse::SSE_FORMS`], what a store of it writes below a fault part-way, and which bytes MASKMOVDQU
    /// checks.
    Sse {
        instruction: Instruction,
        across_fault: StoreAcrossFault,
        masked_faults: sse::MaskedStoreFaults,
    },
    /// The form raises #UD once it is fetched: the chip lacks its feature, or the form is one of [`FORBIDDEN_FORMS`].
    Undefined(Instruction),
    /// A form of [`avx::AVX_FORMS`].
    Avx {
        instruction: Instruction,
        behavior: avx::AvxBehavior,
    },
    /// A form of [`fixed::FIXED_FORMS`].
    Fixed(Instruction),
    /// A form of [`stack::STACK_FORMS`].
    Stack {
        instruction: Instruction,
        behavior: stack::StackBehavior,
    },
    /// A form of [`x87::X87_FORMS`].
    X87 {
        instruction: Instruction,
        behavior: x87::X87Behavior,
    },
    /// A form of [`mmx::MMX_FORMS`].
    Mmx {
        instruction: Instruction,
        behavior: mmx::MmxBehavior,
    },
    /// A form of [`extension::EXTENSION_FORMS`].
    Extension {
        instruction: Instruction,
        behavior: extension::ExtensionBehavior,
    },
}
