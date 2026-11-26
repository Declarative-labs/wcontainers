//! The legacy integer forms that share one skeleton: an operand that is a register or memory, read, combined with a
//! register, an immediate, CL or nothing by the family's operation, and written back unless the family only compares.
//!
//! A form is one row of [`INTEGER_FORMS`]: its family's operation, where its register-or-memory operand lies, and
//! the iced-x86 `Code` of each of its operand sizes. One more form of a family already here costs that one row.

use iced_x86::{Code, OpCodeOperandKind};

use crate::Flags;
use crate::planted::{Bug, is_active};
use crate::behavior::{
    BitScanFlags, CarryBeyondWidth, DoubleShiftFill, ExchangeMismatch, FlagRule, OverflowAfterShift, ScanOfZero, SixteenBitSwap,
    UndefinedAuxiliaryCarry, UndefinedFlags, UndefinedStatusFlags,
};
use crate::forms::Instruction;

/// What a family does with the destination and the source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operation {
    Add,
    Or,
    AddWithCarry,
    SubtractWithBorrow,
    And,
    Subtract,
    Xor,
    Compare,
    Test,
    Increment,
    Decrement,
    Negate,
    Not,
    ShiftLeft,
    ShiftRightLogical,
    ShiftRightArithmetic,
    RotateLeft,
    RotateRight,
    RotateLeftThroughCarry,
    RotateRightThroughCarry,
    Move,
    MoveIf(Condition),
    SetIf(Condition),
    /// IMUL with two or three operands: the low half of the signed product, the third operand's immediate by the
    /// second operand where there is one.
    MultiplySigned,
    /// MOVZX.
    ZeroExtend,
    /// MOVSX and MOVSXD.
    SignExtend,
    /// XCHG.
    Exchange,
    /// XADD.
    ExchangeAdd,
    /// MUL and IMUL with one operand: the accumulator by the operand, the product's halves into the accumulator and
    /// the data register (AL and AH for 8 bits).
    MultiplyWide { signed: bool },
    /// DIV and IDIV: the data register and the accumulator (AX for 8 bits) by the operand, the quotient into the
    /// accumulator and the remainder into the data register (AL and AH for 8 bits); #DE for a zero divisor or a
    /// quotient too wide.
    DivideWide { signed: bool },
    /// CBW, CWDE, CDQE: the accumulator's low half sign-extended into it.
    SignExtendAccumulator,
    /// CWD, CDQ, CQO: the accumulator's sign into every bit of the data register.
    SignIntoData,
    ClearCarry,
    SetCarry,
    ComplementCarry,
    ClearDirection,
    SetDirection,
    /// LAHF: SF, ZF, AF, PF and CF into AH, bit 1 set.
    LoadStatusIntoHigh,
    /// SAHF: SF, ZF, AF, PF and CF from AH.
    StoreHighIntoStatus,
    /// NOP, with or without an operand, which it neither reads nor writes.
    NoOperation,
    /// SHLD.
    ShiftLeftDouble,
    /// SHRD.
    ShiftRightDouble,
    /// BT, BTS, BTR and BTC: the bit at the offset into CF, then changed or not. With a register offset into memory
    /// the access moves by the offset's whole operands, signed.
    BitTest(BitChange),
    /// BSF: the index of the source's lowest set bit; ZF set and the destination as the behavior leaves it for a
    /// source of zero.
    BitScanForward,
    /// BSR: the index of the source's highest set bit, as BSF otherwise.
    BitScanReverse,
    /// BSWAP.
    ByteSwap,
    /// CMPXCHG: compares the accumulator with the destination, as CMP does, and writes the source there when they
    /// are equal, or the destination into the accumulator when not.
    CompareExchange,
    /// LEA: the memory operand's address, before any segment base, in the destination's width.
    LoadEffectiveAddress,
    /// UD0 and UD1, which raise #UD once fetched whole.
    RaiseInvalidOpcode,
    /// POPCNT: the source's set bits; ZF set for a source of zero, every other status flag cleared.
    PopulationCount,
}

/// What BT, BTS, BTR and BTC do with the bit they test.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BitChange {
    Keep,
    Set,
    Reset,
    Complement,
}

/// The general registers an operation reads besides its operands, by number.
const ACCUMULATOR: usize = 0;
const DATA: usize = 2;

/// A condition on the status flags, as Jcc, CMOVcc and SETcc name it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Condition {
    Overflow,
    NotOverflow,
    Below,
    NotBelow,
    Zero,
    NotZero,
    BelowOrEqual,
    NotBelowOrEqual,
    Sign,
    NotSign,
    Parity,
    NotParity,
    Less,
    NotLess,
    LessOrEqual,
    NotLessOrEqual,
}

/// Where the form's register-or-memory operand lies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandPlace {
    Register,
    Memory,
    /// Memory, with the LOCK prefix.
    LockedMemory,
}

/// One legacy integer form: its family's operation, its operand's place, and its codes by operand size, smallest
/// first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IntegerForm {
    pub instruction: Instruction,
    pub operation: Operation,
    pub place: OperandPlace,
    pub codes: &'static [Code],
}

/// What the operation leaves: the value to write to the destination, if it writes one, the value to write to the
/// second operand, a register, if it writes one (XCHG, XADD), RFLAGS, and the registers it writes besides its
/// operands, or the exception it raises.
pub(crate) struct Outcome {
    pub result: Option<u64>,
    pub second: Option<u64>,
    pub flags: Flags,
    pub implicit: [Option<(iced_x86::Register, u64)>; 2],
    pub exception: Option<crate::Exception>,
}

/// Which results the family leaves undefined, and so which fields of [`UndefinedFlags`] its behaviors vary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UndefinedResults {
    /// OF after a shift or rotate by more than one bit.
    pub overflow: bool,
    /// AF after a logical operation, or a shift by at least one bit.
    pub auxiliary_carry: bool,
    /// CF after SHL or SHR of an 8-bit or 16-bit operand by its width or more.
    pub carry_beyond_width: bool,
    /// SF, ZF and PF after IMUL, MUL and IMUL.
    pub result_flags: bool,
    /// Every status flag after DIV and IDIV.
    pub after_divide: bool,
    /// The result and flags of SHLD and SHRD of 16 bits by more than 16.
    pub fill_beyond_width: bool,
    /// OF, SF, AF and PF after BT, BTS, BTR and BTC.
    pub after_bit_test: bool,
    /// CF, OF, SF, AF and PF after BSF and BSR.
    pub after_bit_scan: bool,
    /// The destination of BSF and BSR for a source of zero.
    pub scan_of_zero: bool,
    /// The result of BSWAP of 16 bits.
    pub swap_of_sixteen_bits: bool,
    /// Whether CMPXCHG of a register writes it back when the accumulator differs.
    pub exchange_mismatch: bool,
}

impl Operation {
    /// Whether the operation writes its destination.
    pub const fn writes_destination(self) -> bool {
        !matches!(
            self,
            Self::Compare
                | Self::Test
                | Self::MultiplyWide { .. }
                | Self::DivideWide { .. }
                | Self::NoOperation
                | Self::BitTest(BitChange::Keep)
                | Self::LoadEffectiveAddress
                | Self::RaiseInvalidOpcode
        )
    }

    /// Whether the operation reaches its operand in memory at all: NOP, LEA, UD0 and UD1 do not.
    pub const fn accesses_memory(self) -> bool {
        !matches!(self, Self::NoOperation | Self::LoadEffectiveAddress | Self::RaiseInvalidOpcode)
    }

    /// Whether the operation reads the values of its operands at all: UD0 and UD1 do not.
    pub const fn reads_operands(self) -> bool {
        !matches!(self, Self::RaiseInvalidOpcode)
    }

