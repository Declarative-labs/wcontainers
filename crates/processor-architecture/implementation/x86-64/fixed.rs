//! The forms whose outcome at ring 3 no data of the state decides: the privileged forms that raise #GP(0) at ring 3
//! whatever their operands, the hint-space and reserved forms that run as no-ops, the cache line flushes that only
//! check their byte's page, INT n and INT1, the forms that always raise #UD, and the forms whose register result
//! the time or a random source decides.
//!
//! Each is one row of [`FIXED_FORMS`]; its states come from its sample encoding, with every placement of its memory
//! operand where the variation keeps the form, every code layout and every flag, so the row also says how far the
//! memory operand may vary.

use crate::forms::Instruction;

/// What the form does at ring 3 under Linux, whatever its operands hold.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FixedOutcome {
    /// #GP(0): the form needs CPL 0 (XSAVES and XRSTORS too, before their memory operand is checked), or IOPL 3 as
    /// the I/O forms do, or UMIP stops it.
    GeneralProtection,
    /// #UD once fetched whole: RSM outside SMM, UD0 and 0xD6.
    InvalidOpcode,
    /// INT n through the gate of the vector: #BP for 3 and #OF for 4, which Linux's gates let ring 3 reach, else
    /// #GP with the vector's IDT selector as the error code; INT1 raises #DB.
    SoftwareInterrupt,
    /// Nothing but the instruction pointer moves, and no memory operand is reached.
    NoOperation,
    /// The page of the byte the memory operand names must allow a read, as for CLFLUSH, CLFLUSHOPT and CLWB.
    TouchesByte,
    /// The memory operand is read whole and nothing is written, as PTWRITE does with tracing off.
    ReadsOperand,
    /// Registers are written with values the state does not decide, so a behavior does not compare them: RDTSC's
    /// EDX:EAX the time stamp counter, RDTSCP's also ECX, and RDPID's register IA32_TSC_AUX, which Linux sets to the
    /// CPU's number.
    WritesUndecided,
    /// XTEST outside a transaction: ZF set, CF, OF, SF, AF and PF cleared.
    OutsideTransaction,
    /// The register operand is written with a random value and CF is set, or both cleared when the random source had no
    /// value ready, so a behavior compares neither; OF, SF, ZF, AF and PF are cleared, as RDRAND and RDSEED do.
    WritesRandom,
    /// SYSENTER in 64-bit mode on Intel's chips: the kernel is entered at the MSRs Linux sets, with no return address
    /// kept.
    SystemEnter,
    /// ENQCMD: #GP(0) with no PASID in IA32_PASID; with one, a device's answer decides ZF, so the form is forbidden
    /// there and raises #UD as [`crate::FORBIDDEN_FORMS`] do.
    EnqueueCommand,
}

/// How far a state may move the form's memory operand without its encoding becoming another form's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemoryVariation {
    /// The form has no memory operand, or must keep its sample's: MPX's, and PREFETCHIT0 and PREFETCHIT1, whose
    /// memory operand is relative to the instruction pointer.
    Sample,
    All,
    /// Any but relative to the instruction pointer, which would make the 0x0F 0x18 forms PREFETCHIT0 and PREFETCHIT1.
    NotInstructionPointerRelative,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FixedForm {
    pub instruction: Instruction,
    pub outcome: FixedOutcome,
    pub memory: MemoryVariation,
}

/// The row of a fixed form, or `None` for any other form.
pub fn find_fixed_form(instruction: Instruction) -> Option<&'static FixedForm> {
    FIXED_FORMS.iter().find(|form| form.instruction == instruction)
}

const fn fixed(instruction: Instruction, outcome: FixedOutcome, memory: MemoryVariation) -> FixedForm {
    FixedForm { instruction, outcome, memory }
}

use FixedOutcome::{
    GeneralProtection, InvalidOpcode, NoOperation, OutsideTransaction, ReadsOperand, SoftwareInterrupt, TouchesByte, WritesRandom,
    WritesUndecided,
};
use MemoryVariation::{All, NotInstructionPointerRelative, Sample};

