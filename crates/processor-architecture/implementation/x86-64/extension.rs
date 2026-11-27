//! The general-purpose extensions of the legacy integer forms: BMI1, BMI2, ADX, LZCNT, MOVBE, CMPXCHG8B and
//! CMPXCHG16B.
//!
//! A form is one row of [`EXTENSION_FORMS`]: its operation, where its register-or-memory operand lies, and the
//! iced-x86 `Code` of each of its operand sizes. The results the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) leaves undefined (AF, PF and for some
//! forms SF and OF) are the fields of [`ExtensionFlags`], each a known behavior.

use alloc::vec::Vec;

use iced_x86::Code;

use crate::behavior::{FlagRule, StoreAcrossFault, UndefinedAuxiliaryCarry};
use crate::forms::Instruction;
use crate::integer::{OperandPlace, to_mask, to_result_flags, with_status};
use crate::planted::{Bug, is_active};
use crate::{Flags, PreciseInstruction};

/// What a form computes, in the manual's terms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExtensionOperation {
    /// ANDN: the first source's complement AND the second source.
    AndNot,
    /// BEXTR: the bits of the source from the control's low byte on, as many as its second byte gives.
    ExtractBitField,
    /// BLSI.
    IsolateLowestSetBit,
    /// BLSMSK.
    MaskUpToLowestSetBit,
    /// BLSR.
    ResetLowestSetBit,
    /// BZHI: the source with every bit from the index's low byte up cleared.
    ZeroHighBits,
    /// TZCNT.
    CountTrailingZeros,
    /// LZCNT.
    CountLeadingZeros,
    /// MULX: EDX or RDX times the source, unsigned, the high half into the first operand and the low half into the
    /// second; the flags are left alone.
    MultiplyWithoutFlags,
    /// PDEP.
    DepositBits,
    /// PEXT.
    ExtractBits,
    /// RORX.
    RotateRightWithoutFlags,
    /// SARX.
    ShiftRightArithmeticWithoutFlags,
    /// SHLX.
    ShiftLeftWithoutFlags,
    /// SHRX.
    ShiftRightLogicalWithoutFlags,
    /// ADCX: the sum with CF, which takes the carry out; no other flag changes.
    AddWithCarryFlag,
    /// ADOX: the sum with OF as the carry, which takes the carry out; no other flag changes.
    AddWithOverflowFlag,
    /// MOVBE: a load or a store with the bytes reversed.
    MoveSwappingBytes,
    /// CMPXCHG8B and CMPXCHG16B: EDX:EAX (RDX:RAX) compared with memory; equal, ECX:EBX (RCX:RBX) is stored and ZF
    /// set; not, the memory is stored back, loaded into EDX:EAX (RDX:RAX) and ZF cleared.
    CompareExchangePair,
}

/// One form of the extensions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExtensionForm {
    pub instruction: Instruction,
    pub operation: ExtensionOperation,
    pub place: OperandPlace,
    /// By operand size, smallest first: one code for a VEX form of one width, three for 16, 32 and 64 bits.
    pub codes: &'static [Code],
}

/// The flags an extension form leaves undefined, each by its rule; a flag the form defines is never read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::extension)
)]
pub struct ExtensionFlags {
    pub overflow: FlagRule,
    pub sign: FlagRule,
    pub auxiliary_carry: UndefinedAuxiliaryCarry,
    pub parity: FlagRule,
}

/// An extension form's behavior: its undefined flags, and what a store that faults part-way writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::extension)
)]
pub struct ExtensionBehavior {
    pub undefined: ExtensionFlags,
    pub across_fault: StoreAcrossFault,
}

/// Which flags the operation leaves undefined.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct UndefinedExtensionFlags {
    overflow: bool,
    sign: bool,
    auxiliary_carry: bool,
    parity: bool,
}

impl ExtensionOperation {
    const fn list_undefined_flags(self) -> UndefinedExtensionFlags {
        let (overflow, sign, auxiliary_carry_and_parity) = match self {
            Self::AndNot
            | Self::IsolateLowestSetBit
            | Self::MaskUpToLowestSetBit
            | Self::ResetLowestSetBit
            | Self::ZeroHighBits => (false, false, true),
            Self::ExtractBitField => (false, true, true),
            Self::CountTrailingZeros | Self::CountLeadingZeros => (true, true, true),
            _ => (false, false, false),
        };
        UndefinedExtensionFlags {
            overflow,
            sign,
            auxiliary_carry: auxiliary_carry_and_parity,
            parity: auxiliary_carry_and_parity,
        }
    }

