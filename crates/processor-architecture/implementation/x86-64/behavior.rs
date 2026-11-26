//! The behaviors a form can have where the manuals leave a choice or chips differ.
//!
//! Each result Intel's manual documents as undefined is a field here whose values are the rules a chip could follow;
//! a chip's set names the one it follows, which the probe finds. No value means "anything".

use alloc::vec;
use alloc::vec::Vec;

use crate::integer::{Operation, find_integer_form};
use crate::branch::find_branch_form;
use crate::evex::find_evex_form;
use crate::float::find_float_form;
use crate::vector::find_vector_form;
use crate::avx512_integer::{EvexIntegerPlace, find_evex_integer_form};
use crate::opmask::{OpmaskPlace, find_opmask_form};
use crate::{
    CpuidTable, FORBIDDEN_FORMS, Instruction, PRIVILEGED_FORMS, PreciseInstruction, RING_3_UNDEFINED_FORMS,
    XEON_PLATINUM_8488C_UNDER_KVM, is_optional,
};

/// What a store writes when part of it faults.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StoreAcrossFault {
    WritesNothing,
    WritesBytesBelowFault,
}

/// What CPUID gives: a table that belongs to the chip, or under a hypervisor to the hypervisor, never to the state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CpuidReading {
    Reads(&'static CpuidTable),
    /// Raises #GP(0), as CPUID faulting does once Linux's `arch_prctl(ARCH_SET_CPUID, 0)` turns it on.
    Faults,
}

/// A legacy integer form's behavior: what a store that faults part-way writes, and each undefined result.
///
/// A field the form's family defines is never read, and every behavior of the form holds its first value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IntegerBehavior {
    pub across_fault: StoreAcrossFault,
    pub undefined: UndefinedFlags,
}

/// The flags the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) leaves undefined: OF after a shift or rotate by more than one bit, AF after a logical
/// operation, a shift by at least one bit or IMUL, CF once SHL or SHR shifts an 8-bit or 16-bit operand by its width
/// or more, and SF, ZF and PF after IMUL.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UndefinedFlags {
    pub overflow: OverflowAfterShift,
    pub auxiliary_carry: UndefinedAuxiliaryCarry,
    pub carry_beyond_width: CarryBeyondWidth,
    pub result_flags: UndefinedResultFlags,
    /// Every status flag after DIV and IDIV.
    pub after_divide: UndefinedStatusFlags,
    pub fill_beyond_width: DoubleShiftFill,
    /// OF, SF, AF and PF after BT, BTS, BTR and BTC.
    pub after_bit_test: UndefinedStatusFlags,
    /// CF, OF, SF, AF and PF after BSF and BSR.
    pub after_bit_scan: BitScanFlags,
    /// The destination of BSF and BSR when the source is zero.
    pub scan_of_zero: ScanOfZero,
    /// BSWAP of a 16-bit register.
    pub swap_of_sixteen_bits: SixteenBitSwap,
    /// The register destination of CMPXCHG when the accumulator differs from it.
    pub exchange_mismatch: ExchangeMismatch,
}

/// What BSF and BSR leave in the destination when the source is zero.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScanOfZero {
    Unchanged,
    /// Written with its own value, so that a 32-bit destination's upper half is cleared.
    WrittenBack,
    Zeroed,
}

/// What BSWAP leaves in a 16-bit register.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SixteenBitSwap {
    Zeroed,
    Unchanged,
    /// Its two bytes exchanged.
    BytesExchanged,
}

/// CF, OF, SF, AF and PF after BSF and BSR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BitScanFlags {
    Unchanged,
    Cleared,
    /// CF, OF, SF and AF cleared, PF from the bit index found, taken as 0 for a source of zero.
    ParityFromIndex,
    /// CF, OF, SF and AF cleared, PF from the low byte of the source.
    ParityFromSource,
}

/// What CMPXCHG of a register does with that register when the accumulator differs from it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExchangeMismatch {
    /// Written with its own value, as Intel's manual gives it, so that a 32-bit register's upper half is cleared.
    WrittenBack,
    /// Left as it was, as AMD's manual gives it.
    Unchanged,
}

/// The status flags the manual leaves undefined after an instruction, all by one rule.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UndefinedStatusFlags {
    Unchanged,
    Cleared,
}

/// What SHLD and SHRD of 16 bits by more than 16 shift in once the source's bits are used up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DoubleShiftFill {
    Zeros,
    /// The bits shifted out of the destination and source joined, as if the two rotated.
    Rotation,
}

/// SF, ZF and PF where the manual leaves them undefined, each by a rule of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UndefinedResultFlags {
    pub sign: FlagRule,
    pub zero: FlagRule,
    pub parity: FlagRule,
}