/// Every fixed form modeled.
pub static FIXED_FORMS: &[FixedForm] = &[
    fixed(Instruction::LgdtMems64, GeneralProtection, All),
    fixed(Instruction::LidtMems64, GeneralProtection, All),
    fixed(Instruction::LldtGpr16, GeneralProtection, Sample),
    fixed(Instruction::LldtMemw, GeneralProtection, All),
    fixed(Instruction::LtrGpr16, GeneralProtection, Sample),
    fixed(Instruction::LtrMemw, GeneralProtection, All),
    fixed(Instruction::LmswGpr16, GeneralProtection, Sample),
    fixed(Instruction::LmswMemw, GeneralProtection, All),
    fixed(Instruction::InvlpgMemb, GeneralProtection, All),
    fixed(Instruction::MovCrCrGpr64, GeneralProtection, Sample),
    fixed(Instruction::MovCrGpr64Cr, GeneralProtection, Sample),
    fixed(Instruction::MovDrDrGpr64, GeneralProtection, Sample),
    fixed(Instruction::MovDrGpr64Dr, GeneralProtection, Sample),
    fixed(Instruction::InAlDx, GeneralProtection, Sample),
    fixed(Instruction::InAlImmb, GeneralProtection, Sample),
    fixed(Instruction::InOeaxDx, GeneralProtection, Sample),
    fixed(Instruction::InOeaxImmb, GeneralProtection, Sample),
    fixed(Instruction::OutDxAl, GeneralProtection, Sample),
    fixed(Instruction::OutDxOeax, GeneralProtection, Sample),
    fixed(Instruction::OutImmbAl, GeneralProtection, Sample),
    fixed(Instruction::OutImmbOeax, GeneralProtection, Sample),
    fixed(Instruction::Insb, GeneralProtection, Sample),
    fixed(Instruction::Insd, GeneralProtection, Sample),
    fixed(Instruction::Insw, GeneralProtection, Sample),
    fixed(Instruction::Outsb, GeneralProtection, Sample),
    fixed(Instruction::Outsd, GeneralProtection, Sample),
    fixed(Instruction::Outsw, GeneralProtection, Sample),
    fixed(Instruction::SldtGprv, GeneralProtection, Sample),
    fixed(Instruction::SldtMemw, GeneralProtection, All),
    fixed(Instruction::StrGprv, GeneralProtection, Sample),
    fixed(Instruction::StrMemw, GeneralProtection, All),
    fixed(Instruction::SgdtMems64, GeneralProtection, All),
    fixed(Instruction::SidtMems64, GeneralProtection, All),
    fixed(Instruction::SmswGprv, GeneralProtection, Sample),
    fixed(Instruction::SmswMemw, GeneralProtection, All),
    fixed(Instruction::InvpcidGpr64Memdq, GeneralProtection, All),
    fixed(Instruction::Rsm, InvalidOpcode, Sample),
    fixed(Instruction::Ud0, InvalidOpcode, Sample),
    fixed(Instruction::Udb, InvalidOpcode, Sample),
    fixed(Instruction::IntImmb, SoftwareInterrupt, Sample),
    fixed(Instruction::Int1, SoftwareInterrupt, Sample),
    fixed(Instruction::NopGprv0f18r0, NoOperation, Sample),
    fixed(Instruction::NopGprv0f18r1, NoOperation, Sample),
    fixed(Instruction::NopGprv0f18r2, NoOperation, Sample),
    fixed(Instruction::NopGprv0f18r3, NoOperation, Sample),
    fixed(Instruction::NopGprv0f18r4, NoOperation, Sample),
    fixed(Instruction::NopGprv0f18r5, NoOperation, Sample),
    fixed(Instruction::NopGprv0f18r6, NoOperation, Sample),
    fixed(Instruction::NopGprv0f18r7, NoOperation, Sample),
    fixed(Instruction::NopGprvGprv0f0d, NoOperation, Sample),
    fixed(Instruction::NopGprvGprv0f19, NoOperation, Sample),
    fixed(Instruction::NopGprvGprv0f1a, NoOperation, Sample),
    fixed(Instruction::NopGprvGprv0f1b, NoOperation, Sample),
    fixed(Instruction::NopGprvGprv0f1c, NoOperation, Sample),
    fixed(Instruction::NopGprvGprv0f1d, NoOperation, Sample),
    fixed(Instruction::NopGprvGprv0f1e, NoOperation, Sample),
    fixed(Instruction::NopGprvMem0f1b, NoOperation, All),
    fixed(Instruction::NopGprvMemv0f1a, NoOperation, All),
    fixed(Instruction::NopMemv0f18r4, NoOperation, All),
    fixed(Instruction::NopMemv0f18r5, NoOperation, All),
    fixed(Instruction::NopMemv0f18r6, NoOperation, NotInstructionPointerRelative),
    fixed(Instruction::NopMemv0f18r7, NoOperation, NotInstructionPointerRelative),
    fixed(Instruction::NopMemvGprv0f19, NoOperation, All),
    fixed(Instruction::NopMemvGprv0f1c, NoOperation, All),
    fixed(Instruction::NopMemvGprv0f1d, NoOperation, All),
    fixed(Instruction::NopMemvGprv0f1e, NoOperation, All),
    fixed(Instruction::BndclBndAgen, NoOperation, Sample),
    fixed(Instruction::BndclBndGpr64, NoOperation, Sample),
    fixed(Instruction::BndcnBndAgen, NoOperation, Sample),
    fixed(Instruction::BndcnBndGpr64, NoOperation, Sample),
    fixed(Instruction::BndcuBndAgen, NoOperation, Sample),
    fixed(Instruction::BndcuBndGpr64, NoOperation, Sample),
    fixed(Instruction::BndldxBndMembnd64, NoOperation, Sample),
    fixed(Instruction::BndmkBndAgen, NoOperation, Sample),
    fixed(Instruction::BndmovBndBnd, NoOperation, Sample),
    fixed(Instruction::BndmovBndMemdq, NoOperation, Sample),
    fixed(Instruction::BndmovMemdqBnd, NoOperation, Sample),
    fixed(Instruction::BndstxMembnd64Bnd, NoOperation, Sample),
    fixed(Instruction::Ibhf, NoOperation, Sample),
    fixed(Instruction::PtwriteGpry, NoOperation, Sample),
    fixed(Instruction::ClflushMemmprefetch, TouchesByte, All),
    fixed(Instruction::ClflushoptMemmprefetch, TouchesByte, All),
    fixed(Instruction::ClwbMemmprefetch, TouchesByte, All),
    fixed(Instruction::PtwriteMemy, ReadsOperand, All),
    // Under CR4.TSD, which the executors set so that the kernel answers a read of the time-stamp counter.
    fixed(Instruction::Rdtsc, GeneralProtection, Sample),
    fixed(Instruction::Rdtscp, GeneralProtection, Sample),
    fixed(Instruction::RdpidGpr64u64, WritesUndecided, Sample),
    fixed(Instruction::RdrandGprv, WritesRandom, Sample),
    fixed(Instruction::RdseedGprv, WritesRandom, Sample),
    fixed(Instruction::XsavesMemmxsave, GeneralProtection, All),
    fixed(Instruction::Xsaves64Memmxsave, GeneralProtection, All),
    fixed(Instruction::XrstorsMemmxsave, GeneralProtection, All),
    fixed(Instruction::Xrstors64Memmxsave, GeneralProtection, All),
    // Outside a transaction, which a single step never is in: XEND raises #GP(0), XABORT and the TSXLDTRK forms do
    // nothing.
    fixed(Instruction::Xend, GeneralProtection, Sample),
    fixed(Instruction::XabortImmb, NoOperation, Sample),
    fixed(Instruction::Xsusldtrk, NoOperation, Sample),
    fixed(Instruction::Xresldtrk, NoOperation, Sample),
    fixed(Instruction::Xtest, OutsideTransaction, Sample),
    fixed(Instruction::Prefetchrst2Memu8, NoOperation, All),
    // ENQCMDS needs CPL 0; ENQCMD needs a valid IA32_PASID, which Linux gives only a process bound to a device.
    fixed(Instruction::EnqcmdsGpraMemu32, GeneralProtection, Sample),
    fixed(Instruction::EnqcmdGpraMemu32, FixedOutcome::EnqueueCommand, Sample),
    fixed(Instruction::Sysenter, FixedOutcome::SystemEnter, Sample),
];

/// The general registers, by number, a form of [`FixedOutcome::WritesUndecided`] or [`FixedOutcome::WritesRandom`]
/// writes: EAX and EDX, ECX for RDTSCP, and the register operand of RDPID, RDRAND and RDSEED.
pub fn list_undecided_registers(decoded: &iced_x86::Instruction) -> alloc::vec::Vec<usize> {
    const ACCUMULATOR: usize = 0;
    const COUNTER: usize = 1;
    const DATA: usize = 2;
    match decoded.code() {
        iced_x86::Code::Rdtsc => alloc::vec![ACCUMULATOR, DATA],
        iced_x86::Code::Rdtscp => alloc::vec![ACCUMULATOR, COUNTER, DATA],
        _ => alloc::vec![decoded.op0_register().full_register().number()],
    }
}
