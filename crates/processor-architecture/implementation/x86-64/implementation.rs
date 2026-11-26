//! x86-64 as Linux runs a 64-bit program at ring 3: IF set, IOPL 0, CR0.AM set, UMIP, FSGSBASE, OSXSAVE and PKE
//! on, and XCR0 enabling x87, SSE, AVX and AVX-512 state, of which the state holds SSE's, AVX's and AVX-512's.
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

mod behavior;
pub mod branch;
mod cpuid;
pub mod evex;
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
/// k0 to k7, AVX-512's opmask registers.
pub const OPMASK_REGISTERS: usize = 8;
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
/// INVD, WBINVD, WBNOINVD, RDMSR and WRMSR for CPL other than 0, and RDPMC for CPL other than 0 with CR4.PCE clear, as
/// Linux leaves it for a program that has not mapped a performance counter.
pub const PRIVILEGED_FORMS: [(Instruction, &[Code]); 16] = [
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
    (Instruction::Wbnoinvd, &[Code::Wbnoinvd]),
    (Instruction::Seamcall, &[Code::Seamcall]),
    (Instruction::Tdcall, &[Code::Tdcall]),
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

/// The forms every chip raises #UD for in a program under Linux, outside the environments that give them meaning:
/// SEAMCALL, SEAMOPS and SEAMRET outside VMX root operation, TDCALL outside a trust domain, ENCLV and VMFUNC
/// outside VMX non-root operation (the [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html),
/// volume 2, and Intel's TDX module specification). SEAMCALL and TDCALL check the privilege level first inside a
/// virtual machine, where they raise #GP(0), so they are in [`PRIVILEGED_FORMS`] too.
pub const RING_3_UNDEFINED_FORMS: [Instruction; 6] = [
    Instruction::Seamcall,
    Instruction::Seamops,
    Instruction::Seamret,
    Instruction::Tdcall,
    Instruction::Enclv,
    Instruction::Vmfunc,
];

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
        .chain(evex::list_evex_forms().into_iter().map(|form| form.instruction))
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
            && observed.zmm_upper_halves == expectation.zmm_upper_halves
            && observed.high_vector_registers == expectation.high_vector_registers
            && observed.opmask_registers == expectation.opmask_registers
            && observed.exception == expectation.exception
    }
}

/// The registers a program at ring 3 reads and writes, and the exception its last instruction raised.
///
/// TODO: temporary placeholder until the rest of the state is designed: the x87 stack with its control, status and
/// tag words and MMX; APX's R16 to R31; AMX's tile
/// configuration and tiles; PKRU; CET's shadow stack pointer; the segment selectors; the trap flag. The chip source
/// checks that an instruction leaves the x87 state and the selectors as they were.
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
    /// Bits 256 to 383 and 384 to 511 of ZMM0 to ZMM15, which XCR0's ZMM_Hi256 bit enables.
    pub zmm_upper_halves: [[u128; 2]; VECTOR_REGISTERS],
    /// ZMM16 to ZMM31 whole, lowest 128 bits first, which XCR0's Hi16_ZMM bit enables.
    pub high_vector_registers: [[u128; 4]; VECTOR_REGISTERS],
    /// k0 to k7, which XCR0's opmask bit enables.
    pub opmask_registers: [u64; OPMASK_REGISTERS],
    /// None when the last instruction completed.
    pub exception: Option<Exception>,
}

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
    pub zmm_upper_halves: [[u128; 2]; VECTOR_REGISTERS],
    pub high_vector_registers: [[u128; 4]; VECTOR_REGISTERS],
    pub opmask_registers: [u64; OPMASK_REGISTERS],
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
            zmm_upper_halves: self.zmm_upper_halves,
            high_vector_registers: self.high_vector_registers,
            opmask_registers: self.opmask_registers,
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
    zmm_upper_halves: [[0; 2]; VECTOR_REGISTERS],
    high_vector_registers: [[0; 4]; VECTOR_REGISTERS],
    opmask_registers: [0; OPMASK_REGISTERS],
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
    /// A form of [`evex::EVEX_FORMS`], whose results the manuals define whole.
    Evex(Instruction),
    /// A form of [`integer::INTEGER_FORMS`].
    Integer {
        instruction: Instruction,
        behavior: IntegerBehavior,
    },
    /// The form raises #UD once it is fetched: the chip lacks its feature, or the form is one of [`FORBIDDEN_FORMS`].
    Undefined(Instruction),
}