    /// Whether the operation writes its operand in memory.
    pub const fn writes_memory(self) -> bool {
        matches!(self, Self::CompareExchangePair)
    }

    /// The flags the operation reads.
    pub const fn reads_flags(self) -> Flags {
        match self {
            Self::AddWithCarryFlag => Flags::CARRY,
            Self::AddWithOverflowFlag => Flags::OVERFLOW,
            _ => Flags::empty(),
        }
    }
}

impl ExtensionForm {
    /// Whether the form writes its memory operand: MOVBE to memory and CMPXCHG8B and CMPXCHG16B.
    pub fn writes_memory(&self) -> bool {
        self.place != OperandPlace::Register
            && (self.operation.writes_memory() || matches!(self.codes[0], Code::Movbe_m16_r16))
    }

    /// The alignment the form requires of its memory operand whatever AC holds: CMPXCHG16B's 16.
    pub fn required_alignment(&self) -> Option<u64> {
        (self.codes[0] == Code::Cmpxchg16b_m128).then_some(16)
    }

    /// The bytes of the memory operand at the code: for CMPXCHG8B and CMPXCHG16B two registers' worth.
    pub fn to_memory_bytes(code: Code) -> u64 {
        match code {
            Code::Cmpxchg8b_m64 => 8,
            Code::Cmpxchg16b_m128 => 16,
            // The width of the registers the form names, which VEX.W gives a VEX form.
            code => {
                use iced_x86::OpCodeOperandKind as Kind;
                let kinds = code.op_code().op_kinds();
                if kinds.iter().any(|kind| matches!(kind, Kind::r64_reg | Kind::r64_vvvv | Kind::r64_or_mem)) {
                    8
                } else if kinds.iter().any(|kind| matches!(kind, Kind::r16_reg | Kind::r16_or_mem)) {
                    2
                } else {
                    4
                }
            }
        }
    }
}

/// The row of an extension form, or `None` for any other form.
pub fn find_extension_form(instruction: Instruction) -> Option<&'static ExtensionForm> {
    EXTENSION_FORMS.iter().find(|form| form.instruction == instruction)
}

const RULES: [FlagRule; 3] = [FlagRule::FromResult, FlagRule::Cleared, FlagRule::Unchanged];
/// A count's SF from its result is always clear, so it is the cleared rule.
const COUNT_SIGN_RULES: [FlagRule; 2] = [FlagRule::Cleared, FlagRule::Unchanged];
/// OF has no result to come from.
const OVERFLOW_RULES: [FlagRule; 2] = [FlagRule::Cleared, FlagRule::Unchanged];
const AUXILIARY_CARRY: [UndefinedAuxiliaryCarry; 3] =
    [UndefinedAuxiliaryCarry::Cleared, UndefinedAuxiliaryCarry::Set, UndefinedAuxiliaryCarry::Unchanged];
const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];

/// Every combination of the rules the form leaves open, the first of each where the form defines the flag.
pub(crate) fn list_extension_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_extension_form(instruction) else {
        return Vec::new();
    };
    let undefined = form.operation.list_undefined_flags();
    let counts = matches!(form.operation, ExtensionOperation::CountTrailingZeros | ExtensionOperation::CountLeadingZeros);
    let overflow: &[FlagRule] = if undefined.overflow { &OVERFLOW_RULES } else { &OVERFLOW_RULES[..1] };
    let sign: &[FlagRule] = match (undefined.sign, counts) {
        (false, _) => &RULES[..1],
        (true, true) => &COUNT_SIGN_RULES,
        (true, false) => &RULES,
    };
    let auxiliary: &[UndefinedAuxiliaryCarry] = if undefined.auxiliary_carry { &AUXILIARY_CARRY } else { &AUXILIARY_CARRY[..1] };
    let parity: &[FlagRule] = if undefined.parity { &RULES } else { &RULES[..1] };
    // A pair stored across a page edge can write the bytes below it; CMPXCHG16B is aligned, so it never straddles.
    let across: &[StoreAcrossFault] = if form.writes_memory() && form.required_alignment().is_none() && form.place == OperandPlace::Memory {
        &ACROSS_FAULT
    } else {
        &ACROSS_FAULT[..1]
    };
    let mut behaviors = Vec::new();
    for &across_fault in across {
        for &overflow in overflow {
            for &sign in sign {
                for &auxiliary_carry in auxiliary {
                    for &parity in parity {
                        behaviors.push(PreciseInstruction::Extension {
                            instruction,
                            behavior: ExtensionBehavior {
                                undefined: ExtensionFlags { overflow, sign, auxiliary_carry, parity },
                                across_fault,
                            },
                        });
                    }
                }
            }
        }
    }
    behaviors
}