    /// The general registers the operation reads besides its operands, by number: the accumulator and the data
    /// register.
    pub const fn list_implicit_reads(self) -> &'static [usize] {
        match self {
            Self::MultiplyWide { .. }
            | Self::SignExtendAccumulator
            | Self::SignIntoData
            | Self::LoadStatusIntoHigh
            | Self::StoreHighIntoStatus => &[ACCUMULATOR],
            Self::DivideWide { .. } => &[ACCUMULATOR, DATA],
            Self::CompareExchange => &[ACCUMULATOR],
            _ => &[],
        }
    }

    /// Whether the operation reads its destination before it writes it.
    pub const fn reads_destination(self) -> bool {
        !matches!(
            self,
            Self::Move
                | Self::SetIf(_)
                | Self::ZeroExtend
                | Self::PopulationCount
                | Self::SignExtend
                | Self::NoOperation
                | Self::LoadEffectiveAddress
                | Self::RaiseInvalidOpcode
        )
    }

    /// The flags the operation reads.
    pub const fn reads_flags(self) -> Flags {
        match self {
            Self::AddWithCarry
            | Self::SubtractWithBorrow
            | Self::RotateLeftThroughCarry
            | Self::RotateRightThroughCarry => Flags::CARRY,
            Self::MoveIf(condition) | Self::SetIf(condition) => condition.reads(),
            Self::ComplementCarry => Flags::CARRY,
            Self::LoadStatusIntoHigh => Flags::STATUS.difference(Flags::OVERFLOW),
            _ => Flags::empty(),
        }
    }

    pub const fn is_shift_or_rotate(self) -> bool {
        matches!(
            self,
            Self::ShiftLeft
                | Self::ShiftRightLogical
                | Self::ShiftRightArithmetic
                | Self::RotateLeft
                | Self::RotateRight
                | Self::RotateLeftThroughCarry
                | Self::RotateRightThroughCarry
                | Self::ShiftLeftDouble
                | Self::ShiftRightDouble
        )
    }

    /// The results Intel's manual leaves undefined for the family, with a count other than the constant 1.
    pub const fn list_undefined_results(self) -> UndefinedResults {
        let shift = matches!(
            self,
            Self::ShiftLeft
                | Self::ShiftRightLogical
                | Self::ShiftRightArithmetic
                | Self::ShiftLeftDouble
                | Self::ShiftRightDouble
        );
        UndefinedResults {
            overflow: self.is_shift_or_rotate(),
            auxiliary_carry: shift
                || matches!(
                    self,
                    Self::And | Self::Or | Self::Xor | Self::Test | Self::MultiplySigned | Self::MultiplyWide { .. }
                ),
            carry_beyond_width: matches!(self, Self::ShiftLeft | Self::ShiftRightLogical),
            result_flags: matches!(self, Self::MultiplySigned | Self::MultiplyWide { .. }),
            after_divide: matches!(self, Self::DivideWide { .. }),
            fill_beyond_width: matches!(self, Self::ShiftLeftDouble | Self::ShiftRightDouble),
            after_bit_test: matches!(self, Self::BitTest(_)),
            after_bit_scan: matches!(self, Self::BitScanForward | Self::BitScanReverse),
            scan_of_zero: matches!(self, Self::BitScanForward | Self::BitScanReverse),
            swap_of_sixteen_bits: matches!(self, Self::ByteSwap),
            exchange_mismatch: matches!(self, Self::CompareExchange),
        }
    }

    /// Applies the operation to a destination of `length` bytes and a source of `source_length` bytes (they differ
    /// only for MOVZX, MOVSX and MOVSXD), given the accumulator and the data register, RFLAGS before and the chosen
    /// undefined results.
    #[expect(clippy::too_many_arguments, reason = "each is one input the chip reads")]
    pub(crate) fn apply(
        self,
        destination: u64,
        source: u64,
        length: usize,
        source_length: usize,
        implicit: [u64; 2],
        count: u64,
        flags: Flags,
        undefined: UndefinedFlags,
    ) -> Outcome {
        let outcome = self.compute(destination, source, length, source_length, implicit, count, flags, undefined);
        crate::planted::mutate_integer(
            self, destination, source, length, source_length, implicit, count, flags, undefined, outcome,
        )
    }

    /// The outcome as the manuals define it, which [`Self::apply`] gives unless a planted bug is on.
    #[expect(clippy::too_many_arguments, reason = "each is one input the chip reads")]
    fn compute(
        self,
        destination: u64,
        source: u64,
        length: usize,
        source_length: usize,
        [accumulator, data]: [u64; 2],
        count: u64,
        flags: Flags,
        undefined: UndefinedFlags,
    ) -> Outcome {
        let carry = flags.contains(Flags::CARRY);
        let arithmetic = |(result, status): (u64, Flags)| Outcome {
            second: None,
            result: Some(result),
            flags: with_status(flags, status, Flags::STATUS),
            implicit: [None, None],
            exception: None,
        };
        match self {
            Self::Add => arithmetic(add_with_carry(destination, source, false, length)),
            Self::AddWithCarry => arithmetic(add_with_carry(destination, source, carry, length)),
            Self::Subtract => arithmetic(subtract_with_borrow(destination, source, false, length)),
            Self::SubtractWithBorrow => arithmetic(subtract_with_borrow(destination, source, carry, length)),
            Self::Compare => Outcome {
                result: None,
                ..arithmetic(subtract_with_borrow(destination, source, false, length))
            },
            Self::And => logical(destination & source, length, flags, undefined),
            Self::Or => logical(destination | source, length, flags, undefined),
            Self::Xor => logical(destination ^ source, length, flags, undefined),
            Self::Test => Outcome {
                result: None,
                ..logical(destination & source, length, flags, undefined)
            },
            // INC and DEC leave CF as it was.
            Self::Increment => {
                let (sum, status) = add_with_carry(destination, 1, false, length);
                Outcome {
                    second: None,
                    result: Some(sum),
                    flags: with_status(flags, status, Flags::STATUS.difference(Flags::CARRY)),
                    implicit: [None, None],
                    exception: None,
                }
            }
            Self::Decrement => {
                let (difference, status) = subtract_with_borrow(destination, 1, false, length);
                Outcome {
                    second: None,
                    result: Some(difference),
                    flags: with_status(flags, status, Flags::STATUS.difference(Flags::CARRY)),
                    implicit: [None, None],
                    exception: None,
                }
            }
            Self::Negate => arithmetic(subtract_with_borrow(0, destination, false, length)),
            Self::Not => Outcome {
                second: None,
                result: Some(!destination & to_mask(length)),
                flags,
                implicit: [None, None],
                exception: None,
            },
            Self::Move => Outcome {
                second: None,
                result: Some(source & to_mask(length)),
                flags,
                implicit: [None, None],
                exception: None,
            },
            // A 32-bit destination is written, and so zero-extended, whether the condition holds or not.
            Self::MoveIf(condition) => Outcome {
                second: None,
                result: Some(if condition.holds(flags) { source } else { destination } & to_mask(length)),
                flags,
                implicit: [None, None],
                exception: None,
            },
            Self::SetIf(condition) => Outcome {
                second: None,
                result: Some(u64::from(condition.holds(flags))),
                flags,
                implicit: [None, None],
                exception: None,
            },
            Self::ShiftLeft
            | Self::ShiftRightLogical
            | Self::ShiftRightArithmetic
            | Self::RotateLeft
            | Self::RotateRight
            | Self::RotateLeftThroughCarry
            | Self::RotateRightThroughCarry => self.shift(destination, source, length, flags, undefined),
            Self::MultiplySigned => multiply_signed(destination, source, length, flags, undefined),
            Self::PopulationCount => {
                let value = source & to_mask(source_length);
                let zero = if value == 0 { Flags::ZERO } else { Flags::empty() };
                Outcome {
                    result: Some(u64::from(value.count_ones())),
                    ..Outcome::keeping(with_status(flags, zero, Flags::STATUS))
                }
            }
            Self::ZeroExtend => Outcome {
                result: Some(source & to_mask(source_length) & to_mask(length)),
                second: None,
                flags,
                implicit: [None, None],
                exception: None,
            },
            Self::SignExtend => Outcome {
                result: Some(sign_extend(source, source_length) & to_mask(length)),
                second: None,
                flags,
                implicit: [None, None],
                exception: None,
            },
            Self::Exchange => Outcome {
                result: Some(source & to_mask(length)),
                second: Some(destination & to_mask(length)),
                flags,
                implicit: [None, None],
                exception: None,
            },
            Self::ExchangeAdd => {
                let (sum, status) = add_with_carry(destination, source, false, length);
                Outcome {
                    result: Some(sum),
                    second: Some(destination & to_mask(length)),
                    flags: with_status(flags, status, Flags::STATUS),
                    ..Outcome::keeping(flags)
                }
            }
            Self::MultiplyWide { signed } => multiply_wide(signed, accumulator, destination, length, flags, undefined),
            Self::DivideWide { signed } => divide_wide(signed, [accumulator, data], destination, length, flags, undefined),
            Self::SignExtendAccumulator => Outcome {
                implicit: [Some((to_accumulator(length), sign_extend(accumulator, length / 2))), None],
                ..Outcome::keeping(flags)
            },
            Self::SignIntoData => {
                let negative = accumulator & (1 << (length * 8 - 1)) != 0;
                Outcome {
                    implicit: [Some((to_data(length), if negative { u64::MAX } else { 0 })), None],
                    ..Outcome::keeping(flags)
                }
            }
            Self::ClearCarry => Outcome::keeping(flags.difference(Flags::CARRY)),
            Self::SetCarry => Outcome::keeping(flags.union(Flags::CARRY)),
            Self::ComplementCarry => Outcome::keeping(flags.symmetric_difference(Flags::CARRY)),
            Self::ClearDirection => Outcome::keeping(flags.difference(Flags::DIRECTION)),
            Self::SetDirection => Outcome::keeping(flags.union(Flags::DIRECTION)),
            Self::LoadStatusIntoHigh => {
                const BIT_1: u64 = 1 << 1;
                let high = (flags & Flags::STATUS.difference(Flags::OVERFLOW)).bits() | BIT_1;
                Outcome {
                    implicit: [Some((iced_x86::Register::AH, high)), None],
                    ..Outcome::keeping(flags)
                }
            }
            Self::StoreHighIntoStatus => {
                let written = Flags::STATUS.difference(Flags::OVERFLOW);
                let high = Flags::from_bits_truncate((accumulator >> 8) & 0xFF);
                Outcome::keeping(with_status(flags, high, written))
            }
            Self::NoOperation => Outcome::keeping(flags),
            Self::ShiftLeftDouble | Self::ShiftRightDouble => {
                self.shift_double(destination, source, count, length, flags, undefined)
            }
            Self::BitTest(change) => test_bit(change, destination, source, length, flags, undefined),
            Self::BitScanForward | Self::BitScanReverse => {
                scan_bits(self == Self::BitScanForward, destination, source, length, flags, undefined)
            }
            Self::ByteSwap => Outcome {
                result: Some(swap_bytes(destination, length, undefined.swap_of_sixteen_bits)),
                ..Outcome::keeping(flags)
            },
            Self::CompareExchange => {
                let (_, status) = subtract_with_borrow(accumulator, destination, false, length);
                let flags = with_status(flags, status, Flags::STATUS);
                if (accumulator ^ destination) & to_mask(length) == 0 {
                    Outcome {
                        result: Some(source & to_mask(length)),
                        ..Outcome::keeping(flags)
                    }
                } else {
                    let written_back = undefined.exchange_mismatch == ExchangeMismatch::WrittenBack;
                    Outcome {
                        result: written_back.then_some(destination & to_mask(length)),
                        implicit: [Some((to_accumulator(length), destination)), None],
                        ..Outcome::keeping(flags)
                    }
                }
            }
            Self::LoadEffectiveAddress => Outcome {
                result: Some(source & to_mask(length)),
                ..Outcome::keeping(flags)
            },
            Self::RaiseInvalidOpcode => Outcome {
                exception: Some(crate::Exception::InvalidOpcode),
                ..Outcome::keeping(flags)
            },
        }
    }

    /// SHLD and SHRD, as one-bit steps of the destination and the source joined, the destination at the end the
    /// steps leave: the count masked to 5 bits, or 6 for a 64-bit operand; a count of 0 changes no flag but still
    /// writes the destination. Past the source, the steps shift in zeros or rotate the joined pair, as the behavior
    /// says; only 16 bits can be stepped that far.
    pub(crate) fn shift_double(
        self,
        destination: u64,
        source: u64,
        count: u64,
        length: usize,
        flags: Flags,
        undefined: UndefinedFlags,
    ) -> Outcome {
        let bits = length as u32 * 8;
        let mask = to_mask(length);
        let masked = count & if length == size_of::<u64>() { 0x3F } else { 0x1F };
        let destination = destination & mask;
        if masked == 0 {
            return Outcome {
                result: Some(destination),
                ..Outcome::keeping(flags)
            };
        }
        let width = 2 * bits;
        let joined_mask = if width == 128 { u128::MAX } else { (1 << width) - 1 };
        let left = self == Self::ShiftLeftDouble;
        let (high, low) = if left { (destination, source & mask) } else { (source & mask, destination) };
        let mut joined = (u128::from(high) << bits) | u128::from(low);
        let rotate = undefined.fill_beyond_width == DoubleShiftFill::Rotation;
        let to_destination = |joined: u128| if left { (joined >> bits) as u64 & mask } else { joined as u64 & mask };
        let sign_bit = 1 << (bits - 1);
        let (mut carry, mut first_overflow, mut last_overflow) = (false, false, false);
        for step in 0..masked {
            let before = to_destination(joined);
            if left {
                carry = (joined >> (width - 1)) & 1 != 0;
                joined = ((joined << 1) & joined_mask) | u128::from(rotate && carry);
            } else {
                carry = joined & 1 != 0;
                joined = (joined >> 1) | if rotate && carry { 1 << (width - 1) } else { 0 };
            }
            let overflow = (before ^ to_destination(joined)) & sign_bit != 0;
            if step == 0 {
                first_overflow = overflow;
            }
            last_overflow = overflow;
        }
        let result = to_destination(joined);
        let overflow = if masked == 1 {
            first_overflow
        } else {
            match undefined.overflow {
                OverflowAfterShift::FirstStepRule => first_overflow,
                OverflowAfterShift::LastStepRule => last_overflow,
                OverflowAfterShift::OperandSignXorResultSign => (destination ^ result) & sign_bit != 0,
                OverflowAfterShift::Cleared => false,
                OverflowAfterShift::Unchanged => flags.contains(Flags::OVERFLOW),
            }
        };
        let mut status = to_result_flags(result, length);
        status.set(Flags::CARRY, carry);
        status.set(Flags::OVERFLOW, overflow);
        let auxiliary_carry = match undefined.auxiliary_carry {
            UndefinedAuxiliaryCarry::Cleared => false,
            UndefinedAuxiliaryCarry::Set => true,
            UndefinedAuxiliaryCarry::Unchanged => flags.contains(Flags::AUXILIARY_CARRY),
        };
        status.set(Flags::AUXILIARY_CARRY, auxiliary_carry);
        Outcome {
            result: Some(result),
            ..Outcome::keeping(with_status(flags, status, Flags::STATUS))
        }
    }

    /// A shift or rotate, as one-bit steps: the count masked to 5 bits, or 6 for a 64-bit operand; a count of 0
    /// changes no flag but still writes the destination. RCL and RCR of 8 and 16 bits step the count modulo the
    /// width plus one, so a count that is a multiple of it changes no flag either.
    pub(crate) fn shift(self, value: u64, count: u64, length: usize, flags: Flags, undefined: UndefinedFlags) -> Outcome {
        let bits = (length * 8) as u64;
        let mut masked = count & if length == size_of::<u64>() { 0x3F } else { 0x1F };
        if (is_active(Bug::ShiftLeftQuadwordCountFiveBits) && self == Self::ShiftLeft)
            || (is_active(Bug::RotateLeftQuadwordCountFiveBits) && self == Self::RotateLeft)
        {
            masked = count & 0x1F;
        }
        if is_active(Bug::ShiftLeftByteCountThreeBits) && self == Self::ShiftLeft && length == 1 {
            masked = count & 0x7;
        }
        let count = match self {
            Self::RotateLeftThroughCarry if is_active(Bug::RotateLeftThroughCarryByteModuloEight) && length == 1 => masked % bits,
            Self::RotateRightThroughCarry if is_active(Bug::RotateRightThroughCarryWordModuloSixteen) && length == 2 => {
                masked % bits
            }
            Self::RotateLeftThroughCarry | Self::RotateRightThroughCarry => masked % (bits + 1),
            _ => masked,
        };
        let value = value & to_mask(length);
        if count == 0 {
            let mut flags = flags;
            if is_active(Bug::ShiftRightZeroCountClearsCarry) && self == Self::ShiftRightLogical {
                flags.remove(Flags::CARRY);
            }
            return Outcome {
                second: None,
                result: Some(value),
                flags,
                implicit: [None, None],
                exception: None,
            };
        }
        let (mut result, mut carry) = (value, flags.contains(Flags::CARRY));
        let (mut first_overflow, mut last_overflow) = (false, false);
        for step in 0..count {
            let overflow;
            (result, carry, overflow) = self.step(result, carry, bits);
            if step == 0 {
                first_overflow = overflow;
            }
            last_overflow = overflow;
        }
        let sign_bit = 1 << (bits - 1);
        let overflow = if masked == 1 {
            first_overflow
        } else {
            match undefined.overflow {
                OverflowAfterShift::FirstStepRule => first_overflow,
                OverflowAfterShift::LastStepRule => last_overflow,
                OverflowAfterShift::OperandSignXorResultSign => (value ^ result) & sign_bit != 0,
                OverflowAfterShift::Cleared => false,
                OverflowAfterShift::Unchanged => flags.contains(Flags::OVERFLOW),
            }
        };
        let beyond_width = masked >= bits && self.list_undefined_results().carry_beyond_width;
        let carry = match undefined.carry_beyond_width {
            CarryBeyondWidth::LastBitShiftedOut => carry,
            _ if !beyond_width => carry,
            CarryBeyondWidth::Cleared => false,
            CarryBeyondWidth::Unchanged => flags.contains(Flags::CARRY),
        };
        let mut changed = flags;
        changed.set(Flags::CARRY, carry);
        changed.set(Flags::OVERFLOW, overflow);
        if self.list_undefined_results().auxiliary_carry {
            changed = with_status(changed, to_result_flags(result, length), Flags::ZERO | Flags::SIGN | Flags::PARITY);
            let auxiliary_carry = match undefined.auxiliary_carry {
                UndefinedAuxiliaryCarry::Cleared => false,
                UndefinedAuxiliaryCarry::Set => true,
                UndefinedAuxiliaryCarry::Unchanged => flags.contains(Flags::AUXILIARY_CARRY),
            };
            changed.set(Flags::AUXILIARY_CARRY, auxiliary_carry);
        }
        Outcome {
            second: None,
            result: Some(result),
            flags: changed,
            implicit: [None, None],
            exception: None,
        }
    }

    /// One bit of a shift or rotate: the value and CF after it, and the OF the manual gives a one-bit count.
    fn step(self, value: u64, carry: bool, bits: u64) -> (u64, bool, bool) {
        let mask = to_mask((bits / 8) as usize);
        let sign_bit = 1 << (bits - 1);
        let top = value & sign_bit != 0;
        let bottom = value & 1 != 0;
        let below_top = |value: u64| value & (sign_bit >> 1) != 0;
        match self {
            Self::ShiftLeft => {
                let shifted = (value << 1) & mask;
                (shifted, top, (shifted & sign_bit != 0) != top)
            }
            Self::ShiftRightLogical => (value >> 1, bottom, top && !is_active(Bug::ShiftRightStepOverflowCleared)),
            Self::ShiftRightArithmetic if is_active(Bug::ShiftRightArithmeticByteDropsSign) && bits == 8 => {
                (value >> 1, bottom, false)
            }
            Self::ShiftRightArithmetic => {
                ((value >> 1) | (value & sign_bit), bottom, is_active(Bug::ShiftRightArithmeticOverflowFromSign) && top)
            }
            Self::RotateLeft => {
                let rotated = ((value << 1) & mask) | u64::from(top);
                (rotated, top, (rotated & sign_bit != 0) != top)
            }
            Self::RotateRight => {
                let rotated = (value >> 1) | if bottom { sign_bit } else { 0 };
                let other = if is_active(Bug::RotateRightOverflowFromTopAndCarry) { bottom } else { below_top(rotated) };
                (rotated, bottom, (rotated & sign_bit != 0) != other)
            }
            Self::RotateLeftThroughCarry => {
                let rotated = ((value << 1) & mask) | u64::from(carry);
                (rotated, top, (rotated & sign_bit != 0) != top)
            }
            Self::RotateRightThroughCarry => {
                let rotated_in = carry && !is_active(Bug::RotateRightThroughCarryCarryNotRotatedIn);
                let rotated = (value >> 1) | if rotated_in { sign_bit } else { 0 };
                (rotated, bottom, top != carry)
            }
            _ => unreachable!("{self:?} is no shift or rotate"),
        }
    }
}

impl Condition {
    pub const fn reads(self) -> Flags {
        match self {
            Self::Overflow | Self::NotOverflow => Flags::OVERFLOW,
            Self::Below | Self::NotBelow => Flags::CARRY,
            Self::Zero | Self::NotZero => Flags::ZERO,
            Self::BelowOrEqual | Self::NotBelowOrEqual => Flags::CARRY.union(Flags::ZERO),
            Self::Sign | Self::NotSign => Flags::SIGN,
            Self::Parity | Self::NotParity => Flags::PARITY,
            Self::Less | Self::NotLess => Flags::SIGN.union(Flags::OVERFLOW),
            Self::LessOrEqual | Self::NotLessOrEqual => Flags::ZERO.union(Flags::SIGN).union(Flags::OVERFLOW),
        }
    }

    pub fn holds(self, flags: Flags) -> bool {
        let less = flags.contains(Flags::SIGN) != flags.contains(Flags::OVERFLOW);
        let below_or_equal = flags.intersects(Flags::CARRY | Flags::ZERO);
        let less_or_equal = less || flags.contains(Flags::ZERO);
        match self {
            Self::Overflow => flags.contains(Flags::OVERFLOW),
            Self::NotOverflow => !flags.contains(Flags::OVERFLOW),
            Self::Below => flags.contains(Flags::CARRY),
            Self::NotBelow => !flags.contains(Flags::CARRY),
            Self::Zero => flags.contains(Flags::ZERO),
            Self::NotZero => !flags.contains(Flags::ZERO),
            Self::BelowOrEqual => below_or_equal,
            Self::NotBelowOrEqual => !below_or_equal,
            Self::Sign => flags.contains(Flags::SIGN),
            Self::NotSign => !flags.contains(Flags::SIGN),
            Self::Parity => flags.contains(Flags::PARITY),
            Self::NotParity => !flags.contains(Flags::PARITY),
            Self::Less if is_active(Bug::LessWithoutOverflow) => flags.contains(Flags::SIGN),
            Self::Less => less,
            Self::NotLess => !less,
            Self::LessOrEqual if is_active(Bug::LessOrEqualWithoutOverflow) => flags.intersects(Flags::ZERO | Flags::SIGN),
            Self::LessOrEqual => less_or_equal,
            Self::NotLessOrEqual => !less_or_equal,
        }
    }
}

/// AND, OR, XOR and TEST: CF and OF cleared, SF, ZF and PF from the result, AF as the behavior leaves it.
fn logical(result: u64, length: usize, flags: Flags, undefined: UndefinedFlags) -> Outcome {
    let mut status = to_result_flags(result, length);
    let auxiliary_carry = match undefined.auxiliary_carry {
        UndefinedAuxiliaryCarry::Cleared => false,
        UndefinedAuxiliaryCarry::Set => true,
        UndefinedAuxiliaryCarry::Unchanged => flags.contains(Flags::AUXILIARY_CARRY),
    };
    status.set(Flags::AUXILIARY_CARRY, auxiliary_carry);
    Outcome {
        second: None,
        result: Some(result & to_mask(length)),
        flags: with_status(flags, status, Flags::STATUS),
        implicit: [None, None],
        exception: None,
    }
}