/// One undefined flag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlagRule {
    /// As the value written gives it.
    FromResult,
    Cleared,
    Unchanged,
}

/// OF after a shift or rotate by more than one bit, in terms of the OF the manual gives a one-bit count.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverflowAfterShift {
    /// The one-bit OF of the first one-bit step (for SHL, the operand's top bit XOR the bit below it).
    FirstStepRule,
    /// The one-bit OF of the last one-bit step (for SHL, the result's top bit XOR CF).
    LastStepRule,
    /// The operand's top bit before the shift XOR the result's.
    OperandSignXorResultSign,
    Cleared,
    Unchanged,
}

/// AF where the manual leaves it undefined.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UndefinedAuxiliaryCarry {
    Cleared,
    Set,
    Unchanged,
}

/// CF after SHL or SHR of an 8-bit or 16-bit operand by its width or more.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CarryBeyondWidth {
    /// The last bit shifted out, counting zeros beyond the operand.
    LastBitShiftedOut,
    Cleared,
    Unchanged,
}

const OVERFLOW: [OverflowAfterShift; 5] = [
    OverflowAfterShift::FirstStepRule,
    OverflowAfterShift::LastStepRule,
    OverflowAfterShift::OperandSignXorResultSign,
    OverflowAfterShift::Cleared,
    OverflowAfterShift::Unchanged,
];
const AUXILIARY_CARRY: [UndefinedAuxiliaryCarry; 3] = [
    UndefinedAuxiliaryCarry::Cleared,
    UndefinedAuxiliaryCarry::Set,
    UndefinedAuxiliaryCarry::Unchanged,
];
const CARRY: [CarryBeyondWidth; 3] = [
    CarryBeyondWidth::LastBitShiftedOut,
    CarryBeyondWidth::Cleared,
    CarryBeyondWidth::Unchanged,
];
const RESULT_FLAGS: [UndefinedResultFlags; 27] = {
    const RULES: [FlagRule; 3] = [FlagRule::FromResult, FlagRule::Cleared, FlagRule::Unchanged];
    let mut all = [UndefinedResultFlags {
        sign: FlagRule::FromResult,
        zero: FlagRule::FromResult,
        parity: FlagRule::FromResult,
    }; 27];
    let mut index = 0;
    while index < 27 {
        all[index] = UndefinedResultFlags {
            sign: RULES[index / 9],
            zero: RULES[index / 3 % 3],
            parity: RULES[index % 3],
        };
        index += 1;
    }
    all
};
const STATUS: [UndefinedStatusFlags; 2] = [UndefinedStatusFlags::Unchanged, UndefinedStatusFlags::Cleared];
const FILL: [DoubleShiftFill; 2] = [DoubleShiftFill::Zeros, DoubleShiftFill::Rotation];
const SCAN_OF_ZERO: [ScanOfZero; 3] = [ScanOfZero::Unchanged, ScanOfZero::WrittenBack, ScanOfZero::Zeroed];
const SIXTEEN_BIT_SWAP: [SixteenBitSwap; 3] =
    [SixteenBitSwap::Zeroed, SixteenBitSwap::Unchanged, SixteenBitSwap::BytesExchanged];
const BIT_SCAN_FLAGS: [BitScanFlags; 4] = [
    BitScanFlags::Unchanged,
    BitScanFlags::Cleared,
    BitScanFlags::ParityFromIndex,
    BitScanFlags::ParityFromSource,
];
const EXCHANGE_MISMATCH: [ExchangeMismatch; 2] = [ExchangeMismatch::WrittenBack, ExchangeMismatch::Unchanged];

/// The first rule of every undefined result.
const FIRST_RULES: UndefinedFlags = UndefinedFlags {
    overflow: OVERFLOW[0],
    auxiliary_carry: AUXILIARY_CARRY[0],
    carry_beyond_width: CARRY[0],
    result_flags: RESULT_FLAGS[0],
    after_divide: STATUS[0],
    fill_beyond_width: FILL[0],
    after_bit_test: STATUS[0],
    after_bit_scan: BIT_SCAN_FLAGS[0],
    scan_of_zero: SCAN_OF_ZERO[0],
    swap_of_sixteen_bits: SIXTEEN_BIT_SWAP[0],
    exchange_mismatch: EXCHANGE_MISMATCH[0],
};
const ACROSS_FAULT: [StoreAcrossFault; 2] = [
    StoreAcrossFault::WritesNothing,
    StoreAcrossFault::WritesBytesBelowFault,
];

