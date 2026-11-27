//! The string forms: MOVS, STOS, LODS, SCAS and CMPS of each size, alone and repeated, and INS and OUTS repeated, as
//! the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (vol. 1, 7.3.9,
//! and vol. 2) gives them in 64-bit mode.
//!
//! The source is DS:RSI, which an FS or GS override moves by that segment's base, and the destination ES:RDI; under
//! the 0x67 prefix ESI, EDI and ECX are read and written. Each element moves RSI and RDI by its size, down where DF is
//! set.
//!
//! One step runs one iteration, as the chip stops a repeated form between iterations for a trap or an interrupt: a
//! repeated form whose count is zero only moves the instruction pointer; otherwise it runs the string operation once,
//! lowers the count, and leaves the instruction pointer at its own first byte unless the count reached zero or, for
//! REPE and REPNE, the comparison ended the repetition. INS and OUTS raise #GP(0) at ring 3 under IOPL 0 before they
//! reach a port.

use alloc::vec::Vec;

use iced_x86::Code;

use crate::PreciseInstruction;
use crate::behavior::StoreAcrossFault;
use crate::forms::Instruction;

/// What a string form does with one element.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StringOperation {
    /// MOVS: the source element stored at the destination.
    Move,
    /// STOS: the accumulator stored at the destination.
    Store,
    /// LODS: the source element loaded into the accumulator.
    Load,
    /// SCAS: the accumulator compared with the destination element.
    Scan,
    /// CMPS: the source element compared with the destination element.
    Compare,
    /// INS.
    Input,
    /// OUTS.
    Output,
}

/// The prefix that repeats a string form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Repetition {
    Once,
    /// REP: until the count is zero.
    Repeat,
    /// REPE: until the count is zero or the comparison finds the elements differ.
    WhileEqual,
    /// REPNE: until the count is zero or the comparison finds the elements equal.
    WhileNotEqual,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StringForm {
    pub instruction: Instruction,
    pub operation: StringOperation,
    pub repetition: Repetition,
    /// The size of an element.
    pub bytes: u64,
    pub code: Code,
}

impl StringForm {
    /// Whether the form reads the source at RSI.
    pub const fn reads_source(&self) -> bool {
        matches!(self.operation, StringOperation::Move | StringOperation::Load | StringOperation::Compare | StringOperation::Output)
    }

    /// Whether the form reads or writes the destination at RDI.
    pub const fn reaches_destination(&self) -> bool {
        matches!(
            self.operation,
            StringOperation::Move | StringOperation::Store | StringOperation::Scan | StringOperation::Compare | StringOperation::Input
        )
    }

    /// Whether the form writes the destination.
    pub const fn stores(&self) -> bool {
        matches!(self.operation, StringOperation::Move | StringOperation::Store)
    }

    /// Whether the form compares, so that it writes the status flags.
    pub const fn compares(&self) -> bool {
        matches!(self.operation, StringOperation::Scan | StringOperation::Compare)
    }

    pub const fn is_repeated(&self) -> bool {
        !matches!(self.repetition, Repetition::Once)
    }
}

/// A string form's behavior: what a store that faults part-way writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::string)
)]
pub struct StringBehavior {
    pub across_fault: StoreAcrossFault,
}

/// The row of a string form, or `None` for any other form.
pub fn find_string_form(instruction: Instruction) -> Option<&'static StringForm> {
    STRING_FORMS.iter().find(|form| form.instruction == instruction)
}

const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];

/// Both rules of a store that faults part-way for a form that stores more than a byte, else the first.
pub(crate) fn list_string_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_string_form(instruction) else {
        return Vec::new();
    };
    let across: &[StoreAcrossFault] = if form.stores() && form.bytes > 1 { &ACROSS_FAULT } else { &ACROSS_FAULT[..1] };
    across
        .iter()
        .map(|&across_fault| PreciseInstruction::String { instruction, behavior: StringBehavior { across_fault } })
        .collect()
}

const fn string(instruction: Instruction, operation: StringOperation, repetition: Repetition, bytes: u64, code: Code) -> StringForm {
    StringForm { instruction, operation, repetition, bytes, code }
}

use Repetition::{Once, Repeat, WhileEqual, WhileNotEqual};
use StringOperation::{Compare, Input, Load, Move, Output, Scan, Store};