/// BT, BTS, BTR and BTC: CF the bit at the offset modulo the width, ZF as it was, OF, SF, AF and PF as the behavior
/// leaves them.
pub(crate) fn test_bit(change: BitChange, value: u64, offset: u64, length: usize, flags: Flags, undefined: UndefinedFlags) -> Outcome {
    let bit = 1 << (offset & (length as u64 * 8 - 1));
    let value = value & to_mask(length);
    let result = match change {
        BitChange::Keep => None,
        BitChange::Set => Some(value | bit),
        BitChange::Reset => Some(value & !bit),
        BitChange::Complement => Some(value ^ bit),
    };
    let undefined_flags = Flags::OVERFLOW | Flags::SIGN | Flags::AUXILIARY_CARRY | Flags::PARITY;
    let mut changed = match undefined.after_bit_test {
        UndefinedStatusFlags::Unchanged => flags,
        UndefinedStatusFlags::Cleared => flags.difference(undefined_flags),
    };
    changed.set(Flags::CARRY, value & bit != 0);
    Outcome {
        result,
        ..Outcome::keeping(changed)
    }
}

/// BSF and BSR: the index of the lowest or highest set bit, ZF clear; for a source of zero ZF set and the
/// destination as the behavior leaves it. CF, OF, SF, AF and PF as the behavior leaves them.
fn scan_bits(forward: bool, destination: u64, source: u64, length: usize, flags: Flags, undefined: UndefinedFlags) -> Outcome {
    let source = source & to_mask(length);
    let index = match source {
        0 => 0,
        _ if forward => u64::from(source.trailing_zeros()),
        _ => u64::from(u64::BITS - 1 - source.leading_zeros()),
    };
    let result = match (source, undefined.scan_of_zero) {
        (0, ScanOfZero::Unchanged) => None,
        (0, ScanOfZero::WrittenBack) => Some(destination & to_mask(length)),
        (0, ScanOfZero::Zeroed) => Some(0),
        _ => Some(index),
    };
    let undefined_flags = Flags::STATUS.difference(Flags::ZERO);
    let parity_of = |value: u64| to_result_flags(value, 1).intersection(Flags::PARITY);
    let mut changed = match undefined.after_bit_scan {
        BitScanFlags::Unchanged => flags,
        BitScanFlags::Cleared => flags.difference(undefined_flags),
        BitScanFlags::ParityFromIndex => flags.difference(undefined_flags) | parity_of(index),
        BitScanFlags::ParityFromSource => flags.difference(undefined_flags) | parity_of(source),
    };
    changed.set(Flags::ZERO, source == 0);
    Outcome {
        result,
        ..Outcome::keeping(changed)
    }
}

/// BSWAP: the bytes of 32 or 64 bits in reverse order; 16 bits as the behavior leaves them.
fn swap_bytes(value: u64, length: usize, sixteen_bits: SixteenBitSwap) -> u64 {
    match (length, sixteen_bits) {
        (2, SixteenBitSwap::Zeroed) => 0,
        (2, SixteenBitSwap::Unchanged) => value & to_mask(length),
        (2, SixteenBitSwap::BytesExchanged) => u64::from((value as u16).swap_bytes()),
        (4, _) => u64::from((value as u32).swap_bytes()),
        _ => value.swap_bytes(),
    }
}

impl Outcome {
    /// What an operation that writes nothing but RFLAGS leaves.
    const fn keeping(flags: Flags) -> Self {
        Self {
            result: None,
            second: None,
            flags,
            implicit: [None, None],
            exception: None,
        }
    }
}

pub(crate) const fn to_accumulator(length: usize) -> iced_x86::Register {
    match length {
        1 => iced_x86::Register::AL,
        2 => iced_x86::Register::AX,
        4 => iced_x86::Register::EAX,
        _ => iced_x86::Register::RAX,
    }
}

pub(crate) const fn to_data(length: usize) -> iced_x86::Register {
    match length {
        1 => iced_x86::Register::AH,
        2 => iced_x86::Register::DX,
        4 => iced_x86::Register::EDX,
        _ => iced_x86::Register::RDX,
    }
}

/// SF, ZF and PF of a multiplication's low half by the behavior's rules, AF by its rule, CF and OF as given.
fn to_multiply_flags(low: u64, overflow: bool, length: usize, flags: Flags, undefined: UndefinedFlags) -> Flags {
    let from_result = to_result_flags(low, length);
    let mut status = Flags::empty();
    let rules = undefined.result_flags;
    for (flag, rule) in [(Flags::SIGN, rules.sign), (Flags::ZERO, rules.zero), (Flags::PARITY, rules.parity)] {
        let value = match rule {
            FlagRule::FromResult => from_result.contains(flag),
            FlagRule::Cleared => false,
            FlagRule::Unchanged => flags.contains(flag),
        };
        status.set(flag, value);
    }
    status.set(Flags::CARRY, overflow);
    status.set(Flags::OVERFLOW, overflow);
    let auxiliary_carry = match undefined.auxiliary_carry {
        UndefinedAuxiliaryCarry::Cleared => false,
        UndefinedAuxiliaryCarry::Set => true,
        UndefinedAuxiliaryCarry::Unchanged => flags.contains(Flags::AUXILIARY_CARRY),
    };
    status.set(Flags::AUXILIARY_CARRY, auxiliary_carry);
    with_status(flags, status, Flags::STATUS)
}

/// MUL and IMUL with one operand; CF and OF set where the high half is not the low half's extension.
fn multiply_wide(signed: bool, accumulator: u64, factor: u64, length: usize, flags: Flags, undefined: UndefinedFlags) -> Outcome {
    let bits = length as u32 * 8;
    let mask = to_mask(length);
    let product = if signed {
        (i128::from(sign_extend(accumulator, length) as i64) * i128::from(sign_extend(factor, length) as i64)) as u128
    } else {
        u128::from(accumulator & mask) * u128::from(factor & mask)
    };
    let low = product as u64 & mask;
    let high = (product >> bits) as u64 & mask;
    let extension = if signed && low & (1 << (bits - 1)) != 0 { mask } else { 0 };
    Outcome {
        flags: to_multiply_flags(low, high != extension, length, flags, undefined),
        implicit: [Some((to_accumulator(length), low)), Some((to_data(length), high))],
        ..Outcome::keeping(flags)
    }
}

/// DIV and IDIV, whose status flags all follow the behavior.
fn divide_wide(
    signed: bool,
    [accumulator, data]: [u64; 2],
    divisor: u64,
    length: usize,
    flags: Flags,
    undefined: UndefinedFlags,
) -> Outcome {
    let bits = length as u32 * 8;
    let mask = to_mask(length);
    // An 8-bit division divides AX, which the accumulator holds whole.
    let (low, high) = if length == 1 { (accumulator & 0xFF, (accumulator >> 8) & 0xFF) } else { (accumulator & mask, data & mask) };
    let dividend = (u128::from(high) << bits) | u128::from(low);
    let quotient_and_remainder = if signed {
        let unused = 128 - 2 * bits;
        let dividend = ((dividend << unused) as i128) >> unused;
        let divisor = i128::from(sign_extend(divisor, length) as i64);
        let lowest = -(1i128 << (bits - 1));
        dividend
            .checked_div(divisor)
            .filter(|quotient| (lowest..-lowest).contains(quotient))
            .map(|quotient| (quotient as u64 & mask, (dividend % divisor) as u64 & mask))
    } else {
        let divisor = u128::from(divisor & mask);
        dividend
            .checked_div(divisor)
            .filter(|quotient| *quotient <= u128::from(mask))
            .map(|quotient| (quotient as u64, (dividend % divisor) as u64))
    };
    let Some((quotient, remainder)) = quotient_and_remainder else {
        return Outcome {
            exception: Some(crate::Exception::DivideError),
            ..Outcome::keeping(flags)
        };
    };
    let status = match undefined.after_divide {
        UndefinedStatusFlags::Unchanged => flags,
        UndefinedStatusFlags::Cleared => flags.difference(Flags::STATUS),
    };
    Outcome {
        implicit: [Some((to_accumulator(length), quotient)), Some((to_data(length), remainder))],
        ..Outcome::keeping(status)
    }
}

/// IMUL: the low half of the signed product; CF and OF set where it differs from the whole product, SF, ZF, PF and AF
/// as the behavior leaves them.
fn multiply_signed(first: u64, second: u64, length: usize, flags: Flags, undefined: UndefinedFlags) -> Outcome {
    let product = i128::from(sign_extend(first, length) as i64) * i128::from(sign_extend(second, length) as i64);
    let low = product as u64 & to_mask(length);
    let overflow = i128::from(sign_extend(low, length) as i64) != product;
    Outcome {
        result: Some(low),
        ..Outcome::keeping(to_multiply_flags(low, overflow, length, flags, undefined))
    }
}

/// The value of `length` bytes, sign-extended to 64 bits.
pub(crate) const fn sign_extend(value: u64, length: usize) -> u64 {
    let unused = 64 - length as u32 * 8;
    (((value << unused) as i64) >> unused) as u64
}

/// RFLAGS with the `written` flags taken from `status` and the others as they were.
pub(crate) fn with_status(flags: Flags, status: Flags, written: Flags) -> Flags {
    flags.difference(written) | status.intersection(written)
}

/// The sum of two operands of `length` bytes and a carry, and the status flags ADD and ADC set.
pub(crate) fn add_with_carry(first: u64, second: u64, carry_in: bool, length: usize) -> (u64, Flags) {
    let mask = to_mask(length);
    let (first, second) = (first & mask, second & mask);
    let wide = u128::from(first) + u128::from(second) + u128::from(carry_in);
    let sum = wide as u64 & mask;
    let bits = length as u32 * 8;
    let sign_bit = 1 << (bits - 1);
    let mut status = to_result_flags(sum, length);
    status.set(Flags::CARRY, (wide >> bits) & 1 != 0);
    status.set(Flags::OVERFLOW, (first ^ sum) & (second ^ sum) & sign_bit != 0);
    status.set(Flags::AUXILIARY_CARRY, (first ^ second ^ sum) & 0x10 != 0);
    (sum, status)
}

/// The difference of two operands of `length` bytes less a borrow, and the status flags SUB, SBB and CMP set.
pub(crate) fn subtract_with_borrow(first: u64, second: u64, borrow: bool, length: usize) -> (u64, Flags) {
    let mask = to_mask(length);
    let (first, second) = (first & mask, second & mask);
    let wide = i128::from(first) - i128::from(second) - i128::from(borrow);
    let difference = wide as u64 & mask;
    let sign_bit = 1 << (length as u32 * 8 - 1);
    let mut status = to_result_flags(difference, length);
    status.set(Flags::CARRY, wide < 0);
    status.set(Flags::OVERFLOW, (first ^ second) & (first ^ difference) & sign_bit != 0);
    status.set(Flags::AUXILIARY_CARRY, (first ^ second ^ difference) & 0x10 != 0);
    (difference, status)
}

/// ZF, SF and PF of a result of `length` bytes.
pub(crate) fn to_result_flags(result: u64, length: usize) -> Flags {
    let sign_bit = 1 << (length as u32 * 8 - 1);
    let result = result & to_mask(length);
    let mut flags = Flags::empty();
    flags.set(Flags::ZERO, result == 0);
    flags.set(Flags::SIGN, result & sign_bit != 0);
    let ones = if is_active(Bug::ParityOverWholeResult) { result.count_ones() } else { (result as u8).count_ones() };
    flags.set(Flags::PARITY, ones.is_multiple_of(2));
    flags
}

pub(crate) const fn to_mask(length: usize) -> u64 {
    if length >= size_of::<u64>() {
        u64::MAX
    } else {
        (1 << (length * 8)) - 1
    }
}

impl IntegerForm {
    /// The results the manual leaves undefined for this form: a shift or rotate by the constant 1 leaves only AF;
    /// CMPXCHG writing back its destination shows only in a register wider than a byte.
    pub fn list_undefined_results(&self) -> UndefinedResults {
        let by_one = self.codes[0].op_code().op_kinds().contains(&OpCodeOperandKind::imm8_const_1);
        let undefined = self.operation.list_undefined_results();
        UndefinedResults {
            overflow: undefined.overflow && !by_one,
            carry_beyond_width: undefined.carry_beyond_width && !by_one,
            exchange_mismatch: undefined.exchange_mismatch && self.place == OperandPlace::Register && self.codes.len() > 1,
            ..undefined
        }
    }
}

impl IntegerForm {
    /// Whether the form can store across a page edge: it writes a destination in memory wider than one byte, without
    /// LOCK (a locked access across a page is a split lock, which the generated states leave out; XCHG with memory
    /// is locked always).
    pub fn can_store_across_pages(&self) -> bool {
        use OpCodeOperandKind as Kind;
        let destination = self.codes[0].op_code().op_kind(0);
        let destination_in_memory = matches!(
            destination,
            Kind::r16_or_mem | Kind::r32_or_mem | Kind::r64_or_mem | Kind::mem_offs
        );
        self.place == OperandPlace::Memory
            && destination_in_memory
            && self.codes.len() > 1
            && self.operation.writes_destination()
            && self.operation != Operation::Exchange
    }
}

/// The row of a legacy integer form, or `None` for any other form.
pub fn find_integer_form(instruction: Instruction) -> Option<&'static IntegerForm> {
    INTEGER_FORMS.iter().find(|form| form.instruction == instruction)
}

const fn form(
    instruction: Instruction,
    operation: Operation,
    place: OperandPlace,
    codes: &'static [Code],
) -> IntegerForm {
    IntegerForm {
        instruction,
        operation,
        place,
        codes,
    }
}

use OperandPlace::{LockedMemory, Memory, Register};
use Operation::{
    Add, AddWithCarry, And, BitScanForward, BitScanReverse, BitTest, ByteSwap, ClearCarry, CompareExchange,
    LoadEffectiveAddress, RaiseInvalidOpcode, ClearDirection, Compare, ComplementCarry, Decrement, DivideWide, Exchange,
    ExchangeAdd, Increment, LoadStatusIntoHigh, Move, MoveIf, MultiplySigned, MultiplyWide, Negate, NoOperation, Not, PopulationCount,
    Or, RotateLeft, RotateLeftThroughCarry, RotateRight, RotateRightThroughCarry, SetCarry, SetDirection, SetIf,
    ShiftLeft, ShiftLeftDouble, ShiftRightArithmetic, ShiftRightDouble, ShiftRightLogical, SignExtend, SignExtendAccumulator, SignIntoData,
    StoreHighIntoStatus, Subtract, SubtractWithBorrow, Test, Xor, ZeroExtend,
};