impl PreciseInstruction {
    pub const fn to_instruction(self) -> Instruction {
        match self {
            Self::PushGprv50 { .. } => Instruction::PushGprv50,
            Self::Ud2 => Instruction::Ud2,
            Self::Int3 => Instruction::Int3,
            Self::Cpuid(_) => Instruction::Cpuid,
            Self::Vector(instruction) | Self::Float(instruction) => instruction,
            Self::Privileged(instruction) | Self::Branch(instruction) => instruction,
            Self::Integer { instruction, .. } | Self::Undefined(instruction) => instruction,
            Self::Sse { instruction, .. } => instruction,
            Self::Avx { instruction, .. } => instruction,
            Self::Extension { instruction, .. } | Self::Fixed(instruction) => instruction,
            Self::Mmx { instruction, .. } | Self::X87 { instruction, .. } | Self::Stack { instruction, .. } => instruction,
            Self::Evex(instruction) => instruction,
            Self::Opmask { instruction, .. } | Self::EvexInteger { instruction, .. } => instruction,
            Self::String { instruction, .. } | Self::System { instruction, .. } => instruction,
            Self::MmxConvert { instruction, .. } | Self::Descriptor(instruction) => instruction,
            Self::Reciprocal { instruction, .. } | Self::Segment { instruction, .. } => instruction,
        }
    }
}

/// Every behavior this crate defines for the form, the one the manuals document first, then for a form a chip may
/// lack the behavior that raises #UD; for a forbidden form that behavior alone, and none for a form every chip has
/// that is not yet modeled.
pub(crate) fn list_behaviors(instruction: Instruction) -> vec::IntoIter<PreciseInstruction> {
    if FORBIDDEN_FORMS.contains(&instruction) {
        return vec![PreciseInstruction::Undefined(instruction)].into_iter();
    }
    let mut behaviors = list_modeled_behaviors(instruction);
    if is_optional(instruction) {
        behaviors.push(PreciseInstruction::Undefined(instruction));
    }
    behaviors.into_iter()
}

/// The behaviors of the form's model; none for a form not yet modeled.
fn list_modeled_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    if let Some(form) = find_opmask_form(instruction) {
        // A store of one byte cannot fault part-way.
        return take(&ACROSS_FAULT, form.place == OpmaskPlace::Store && form.bits > u8::BITS)
            .iter()
            .map(|&across_fault| PreciseInstruction::Opmask { instruction, across_fault })
            .collect();
    }
    if let Some(form) = find_evex_integer_form(instruction) {
        // An aligned store lies on one page.
        return take(&ACROSS_FAULT, form.place == EvexIntegerPlace::Store && form.to_alignment().is_none())
            .iter()
            .map(|&across_fault| PreciseInstruction::EvexInteger { instruction, across_fault })
            .collect();
    }
    match instruction {
        Instruction::PushGprv50 => ACROSS_FAULT
            .map(|across_fault| PreciseInstruction::PushGprv50 { across_fault })
            .to_vec(),
        Instruction::Ud2 => [PreciseInstruction::Ud2].to_vec(),
        Instruction::Int3 => [PreciseInstruction::Int3].to_vec(),
        Instruction::Cpuid => [
            CpuidReading::Reads(&XEON_PLATINUM_8488C_UNDER_KVM),
            CpuidReading::Faults,
        ]
        .map(PreciseInstruction::Cpuid)
        .to_vec(),
        _ if find_vector_form(instruction).is_some() => [PreciseInstruction::Vector(instruction)].to_vec(),
        _ if crate::avx::find_avx_form(instruction).is_some() => crate::avx::list_avx_behaviors(instruction),
        _ if find_float_form(instruction).is_some() => [PreciseInstruction::Float(instruction)].to_vec(),
        _ if crate::sse::find_sse_form(instruction).is_some() => crate::sse::list_sse_behaviors(instruction),
        _ if find_branch_form(instruction).is_some() => [PreciseInstruction::Branch(instruction)].to_vec(),
        _ if find_evex_form(instruction).is_some() => [PreciseInstruction::Evex(instruction)].to_vec(),
        _ if RING_3_UNDEFINED_FORMS.contains(&instruction) => {
            let in_a_machine = PRIVILEGED_FORMS.iter().any(|(form, _)| *form == instruction);
            let privileged = in_a_machine.then_some(PreciseInstruction::Privileged(instruction));
            privileged.into_iter().chain([PreciseInstruction::Undefined(instruction)]).collect()
        }
        _ if PRIVILEGED_FORMS.iter().any(|(form, _)| *form == instruction) => {
            [PreciseInstruction::Privileged(instruction)].to_vec()
        }
        _ if crate::fixed::find_fixed_form(instruction).is_some() => [PreciseInstruction::Fixed(instruction)].to_vec(),
        _ if crate::mmx::find_mmx_form(instruction).is_some() => crate::mmx::list_mmx_behaviors(instruction),
        _ if crate::x87::find_x87_form(instruction).is_some() => crate::x87::list_x87_behaviors(instruction),
        _ if crate::stack::find_stack_form(instruction).is_some() => crate::stack::list_stack_behaviors(instruction),
        _ if crate::extension::find_extension_form(instruction).is_some() => {
            crate::extension::list_extension_behaviors(instruction)
        }
        _ if crate::string::find_string_form(instruction).is_some() => crate::string::list_string_behaviors(instruction),
        _ if crate::system::find_system_form(instruction).is_some() => crate::system::list_system_behaviors(instruction),
        _ if crate::descriptor::find_descriptor_form(instruction).is_some() => [PreciseInstruction::Descriptor(instruction)].to_vec(),
        _ if crate::segment::find_segment_form(instruction).is_some() => crate::segment::list_segment_behaviors(instruction),
        _ if crate::reciprocal::find_reciprocal_form(instruction).is_some() => {
            [PreciseInstruction::Reciprocal { instruction, table: &crate::reciprocal::XEON_PLATINUM_8488C_RECIPROCALS }].to_vec()
        }
        _ if crate::mmx_convert::find_mmx_convert_form(instruction).is_some() => {
            crate::mmx_convert::list_mmx_convert_behaviors(instruction)
        }
        _ => list_integer_behaviors(instruction),
    }
}