/// Every string form modeled.
pub static STRING_FORMS: &[StringForm] = &[
    string(Instruction::Movsb, Move, Once, 1, Code::Movsb_m8_m8),
    string(Instruction::Movsw, Move, Once, 2, Code::Movsw_m16_m16),
    string(Instruction::Movsd, Move, Once, 4, Code::Movsd_m32_m32),
    string(Instruction::Movsq, Move, Once, 8, Code::Movsq_m64_m64),
    string(Instruction::Stosb, Store, Once, 1, Code::Stosb_m8_AL),
    string(Instruction::Stosw, Store, Once, 2, Code::Stosw_m16_AX),
    string(Instruction::Stosd, Store, Once, 4, Code::Stosd_m32_EAX),
    string(Instruction::Stosq, Store, Once, 8, Code::Stosq_m64_RAX),
    string(Instruction::Lodsb, Load, Once, 1, Code::Lodsb_AL_m8),
    string(Instruction::Lodsw, Load, Once, 2, Code::Lodsw_AX_m16),
    string(Instruction::Lodsd, Load, Once, 4, Code::Lodsd_EAX_m32),
    string(Instruction::Lodsq, Load, Once, 8, Code::Lodsq_RAX_m64),
    string(Instruction::Scasb, Scan, Once, 1, Code::Scasb_AL_m8),
    string(Instruction::Scasw, Scan, Once, 2, Code::Scasw_AX_m16),
    string(Instruction::Scasd, Scan, Once, 4, Code::Scasd_EAX_m32),
    string(Instruction::Scasq, Scan, Once, 8, Code::Scasq_RAX_m64),
    string(Instruction::Cmpsb, Compare, Once, 1, Code::Cmpsb_m8_m8),
    string(Instruction::Cmpsw, Compare, Once, 2, Code::Cmpsw_m16_m16),
    string(Instruction::Cmpsd, Compare, Once, 4, Code::Cmpsd_m32_m32),
    string(Instruction::Cmpsq, Compare, Once, 8, Code::Cmpsq_m64_m64),
    string(Instruction::RepMovsb, Move, Repeat, 1, Code::Movsb_m8_m8),
    string(Instruction::RepMovsw, Move, Repeat, 2, Code::Movsw_m16_m16),
    string(Instruction::RepMovsd, Move, Repeat, 4, Code::Movsd_m32_m32),
    string(Instruction::RepMovsq, Move, Repeat, 8, Code::Movsq_m64_m64),
    string(Instruction::RepStosb, Store, Repeat, 1, Code::Stosb_m8_AL),
    string(Instruction::RepStosw, Store, Repeat, 2, Code::Stosw_m16_AX),
    string(Instruction::RepStosd, Store, Repeat, 4, Code::Stosd_m32_EAX),
    string(Instruction::RepStosq, Store, Repeat, 8, Code::Stosq_m64_RAX),
    string(Instruction::RepLodsb, Load, Repeat, 1, Code::Lodsb_AL_m8),
    string(Instruction::RepLodsw, Load, Repeat, 2, Code::Lodsw_AX_m16),
    string(Instruction::RepLodsd, Load, Repeat, 4, Code::Lodsd_EAX_m32),
    string(Instruction::RepLodsq, Load, Repeat, 8, Code::Lodsq_RAX_m64),
    string(Instruction::RepeScasb, Scan, WhileEqual, 1, Code::Scasb_AL_m8),
    string(Instruction::RepeScasw, Scan, WhileEqual, 2, Code::Scasw_AX_m16),
    string(Instruction::RepeScasd, Scan, WhileEqual, 4, Code::Scasd_EAX_m32),
    string(Instruction::RepeScasq, Scan, WhileEqual, 8, Code::Scasq_RAX_m64),
    string(Instruction::RepneScasb, Scan, WhileNotEqual, 1, Code::Scasb_AL_m8),
    string(Instruction::RepneScasw, Scan, WhileNotEqual, 2, Code::Scasw_AX_m16),
    string(Instruction::RepneScasd, Scan, WhileNotEqual, 4, Code::Scasd_EAX_m32),
    string(Instruction::RepneScasq, Scan, WhileNotEqual, 8, Code::Scasq_RAX_m64),
    string(Instruction::RepeCmpsb, Compare, WhileEqual, 1, Code::Cmpsb_m8_m8),
    string(Instruction::RepeCmpsw, Compare, WhileEqual, 2, Code::Cmpsw_m16_m16),
    string(Instruction::RepeCmpsd, Compare, WhileEqual, 4, Code::Cmpsd_m32_m32),
    string(Instruction::RepeCmpsq, Compare, WhileEqual, 8, Code::Cmpsq_m64_m64),
    string(Instruction::RepneCmpsb, Compare, WhileNotEqual, 1, Code::Cmpsb_m8_m8),
    string(Instruction::RepneCmpsw, Compare, WhileNotEqual, 2, Code::Cmpsw_m16_m16),
    string(Instruction::RepneCmpsd, Compare, WhileNotEqual, 4, Code::Cmpsd_m32_m32),
    string(Instruction::RepneCmpsq, Compare, WhileNotEqual, 8, Code::Cmpsq_m64_m64),
    string(Instruction::RepInsb, Input, Repeat, 1, Code::Insb_m8_DX),
    string(Instruction::RepInsw, Input, Repeat, 2, Code::Insw_m16_DX),
    string(Instruction::RepInsd, Input, Repeat, 4, Code::Insd_m32_DX),
    string(Instruction::RepOutsb, Output, Repeat, 1, Code::Outsb_DX_m8),
    string(Instruction::RepOutsw, Output, Repeat, 2, Code::Outsw_DX_m16),
    string(Instruction::RepOutsd, Output, Repeat, 4, Code::Outsd_DX_m32),
];