/// Every legacy integer form modeled so far, family by family.
pub static INTEGER_FORMS: &[IntegerForm] = &[
    // ADD
    form(Instruction::AddAlImmb, Add, Register, &[Code::Add_AL_imm8]),
    form(Instruction::AddGpr8Gpr800, Add, Register, &[Code::Add_rm8_r8]),
    form(Instruction::AddGpr8Gpr802, Add, Register, &[Code::Add_r8_rm8]),
    form(Instruction::AddGpr8Immb80r0, Add, Register, &[Code::Add_rm8_imm8]),
    form(Instruction::AddGpr8Memb, Add, Memory, &[Code::Add_r8_rm8]),
    form(Instruction::AddGprvGprv01, Add, Register, &[Code::Add_rm16_r16, Code::Add_rm32_r32, Code::Add_rm64_r64]),
    form(Instruction::AddGprvGprv03, Add, Register, &[Code::Add_r16_rm16, Code::Add_r32_rm32, Code::Add_r64_rm64]),
    form(Instruction::AddGprvImmb, Add, Register, &[Code::Add_rm16_imm8, Code::Add_rm32_imm8, Code::Add_rm64_imm8]),
    form(Instruction::AddGprvImmz, Add, Register, &[Code::Add_rm16_imm16, Code::Add_rm32_imm32, Code::Add_rm64_imm32]),
    form(Instruction::AddGprvMemv, Add, Memory, &[Code::Add_r16_rm16, Code::Add_r32_rm32, Code::Add_r64_rm64]),
    form(Instruction::AddLockMembGpr8, Add, LockedMemory, &[Code::Add_rm8_r8]),
    form(Instruction::AddLockMembImmb80r0, Add, LockedMemory, &[Code::Add_rm8_imm8]),
    form(Instruction::AddLockMemvGprv, Add, LockedMemory, &[Code::Add_rm16_r16, Code::Add_rm32_r32, Code::Add_rm64_r64]),
    form(Instruction::AddLockMemvImmb, Add, LockedMemory, &[Code::Add_rm16_imm8, Code::Add_rm32_imm8, Code::Add_rm64_imm8]),
    form(Instruction::AddLockMemvImmz, Add, LockedMemory, &[Code::Add_rm16_imm16, Code::Add_rm32_imm32, Code::Add_rm64_imm32]),
    form(Instruction::AddMembGpr8, Add, Memory, &[Code::Add_rm8_r8]),
    form(Instruction::AddMembImmb80r0, Add, Memory, &[Code::Add_rm8_imm8]),
    form(Instruction::AddMemvGprv, Add, Memory, &[Code::Add_rm16_r16, Code::Add_rm32_r32, Code::Add_rm64_r64]),
    form(Instruction::AddMemvImmb, Add, Memory, &[Code::Add_rm16_imm8, Code::Add_rm32_imm8, Code::Add_rm64_imm8]),
    form(Instruction::AddMemvImmz, Add, Memory, &[Code::Add_rm16_imm16, Code::Add_rm32_imm32, Code::Add_rm64_imm32]),
    form(Instruction::AddOraxImmz, Add, Register, &[Code::Add_AX_imm16, Code::Add_EAX_imm32, Code::Add_RAX_imm32]),
    // OR
    form(Instruction::OrAlImmb, Or, Register, &[Code::Or_AL_imm8]),
    form(Instruction::OrGpr8Gpr808, Or, Register, &[Code::Or_rm8_r8]),
    form(Instruction::OrGpr8Gpr80a, Or, Register, &[Code::Or_r8_rm8]),
    form(Instruction::OrGpr8Immb80r1, Or, Register, &[Code::Or_rm8_imm8]),
    form(Instruction::OrGpr8Memb, Or, Memory, &[Code::Or_r8_rm8]),
    form(Instruction::OrGprvGprv09, Or, Register, &[Code::Or_rm16_r16, Code::Or_rm32_r32, Code::Or_rm64_r64]),
    form(Instruction::OrGprvGprv0b, Or, Register, &[Code::Or_r16_rm16, Code::Or_r32_rm32, Code::Or_r64_rm64]),
    form(Instruction::OrGprvImmb, Or, Register, &[Code::Or_rm16_imm8, Code::Or_rm32_imm8, Code::Or_rm64_imm8]),
    form(Instruction::OrGprvImmz, Or, Register, &[Code::Or_rm16_imm16, Code::Or_rm32_imm32, Code::Or_rm64_imm32]),
    form(Instruction::OrGprvMemv, Or, Memory, &[Code::Or_r16_rm16, Code::Or_r32_rm32, Code::Or_r64_rm64]),
    form(Instruction::OrLockMembGpr8, Or, LockedMemory, &[Code::Or_rm8_r8]),
    form(Instruction::OrLockMembImmb80r1, Or, LockedMemory, &[Code::Or_rm8_imm8]),
    form(Instruction::OrLockMemvGprv, Or, LockedMemory, &[Code::Or_rm16_r16, Code::Or_rm32_r32, Code::Or_rm64_r64]),
    form(Instruction::OrLockMemvImmb, Or, LockedMemory, &[Code::Or_rm16_imm8, Code::Or_rm32_imm8, Code::Or_rm64_imm8]),
    form(Instruction::OrLockMemvImmz, Or, LockedMemory, &[Code::Or_rm16_imm16, Code::Or_rm32_imm32, Code::Or_rm64_imm32]),
    form(Instruction::OrMembGpr8, Or, Memory, &[Code::Or_rm8_r8]),
    form(Instruction::OrMembImmb80r1, Or, Memory, &[Code::Or_rm8_imm8]),
    form(Instruction::OrMemvGprv, Or, Memory, &[Code::Or_rm16_r16, Code::Or_rm32_r32, Code::Or_rm64_r64]),
    form(Instruction::OrMemvImmb, Or, Memory, &[Code::Or_rm16_imm8, Code::Or_rm32_imm8, Code::Or_rm64_imm8]),
    form(Instruction::OrMemvImmz, Or, Memory, &[Code::Or_rm16_imm16, Code::Or_rm32_imm32, Code::Or_rm64_imm32]),
    form(Instruction::OrOraxImmz, Or, Register, &[Code::Or_AX_imm16, Code::Or_EAX_imm32, Code::Or_RAX_imm32]),
    // ADC
    form(Instruction::AdcAlImmb, AddWithCarry, Register, &[Code::Adc_AL_imm8]),
    form(Instruction::AdcGpr8Gpr810, AddWithCarry, Register, &[Code::Adc_rm8_r8]),
    form(Instruction::AdcGpr8Gpr812, AddWithCarry, Register, &[Code::Adc_r8_rm8]),
    form(Instruction::AdcGpr8Immb80r2, AddWithCarry, Register, &[Code::Adc_rm8_imm8]),
    form(Instruction::AdcGpr8Memb, AddWithCarry, Memory, &[Code::Adc_r8_rm8]),
    form(Instruction::AdcGprvGprv11, AddWithCarry, Register, &[Code::Adc_rm16_r16, Code::Adc_rm32_r32, Code::Adc_rm64_r64]),
    form(Instruction::AdcGprvGprv13, AddWithCarry, Register, &[Code::Adc_r16_rm16, Code::Adc_r32_rm32, Code::Adc_r64_rm64]),
    form(Instruction::AdcGprvImmb, AddWithCarry, Register, &[Code::Adc_rm16_imm8, Code::Adc_rm32_imm8, Code::Adc_rm64_imm8]),
    form(Instruction::AdcGprvImmz, AddWithCarry, Register, &[Code::Adc_rm16_imm16, Code::Adc_rm32_imm32, Code::Adc_rm64_imm32]),
    form(Instruction::AdcGprvMemv, AddWithCarry, Memory, &[Code::Adc_r16_rm16, Code::Adc_r32_rm32, Code::Adc_r64_rm64]),
    form(Instruction::AdcLockMembGpr8, AddWithCarry, LockedMemory, &[Code::Adc_rm8_r8]),
    form(Instruction::AdcLockMembImmb80r2, AddWithCarry, LockedMemory, &[Code::Adc_rm8_imm8]),
    form(Instruction::AdcLockMemvGprv, AddWithCarry, LockedMemory, &[Code::Adc_rm16_r16, Code::Adc_rm32_r32, Code::Adc_rm64_r64]),
    form(Instruction::AdcLockMemvImmb, AddWithCarry, LockedMemory, &[Code::Adc_rm16_imm8, Code::Adc_rm32_imm8, Code::Adc_rm64_imm8]),
    form(Instruction::AdcLockMemvImmz, AddWithCarry, LockedMemory, &[Code::Adc_rm16_imm16, Code::Adc_rm32_imm32, Code::Adc_rm64_imm32]),
    form(Instruction::AdcMembGpr8, AddWithCarry, Memory, &[Code::Adc_rm8_r8]),
    form(Instruction::AdcMembImmb80r2, AddWithCarry, Memory, &[Code::Adc_rm8_imm8]),
    form(Instruction::AdcMemvGprv, AddWithCarry, Memory, &[Code::Adc_rm16_r16, Code::Adc_rm32_r32, Code::Adc_rm64_r64]),
    form(Instruction::AdcMemvImmb, AddWithCarry, Memory, &[Code::Adc_rm16_imm8, Code::Adc_rm32_imm8, Code::Adc_rm64_imm8]),
    form(Instruction::AdcMemvImmz, AddWithCarry, Memory, &[Code::Adc_rm16_imm16, Code::Adc_rm32_imm32, Code::Adc_rm64_imm32]),
    form(Instruction::AdcOraxImmz, AddWithCarry, Register, &[Code::Adc_AX_imm16, Code::Adc_EAX_imm32, Code::Adc_RAX_imm32]),
    // SBB
    form(Instruction::SbbAlImmb, SubtractWithBorrow, Register, &[Code::Sbb_AL_imm8]),
    form(Instruction::SbbGpr8Gpr818, SubtractWithBorrow, Register, &[Code::Sbb_rm8_r8]),
    form(Instruction::SbbGpr8Gpr81a, SubtractWithBorrow, Register, &[Code::Sbb_r8_rm8]),
    form(Instruction::SbbGpr8Immb80r3, SubtractWithBorrow, Register, &[Code::Sbb_rm8_imm8]),
    form(Instruction::SbbGpr8Memb, SubtractWithBorrow, Memory, &[Code::Sbb_r8_rm8]),
    form(Instruction::SbbGprvGprv19, SubtractWithBorrow, Register, &[Code::Sbb_rm16_r16, Code::Sbb_rm32_r32, Code::Sbb_rm64_r64]),
    form(Instruction::SbbGprvGprv1b, SubtractWithBorrow, Register, &[Code::Sbb_r16_rm16, Code::Sbb_r32_rm32, Code::Sbb_r64_rm64]),
    form(Instruction::SbbGprvImmb, SubtractWithBorrow, Register, &[Code::Sbb_rm16_imm8, Code::Sbb_rm32_imm8, Code::Sbb_rm64_imm8]),
    form(Instruction::SbbGprvImmz, SubtractWithBorrow, Register, &[Code::Sbb_rm16_imm16, Code::Sbb_rm32_imm32, Code::Sbb_rm64_imm32]),
    form(Instruction::SbbGprvMemv, SubtractWithBorrow, Memory, &[Code::Sbb_r16_rm16, Code::Sbb_r32_rm32, Code::Sbb_r64_rm64]),
    form(Instruction::SbbLockMembGpr8, SubtractWithBorrow, LockedMemory, &[Code::Sbb_rm8_r8]),
    form(Instruction::SbbLockMembImmb80r3, SubtractWithBorrow, LockedMemory, &[Code::Sbb_rm8_imm8]),
    form(Instruction::SbbLockMemvGprv, SubtractWithBorrow, LockedMemory, &[Code::Sbb_rm16_r16, Code::Sbb_rm32_r32, Code::Sbb_rm64_r64]),
    form(Instruction::SbbLockMemvImmb, SubtractWithBorrow, LockedMemory, &[Code::Sbb_rm16_imm8, Code::Sbb_rm32_imm8, Code::Sbb_rm64_imm8]),
    form(Instruction::SbbLockMemvImmz, SubtractWithBorrow, LockedMemory, &[Code::Sbb_rm16_imm16, Code::Sbb_rm32_imm32, Code::Sbb_rm64_imm32]),
    form(Instruction::SbbMembGpr8, SubtractWithBorrow, Memory, &[Code::Sbb_rm8_r8]),
    form(Instruction::SbbMembImmb80r3, SubtractWithBorrow, Memory, &[Code::Sbb_rm8_imm8]),
    form(Instruction::SbbMemvGprv, SubtractWithBorrow, Memory, &[Code::Sbb_rm16_r16, Code::Sbb_rm32_r32, Code::Sbb_rm64_r64]),
    form(Instruction::SbbMemvImmb, SubtractWithBorrow, Memory, &[Code::Sbb_rm16_imm8, Code::Sbb_rm32_imm8, Code::Sbb_rm64_imm8]),
    form(Instruction::SbbMemvImmz, SubtractWithBorrow, Memory, &[Code::Sbb_rm16_imm16, Code::Sbb_rm32_imm32, Code::Sbb_rm64_imm32]),
    form(Instruction::SbbOraxImmz, SubtractWithBorrow, Register, &[Code::Sbb_AX_imm16, Code::Sbb_EAX_imm32, Code::Sbb_RAX_imm32]),
    // AND
    form(Instruction::AndAlImmb, And, Register, &[Code::And_AL_imm8]),
    form(Instruction::AndGpr8Gpr820, And, Register, &[Code::And_rm8_r8]),
    form(Instruction::AndGpr8Gpr822, And, Register, &[Code::And_r8_rm8]),
    form(Instruction::AndGpr8Immb80r4, And, Register, &[Code::And_rm8_imm8]),
    form(Instruction::AndGpr8Memb, And, Memory, &[Code::And_r8_rm8]),
    form(Instruction::AndGprvGprv21, And, Register, &[Code::And_rm16_r16, Code::And_rm32_r32, Code::And_rm64_r64]),
    form(Instruction::AndGprvGprv23, And, Register, &[Code::And_r16_rm16, Code::And_r32_rm32, Code::And_r64_rm64]),
    form(Instruction::AndGprvImmb, And, Register, &[Code::And_rm16_imm8, Code::And_rm32_imm8, Code::And_rm64_imm8]),
    form(Instruction::AndGprvImmz, And, Register, &[Code::And_rm16_imm16, Code::And_rm32_imm32, Code::And_rm64_imm32]),
    form(Instruction::AndGprvMemv, And, Memory, &[Code::And_r16_rm16, Code::And_r32_rm32, Code::And_r64_rm64]),
    form(Instruction::AndLockMembGpr8, And, LockedMemory, &[Code::And_rm8_r8]),
    form(Instruction::AndLockMembImmb80r4, And, LockedMemory, &[Code::And_rm8_imm8]),
    form(Instruction::AndLockMemvGprv, And, LockedMemory, &[Code::And_rm16_r16, Code::And_rm32_r32, Code::And_rm64_r64]),
    form(Instruction::AndLockMemvImmb, And, LockedMemory, &[Code::And_rm16_imm8, Code::And_rm32_imm8, Code::And_rm64_imm8]),
    form(Instruction::AndLockMemvImmz, And, LockedMemory, &[Code::And_rm16_imm16, Code::And_rm32_imm32, Code::And_rm64_imm32]),
    form(Instruction::AndMembGpr8, And, Memory, &[Code::And_rm8_r8]),
    form(Instruction::AndMembImmb80r4, And, Memory, &[Code::And_rm8_imm8]),
    form(Instruction::AndMemvGprv, And, Memory, &[Code::And_rm16_r16, Code::And_rm32_r32, Code::And_rm64_r64]),
    form(Instruction::AndMemvImmb, And, Memory, &[Code::And_rm16_imm8, Code::And_rm32_imm8, Code::And_rm64_imm8]),
    form(Instruction::AndMemvImmz, And, Memory, &[Code::And_rm16_imm16, Code::And_rm32_imm32, Code::And_rm64_imm32]),
    form(Instruction::AndOraxImmz, And, Register, &[Code::And_AX_imm16, Code::And_EAX_imm32, Code::And_RAX_imm32]),
    // SUB
    form(Instruction::SubAlImmb, Subtract, Register, &[Code::Sub_AL_imm8]),
    form(Instruction::SubGpr8Gpr828, Subtract, Register, &[Code::Sub_rm8_r8]),
    form(Instruction::SubGpr8Gpr82a, Subtract, Register, &[Code::Sub_r8_rm8]),
    form(Instruction::SubGpr8Immb80r5, Subtract, Register, &[Code::Sub_rm8_imm8]),
    form(Instruction::SubGpr8Memb, Subtract, Memory, &[Code::Sub_r8_rm8]),
    form(Instruction::SubGprvGprv29, Subtract, Register, &[Code::Sub_rm16_r16, Code::Sub_rm32_r32, Code::Sub_rm64_r64]),
    form(Instruction::SubGprvGprv2b, Subtract, Register, &[Code::Sub_r16_rm16, Code::Sub_r32_rm32, Code::Sub_r64_rm64]),
    form(Instruction::SubGprvImmb, Subtract, Register, &[Code::Sub_rm16_imm8, Code::Sub_rm32_imm8, Code::Sub_rm64_imm8]),
    form(Instruction::SubGprvImmz, Subtract, Register, &[Code::Sub_rm16_imm16, Code::Sub_rm32_imm32, Code::Sub_rm64_imm32]),
    form(Instruction::SubGprvMemv, Subtract, Memory, &[Code::Sub_r16_rm16, Code::Sub_r32_rm32, Code::Sub_r64_rm64]),
    form(Instruction::SubLockMembGpr8, Subtract, LockedMemory, &[Code::Sub_rm8_r8]),
    form(Instruction::SubLockMembImmb80r5, Subtract, LockedMemory, &[Code::Sub_rm8_imm8]),
    form(Instruction::SubLockMemvGprv, Subtract, LockedMemory, &[Code::Sub_rm16_r16, Code::Sub_rm32_r32, Code::Sub_rm64_r64]),
    form(Instruction::SubLockMemvImmb, Subtract, LockedMemory, &[Code::Sub_rm16_imm8, Code::Sub_rm32_imm8, Code::Sub_rm64_imm8]),
    form(Instruction::SubLockMemvImmz, Subtract, LockedMemory, &[Code::Sub_rm16_imm16, Code::Sub_rm32_imm32, Code::Sub_rm64_imm32]),
    form(Instruction::SubMembGpr8, Subtract, Memory, &[Code::Sub_rm8_r8]),
    form(Instruction::SubMembImmb80r5, Subtract, Memory, &[Code::Sub_rm8_imm8]),
    form(Instruction::SubMemvGprv, Subtract, Memory, &[Code::Sub_rm16_r16, Code::Sub_rm32_r32, Code::Sub_rm64_r64]),
    form(Instruction::SubMemvImmb, Subtract, Memory, &[Code::Sub_rm16_imm8, Code::Sub_rm32_imm8, Code::Sub_rm64_imm8]),
    form(Instruction::SubMemvImmz, Subtract, Memory, &[Code::Sub_rm16_imm16, Code::Sub_rm32_imm32, Code::Sub_rm64_imm32]),
    form(Instruction::SubOraxImmz, Subtract, Register, &[Code::Sub_AX_imm16, Code::Sub_EAX_imm32, Code::Sub_RAX_imm32]),
    // XOR
    form(Instruction::XorAlImmb, Xor, Register, &[Code::Xor_AL_imm8]),
    form(Instruction::XorGpr8Gpr830, Xor, Register, &[Code::Xor_rm8_r8]),
    form(Instruction::XorGpr8Gpr832, Xor, Register, &[Code::Xor_r8_rm8]),
    form(Instruction::XorGpr8Immb80r6, Xor, Register, &[Code::Xor_rm8_imm8]),
    form(Instruction::XorGpr8Memb, Xor, Memory, &[Code::Xor_r8_rm8]),
    form(Instruction::XorGprvGprv31, Xor, Register, &[Code::Xor_rm16_r16, Code::Xor_rm32_r32, Code::Xor_rm64_r64]),
    form(Instruction::XorGprvGprv33, Xor, Register, &[Code::Xor_r16_rm16, Code::Xor_r32_rm32, Code::Xor_r64_rm64]),
    form(Instruction::XorGprvImmb, Xor, Register, &[Code::Xor_rm16_imm8, Code::Xor_rm32_imm8, Code::Xor_rm64_imm8]),
    form(Instruction::XorGprvImmz, Xor, Register, &[Code::Xor_rm16_imm16, Code::Xor_rm32_imm32, Code::Xor_rm64_imm32]),
    form(Instruction::XorGprvMemv, Xor, Memory, &[Code::Xor_r16_rm16, Code::Xor_r32_rm32, Code::Xor_r64_rm64]),
    form(Instruction::XorLockMembGpr8, Xor, LockedMemory, &[Code::Xor_rm8_r8]),
    form(Instruction::XorLockMembImmb80r6, Xor, LockedMemory, &[Code::Xor_rm8_imm8]),
    form(Instruction::XorLockMemvGprv, Xor, LockedMemory, &[Code::Xor_rm16_r16, Code::Xor_rm32_r32, Code::Xor_rm64_r64]),
    form(Instruction::XorLockMemvImmb, Xor, LockedMemory, &[Code::Xor_rm16_imm8, Code::Xor_rm32_imm8, Code::Xor_rm64_imm8]),
    form(Instruction::XorLockMemvImmz, Xor, LockedMemory, &[Code::Xor_rm16_imm16, Code::Xor_rm32_imm32, Code::Xor_rm64_imm32]),
    form(Instruction::XorMembGpr8, Xor, Memory, &[Code::Xor_rm8_r8]),
    form(Instruction::XorMembImmb80r6, Xor, Memory, &[Code::Xor_rm8_imm8]),
    form(Instruction::XorMemvGprv, Xor, Memory, &[Code::Xor_rm16_r16, Code::Xor_rm32_r32, Code::Xor_rm64_r64]),
    form(Instruction::XorMemvImmb, Xor, Memory, &[Code::Xor_rm16_imm8, Code::Xor_rm32_imm8, Code::Xor_rm64_imm8]),
    form(Instruction::XorMemvImmz, Xor, Memory, &[Code::Xor_rm16_imm16, Code::Xor_rm32_imm32, Code::Xor_rm64_imm32]),
    form(Instruction::XorOraxImmz, Xor, Register, &[Code::Xor_AX_imm16, Code::Xor_EAX_imm32, Code::Xor_RAX_imm32]),
    // CMP
    form(Instruction::CmpAlImmb, Compare, Register, &[Code::Cmp_AL_imm8]),
    form(Instruction::CmpGpr8Gpr838, Compare, Register, &[Code::Cmp_rm8_r8]),
    form(Instruction::CmpGpr8Gpr83a, Compare, Register, &[Code::Cmp_r8_rm8]),
    form(Instruction::CmpGpr8Immb80r7, Compare, Register, &[Code::Cmp_rm8_imm8]),
    form(Instruction::CmpGpr8Memb, Compare, Memory, &[Code::Cmp_r8_rm8]),
    form(Instruction::CmpGprvGprv39, Compare, Register, &[Code::Cmp_rm16_r16, Code::Cmp_rm32_r32, Code::Cmp_rm64_r64]),
    form(Instruction::CmpGprvGprv3b, Compare, Register, &[Code::Cmp_r16_rm16, Code::Cmp_r32_rm32, Code::Cmp_r64_rm64]),
    form(Instruction::CmpGprvImmb, Compare, Register, &[Code::Cmp_rm16_imm8, Code::Cmp_rm32_imm8, Code::Cmp_rm64_imm8]),
    form(Instruction::CmpGprvImmz, Compare, Register, &[Code::Cmp_rm16_imm16, Code::Cmp_rm32_imm32, Code::Cmp_rm64_imm32]),
    form(Instruction::CmpGprvMemv, Compare, Memory, &[Code::Cmp_r16_rm16, Code::Cmp_r32_rm32, Code::Cmp_r64_rm64]),
    form(Instruction::CmpMembGpr8, Compare, Memory, &[Code::Cmp_rm8_r8]),
    form(Instruction::CmpMembImmb80r7, Compare, Memory, &[Code::Cmp_rm8_imm8]),
    form(Instruction::CmpMemvGprv, Compare, Memory, &[Code::Cmp_rm16_r16, Code::Cmp_rm32_r32, Code::Cmp_rm64_r64]),
    form(Instruction::CmpMemvImmb, Compare, Memory, &[Code::Cmp_rm16_imm8, Code::Cmp_rm32_imm8, Code::Cmp_rm64_imm8]),
    form(Instruction::CmpMemvImmz, Compare, Memory, &[Code::Cmp_rm16_imm16, Code::Cmp_rm32_imm32, Code::Cmp_rm64_imm32]),
    form(Instruction::CmpOraxImmz, Compare, Register, &[Code::Cmp_AX_imm16, Code::Cmp_EAX_imm32, Code::Cmp_RAX_imm32]),
    // TEST
    form(Instruction::TestAlImmb, Test, Register, &[Code::Test_AL_imm8]),
    form(Instruction::TestGpr8Gpr8, Test, Register, &[Code::Test_rm8_r8]),
    form(Instruction::TestGpr8ImmbF6r0, Test, Register, &[Code::Test_rm8_imm8]),
    form(Instruction::TestGpr8ImmbF6r1, Test, Register, &[Code::Test_rm8_imm8_F6r1]),
    form(Instruction::TestGprvGprv, Test, Register, &[Code::Test_rm16_r16, Code::Test_rm32_r32, Code::Test_rm64_r64]),
    form(Instruction::TestGprvImmzF7r0, Test, Register, &[Code::Test_rm16_imm16, Code::Test_rm32_imm32, Code::Test_rm64_imm32]),
    form(Instruction::TestGprvImmzF7r1, Test, Register, &[Code::Test_rm16_imm16_F7r1, Code::Test_rm32_imm32_F7r1, Code::Test_rm64_imm32_F7r1]),
    form(Instruction::TestMembGpr8, Test, Memory, &[Code::Test_rm8_r8]),
    form(Instruction::TestMembImmbF6r0, Test, Memory, &[Code::Test_rm8_imm8]),
    form(Instruction::TestMembImmbF6r1, Test, Memory, &[Code::Test_rm8_imm8_F6r1]),
    form(Instruction::TestMemvGprv, Test, Memory, &[Code::Test_rm16_r16, Code::Test_rm32_r32, Code::Test_rm64_r64]),
    form(Instruction::TestMemvImmzF7r0, Test, Memory, &[Code::Test_rm16_imm16, Code::Test_rm32_imm32, Code::Test_rm64_imm32]),
    form(Instruction::TestMemvImmzF7r1, Test, Memory, &[Code::Test_rm16_imm16_F7r1, Code::Test_rm32_imm32_F7r1, Code::Test_rm64_imm32_F7r1]),
    form(Instruction::TestOraxImmz, Test, Register, &[Code::Test_AX_imm16, Code::Test_EAX_imm32, Code::Test_RAX_imm32]),
    // INC
    form(Instruction::IncGpr8, Increment, Register, &[Code::Inc_rm8]),
    form(Instruction::IncGprvFfr0, Increment, Register, &[Code::Inc_rm16, Code::Inc_rm32, Code::Inc_rm64]),
    form(Instruction::IncLockMemb, Increment, LockedMemory, &[Code::Inc_rm8]),
    form(Instruction::IncLockMemv, Increment, LockedMemory, &[Code::Inc_rm16, Code::Inc_rm32, Code::Inc_rm64]),
    form(Instruction::IncMemb, Increment, Memory, &[Code::Inc_rm8]),
    form(Instruction::IncMemv, Increment, Memory, &[Code::Inc_rm16, Code::Inc_rm32, Code::Inc_rm64]),
    // DEC
    form(Instruction::DecGpr8, Decrement, Register, &[Code::Dec_rm8]),
    form(Instruction::DecGprvFfr1, Decrement, Register, &[Code::Dec_rm16, Code::Dec_rm32, Code::Dec_rm64]),
    form(Instruction::DecLockMemb, Decrement, LockedMemory, &[Code::Dec_rm8]),
    form(Instruction::DecLockMemv, Decrement, LockedMemory, &[Code::Dec_rm16, Code::Dec_rm32, Code::Dec_rm64]),
    form(Instruction::DecMemb, Decrement, Memory, &[Code::Dec_rm8]),
    form(Instruction::DecMemv, Decrement, Memory, &[Code::Dec_rm16, Code::Dec_rm32, Code::Dec_rm64]),
    // NEG
    form(Instruction::NegGpr8, Negate, Register, &[Code::Neg_rm8]),
    form(Instruction::NegGprv, Negate, Register, &[Code::Neg_rm16, Code::Neg_rm32, Code::Neg_rm64]),
    form(Instruction::NegLockMemb, Negate, LockedMemory, &[Code::Neg_rm8]),
    form(Instruction::NegLockMemv, Negate, LockedMemory, &[Code::Neg_rm16, Code::Neg_rm32, Code::Neg_rm64]),
    form(Instruction::NegMemb, Negate, Memory, &[Code::Neg_rm8]),
    form(Instruction::NegMemv, Negate, Memory, &[Code::Neg_rm16, Code::Neg_rm32, Code::Neg_rm64]),
    // NOT
    form(Instruction::NotGpr8, Not, Register, &[Code::Not_rm8]),
    form(Instruction::NotGprv, Not, Register, &[Code::Not_rm16, Code::Not_rm32, Code::Not_rm64]),
    form(Instruction::NotLockMemb, Not, LockedMemory, &[Code::Not_rm8]),
    form(Instruction::NotLockMemv, Not, LockedMemory, &[Code::Not_rm16, Code::Not_rm32, Code::Not_rm64]),
    form(Instruction::NotMemb, Not, Memory, &[Code::Not_rm8]),
    form(Instruction::NotMemv, Not, Memory, &[Code::Not_rm16, Code::Not_rm32, Code::Not_rm64]),
    // SHL
    form(Instruction::ShlGpr8ClD2r4, ShiftLeft, Register, &[Code::Shl_rm8_CL]),
    form(Instruction::ShlGpr8ClD2r6, ShiftLeft, Register, &[Code::Sal_rm8_CL]),
    form(Instruction::ShlGpr8ImmbC0r4, ShiftLeft, Register, &[Code::Shl_rm8_imm8]),
    form(Instruction::ShlGpr8ImmbC0r6, ShiftLeft, Register, &[Code::Sal_rm8_imm8]),
    form(Instruction::ShlGpr8OneD0r4, ShiftLeft, Register, &[Code::Shl_rm8_1]),
    form(Instruction::ShlGpr8OneD0r6, ShiftLeft, Register, &[Code::Sal_rm8_1]),
    form(Instruction::ShlGprvClD3r4, ShiftLeft, Register, &[Code::Shl_rm16_CL, Code::Shl_rm32_CL, Code::Shl_rm64_CL]),
    form(Instruction::ShlGprvClD3r6, ShiftLeft, Register, &[Code::Sal_rm16_CL, Code::Sal_rm32_CL, Code::Sal_rm64_CL]),
    form(Instruction::ShlGprvImmbC1r4, ShiftLeft, Register, &[Code::Shl_rm16_imm8, Code::Shl_rm32_imm8, Code::Shl_rm64_imm8]),
    form(Instruction::ShlGprvImmbC1r6, ShiftLeft, Register, &[Code::Sal_rm16_imm8, Code::Sal_rm32_imm8, Code::Sal_rm64_imm8]),
    form(Instruction::ShlGprvOneD1r4, ShiftLeft, Register, &[Code::Shl_rm16_1, Code::Shl_rm32_1, Code::Shl_rm64_1]),
    form(Instruction::ShlGprvOneD1r6, ShiftLeft, Register, &[Code::Sal_rm16_1, Code::Sal_rm32_1, Code::Sal_rm64_1]),
    form(Instruction::ShlMembClD2r4, ShiftLeft, Memory, &[Code::Shl_rm8_CL]),
    form(Instruction::ShlMembClD2r6, ShiftLeft, Memory, &[Code::Sal_rm8_CL]),
    form(Instruction::ShlMembImmbC0r4, ShiftLeft, Memory, &[Code::Shl_rm8_imm8]),
    form(Instruction::ShlMembImmbC0r6, ShiftLeft, Memory, &[Code::Sal_rm8_imm8]),
    form(Instruction::ShlMembOneD0r4, ShiftLeft, Memory, &[Code::Shl_rm8_1]),
    form(Instruction::ShlMembOneD0r6, ShiftLeft, Memory, &[Code::Sal_rm8_1]),
    form(Instruction::ShlMemvClD3r4, ShiftLeft, Memory, &[Code::Shl_rm16_CL, Code::Shl_rm32_CL, Code::Shl_rm64_CL]),
    form(Instruction::ShlMemvClD3r6, ShiftLeft, Memory, &[Code::Sal_rm16_CL, Code::Sal_rm32_CL, Code::Sal_rm64_CL]),
    form(Instruction::ShlMemvImmbC1r4, ShiftLeft, Memory, &[Code::Shl_rm16_imm8, Code::Shl_rm32_imm8, Code::Shl_rm64_imm8]),
    form(Instruction::ShlMemvImmbC1r6, ShiftLeft, Memory, &[Code::Sal_rm16_imm8, Code::Sal_rm32_imm8, Code::Sal_rm64_imm8]),
    form(Instruction::ShlMemvOneD1r4, ShiftLeft, Memory, &[Code::Shl_rm16_1, Code::Shl_rm32_1, Code::Shl_rm64_1]),
    form(Instruction::ShlMemvOneD1r6, ShiftLeft, Memory, &[Code::Sal_rm16_1, Code::Sal_rm32_1, Code::Sal_rm64_1]),
    // SHR
    form(Instruction::ShrGpr8Cl, ShiftRightLogical, Register, &[Code::Shr_rm8_CL]),
    form(Instruction::ShrGpr8Immb, ShiftRightLogical, Register, &[Code::Shr_rm8_imm8]),
    form(Instruction::ShrGpr8One, ShiftRightLogical, Register, &[Code::Shr_rm8_1]),
    form(Instruction::ShrGprvCl, ShiftRightLogical, Register, &[Code::Shr_rm16_CL, Code::Shr_rm32_CL, Code::Shr_rm64_CL]),
    form(Instruction::ShrGprvImmb, ShiftRightLogical, Register, &[Code::Shr_rm16_imm8, Code::Shr_rm32_imm8, Code::Shr_rm64_imm8]),
    form(Instruction::ShrGprvOne, ShiftRightLogical, Register, &[Code::Shr_rm16_1, Code::Shr_rm32_1, Code::Shr_rm64_1]),
    form(Instruction::ShrMembCl, ShiftRightLogical, Memory, &[Code::Shr_rm8_CL]),
    form(Instruction::ShrMembImmb, ShiftRightLogical, Memory, &[Code::Shr_rm8_imm8]),
    form(Instruction::ShrMembOne, ShiftRightLogical, Memory, &[Code::Shr_rm8_1]),
    form(Instruction::ShrMemvCl, ShiftRightLogical, Memory, &[Code::Shr_rm16_CL, Code::Shr_rm32_CL, Code::Shr_rm64_CL]),
    form(Instruction::ShrMemvImmb, ShiftRightLogical, Memory, &[Code::Shr_rm16_imm8, Code::Shr_rm32_imm8, Code::Shr_rm64_imm8]),
    form(Instruction::ShrMemvOne, ShiftRightLogical, Memory, &[Code::Shr_rm16_1, Code::Shr_rm32_1, Code::Shr_rm64_1]),
    // SAR
    form(Instruction::SarGpr8Cl, ShiftRightArithmetic, Register, &[Code::Sar_rm8_CL]),
    form(Instruction::SarGpr8Immb, ShiftRightArithmetic, Register, &[Code::Sar_rm8_imm8]),
    form(Instruction::SarGpr8One, ShiftRightArithmetic, Register, &[Code::Sar_rm8_1]),
    form(Instruction::SarGprvCl, ShiftRightArithmetic, Register, &[Code::Sar_rm16_CL, Code::Sar_rm32_CL, Code::Sar_rm64_CL]),
    form(Instruction::SarGprvImmb, ShiftRightArithmetic, Register, &[Code::Sar_rm16_imm8, Code::Sar_rm32_imm8, Code::Sar_rm64_imm8]),
    form(Instruction::SarGprvOne, ShiftRightArithmetic, Register, &[Code::Sar_rm16_1, Code::Sar_rm32_1, Code::Sar_rm64_1]),
    form(Instruction::SarMembCl, ShiftRightArithmetic, Memory, &[Code::Sar_rm8_CL]),
    form(Instruction::SarMembImmb, ShiftRightArithmetic, Memory, &[Code::Sar_rm8_imm8]),
    form(Instruction::SarMembOne, ShiftRightArithmetic, Memory, &[Code::Sar_rm8_1]),
    form(Instruction::SarMemvCl, ShiftRightArithmetic, Memory, &[Code::Sar_rm16_CL, Code::Sar_rm32_CL, Code::Sar_rm64_CL]),
    form(Instruction::SarMemvImmb, ShiftRightArithmetic, Memory, &[Code::Sar_rm16_imm8, Code::Sar_rm32_imm8, Code::Sar_rm64_imm8]),
    form(Instruction::SarMemvOne, ShiftRightArithmetic, Memory, &[Code::Sar_rm16_1, Code::Sar_rm32_1, Code::Sar_rm64_1]),
    // ROL
    form(Instruction::RolGpr8Cl, RotateLeft, Register, &[Code::Rol_rm8_CL]),
    form(Instruction::RolGpr8Immb, RotateLeft, Register, &[Code::Rol_rm8_imm8]),
    form(Instruction::RolGpr8One, RotateLeft, Register, &[Code::Rol_rm8_1]),
    form(Instruction::RolGprvCl, RotateLeft, Register, &[Code::Rol_rm16_CL, Code::Rol_rm32_CL, Code::Rol_rm64_CL]),
    form(Instruction::RolGprvImmb, RotateLeft, Register, &[Code::Rol_rm16_imm8, Code::Rol_rm32_imm8, Code::Rol_rm64_imm8]),
    form(Instruction::RolGprvOne, RotateLeft, Register, &[Code::Rol_rm16_1, Code::Rol_rm32_1, Code::Rol_rm64_1]),
    form(Instruction::RolMembCl, RotateLeft, Memory, &[Code::Rol_rm8_CL]),
    form(Instruction::RolMembImmb, RotateLeft, Memory, &[Code::Rol_rm8_imm8]),
    form(Instruction::RolMembOne, RotateLeft, Memory, &[Code::Rol_rm8_1]),
    form(Instruction::RolMemvCl, RotateLeft, Memory, &[Code::Rol_rm16_CL, Code::Rol_rm32_CL, Code::Rol_rm64_CL]),
    form(Instruction::RolMemvImmb, RotateLeft, Memory, &[Code::Rol_rm16_imm8, Code::Rol_rm32_imm8, Code::Rol_rm64_imm8]),
    form(Instruction::RolMemvOne, RotateLeft, Memory, &[Code::Rol_rm16_1, Code::Rol_rm32_1, Code::Rol_rm64_1]),
    // ROR
    form(Instruction::RorGpr8Cl, RotateRight, Register, &[Code::Ror_rm8_CL]),
    form(Instruction::RorGpr8Immb, RotateRight, Register, &[Code::Ror_rm8_imm8]),
    form(Instruction::RorGpr8One, RotateRight, Register, &[Code::Ror_rm8_1]),
    form(Instruction::RorGprvCl, RotateRight, Register, &[Code::Ror_rm16_CL, Code::Ror_rm32_CL, Code::Ror_rm64_CL]),
    form(Instruction::RorGprvImmb, RotateRight, Register, &[Code::Ror_rm16_imm8, Code::Ror_rm32_imm8, Code::Ror_rm64_imm8]),
    form(Instruction::RorGprvOne, RotateRight, Register, &[Code::Ror_rm16_1, Code::Ror_rm32_1, Code::Ror_rm64_1]),
    form(Instruction::RorMembCl, RotateRight, Memory, &[Code::Ror_rm8_CL]),
    form(Instruction::RorMembImmb, RotateRight, Memory, &[Code::Ror_rm8_imm8]),
    form(Instruction::RorMembOne, RotateRight, Memory, &[Code::Ror_rm8_1]),
    form(Instruction::RorMemvCl, RotateRight, Memory, &[Code::Ror_rm16_CL, Code::Ror_rm32_CL, Code::Ror_rm64_CL]),
    form(Instruction::RorMemvImmb, RotateRight, Memory, &[Code::Ror_rm16_imm8, Code::Ror_rm32_imm8, Code::Ror_rm64_imm8]),
    form(Instruction::RorMemvOne, RotateRight, Memory, &[Code::Ror_rm16_1, Code::Ror_rm32_1, Code::Ror_rm64_1]),
    // RCL
    form(Instruction::RclGpr8Cl, RotateLeftThroughCarry, Register, &[Code::Rcl_rm8_CL]),
    form(Instruction::RclGpr8Immb, RotateLeftThroughCarry, Register, &[Code::Rcl_rm8_imm8]),
    form(Instruction::RclGpr8One, RotateLeftThroughCarry, Register, &[Code::Rcl_rm8_1]),
    form(Instruction::RclGprvCl, RotateLeftThroughCarry, Register, &[Code::Rcl_rm16_CL, Code::Rcl_rm32_CL, Code::Rcl_rm64_CL]),
    form(Instruction::RclGprvImmb, RotateLeftThroughCarry, Register, &[Code::Rcl_rm16_imm8, Code::Rcl_rm32_imm8, Code::Rcl_rm64_imm8]),
    form(Instruction::RclGprvOne, RotateLeftThroughCarry, Register, &[Code::Rcl_rm16_1, Code::Rcl_rm32_1, Code::Rcl_rm64_1]),
    form(Instruction::RclMembCl, RotateLeftThroughCarry, Memory, &[Code::Rcl_rm8_CL]),
    form(Instruction::RclMembImmb, RotateLeftThroughCarry, Memory, &[Code::Rcl_rm8_imm8]),
    form(Instruction::RclMembOne, RotateLeftThroughCarry, Memory, &[Code::Rcl_rm8_1]),
    form(Instruction::RclMemvCl, RotateLeftThroughCarry, Memory, &[Code::Rcl_rm16_CL, Code::Rcl_rm32_CL, Code::Rcl_rm64_CL]),
    form(Instruction::RclMemvImmb, RotateLeftThroughCarry, Memory, &[Code::Rcl_rm16_imm8, Code::Rcl_rm32_imm8, Code::Rcl_rm64_imm8]),
    form(Instruction::RclMemvOne, RotateLeftThroughCarry, Memory, &[Code::Rcl_rm16_1, Code::Rcl_rm32_1, Code::Rcl_rm64_1]),
    // RCR
    form(Instruction::RcrGpr8Cl, RotateRightThroughCarry, Register, &[Code::Rcr_rm8_CL]),
    form(Instruction::RcrGpr8Immb, RotateRightThroughCarry, Register, &[Code::Rcr_rm8_imm8]),
    form(Instruction::RcrGpr8One, RotateRightThroughCarry, Register, &[Code::Rcr_rm8_1]),
    form(Instruction::RcrGprvCl, RotateRightThroughCarry, Register, &[Code::Rcr_rm16_CL, Code::Rcr_rm32_CL, Code::Rcr_rm64_CL]),
    form(Instruction::RcrGprvImmb, RotateRightThroughCarry, Register, &[Code::Rcr_rm16_imm8, Code::Rcr_rm32_imm8, Code::Rcr_rm64_imm8]),
    form(Instruction::RcrGprvOne, RotateRightThroughCarry, Register, &[Code::Rcr_rm16_1, Code::Rcr_rm32_1, Code::Rcr_rm64_1]),
    form(Instruction::RcrMembCl, RotateRightThroughCarry, Memory, &[Code::Rcr_rm8_CL]),
    form(Instruction::RcrMembImmb, RotateRightThroughCarry, Memory, &[Code::Rcr_rm8_imm8]),
    form(Instruction::RcrMembOne, RotateRightThroughCarry, Memory, &[Code::Rcr_rm8_1]),
    form(Instruction::RcrMemvCl, RotateRightThroughCarry, Memory, &[Code::Rcr_rm16_CL, Code::Rcr_rm32_CL, Code::Rcr_rm64_CL]),
    form(Instruction::RcrMemvImmb, RotateRightThroughCarry, Memory, &[Code::Rcr_rm16_imm8, Code::Rcr_rm32_imm8, Code::Rcr_rm64_imm8]),
    form(Instruction::RcrMemvOne, RotateRightThroughCarry, Memory, &[Code::Rcr_rm16_1, Code::Rcr_rm32_1, Code::Rcr_rm64_1]),
    // MOV
    form(Instruction::MovAlMemb, Move, Memory, &[Code::Mov_AL_moffs8]),
    form(Instruction::MovGpr8Gpr888, Move, Register, &[Code::Mov_rm8_r8]),
    form(Instruction::MovGpr8Gpr88a, Move, Register, &[Code::Mov_r8_rm8]),
    form(Instruction::MovGpr8ImmbB0, Move, Register, &[Code::Mov_r8_imm8]),
    form(Instruction::MovGpr8ImmbC6r0, Move, Register, &[Code::Mov_rm8_imm8]),
    form(Instruction::MovGpr8Memb, Move, Memory, &[Code::Mov_r8_rm8]),
    form(Instruction::MovGprvGprv89, Move, Register, &[Code::Mov_rm16_r16, Code::Mov_rm32_r32, Code::Mov_rm64_r64]),
    form(Instruction::MovGprvGprv8b, Move, Register, &[Code::Mov_r16_rm16, Code::Mov_r32_rm32, Code::Mov_r64_rm64]),
    form(Instruction::MovGprvImmv, Move, Register, &[Code::Mov_r16_imm16, Code::Mov_r32_imm32, Code::Mov_r64_imm64]),
    form(Instruction::MovGprvImmz, Move, Register, &[Code::Mov_rm16_imm16, Code::Mov_rm32_imm32, Code::Mov_rm64_imm32]),
    form(Instruction::MovGprvMemv, Move, Memory, &[Code::Mov_r16_rm16, Code::Mov_r32_rm32, Code::Mov_r64_rm64]),
    form(Instruction::MovMembAl, Move, Memory, &[Code::Mov_moffs8_AL]),
    form(Instruction::MovMembGpr8, Move, Memory, &[Code::Mov_rm8_r8]),
    form(Instruction::MovMembImmb, Move, Memory, &[Code::Mov_rm8_imm8]),
    form(Instruction::MovMemvGprv, Move, Memory, &[Code::Mov_rm16_r16, Code::Mov_rm32_r32, Code::Mov_rm64_r64]),
    form(Instruction::MovMemvImmz, Move, Memory, &[Code::Mov_rm16_imm16, Code::Mov_rm32_imm32, Code::Mov_rm64_imm32]),
    form(Instruction::MovMemvOrax, Move, Memory, &[Code::Mov_moffs16_AX, Code::Mov_moffs32_EAX, Code::Mov_moffs64_RAX]),
    form(Instruction::MovOraxMemv, Move, Memory, &[Code::Mov_AX_moffs16, Code::Mov_EAX_moffs32, Code::Mov_RAX_moffs64]),
    // CMOVcc
    form(Instruction::CmovbeGprvGprv, MoveIf(Condition::BelowOrEqual), Register, &[Code::Cmovbe_r16_rm16, Code::Cmovbe_r32_rm32, Code::Cmovbe_r64_rm64]),
    form(Instruction::CmovbeGprvMemv, MoveIf(Condition::BelowOrEqual), Memory, &[Code::Cmovbe_r16_rm16, Code::Cmovbe_r32_rm32, Code::Cmovbe_r64_rm64]),
    form(Instruction::CmovbGprvGprv, MoveIf(Condition::Below), Register, &[Code::Cmovb_r16_rm16, Code::Cmovb_r32_rm32, Code::Cmovb_r64_rm64]),
    form(Instruction::CmovbGprvMemv, MoveIf(Condition::Below), Memory, &[Code::Cmovb_r16_rm16, Code::Cmovb_r32_rm32, Code::Cmovb_r64_rm64]),
    form(Instruction::CmovleGprvGprv, MoveIf(Condition::LessOrEqual), Register, &[Code::Cmovle_r16_rm16, Code::Cmovle_r32_rm32, Code::Cmovle_r64_rm64]),
    form(Instruction::CmovleGprvMemv, MoveIf(Condition::LessOrEqual), Memory, &[Code::Cmovle_r16_rm16, Code::Cmovle_r32_rm32, Code::Cmovle_r64_rm64]),
    form(Instruction::CmovlGprvGprv, MoveIf(Condition::Less), Register, &[Code::Cmovl_r16_rm16, Code::Cmovl_r32_rm32, Code::Cmovl_r64_rm64]),
    form(Instruction::CmovlGprvMemv, MoveIf(Condition::Less), Memory, &[Code::Cmovl_r16_rm16, Code::Cmovl_r32_rm32, Code::Cmovl_r64_rm64]),
    form(Instruction::CmovnbeGprvGprv, MoveIf(Condition::NotBelowOrEqual), Register, &[Code::Cmova_r16_rm16, Code::Cmova_r32_rm32, Code::Cmova_r64_rm64]),
    form(Instruction::CmovnbeGprvMemv, MoveIf(Condition::NotBelowOrEqual), Memory, &[Code::Cmova_r16_rm16, Code::Cmova_r32_rm32, Code::Cmova_r64_rm64]),
    form(Instruction::CmovnbGprvGprv, MoveIf(Condition::NotBelow), Register, &[Code::Cmovae_r16_rm16, Code::Cmovae_r32_rm32, Code::Cmovae_r64_rm64]),
    form(Instruction::CmovnbGprvMemv, MoveIf(Condition::NotBelow), Memory, &[Code::Cmovae_r16_rm16, Code::Cmovae_r32_rm32, Code::Cmovae_r64_rm64]),
    form(Instruction::CmovnleGprvGprv, MoveIf(Condition::NotLessOrEqual), Register, &[Code::Cmovg_r16_rm16, Code::Cmovg_r32_rm32, Code::Cmovg_r64_rm64]),
    form(Instruction::CmovnleGprvMemv, MoveIf(Condition::NotLessOrEqual), Memory, &[Code::Cmovg_r16_rm16, Code::Cmovg_r32_rm32, Code::Cmovg_r64_rm64]),
    form(Instruction::CmovnlGprvGprv, MoveIf(Condition::NotLess), Register, &[Code::Cmovge_r16_rm16, Code::Cmovge_r32_rm32, Code::Cmovge_r64_rm64]),
    form(Instruction::CmovnlGprvMemv, MoveIf(Condition::NotLess), Memory, &[Code::Cmovge_r16_rm16, Code::Cmovge_r32_rm32, Code::Cmovge_r64_rm64]),
    form(Instruction::CmovnoGprvGprv, MoveIf(Condition::NotOverflow), Register, &[Code::Cmovno_r16_rm16, Code::Cmovno_r32_rm32, Code::Cmovno_r64_rm64]),
    form(Instruction::CmovnoGprvMemv, MoveIf(Condition::NotOverflow), Memory, &[Code::Cmovno_r16_rm16, Code::Cmovno_r32_rm32, Code::Cmovno_r64_rm64]),
    form(Instruction::CmovnpGprvGprv, MoveIf(Condition::NotParity), Register, &[Code::Cmovnp_r16_rm16, Code::Cmovnp_r32_rm32, Code::Cmovnp_r64_rm64]),
    form(Instruction::CmovnpGprvMemv, MoveIf(Condition::NotParity), Memory, &[Code::Cmovnp_r16_rm16, Code::Cmovnp_r32_rm32, Code::Cmovnp_r64_rm64]),
    form(Instruction::CmovnsGprvGprv, MoveIf(Condition::NotSign), Register, &[Code::Cmovns_r16_rm16, Code::Cmovns_r32_rm32, Code::Cmovns_r64_rm64]),
    form(Instruction::CmovnsGprvMemv, MoveIf(Condition::NotSign), Memory, &[Code::Cmovns_r16_rm16, Code::Cmovns_r32_rm32, Code::Cmovns_r64_rm64]),
    form(Instruction::CmovnzGprvGprv, MoveIf(Condition::NotZero), Register, &[Code::Cmovne_r16_rm16, Code::Cmovne_r32_rm32, Code::Cmovne_r64_rm64]),
    form(Instruction::CmovnzGprvMemv, MoveIf(Condition::NotZero), Memory, &[Code::Cmovne_r16_rm16, Code::Cmovne_r32_rm32, Code::Cmovne_r64_rm64]),
    form(Instruction::CmovoGprvGprv, MoveIf(Condition::Overflow), Register, &[Code::Cmovo_r16_rm16, Code::Cmovo_r32_rm32, Code::Cmovo_r64_rm64]),
    form(Instruction::CmovoGprvMemv, MoveIf(Condition::Overflow), Memory, &[Code::Cmovo_r16_rm16, Code::Cmovo_r32_rm32, Code::Cmovo_r64_rm64]),
    form(Instruction::CmovpGprvGprv, MoveIf(Condition::Parity), Register, &[Code::Cmovp_r16_rm16, Code::Cmovp_r32_rm32, Code::Cmovp_r64_rm64]),
    form(Instruction::CmovpGprvMemv, MoveIf(Condition::Parity), Memory, &[Code::Cmovp_r16_rm16, Code::Cmovp_r32_rm32, Code::Cmovp_r64_rm64]),
    form(Instruction::CmovsGprvGprv, MoveIf(Condition::Sign), Register, &[Code::Cmovs_r16_rm16, Code::Cmovs_r32_rm32, Code::Cmovs_r64_rm64]),
    form(Instruction::CmovsGprvMemv, MoveIf(Condition::Sign), Memory, &[Code::Cmovs_r16_rm16, Code::Cmovs_r32_rm32, Code::Cmovs_r64_rm64]),
    form(Instruction::CmovzGprvGprv, MoveIf(Condition::Zero), Register, &[Code::Cmove_r16_rm16, Code::Cmove_r32_rm32, Code::Cmove_r64_rm64]),
    form(Instruction::CmovzGprvMemv, MoveIf(Condition::Zero), Memory, &[Code::Cmove_r16_rm16, Code::Cmove_r32_rm32, Code::Cmove_r64_rm64]),
    // SETcc
    form(Instruction::SetbeGpr8, SetIf(Condition::BelowOrEqual), Register, &[Code::Setbe_rm8]),
    form(Instruction::SetbeMemb, SetIf(Condition::BelowOrEqual), Memory, &[Code::Setbe_rm8]),
    form(Instruction::SetbGpr8, SetIf(Condition::Below), Register, &[Code::Setb_rm8]),
    form(Instruction::SetbMemb, SetIf(Condition::Below), Memory, &[Code::Setb_rm8]),
    form(Instruction::SetleGpr8, SetIf(Condition::LessOrEqual), Register, &[Code::Setle_rm8]),
    form(Instruction::SetleMemb, SetIf(Condition::LessOrEqual), Memory, &[Code::Setle_rm8]),
    form(Instruction::SetlGpr8, SetIf(Condition::Less), Register, &[Code::Setl_rm8]),
    form(Instruction::SetlMemb, SetIf(Condition::Less), Memory, &[Code::Setl_rm8]),
    form(Instruction::SetnbeGpr8, SetIf(Condition::NotBelowOrEqual), Register, &[Code::Seta_rm8]),
    form(Instruction::SetnbeMemb, SetIf(Condition::NotBelowOrEqual), Memory, &[Code::Seta_rm8]),
    form(Instruction::SetnbGpr8, SetIf(Condition::NotBelow), Register, &[Code::Setae_rm8]),
    form(Instruction::SetnbMemb, SetIf(Condition::NotBelow), Memory, &[Code::Setae_rm8]),
    form(Instruction::SetnleGpr8, SetIf(Condition::NotLessOrEqual), Register, &[Code::Setg_rm8]),
    form(Instruction::SetnleMemb, SetIf(Condition::NotLessOrEqual), Memory, &[Code::Setg_rm8]),
    form(Instruction::SetnlGpr8, SetIf(Condition::NotLess), Register, &[Code::Setge_rm8]),
    form(Instruction::SetnlMemb, SetIf(Condition::NotLess), Memory, &[Code::Setge_rm8]),
    form(Instruction::SetnoGpr8, SetIf(Condition::NotOverflow), Register, &[Code::Setno_rm8]),
    form(Instruction::SetnoMemb, SetIf(Condition::NotOverflow), Memory, &[Code::Setno_rm8]),
    form(Instruction::SetnpGpr8, SetIf(Condition::NotParity), Register, &[Code::Setnp_rm8]),
    form(Instruction::SetnpMemb, SetIf(Condition::NotParity), Memory, &[Code::Setnp_rm8]),
    form(Instruction::SetnsGpr8, SetIf(Condition::NotSign), Register, &[Code::Setns_rm8]),
    form(Instruction::SetnsMemb, SetIf(Condition::NotSign), Memory, &[Code::Setns_rm8]),
    form(Instruction::SetnzGpr8, SetIf(Condition::NotZero), Register, &[Code::Setne_rm8]),
    form(Instruction::SetnzMemb, SetIf(Condition::NotZero), Memory, &[Code::Setne_rm8]),
    form(Instruction::SetoGpr8, SetIf(Condition::Overflow), Register, &[Code::Seto_rm8]),
    form(Instruction::SetoMemb, SetIf(Condition::Overflow), Memory, &[Code::Seto_rm8]),
    form(Instruction::SetpGpr8, SetIf(Condition::Parity), Register, &[Code::Setp_rm8]),
    form(Instruction::SetpMemb, SetIf(Condition::Parity), Memory, &[Code::Setp_rm8]),
    form(Instruction::SetsGpr8, SetIf(Condition::Sign), Register, &[Code::Sets_rm8]),
    form(Instruction::SetsMemb, SetIf(Condition::Sign), Memory, &[Code::Sets_rm8]),
    form(Instruction::SetzGpr8, SetIf(Condition::Zero), Register, &[Code::Sete_rm8]),
    form(Instruction::SetzMemb, SetIf(Condition::Zero), Memory, &[Code::Sete_rm8]),
    // IMUL with two and three operands
    form(Instruction::ImulGprvGprv, MultiplySigned, Register, &[Code::Imul_r16_rm16, Code::Imul_r32_rm32, Code::Imul_r64_rm64]),
    form(Instruction::ImulGprvMemv, MultiplySigned, Memory, &[Code::Imul_r16_rm16, Code::Imul_r32_rm32, Code::Imul_r64_rm64]),
    form(Instruction::ImulGprvGprvImmb, MultiplySigned, Register, &[Code::Imul_r16_rm16_imm8, Code::Imul_r32_rm32_imm8, Code::Imul_r64_rm64_imm8]),
    form(Instruction::ImulGprvMemvImmb, MultiplySigned, Memory, &[Code::Imul_r16_rm16_imm8, Code::Imul_r32_rm32_imm8, Code::Imul_r64_rm64_imm8]),
    form(Instruction::ImulGprvGprvImmz, MultiplySigned, Register, &[Code::Imul_r16_rm16_imm16, Code::Imul_r32_rm32_imm32, Code::Imul_r64_rm64_imm32]),
    form(Instruction::ImulGprvMemvImmz, MultiplySigned, Memory, &[Code::Imul_r16_rm16_imm16, Code::Imul_r32_rm32_imm32, Code::Imul_r64_rm64_imm32]),
    // MOVZX
    form(Instruction::PopcntGprvGprv, PopulationCount, Register, &[Code::Popcnt_r16_rm16, Code::Popcnt_r32_rm32, Code::Popcnt_r64_rm64]),
    form(Instruction::PopcntGprvMemv, PopulationCount, Memory, &[Code::Popcnt_r16_rm16, Code::Popcnt_r32_rm32, Code::Popcnt_r64_rm64]),
    form(Instruction::MovzxGprvGpr8, ZeroExtend, Register, &[Code::Movzx_r16_rm8, Code::Movzx_r32_rm8, Code::Movzx_r64_rm8]),
    form(Instruction::MovzxGprvMemb, ZeroExtend, Memory, &[Code::Movzx_r16_rm8, Code::Movzx_r32_rm8, Code::Movzx_r64_rm8]),
    form(Instruction::MovzxGprvGpr16, ZeroExtend, Register, &[Code::Movzx_r16_rm16, Code::Movzx_r32_rm16, Code::Movzx_r64_rm16]),
    form(Instruction::MovzxGpr16Memw, ZeroExtend, Memory, &[Code::Movzx_r16_rm16]),
    form(Instruction::MovzxGpr32Memw, ZeroExtend, Memory, &[Code::Movzx_r32_rm16]),
    form(Instruction::MovzxGpr64Memw, ZeroExtend, Memory, &[Code::Movzx_r64_rm16]),
    // MOVSX and MOVSXD
    form(Instruction::MovsxGprvGpr8, SignExtend, Register, &[Code::Movsx_r16_rm8, Code::Movsx_r32_rm8, Code::Movsx_r64_rm8]),
    form(Instruction::MovsxGprvMemb, SignExtend, Memory, &[Code::Movsx_r16_rm8, Code::Movsx_r32_rm8, Code::Movsx_r64_rm8]),
    form(Instruction::MovsxGprvGpr16, SignExtend, Register, &[Code::Movsx_r16_rm16, Code::Movsx_r32_rm16, Code::Movsx_r64_rm16]),
    form(Instruction::MovsxGpr16Memw, SignExtend, Memory, &[Code::Movsx_r16_rm16]),
    form(Instruction::MovsxGpr32Memw, SignExtend, Memory, &[Code::Movsx_r32_rm16]),
    form(Instruction::MovsxGpr64Memw, SignExtend, Memory, &[Code::Movsx_r64_rm16]),
    form(Instruction::MovsxdGprvGprz, SignExtend, Register, &[Code::Movsxd_r16_rm16, Code::Movsxd_r32_rm32, Code::Movsxd_r64_rm32]),
    form(Instruction::MovsxdGprzMemz, SignExtend, Memory, &[Code::Movsxd_r16_rm16, Code::Movsxd_r32_rm32]),
    form(Instruction::MovsxdGpr64Memd, SignExtend, Memory, &[Code::Movsxd_r64_rm32]),
    // XCHG
    form(Instruction::XchgGpr8Gpr8, Exchange, Register, &[Code::Xchg_rm8_r8]),
    form(Instruction::XchgGprvGprv, Exchange, Register, &[Code::Xchg_rm16_r16, Code::Xchg_rm32_r32, Code::Xchg_rm64_r64]),
    form(Instruction::XchgGprvOrax, Exchange, Register, &[Code::Xchg_r16_AX, Code::Xchg_r32_EAX, Code::Xchg_r64_RAX]),
    form(Instruction::XchgMembGpr8, Exchange, Memory, &[Code::Xchg_rm8_r8]),
    form(Instruction::XchgMemvGprv, Exchange, Memory, &[Code::Xchg_rm16_r16, Code::Xchg_rm32_r32, Code::Xchg_rm64_r64]),
    // XADD
    form(Instruction::XaddGpr8Gpr8, ExchangeAdd, Register, &[Code::Xadd_rm8_r8]),
    form(Instruction::XaddGprvGprv, ExchangeAdd, Register, &[Code::Xadd_rm16_r16, Code::Xadd_rm32_r32, Code::Xadd_rm64_r64]),
    form(Instruction::XaddLockMembGpr8, ExchangeAdd, LockedMemory, &[Code::Xadd_rm8_r8]),
    form(Instruction::XaddLockMemvGprv, ExchangeAdd, LockedMemory, &[Code::Xadd_rm16_r16, Code::Xadd_rm32_r32, Code::Xadd_rm64_r64]),
    form(Instruction::XaddMembGpr8, ExchangeAdd, Memory, &[Code::Xadd_rm8_r8]),
    form(Instruction::XaddMemvGprv, ExchangeAdd, Memory, &[Code::Xadd_rm16_r16, Code::Xadd_rm32_r32, Code::Xadd_rm64_r64]),
    // MUL and IMUL with one operand
    form(Instruction::MulGpr8, MultiplyWide { signed: false }, Register, &[Code::Mul_rm8]),
    form(Instruction::MulGprv, MultiplyWide { signed: false }, Register, &[Code::Mul_rm16, Code::Mul_rm32, Code::Mul_rm64]),
    form(Instruction::MulMemb, MultiplyWide { signed: false }, Memory, &[Code::Mul_rm8]),
    form(Instruction::MulMemv, MultiplyWide { signed: false }, Memory, &[Code::Mul_rm16, Code::Mul_rm32, Code::Mul_rm64]),
    form(Instruction::ImulGpr8, MultiplyWide { signed: true }, Register, &[Code::Imul_rm8]),
    form(Instruction::ImulGprv, MultiplyWide { signed: true }, Register, &[Code::Imul_rm16, Code::Imul_rm32, Code::Imul_rm64]),
    form(Instruction::ImulMemb, MultiplyWide { signed: true }, Memory, &[Code::Imul_rm8]),
    form(Instruction::ImulMemv, MultiplyWide { signed: true }, Memory, &[Code::Imul_rm16, Code::Imul_rm32, Code::Imul_rm64]),
    // DIV and IDIV
    form(Instruction::DivGpr8, DivideWide { signed: false }, Register, &[Code::Div_rm8]),
    form(Instruction::DivGprv, DivideWide { signed: false }, Register, &[Code::Div_rm16, Code::Div_rm32, Code::Div_rm64]),
    form(Instruction::DivMemb, DivideWide { signed: false }, Memory, &[Code::Div_rm8]),
    form(Instruction::DivMemv, DivideWide { signed: false }, Memory, &[Code::Div_rm16, Code::Div_rm32, Code::Div_rm64]),
    form(Instruction::IdivGpr8, DivideWide { signed: true }, Register, &[Code::Idiv_rm8]),
    form(Instruction::IdivGprv, DivideWide { signed: true }, Register, &[Code::Idiv_rm16, Code::Idiv_rm32, Code::Idiv_rm64]),
    form(Instruction::IdivMemb, DivideWide { signed: true }, Memory, &[Code::Idiv_rm8]),
    form(Instruction::IdivMemv, DivideWide { signed: true }, Memory, &[Code::Idiv_rm16, Code::Idiv_rm32, Code::Idiv_rm64]),
    // CBW, CWDE, CDQE and CWD, CDQ, CQO
    form(Instruction::Cbw, SignExtendAccumulator, Register, &[Code::Cbw]),
    form(Instruction::Cwde, SignExtendAccumulator, Register, &[Code::Cwde]),
    form(Instruction::Cdqe, SignExtendAccumulator, Register, &[Code::Cdqe]),
    form(Instruction::Cwd, SignIntoData, Register, &[Code::Cwd]),
    form(Instruction::Cdq, SignIntoData, Register, &[Code::Cdq]),
    form(Instruction::Cqo, SignIntoData, Register, &[Code::Cqo]),
    // The flag operations
    form(Instruction::Clc, ClearCarry, Register, &[Code::Clc]),
    form(Instruction::Stc, SetCarry, Register, &[Code::Stc]),
    form(Instruction::Cmc, ComplementCarry, Register, &[Code::Cmc]),
    form(Instruction::Cld, ClearDirection, Register, &[Code::Cld]),
    form(Instruction::Std, SetDirection, Register, &[Code::Std]),
    form(Instruction::Lahf, LoadStatusIntoHigh, Register, &[Code::Lahf]),
    form(Instruction::Sahf, StoreHighIntoStatus, Register, &[Code::Sahf]),
    // SHLD and SHRD
    form(Instruction::ShldGprvGprvImmb, ShiftLeftDouble, Register, &[Code::Shld_rm16_r16_imm8, Code::Shld_rm32_r32_imm8, Code::Shld_rm64_r64_imm8]),
    form(Instruction::ShldMemvGprvImmb, ShiftLeftDouble, Memory, &[Code::Shld_rm16_r16_imm8, Code::Shld_rm32_r32_imm8, Code::Shld_rm64_r64_imm8]),
    form(Instruction::ShldGprvGprvCl, ShiftLeftDouble, Register, &[Code::Shld_rm16_r16_CL, Code::Shld_rm32_r32_CL, Code::Shld_rm64_r64_CL]),
    form(Instruction::ShldMemvGprvCl, ShiftLeftDouble, Memory, &[Code::Shld_rm16_r16_CL, Code::Shld_rm32_r32_CL, Code::Shld_rm64_r64_CL]),
    form(Instruction::ShrdGprvGprvImmb, ShiftRightDouble, Register, &[Code::Shrd_rm16_r16_imm8, Code::Shrd_rm32_r32_imm8, Code::Shrd_rm64_r64_imm8]),
    form(Instruction::ShrdMemvGprvImmb, ShiftRightDouble, Memory, &[Code::Shrd_rm16_r16_imm8, Code::Shrd_rm32_r32_imm8, Code::Shrd_rm64_r64_imm8]),
    form(Instruction::ShrdGprvGprvCl, ShiftRightDouble, Register, &[Code::Shrd_rm16_r16_CL, Code::Shrd_rm32_r32_CL, Code::Shrd_rm64_r64_CL]),
    form(Instruction::ShrdMemvGprvCl, ShiftRightDouble, Memory, &[Code::Shrd_rm16_r16_CL, Code::Shrd_rm32_r32_CL, Code::Shrd_rm64_r64_CL]),
    // BT, BTS, BTR and BTC
    form(Instruction::BtGprvGprv, BitTest(BitChange::Keep), Register, &[Code::Bt_rm16_r16, Code::Bt_rm32_r32, Code::Bt_rm64_r64]),
    form(Instruction::BtMemvGprv, BitTest(BitChange::Keep), Memory, &[Code::Bt_rm16_r16, Code::Bt_rm32_r32, Code::Bt_rm64_r64]),
    form(Instruction::BtGprvImmb, BitTest(BitChange::Keep), Register, &[Code::Bt_rm16_imm8, Code::Bt_rm32_imm8, Code::Bt_rm64_imm8]),
    form(Instruction::BtMemvImmb, BitTest(BitChange::Keep), Memory, &[Code::Bt_rm16_imm8, Code::Bt_rm32_imm8, Code::Bt_rm64_imm8]),
    form(Instruction::BtsGprvGprv, BitTest(BitChange::Set), Register, &[Code::Bts_rm16_r16, Code::Bts_rm32_r32, Code::Bts_rm64_r64]),
    form(Instruction::BtsMemvGprv, BitTest(BitChange::Set), Memory, &[Code::Bts_rm16_r16, Code::Bts_rm32_r32, Code::Bts_rm64_r64]),
    form(Instruction::BtsLockMemvGprv, BitTest(BitChange::Set), LockedMemory, &[Code::Bts_rm16_r16, Code::Bts_rm32_r32, Code::Bts_rm64_r64]),
    form(Instruction::BtsGprvImmb, BitTest(BitChange::Set), Register, &[Code::Bts_rm16_imm8, Code::Bts_rm32_imm8, Code::Bts_rm64_imm8]),
    form(Instruction::BtsMemvImmb, BitTest(BitChange::Set), Memory, &[Code::Bts_rm16_imm8, Code::Bts_rm32_imm8, Code::Bts_rm64_imm8]),
    form(Instruction::BtsLockMemvImmb, BitTest(BitChange::Set), LockedMemory, &[Code::Bts_rm16_imm8, Code::Bts_rm32_imm8, Code::Bts_rm64_imm8]),
    form(Instruction::BtrGprvGprv, BitTest(BitChange::Reset), Register, &[Code::Btr_rm16_r16, Code::Btr_rm32_r32, Code::Btr_rm64_r64]),
    form(Instruction::BtrMemvGprv, BitTest(BitChange::Reset), Memory, &[Code::Btr_rm16_r16, Code::Btr_rm32_r32, Code::Btr_rm64_r64]),
    form(Instruction::BtrLockMemvGprv, BitTest(BitChange::Reset), LockedMemory, &[Code::Btr_rm16_r16, Code::Btr_rm32_r32, Code::Btr_rm64_r64]),
    form(Instruction::BtrGprvImmb, BitTest(BitChange::Reset), Register, &[Code::Btr_rm16_imm8, Code::Btr_rm32_imm8, Code::Btr_rm64_imm8]),
    form(Instruction::BtrMemvImmb, BitTest(BitChange::Reset), Memory, &[Code::Btr_rm16_imm8, Code::Btr_rm32_imm8, Code::Btr_rm64_imm8]),
    form(Instruction::BtrLockMemvImmb, BitTest(BitChange::Reset), LockedMemory, &[Code::Btr_rm16_imm8, Code::Btr_rm32_imm8, Code::Btr_rm64_imm8]),
    form(Instruction::BtcGprvGprv, BitTest(BitChange::Complement), Register, &[Code::Btc_rm16_r16, Code::Btc_rm32_r32, Code::Btc_rm64_r64]),
    form(Instruction::BtcMemvGprv, BitTest(BitChange::Complement), Memory, &[Code::Btc_rm16_r16, Code::Btc_rm32_r32, Code::Btc_rm64_r64]),
    form(Instruction::BtcLockMemvGprv, BitTest(BitChange::Complement), LockedMemory, &[Code::Btc_rm16_r16, Code::Btc_rm32_r32, Code::Btc_rm64_r64]),
    form(Instruction::BtcGprvImmb, BitTest(BitChange::Complement), Register, &[Code::Btc_rm16_imm8, Code::Btc_rm32_imm8, Code::Btc_rm64_imm8]),
    form(Instruction::BtcMemvImmb, BitTest(BitChange::Complement), Memory, &[Code::Btc_rm16_imm8, Code::Btc_rm32_imm8, Code::Btc_rm64_imm8]),
    form(Instruction::BtcLockMemvImmb, BitTest(BitChange::Complement), LockedMemory, &[Code::Btc_rm16_imm8, Code::Btc_rm32_imm8, Code::Btc_rm64_imm8]),
    // NOP
    form(Instruction::Nop90, NoOperation, Register, &[Code::Nopw, Code::Nopd, Code::Nopq]),
    form(Instruction::NopGprv0f1f, NoOperation, Register, &[Code::Nop_rm16, Code::Nop_rm32, Code::Nop_rm64]),
    form(Instruction::NopMemv0f1f, NoOperation, Memory, &[Code::Nop_rm16, Code::Nop_rm32, Code::Nop_rm64]),
    // BSF and BSR
    form(Instruction::BsfGprvGprv, BitScanForward, Register, &[Code::Bsf_r16_rm16, Code::Bsf_r32_rm32, Code::Bsf_r64_rm64]),
    form(Instruction::BsfGprvMemv, BitScanForward, Memory, &[Code::Bsf_r16_rm16, Code::Bsf_r32_rm32, Code::Bsf_r64_rm64]),
    form(Instruction::BsrGprvGprv, BitScanReverse, Register, &[Code::Bsr_r16_rm16, Code::Bsr_r32_rm32, Code::Bsr_r64_rm64]),
    form(Instruction::BsrGprvMemv, BitScanReverse, Memory, &[Code::Bsr_r16_rm16, Code::Bsr_r32_rm32, Code::Bsr_r64_rm64]),
    // BSWAP
    form(Instruction::BswapGprv, ByteSwap, Register, &[Code::Bswap_r16, Code::Bswap_r32, Code::Bswap_r64]),
    // CMPXCHG
    form(Instruction::CmpxchgGpr8Gpr8, CompareExchange, Register, &[Code::Cmpxchg_rm8_r8]),
    form(Instruction::CmpxchgGprvGprv, CompareExchange, Register, &[Code::Cmpxchg_rm16_r16, Code::Cmpxchg_rm32_r32, Code::Cmpxchg_rm64_r64]),
    form(Instruction::CmpxchgMembGpr8, CompareExchange, Memory, &[Code::Cmpxchg_rm8_r8]),
    form(Instruction::CmpxchgMemvGprv, CompareExchange, Memory, &[Code::Cmpxchg_rm16_r16, Code::Cmpxchg_rm32_r32, Code::Cmpxchg_rm64_r64]),
    form(Instruction::CmpxchgLockMembGpr8, CompareExchange, LockedMemory, &[Code::Cmpxchg_rm8_r8]),
    form(Instruction::CmpxchgLockMemvGprv, CompareExchange, LockedMemory, &[Code::Cmpxchg_rm16_r16, Code::Cmpxchg_rm32_r32, Code::Cmpxchg_rm64_r64]),
    // LEA
    form(Instruction::LeaGprvAgen, LoadEffectiveAddress, Memory, &[Code::Lea_r16_m, Code::Lea_r32_m, Code::Lea_r64_m]),
    // UD0 and UD1 with their ModRM byte
    form(Instruction::Ud0Gpr32Gpr32, RaiseInvalidOpcode, Register, &[Code::Ud0_r32_rm32]),
    form(Instruction::Ud0Gpr32Memd, RaiseInvalidOpcode, Memory, &[Code::Ud0_r32_rm32]),
    form(Instruction::Ud1Gpr32Gpr32, RaiseInvalidOpcode, Register, &[Code::Ud1_r32_rm32]),
    form(Instruction::Ud1Gpr32Memd, RaiseInvalidOpcode, Memory, &[Code::Ud1_r32_rm32]),
    // The hints a ring 3 program on this chip runs as no operation: PAUSE, the prefetches (never a fault), CLDEMOTE,
    // SERIALIZE (nothing a single step sees), ENDBR32 and ENDBR64, and RDSSPD and RDSSPQ, which leave their register
    // as it was while the shadow stack is off, as Linux leaves it for a program that did not ask for one.
    form(Instruction::Pause, NoOperation, Register, &[Code::Pause]),
    form(Instruction::Prefetchw0f0dr1, NoOperation, Memory, &[Code::Prefetchw_m8]),
    form(Instruction::Prefetchw0f0dr3, NoOperation, Memory, &[Code::Prefetchreserved3_m8]),
    form(Instruction::PrefetchExclusiveMemmprefetch, NoOperation, Memory, &[Code::Prefetch_m8]),
    form(Instruction::PrefetchReserved0f0dr4, NoOperation, Memory, &[Code::Prefetchreserved4_m8]),
    form(Instruction::PrefetchReserved0f0dr5, NoOperation, Memory, &[Code::Prefetchreserved5_m8]),
    form(Instruction::PrefetchReserved0f0dr6, NoOperation, Memory, &[Code::Prefetchreserved6_m8]),
    form(Instruction::PrefetchReserved0f0dr7, NoOperation, Memory, &[Code::Prefetchreserved7_m8]),
    form(Instruction::Prefetchwt1Memu8, NoOperation, Memory, &[Code::Prefetchwt1_m8]),
    form(Instruction::Prefetchit0Memu8, NoOperation, Memory, &[Code::Prefetchit0_m8]),
    form(Instruction::Prefetchit1Memu8, NoOperation, Memory, &[Code::Prefetchit1_m8]),
    form(Instruction::CldemoteMemu8, NoOperation, Memory, &[Code::Cldemote_m8]),
    form(Instruction::Serialize, NoOperation, Register, &[Code::Serialize]),
    form(Instruction::Endbr32, NoOperation, Register, &[Code::Endbr32]),
    form(Instruction::Endbr64, NoOperation, Register, &[Code::Endbr64]),
    form(Instruction::RdsspdGpr32u32, NoOperation, Register, &[Code::Rdsspd_r32]),
    form(Instruction::RdsspqGpr64u64, NoOperation, Register, &[Code::Rdsspq_r64]),
];