/// Every combination of the choices the form's family leaves open; none for a form that is not an integer form.
fn list_integer_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_integer_form(instruction) else {
        return Vec::new();
    };
    let undefined = form.list_undefined_results();
    let stores_to_memory = form.can_store_across_pages();
    // Rules that give the same OF on every input are one behavior: for SHR, whose result's top bit is 0 after a
    // step, the operand's sign XOR the result's is the first step's rule and the last step's is a cleared OF; SAR
    // keeps the sign, so every rule but leaving OF as it was clears it.
    let overflow_rules: &[OverflowAfterShift] = match form.operation {
        Operation::ShiftRightLogical => &[
            OverflowAfterShift::FirstStepRule,
            OverflowAfterShift::LastStepRule,
            OverflowAfterShift::Unchanged,
        ],
        Operation::ShiftRightArithmetic => &[OverflowAfterShift::FirstStepRule, OverflowAfterShift::Unchanged],
        _ => &OVERFLOW,
    };
    let mut rules = vec![FIRST_RULES];
    vary(&mut rules, undefined.overflow, overflow_rules, |flags, rule| flags.overflow = rule);
    vary(&mut rules, undefined.auxiliary_carry, &AUXILIARY_CARRY, |flags, rule| flags.auxiliary_carry = rule);
    vary(&mut rules, undefined.carry_beyond_width, &CARRY, |flags, rule| flags.carry_beyond_width = rule);
    vary(&mut rules, undefined.result_flags, &RESULT_FLAGS, |flags, rule| flags.result_flags = rule);
    vary(&mut rules, undefined.after_divide, &STATUS, |flags, rule| flags.after_divide = rule);
    vary(&mut rules, undefined.fill_beyond_width, &FILL, |flags, rule| flags.fill_beyond_width = rule);
    vary(&mut rules, undefined.after_bit_test, &STATUS, |flags, rule| flags.after_bit_test = rule);
    vary(&mut rules, undefined.after_bit_scan, &BIT_SCAN_FLAGS, |flags, rule| flags.after_bit_scan = rule);
    vary(&mut rules, undefined.scan_of_zero, &SCAN_OF_ZERO, |flags, rule| flags.scan_of_zero = rule);
    vary(&mut rules, undefined.swap_of_sixteen_bits, &SIXTEEN_BIT_SWAP, |flags, rule| flags.swap_of_sixteen_bits = rule);
    vary(&mut rules, undefined.exchange_mismatch, &EXCHANGE_MISMATCH, |flags, rule| flags.exchange_mismatch = rule);
    take(&ACROSS_FAULT, stores_to_memory)
        .iter()
        .flat_map(|&across_fault| {
            rules.iter().map(move |&undefined| PreciseInstruction::Integer {
                instruction,
                behavior: IntegerBehavior { across_fault, undefined },
            })
        })
        .collect()
}

/// Every rule list so far once with each rule of a result the form leaves open, the last result changing fastest.
fn vary<T: Copy>(rules: &mut Vec<UndefinedFlags>, varies: bool, values: &[T], set: impl Fn(&mut UndefinedFlags, T)) {
    let mut varied = Vec::new();
    for &rule in rules.iter() {
        for &value in take(values, varies) {
            let mut changed = rule;
            set(&mut changed, value);
            varied.push(changed);
        }
    }
    *rules = varied;
}

/// All the values when the form leaves the result open, else the first alone.
fn take<T>(all: &[T], varies: bool) -> &[T] {
    if varies { all } else { &all[..1] }
}