/// What an operation of one register-wide result leaves: the result and RFLAGS.
pub(crate) struct ExtensionOutcome {
    pub result: u64,
    /// MULX's low half, which goes to its second operand.
    pub low: Option<u64>,
    pub flags: Flags,
}

/// Applies an operation that computes one result from its sources, `length` bytes wide; `sources` are in the order
/// the operation names them, `data` EDX or RDX for MULX.
pub(crate) fn compute(
    operation: ExtensionOperation,
    sources: [u64; 2],
    data: u64,
    length: usize,
    flags: Flags,
    undefined: ExtensionFlags,
) -> ExtensionOutcome {
    use ExtensionOperation as Operation;
    let bits = length as u32 * 8;
    let mask = to_mask(length);
    let [first, second] = sources.map(|source| source & mask);
    let keeping = |result: u64| ExtensionOutcome { result: result & mask, low: None, flags };
    let (result, defined) = match operation {
        Operation::AndNot => {
            let result = if is_active(Bug::AndNotComplementsSecond) { first & !second } else { !first & second } & mask;
            (result, to_result_flags(result, length).difference(Flags::PARITY))
        }
        Operation::ExtractBitField => {
            let start = second & 0xFF;
            let field = (second >> 8) & 0xFF;
            let field = if is_active(Bug::ExtractBitFieldLengthFromLowByte) { start } else { field };
            let result = if start >= u64::from(bits) {
                0
            } else {
                let shifted = first >> start;
                if field >= 64 { shifted } else { shifted & ((1 << field) - 1) }
            };
            let mut status = Flags::empty();
            status.set(Flags::ZERO, result == 0);
            (result, status)
        }
        Operation::IsolateLowestSetBit => {
            let result = first.isolate_lowest_one() & mask;
            let mut status = to_result_flags(result, length).difference(Flags::PARITY);
            status.set(Flags::CARRY, first != 0);
            (result, status)
        }
        Operation::MaskUpToLowestSetBit => {
            let result = (first.wrapping_sub(1) ^ first) & mask;
            let mut status = to_result_flags(result, length).intersection(Flags::SIGN);
            status.set(Flags::CARRY, first == 0);
            (result, status)
        }
        Operation::ResetLowestSetBit => {
            let result = first.wrapping_sub(1) & first & mask;
            let mut status = to_result_flags(result, length).difference(Flags::PARITY);
            status.set(Flags::CARRY, first == 0 || is_active(Bug::ResetLowestSetBitCarryFromResult) && result == 0);
            (result, status)
        }
        Operation::ZeroHighBits => {
            let index = second & 0xFF;
            let index = if is_active(Bug::ZeroHighBitsIndexWholeRegister) { second } else { index };
            let result = if index < u64::from(bits) { first & ((1 << index) - 1) } else { first };
            let mut status = to_result_flags(result, length).difference(Flags::PARITY);
            status.set(Flags::CARRY, index > u64::from(bits) - 1);
            (result, status)
        }
        Operation::CountTrailingZeros | Operation::CountLeadingZeros => {
            let count = if operation == Operation::CountTrailingZeros {
                if first == 0 { bits } else { first.trailing_zeros() }
            } else if first == 0 {
                bits
            } else {
                first.leading_zeros() - (64 - bits)
            };
            let count = u64::from(count);
            let mut status = Flags::empty();
            let zero_source_carry = !(is_active(Bug::CountCarryFromResult) && operation == Operation::CountLeadingZeros);
            status.set(Flags::CARRY, if zero_source_carry { first == 0 } else { count == 0 });
            status.set(Flags::ZERO, count == 0);
            (count, status)
        }
        Operation::MultiplyWithoutFlags => {
            let product = u128::from(data & mask) * u128::from(first);
            return ExtensionOutcome {
                result: (product >> bits) as u64 & mask,
                low: Some(product as u64 & mask),
                flags,
            };
        }
        Operation::DepositBits => {
            let (mut result, mut taken) = (0, 0);
            for bit in 0..bits {
                if second >> bit & 1 != 0 {
                    result |= (first >> taken & 1) << bit;
                    taken += 1;
                }
            }
            return keeping(result);
        }
        Operation::ExtractBits => {
            let (mut result, mut placed) = (0, 0);
            for bit in 0..bits {
                if second >> bit & 1 != 0 {
                    result |= (first >> bit & 1) << placed;
                    placed += 1;
                }
            }
            if is_active(Bug::ExtractBitsReversed) {
                result = result.reverse_bits() >> (64 - placed.max(1));
            }
            return keeping(result);
        }
        Operation::RotateRightWithoutFlags => {
            let count = (second & u64::from(bits - 1)) as u32;
            let rotated = if count == 0 { first } else { (first >> count) | (first << (bits - count)) };
            return keeping(rotated);
        }
        Operation::ShiftRightArithmeticWithoutFlags => {
            let count = second & u64::from(bits - 1);
            let signed = crate::integer::sign_extend(first, length) as i64;
            return keeping((signed >> count) as u64);
        }
        Operation::ShiftLeftWithoutFlags => {
            let count = second & if is_active(Bug::ShiftLeftWithoutFlagsCountUnmasked) { 0xFF } else { u64::from(bits - 1) };
            return keeping(first.checked_shl(count as u32).unwrap_or(0));
        }
        Operation::ShiftRightLogicalWithoutFlags => {
            let count = second & u64::from(bits - 1);
            return keeping(first >> count);
        }
        Operation::AddWithCarryFlag | Operation::AddWithOverflowFlag => {
            let flag = if operation == Operation::AddWithCarryFlag { Flags::CARRY } else { Flags::OVERFLOW };
            let carry_in = flags.contains(flag) && !(is_active(Bug::AddWithOverflowFlagIgnoresOverflow) && flag == Flags::OVERFLOW);
            let wide = u128::from(first) + u128::from(second) + u128::from(carry_in);
            let mut changed = flags;
            changed.set(flag, (wide >> bits) & 1 != 0);
            return ExtensionOutcome { result: wide as u64 & mask, low: None, flags: changed };
        }
        Operation::MoveSwappingBytes => {
            let swapped = first.swap_bytes() >> (64 - bits);
            return keeping(swapped);
        }
        Operation::CompareExchangePair => panic!("CMPXCHG8B and CMPXCHG16B compute a pair"),
    };
    let undefined_flags = operation.list_undefined_flags();
    let from_result = to_result_flags(result, length);
    let mut status = defined;
    let mut rule = |flag: Flags, varies: bool, rule: FlagRule| {
        if varies {
            let value = match rule {
                FlagRule::FromResult => from_result.contains(flag),
                FlagRule::Cleared => false,
                FlagRule::Unchanged => flags.contains(flag),
            };
            status.set(flag, value);
        }
    };
    rule(Flags::OVERFLOW, undefined_flags.overflow, undefined.overflow);
    rule(Flags::SIGN, undefined_flags.sign, undefined.sign);
    rule(Flags::PARITY, undefined_flags.parity, undefined.parity);
    if undefined_flags.auxiliary_carry {
        let value = match undefined.auxiliary_carry {
            UndefinedAuxiliaryCarry::Cleared => false,
            UndefinedAuxiliaryCarry::Set => true,
            UndefinedAuxiliaryCarry::Unchanged => flags.contains(Flags::AUXILIARY_CARRY),
        };
        status.set(Flags::AUXILIARY_CARRY, value);
    }
    ExtensionOutcome { result, low: None, flags: with_status(flags, status, Flags::STATUS) }
}

use ExtensionOperation::{
    AddWithCarryFlag, AddWithOverflowFlag, AndNot, CompareExchangePair, CountLeadingZeros, CountTrailingZeros,
    DepositBits, ExtractBitField, ExtractBits, IsolateLowestSetBit, MaskUpToLowestSetBit, MoveSwappingBytes,
    MultiplyWithoutFlags, ResetLowestSetBit, RotateRightWithoutFlags, ShiftLeftWithoutFlags,
    ShiftRightArithmeticWithoutFlags, ShiftRightLogicalWithoutFlags, ZeroHighBits,
};
use OperandPlace::{LockedMemory, Memory, Register};

const fn form(instruction: Instruction, operation: ExtensionOperation, place: OperandPlace, codes: &'static [Code]) -> ExtensionForm {
    ExtensionForm { instruction, operation, place, codes }
}

/// Every extension form modeled, extension by extension.
pub static EXTENSION_FORMS: &[ExtensionForm] = &[
    // BMI1
    form(Instruction::AndnGpr32dGpr32dGpr32d, AndNot, Register, &[Code::VEX_Andn_r32_r32_rm32]),
    form(Instruction::AndnGpr32dGpr32dMemd, AndNot, Memory, &[Code::VEX_Andn_r32_r32_rm32]),
    form(Instruction::AndnGpr64qGpr64qGpr64q, AndNot, Register, &[Code::VEX_Andn_r64_r64_rm64]),
    form(Instruction::AndnGpr64qGpr64qMemq, AndNot, Memory, &[Code::VEX_Andn_r64_r64_rm64]),
    form(Instruction::BextrGpr32dGpr32dGpr32d, ExtractBitField, Register, &[Code::VEX_Bextr_r32_rm32_r32]),
    form(Instruction::BextrGpr32dMemdGpr32d, ExtractBitField, Memory, &[Code::VEX_Bextr_r32_rm32_r32]),
    form(Instruction::BextrGpr64qGpr64qGpr64q, ExtractBitField, Register, &[Code::VEX_Bextr_r64_rm64_r64]),
    form(Instruction::BextrGpr64qMemqGpr64q, ExtractBitField, Memory, &[Code::VEX_Bextr_r64_rm64_r64]),
    form(Instruction::BlsiGpr32dGpr32d, IsolateLowestSetBit, Register, &[Code::VEX_Blsi_r32_rm32]),
    form(Instruction::BlsiGpr32dMemd, IsolateLowestSetBit, Memory, &[Code::VEX_Blsi_r32_rm32]),
    form(Instruction::BlsiGpr64qGpr64q, IsolateLowestSetBit, Register, &[Code::VEX_Blsi_r64_rm64]),
    form(Instruction::BlsiGpr64qMemq, IsolateLowestSetBit, Memory, &[Code::VEX_Blsi_r64_rm64]),
    form(Instruction::BlsmskGpr32dGpr32d, MaskUpToLowestSetBit, Register, &[Code::VEX_Blsmsk_r32_rm32]),
    form(Instruction::BlsmskGpr32dMemd, MaskUpToLowestSetBit, Memory, &[Code::VEX_Blsmsk_r32_rm32]),
    form(Instruction::BlsmskGpr64qGpr64q, MaskUpToLowestSetBit, Register, &[Code::VEX_Blsmsk_r64_rm64]),
    form(Instruction::BlsmskGpr64qMemq, MaskUpToLowestSetBit, Memory, &[Code::VEX_Blsmsk_r64_rm64]),
    form(Instruction::BlsrGpr32dGpr32d, ResetLowestSetBit, Register, &[Code::VEX_Blsr_r32_rm32]),
    form(Instruction::BlsrGpr32dMemd, ResetLowestSetBit, Memory, &[Code::VEX_Blsr_r32_rm32]),
    form(Instruction::BlsrGpr64qGpr64q, ResetLowestSetBit, Register, &[Code::VEX_Blsr_r64_rm64]),
    form(Instruction::BlsrGpr64qMemq, ResetLowestSetBit, Memory, &[Code::VEX_Blsr_r64_rm64]),
    form(Instruction::TzcntGprvGprv, CountTrailingZeros, Register, &[Code::Tzcnt_r16_rm16, Code::Tzcnt_r32_rm32, Code::Tzcnt_r64_rm64]),
    form(Instruction::TzcntGprvMemv, CountTrailingZeros, Memory, &[Code::Tzcnt_r16_rm16, Code::Tzcnt_r32_rm32, Code::Tzcnt_r64_rm64]),
    // BMI2
    form(Instruction::BzhiGpr32dGpr32dGpr32d, ZeroHighBits, Register, &[Code::VEX_Bzhi_r32_rm32_r32]),
    form(Instruction::BzhiGpr32dMemdGpr32d, ZeroHighBits, Memory, &[Code::VEX_Bzhi_r32_rm32_r32]),
    form(Instruction::BzhiGpr64qGpr64qGpr64q, ZeroHighBits, Register, &[Code::VEX_Bzhi_r64_rm64_r64]),
    form(Instruction::BzhiGpr64qMemqGpr64q, ZeroHighBits, Memory, &[Code::VEX_Bzhi_r64_rm64_r64]),
    form(Instruction::MulxGpr32dGpr32dGpr32d, MultiplyWithoutFlags, Register, &[Code::VEX_Mulx_r32_r32_rm32]),
    form(Instruction::MulxGpr32dGpr32dMemd, MultiplyWithoutFlags, Memory, &[Code::VEX_Mulx_r32_r32_rm32]),
    form(Instruction::MulxGpr64qGpr64qGpr64q, MultiplyWithoutFlags, Register, &[Code::VEX_Mulx_r64_r64_rm64]),
    form(Instruction::MulxGpr64qGpr64qMemq, MultiplyWithoutFlags, Memory, &[Code::VEX_Mulx_r64_r64_rm64]),
    form(Instruction::PdepGpr32dGpr32dGpr32d, DepositBits, Register, &[Code::VEX_Pdep_r32_r32_rm32]),
    form(Instruction::PdepGpr32dGpr32dMemd, DepositBits, Memory, &[Code::VEX_Pdep_r32_r32_rm32]),
    form(Instruction::PdepGpr64qGpr64qGpr64q, DepositBits, Register, &[Code::VEX_Pdep_r64_r64_rm64]),
    form(Instruction::PdepGpr64qGpr64qMemq, DepositBits, Memory, &[Code::VEX_Pdep_r64_r64_rm64]),
    form(Instruction::PextGpr32dGpr32dGpr32d, ExtractBits, Register, &[Code::VEX_Pext_r32_r32_rm32]),
    form(Instruction::PextGpr32dGpr32dMemd, ExtractBits, Memory, &[Code::VEX_Pext_r32_r32_rm32]),
    form(Instruction::PextGpr64qGpr64qGpr64q, ExtractBits, Register, &[Code::VEX_Pext_r64_r64_rm64]),
    form(Instruction::PextGpr64qGpr64qMemq, ExtractBits, Memory, &[Code::VEX_Pext_r64_r64_rm64]),
    form(Instruction::RorxGpr32dGpr32dImmb, RotateRightWithoutFlags, Register, &[Code::VEX_Rorx_r32_rm32_imm8]),
    form(Instruction::RorxGpr32dMemdImmb, RotateRightWithoutFlags, Memory, &[Code::VEX_Rorx_r32_rm32_imm8]),
    form(Instruction::RorxGpr64qGpr64qImmb, RotateRightWithoutFlags, Register, &[Code::VEX_Rorx_r64_rm64_imm8]),
    form(Instruction::RorxGpr64qMemqImmb, RotateRightWithoutFlags, Memory, &[Code::VEX_Rorx_r64_rm64_imm8]),
    form(Instruction::SarxGpr32dGpr32dGpr32d, ShiftRightArithmeticWithoutFlags, Register, &[Code::VEX_Sarx_r32_rm32_r32]),
    form(Instruction::SarxGpr32dMemdGpr32d, ShiftRightArithmeticWithoutFlags, Memory, &[Code::VEX_Sarx_r32_rm32_r32]),
    form(Instruction::SarxGpr64qGpr64qGpr64q, ShiftRightArithmeticWithoutFlags, Register, &[Code::VEX_Sarx_r64_rm64_r64]),
    form(Instruction::SarxGpr64qMemqGpr64q, ShiftRightArithmeticWithoutFlags, Memory, &[Code::VEX_Sarx_r64_rm64_r64]),
    form(Instruction::ShlxGpr32dGpr32dGpr32d, ShiftLeftWithoutFlags, Register, &[Code::VEX_Shlx_r32_rm32_r32]),
    form(Instruction::ShlxGpr32dMemdGpr32d, ShiftLeftWithoutFlags, Memory, &[Code::VEX_Shlx_r32_rm32_r32]),
    form(Instruction::ShlxGpr64qGpr64qGpr64q, ShiftLeftWithoutFlags, Register, &[Code::VEX_Shlx_r64_rm64_r64]),
    form(Instruction::ShlxGpr64qMemqGpr64q, ShiftLeftWithoutFlags, Memory, &[Code::VEX_Shlx_r64_rm64_r64]),
    form(Instruction::ShrxGpr32dGpr32dGpr32d, ShiftRightLogicalWithoutFlags, Register, &[Code::VEX_Shrx_r32_rm32_r32]),
    form(Instruction::ShrxGpr32dMemdGpr32d, ShiftRightLogicalWithoutFlags, Memory, &[Code::VEX_Shrx_r32_rm32_r32]),
    form(Instruction::ShrxGpr64qGpr64qGpr64q, ShiftRightLogicalWithoutFlags, Register, &[Code::VEX_Shrx_r64_rm64_r64]),
    form(Instruction::ShrxGpr64qMemqGpr64q, ShiftRightLogicalWithoutFlags, Memory, &[Code::VEX_Shrx_r64_rm64_r64]),
    // ADX
    form(Instruction::AdcxGpr32dGpr32d, AddWithCarryFlag, Register, &[Code::Adcx_r32_rm32]),
    form(Instruction::AdcxGpr32dMemd, AddWithCarryFlag, Memory, &[Code::Adcx_r32_rm32]),
    form(Instruction::AdcxGpr64qGpr64q, AddWithCarryFlag, Register, &[Code::Adcx_r64_rm64]),
    form(Instruction::AdcxGpr64qMemq, AddWithCarryFlag, Memory, &[Code::Adcx_r64_rm64]),
    form(Instruction::AdoxGpr32dGpr32d, AddWithOverflowFlag, Register, &[Code::Adox_r32_rm32]),
    form(Instruction::AdoxGpr32dMemd, AddWithOverflowFlag, Memory, &[Code::Adox_r32_rm32]),
    form(Instruction::AdoxGpr64qGpr64q, AddWithOverflowFlag, Register, &[Code::Adox_r64_rm64]),
    form(Instruction::AdoxGpr64qMemq, AddWithOverflowFlag, Memory, &[Code::Adox_r64_rm64]),
    // LZCNT
    form(Instruction::LzcntGprvGprv, CountLeadingZeros, Register, &[Code::Lzcnt_r16_rm16, Code::Lzcnt_r32_rm32, Code::Lzcnt_r64_rm64]),
    form(Instruction::LzcntGprvMemv, CountLeadingZeros, Memory, &[Code::Lzcnt_r16_rm16, Code::Lzcnt_r32_rm32, Code::Lzcnt_r64_rm64]),
    // MOVBE
    form(Instruction::MovbeGprvMemv, MoveSwappingBytes, Memory, &[Code::Movbe_r16_m16, Code::Movbe_r32_m32, Code::Movbe_r64_m64]),
    form(Instruction::MovbeMemvGprv, MoveSwappingBytes, Memory, &[Code::Movbe_m16_r16, Code::Movbe_m32_r32, Code::Movbe_m64_r64]),
    // CMPXCHG8B, CMPXCHG16B
    form(Instruction::Cmpxchg8bMemq, CompareExchangePair, Memory, &[Code::Cmpxchg8b_m64]),
    form(Instruction::Cmpxchg8bLockMemq, CompareExchangePair, LockedMemory, &[Code::Cmpxchg8b_m64]),
    form(Instruction::Cmpxchg16bMemdq, CompareExchangePair, Memory, &[Code::Cmpxchg16b_m128]),
    form(Instruction::Cmpxchg16bLockMemdq, CompareExchangePair, LockedMemory, &[Code::Cmpxchg16b_m128]),
];
