//! The x87 unit in software (plan 5.8): what the x87 hook does to the block's FXSAVE area. Arithmetic is
//! `rustc_apfloat`'s, done wide and rounded to odd, then rounded once to the precision control's width
//! (Boldo and Melquiond); special operands, stack faults and the status word follow Intel's manual and the
//! reference CPU.

use crate::Fxsave;
use rustc_apfloat::ieee::{
    Double, IeeeFloat, Semantics, Single, X87DoubleExtended, X87DoubleExtendedS,
};
use rustc_apfloat::{Float, FloatConvert, Round, Status, StatusAnd};
use zerocopy::FromBytes;

/// The status word (Intel SDM Vol. 1 8.1.3): the six exception flags in MXCSR's order, the stack fault, the
/// summary, the condition codes and TOP.
pub const INVALID: u16 = 1;
pub const DENORMAL: u16 = 1 << 1;
pub const DIVIDE_BY_ZERO: u16 = 1 << 2;
pub const OVERFLOW: u16 = 1 << 3;
pub const UNDERFLOW: u16 = 1 << 4;
pub const PRECISION: u16 = 1 << 5;
pub const EXCEPTIONS: u16 = (PRECISION << 1) - 1;
pub const STACK_FAULT: u16 = 1 << 6;
pub const SUMMARY: u16 = 1 << 7;
pub const C0: u16 = 1 << 8;
pub const C1: u16 = 1 << 9;
pub const C2: u16 = 1 << 10;
pub const TOP_SHIFT: u16 = 11;
pub const TOP: u16 = (REGISTERS as u16 - 1) << TOP_SHIFT;
pub const C3: u16 = 1 << 14;
pub const BUSY: u16 = 1 << 15;
pub const CONDITIONS: u16 = C0 | C1 | C2 | C3;
/// The exceptions met before a result exists.
const BEFORE_RESULT: u16 = INVALID | DENORMAL | DIVIDE_BY_ZERO;
/// What `fnclex` clears.
const CLEARED: u16 = EXCEPTIONS | STACK_FAULT | SUMMARY | BUSY;

/// The control word: the six masks, the precision control (24, reserved, 53, 64 bits) and the rounding
/// control (nearest, down, up, toward zero). Bit 6 always reads as set; the infinity control bit 12 is kept
/// and does nothing.
pub const PRECISION_SHIFT: u16 = 8;
pub const ROUNDING_SHIFT: u16 = 10;
const FIELD: u16 = 3;
pub const CONTROL_ALWAYS: u16 = 1 << 6;
const INFINITY_CONTROL: u16 = 1 << 12;
pub const CONTROL_KEPT: u16 = INFINITY_CONTROL
    | FIELD << ROUNDING_SHIFT
    | FIELD << PRECISION_SHIFT
    | CONTROL_ALWAYS
    | EXCEPTIONS;
pub const CONTROL_AT_START: u16 = crate::FCW_AT_START;

/// The eight registers.
pub const REGISTERS: usize = Fxsave::ZERO.st.len();
const REGISTER_MASK: usize = REGISTERS - 1;

/// The full tag word's two bits for each register.
const TAG_VALID: u16 = 0;
const TAG_ZERO: u16 = 1;
const TAG_SPECIAL: u16 = 2;
const TAG_EMPTY: u16 = 3;
const TAG_BITS: u32 = 2;

/// An 80-bit value: sign, biased 15-bit exponent and the 64-bit significand with its explicit integer bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extended {
    pub sign: bool,
    pub exponent: u16,
    pub significand: u64,
}

pub const EXPONENT_BITS: u32 = X87DoubleExtendedS::EXP_BITS as u32;
pub const EXPONENT_ALL: u16 = (1 << EXPONENT_BITS) - 1;
const SIGN: u16 = 1 << EXPONENT_BITS;
pub const BIAS: i32 = (1 << (EXPONENT_BITS - 1)) - 1;
pub const INTEGER_BIT: u64 = 1 << (u64::BITS - 1);
pub const QUIET_BIT: u64 = INTEGER_BIT >> 1;
/// What an unmasked overflow or underflow takes from or adds to the exponent of a result kept in a register
/// (Intel SDM Vol. 1 4.9.1.4).
const EXPONENT_ADJUST: i32 = 0x6000;

impl Extended {
    /// The real indefinite, the default NaN.
    pub const INDEFINITE: Self = Self {
        sign: true,
        exponent: EXPONENT_ALL,
        significand: INTEGER_BIT | QUIET_BIT,
    };
    pub const ZERO: Self = Self {
        sign: false,
        exponent: 0,
        significand: 0,
    };
    pub const INFINITY: Self = Self {
        sign: false,
        exponent: EXPONENT_ALL,
        significand: INTEGER_BIT,
    };

    pub fn from_bits(bits: u128) -> Self {
        let high = (bits >> u64::BITS) as u16;
        Self {
            sign: high & SIGN != 0,
            exponent: high & EXPONENT_ALL,
            significand: bits as u64,
        }
    }

    pub fn to_bits(self) -> u128 {
        let high = self.exponent | if self.sign { SIGN } else { 0 };
        u128::from(high) << u64::BITS | u128::from(self.significand)
    }

    pub fn class(self) -> Class {
        let integer = self.significand & INTEGER_BIT != 0;
        let fraction = self.significand & !INTEGER_BIT;
        match self.exponent {
            0 if self.significand == 0 => Class::Zero,
            0 => Class::Denormal,
            EXPONENT_ALL if !integer => Class::Unsupported,
            EXPONENT_ALL if fraction == 0 => Class::Infinity,
            EXPONENT_ALL if fraction & QUIET_BIT == 0 => Class::Signaling,
            EXPONENT_ALL => Class::Quiet,
            _ if integer => Class::Normal,
            _ => Class::Unsupported,
        }
    }

    fn negate(self) -> Self {
        Self {
            sign: !self.sign,
            ..self
        }
    }

    fn with_sign(self, sign: bool) -> Self {
        Self { sign, ..self }
    }

    fn quieted(self) -> Self {
        Self {
            significand: self.significand | QUIET_BIT,
            ..self
        }
    }

    fn is_nan(self) -> bool {
        matches!(self.class(), Class::Signaling | Class::Quiet)
    }

    /// The value as apfloat's; finite, well-formed values only (a pseudo-denormal is the normal it equals).
    fn float(self) -> X87DoubleExtended {
        let exponent = if self.exponent == 0 && self.significand & INTEGER_BIT != 0 {
            1
        } else {
            self.exponent
        };
        X87DoubleExtended::from_bits(Self { exponent, ..self }.to_bits())
    }

    fn of(f: X87DoubleExtended) -> Self {
        Self::from_bits(f.to_bits())
    }

    fn wide(self) -> Wide {
        exactly(self.float())
    }

    /// The full tag word's two bits.
    fn tag(self) -> u16 {
        match self.class() {
            Class::Zero => TAG_ZERO,
            Class::Normal => TAG_VALID,
            _ => TAG_SPECIAL,
        }
    }
}

/// What an 80-bit value is, as `fxam` and the special-operand rules see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Zero,
    /// A denormal or pseudo-denormal.
    Denormal,
    Normal,
    Infinity,
    Signaling,
    Quiet,
    /// Unnormals, pseudo-infinities and pseudo-NaNs, which every x87 since the 387 refuses.
    Unsupported,
}

/// Bits a value needs beyond its target precision so that rounding it to odd and then once more rounds
/// correctly (Boldo and Melquiond, 2008).
const ODD_EXTRA: usize = 2;
/// Exponent bits wide enough that no product or quotient of 80-bit values, nor one moved by `SCALE_LIMIT`,
/// leaves the range.
const WIDE_EXPONENT_BITS: usize = X87DoubleExtendedS::EXP_BITS + WIDE_EXTRA_EXPONENT_BITS;
const WIDE_EXTRA_EXPONENT_BITS: usize = 3;
const SIGN_BITS: usize = 1;
const EXTENDED_PRECISION: usize = X87DoubleExtendedS::PRECISION;
const DOUBLE_PRECISION: usize = f64::MANTISSA_DIGITS as usize;
const SINGLE_PRECISION: usize = f32::MANTISSA_DIGITS as usize;

/// Semantics of `precision` bits with the 80-bit format's exponent range, or an exponent range no result
/// leaves.
macro_rules! semantics {
    ($name:ident, $precision:expr, $exponent:expr) => {
        pub(crate) struct $name;
        impl Semantics for $name {
            const BITS: usize = SIGN_BITS + $exponent + $precision - 1;
            const EXP_BITS: usize = $exponent;
        }
    };
}
semantics!(WideS, EXTENDED_PRECISION + ODD_EXTRA, WIDE_EXPONENT_BITS);
semantics!(SingleRangeS, SINGLE_PRECISION, X87DoubleExtendedS::EXP_BITS);
semantics!(DoubleRangeS, DOUBLE_PRECISION, X87DoubleExtendedS::EXP_BITS);
semantics!(SingleWideS, SINGLE_PRECISION, WIDE_EXPONENT_BITS);
semantics!(DoubleWideS, DOUBLE_PRECISION, WIDE_EXPONENT_BITS);
semantics!(ExtendedWideS, EXTENDED_PRECISION, WIDE_EXPONENT_BITS);
pub(crate) type Wide = IeeeFloat<WideS>;

fn exactly<A: FloatConvert<B>, B: Float>(a: A) -> B {
    a.convert_r(Round::TowardZero, &mut false).value
}

/// A wide result rounded to odd: its lowest bit set when it is not exact.
fn odd(r: StatusAnd<Wide>) -> Wide {
    if r.status.contains(Status::INEXACT) {
        Wide::from_bits(r.value.to_bits() | 1)
    } else {
        r.value
    }
}

/// The precision control's widths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Single,
    Double,
    Extended,
}

impl Precision {
    fn of(fcw: u16) -> Self {
        match fcw >> PRECISION_SHIFT & FIELD {
            0 => Precision::Single,
            2 => Precision::Double,
            _ => Precision::Extended,
        }
    }
}

fn rounding(fcw: u16) -> Round {
    match fcw >> ROUNDING_SHIFT & FIELD {
        0 => Round::NearestTiesToEven,
        1 => Round::TowardNegative,
        2 => Round::TowardPositive,
        _ => Round::TowardZero,
    }
}

/// A result rounded for its destination: the value (none when an unmasked exception keeps it from its
/// destination), the exceptions met and whether the rounding went up in magnitude (C1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rounded<T> {
    pub value: Option<T>,
    pub exceptions: u16,
    pub up: bool,
}

/// Round `odd` once into the format `B` (with the exponent range of `U`, where no result overflows, to see
/// tininess after rounding as x86 does), with the unmasked-overflow and unmasked-underflow rules:
/// `register` results are kept with their exponent adjusted, memory results are not stored.
fn round_into<B: Semantics, U: Semantics>(
    odd: Wide,
    fcw: u16,
    register: bool,
) -> Rounded<IeeeFloat<B>>
where
    Wide: FloatConvert<IeeeFloat<B>> + FloatConvert<IeeeFloat<U>>,
    IeeeFloat<U>: FloatConvert<IeeeFloat<B>> + FloatConvert<Wide>,
    IeeeFloat<B>: FloatConvert<Wide>,
{
    let round = rounding(fcw);
    let masked = |flag: u16| fcw & flag != 0;
    if odd.is_zero() {
        return Rounded {
            value: Some(exactly(odd)),
            exceptions: 0,
            up: false,
        };
    }
    let wide = odd.convert_r(round, &mut false);
    let unbounded: IeeeFloat<U> = wide.value;
    let inexact = wide.status.contains(Status::INEXACT);
    let above = |r: Wide| r.abs() > odd.abs();
    let precision = if inexact { PRECISION } else { 0 };
    let exponent = unbounded.ilogb();
    if exponent > B::MAX_EXP {
        if !masked(OVERFLOW) {
            if !register {
                return Rounded {
                    value: None,
                    exceptions: OVERFLOW,
                    up: false,
                };
            }
            return adjusted(unbounded.scalbn(-EXPONENT_ADJUST), OVERFLOW, inexact, above);
        }
        let infinite = match round {
            Round::NearestTiesToEven | Round::NearestTiesToAway => true,
            Round::TowardZero => false,
            Round::TowardPositive => !odd.is_negative(),
            Round::TowardNegative => odd.is_negative(),
        };
        let value = if infinite {
            IeeeFloat::<B>::INFINITY
        } else {
            IeeeFloat::<B>::largest()
        };
        let value = if odd.is_negative() { -value } else { value };
        return Rounded {
            value: Some(value),
            exceptions: OVERFLOW | PRECISION,
            up: infinite,
        };
    }
    if exponent < B::MIN_EXP {
        if !masked(UNDERFLOW) {
            if !register {
                return Rounded {
                    value: None,
                    exceptions: UNDERFLOW,
                    up: false,
                };
            }
            return adjusted(unbounded.scalbn(EXPONENT_ADJUST), UNDERFLOW, inexact, above);
        }
        let denormal = odd.convert_r(round, &mut false);
        let lost = denormal.status.contains(Status::INEXACT);
        return Rounded {
            value: Some(denormal.value),
            exceptions: if lost { UNDERFLOW | PRECISION } else { 0 },
            up: above(exactly(denormal.value)),
        };
    }
    Rounded {
        value: Some(exactly(unbounded)),
        exceptions: precision,
        up: above(exactly(unbounded)),
    }
}

/// The result an unmasked overflow or underflow keeps in a register, its exponent adjusted (`moved`), inexact
/// when the rounding was. Where even that leaves the format's range (`fscale` can), the reference keeps a
/// signed zero or infinity, inexact.
fn adjusted<B: Semantics, U: Semantics>(
    moved: IeeeFloat<U>,
    flag: u16,
    inexact: bool,
    above: impl Fn(Wide) -> bool,
) -> Rounded<IeeeFloat<B>>
where
    IeeeFloat<U>: FloatConvert<IeeeFloat<B>> + FloatConvert<Wide>,
    IeeeFloat<B>: FloatConvert<Wide>,
{
    let exponent = moved.ilogb();
    if exponent > B::MAX_EXP || exponent < B::MIN_EXP {
        let none = if exponent > B::MAX_EXP {
            IeeeFloat::<B>::INFINITY
        } else {
            IeeeFloat::<B>::ZERO
        };
        return Rounded {
            value: Some(if moved.is_negative() { -none } else { none }),
            exceptions: flag | PRECISION,
            up: false,
        };
    }
    let back = if flag == OVERFLOW {
        EXPONENT_ADJUST
    } else {
        -EXPONENT_ADJUST
    };
    let kept: IeeeFloat<B> = exactly(moved);
    Rounded {
        value: Some(kept),
        exceptions: flag | if inexact { PRECISION } else { 0 },
        up: above(exactly::<IeeeFloat<B>, Wide>(kept).scalbn(back)),
    }
}

/// Round `odd` for a register at the precision control's width.
fn round_register(odd: Wide, fcw: u16, precision: Precision) -> Rounded<Extended> {
    fn widened<B: Semantics>(r: Rounded<IeeeFloat<B>>) -> Rounded<Extended>
    where
        IeeeFloat<B>: FloatConvert<X87DoubleExtended>,
    {
        Rounded {
            value: r.value.map(|v| Extended::of(exactly(v))),
            exceptions: r.exceptions,
            up: r.up,
        }
    }
    match precision {
        Precision::Single => widened(round_into::<SingleRangeS, SingleWideS>(odd, fcw, true)),
        Precision::Double => widened(round_into::<DoubleRangeS, DoubleWideS>(odd, fcw, true)),
        Precision::Extended => widened(round_into::<X87DoubleExtendedS, ExtendedWideS>(
            odd, fcw, true,
        )),
    }
}

/// The memory formats, and the register itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Register,
    Single,
    Double,
    Extended,
    Word,
    Integer,
    Long,
    Decimal,
}

impl Format {
    const ALL: &'static [Format] = &[
        Format::Register,
        Format::Single,
        Format::Double,
        Format::Extended,
        Format::Word,
        Format::Integer,
        Format::Long,
        Format::Decimal,
    ];

    /// The bits of an integer format.
    fn integer_bits(self) -> Option<u32> {
        match self {
            Format::Word => Some(u16::BITS),
            Format::Integer => Some(u32::BITS),
            Format::Long => Some(u64::BITS),
            _ => None,
        }
    }
}

/// Packed decimal: 18 digits in nine bytes, then the sign byte.
pub const DECIMAL_DIGITS: u32 = 18;
pub const DIGIT_BITS: u32 = 4;
const DECIMAL_SIGN: u128 = 1 << (DECIMAL_DIGITS * DIGIT_BITS + 2 * DIGIT_BITS - 1);
pub const DECIMAL_RADIX: u128 = 10;
/// The packed decimal indefinite (Intel SDM Vol. 1 4.7, table 4-4).
const DECIMAL_INDEFINITE: u128 = 0xffff_c000_0000_0000_0000;

/// What the x87 hook does, with its fields packed into the hook's i32 code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Push a memory value of `format` (`fld`, `fild`, `fbld`) or ST(`register`).
    Load,
    /// Push constant `register` of `CONSTANTS`.
    LoadConstant,
    /// ST(0) to memory of `format` or to ST(`register`), then `pops` pops; `truncate` for `fisttp`.
    Store,
    /// `arithmetic` of ST(0) or ST(`register`) (the destination when `to_register`) with the other or memory.
    Arithmetic,
    /// ST(0) against ST(`register`) or memory; `quiet` for `fucom`, `flags` for `fcomi`.
    Compare,
    Test,
    Examine,
    ChangeSign,
    Absolute,
    Exchange,
    /// ST(0) = ST(`register`) when the hook's first argument is not zero.
    ConditionalMove,
    Free,
    Increment,
    Decrement,
    Initialize,
    ClearExceptions,
    /// The control word from the first argument.
    LoadControl,
    RoundToInteger,
    Scale,
    Extract,
    SquareRoot,
    /// The full tag word, as `fnstenv` and `fnsave` store it; `fnstenv` then masks every exception.
    StoreEnvironment,
    /// `fnsave`'s: the full tag word, then the unit is initialised.
    Save,
    /// The control, status and full tag words and FOP from the first argument, the instruction pointer
    /// from the second and the data pointer from the third, as `fldenv` and `frstor` load them.
    LoadEnvironment,
    /// `fprem` (`truncate`) or `fprem1`: ST(0) less a multiple of ST(1), the quotient's low bits in C0, C3
    /// and C1, or C2 when only part of the way.
    PartialRemainder,
    /// ST(`register`)'s 80 bits, whatever its tag.
    Register,
    /// ST(`register`)'s 80 bits from the two arguments, its tag as it is.
    SetRegister,
}

impl Operation {
    const ALL: &'static [Operation] = &[
        Operation::Load,
        Operation::LoadConstant,
        Operation::Store,
        Operation::Arithmetic,
        Operation::Compare,
        Operation::Test,
        Operation::Examine,
        Operation::ChangeSign,
        Operation::Absolute,
        Operation::Exchange,
        Operation::ConditionalMove,
        Operation::Free,
        Operation::Increment,
        Operation::Decrement,
        Operation::Initialize,
        Operation::ClearExceptions,
        Operation::LoadControl,
        Operation::RoundToInteger,
        Operation::Scale,
        Operation::Extract,
        Operation::SquareRoot,
        Operation::StoreEnvironment,
        Operation::Save,
        Operation::LoadEnvironment,
        Operation::Register,
        Operation::SetRegister,
        Operation::PartialRemainder,
    ];
}

/// The bytes of the protected-mode environment `fnstenv` stores, in its 32-bit and 16-bit forms (Intel SDM
/// Vol. 1 8.1.10), and of the eight registers `fnsave` stores after it.
pub const ENVIRONMENT_32_BYTES: u32 = 28;
pub const ENVIRONMENT_16_BYTES: u32 = 14;
pub const REGISTERS_BYTES: u32 = REGISTERS as u32 * crate::X87_REGISTER_BITS / u8::BITS;

/// The words `fnstenv` stores before its pointers: control, status and tag.
pub const ENVIRONMENT_WORDS: u32 = 3;
/// Where FOP sits in the environment word the hook takes and gives.
pub const FOP_SHIFT: u32 = ENVIRONMENT_WORDS * u16::BITS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arithmetic {
    Add,
    Subtract,
    SubtractReversed,
    Multiply,
    Divide,
    DivideReversed,
}

impl Arithmetic {
    const ALL: &'static [Arithmetic] = &[
        Arithmetic::Add,
        Arithmetic::Subtract,
        Arithmetic::SubtractReversed,
        Arithmetic::Multiply,
        Arithmetic::Divide,
        Arithmetic::DivideReversed,
    ];
}

/// One call of the x87 hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Code {
    pub operation: Operation,
    pub register: u8,
    pub format: Format,
    pub arithmetic: Arithmetic,
    pub pops: u8,
    pub to_register: bool,
    /// No invalid operation for a quiet NaN (`fucom`), or none for an empty ST(0) (`fstpnce`).
    pub quiet: bool,
    pub flags: bool,
    pub truncate: bool,
    /// The low 11 bits of the instruction's opcode and ModRM, which an unmasked exception leaves in FOP.
    pub opcode: u16,
}

/// The fields' widths in the code, from the lowest bits.
const OPERATION_BITS: u32 = u8::BITS - 2;
const REGISTER_BITS: u32 = REGISTERS.trailing_zeros();
const FORMAT_BITS: u32 = 3;
const ARITHMETIC_BITS: u32 = 3;
const POPS_BITS: u32 = 2;
pub const OPCODE_BITS: u32 = 11;
const REGISTER_AT: u32 = OPERATION_BITS;
const FORMAT_AT: u32 = REGISTER_AT + REGISTER_BITS;
const ARITHMETIC_AT: u32 = FORMAT_AT + FORMAT_BITS;
const POPS_AT: u32 = ARITHMETIC_AT + ARITHMETIC_BITS;
const TO_REGISTER_AT: u32 = POPS_AT + POPS_BITS;
const QUIET_AT: u32 = TO_REGISTER_AT + 1;
const FLAGS_AT: u32 = QUIET_AT + 1;
const TRUNCATE_AT: u32 = FLAGS_AT + 1;
const OPCODE_AT: u32 = TRUNCATE_AT + 1;
const _: () = assert!(Operation::ALL.len() <= 1 << OPERATION_BITS);
const _: () = assert!(Format::ALL.len() <= 1 << FORMAT_BITS);
const _: () = assert!(Arithmetic::ALL.len() <= 1 << ARITHMETIC_BITS);
const _: () = assert!(OPCODE_AT + OPCODE_BITS <= u32::BITS);

fn index<T: PartialEq>(all: &[T], x: T) -> u32 {
    all.iter().position(|y| *y == x).expect("listed") as u32
}

const fn field(code: u32, at: u32, bits: u32) -> u32 {
    code >> at & ((1 << bits) - 1)
}

impl Code {
    pub const fn new(operation: Operation) -> Self {
        Code {
            operation,
            register: 0,
            format: Format::Register,
            arithmetic: Arithmetic::Add,
            pops: 0,
            to_register: false,
            quiet: false,
            flags: false,
            truncate: false,
            opcode: 0,
        }
    }

    pub fn encode(self) -> i32 {
        (index(Operation::ALL, self.operation)
            | u32::from(self.register) << REGISTER_AT
            | index(Format::ALL, self.format) << FORMAT_AT
            | index(Arithmetic::ALL, self.arithmetic) << ARITHMETIC_AT
            | u32::from(self.pops) << POPS_AT
            | u32::from(self.to_register) << TO_REGISTER_AT
            | u32::from(self.quiet) << QUIET_AT
            | u32::from(self.flags) << FLAGS_AT
            | u32::from(self.truncate) << TRUNCATE_AT
            | u32::from(self.opcode) << OPCODE_AT) as i32
    }

    pub fn decode(code: i32) -> Self {
        let code = code as u32;
        Code {
            operation: Operation::ALL[field(code, 0, OPERATION_BITS) as usize],
            register: field(code, REGISTER_AT, REGISTER_BITS) as u8,
            format: Format::ALL[field(code, FORMAT_AT, FORMAT_BITS) as usize],
            arithmetic: Arithmetic::ALL[field(code, ARITHMETIC_AT, ARITHMETIC_BITS) as usize],
            pops: field(code, POPS_AT, POPS_BITS) as u8,
            to_register: field(code, TO_REGISTER_AT, 1) != 0,
            quiet: field(code, QUIET_AT, 1) != 0,
            flags: field(code, FLAGS_AT, 1) != 0,
            truncate: field(code, TRUNCATE_AT, 1) != 0,
            opcode: field(code, OPCODE_AT, OPCODE_BITS) as u16,
        }
    }
}

/// The hook's second result when the value in the first must not be stored: an unmasked exception kept it
/// from memory, which is not looked at.
pub const NOT_STORED: u64 = 1 << (u64::BITS - 1);
/// The rflags bits `fcomi` and its kin set: unordered sets all three.
const GREATER: u64 = 0;
const LESS: u64 = crate::CF;
const EQUAL: u64 = crate::ZF;
const UNORDERED: u64 = crate::ZF | crate::PF | crate::CF;

/// The constants `fld1`, `fldl2t`, `fldl2e`, `fldpi`, `fldlg2`, `fldln2` and `fldz` push, to 128 bits
/// (sign, exponent, significand), rounded by the rounding control.
pub const CONSTANTS: [(u16, u128); 7] = [
    (BIAS as u16, 1 << (u128::BITS - 1)),
    (BIAS as u16 + 1, 0xd49a_784b_cd1b_8afe_492b_f6ff_4daf_db4d),
    (BIAS as u16, 0xb8aa_3b29_5c17_f0bb_be87_fed0_691d_3e89),
    (BIAS as u16 + 1, 0xc90f_daa2_2168_c234_c4c6_628b_80dc_1cd1),
    (BIAS as u16 - 2, 0x9a20_9a84_fbcf_f798_8f89_59ac_0b7c_9178),
    (BIAS as u16 - 1, 0xb172_17f7_d1cf_79ab_c9e3_b398_03f2_f6af),
    (0, 0),
];

/// What a hook call gives back: two i64 results.
pub type Results = [i64; 2];

/// The unit, over the FXSAVE area of the block.
pub struct Unit<'a> {
    pub area: &'a mut Fxsave,
    /// The data address of the instruction, which an unmasked exception leaves in FDP.
    pub data: u64,
    pub code: Code,
}

impl<'a> Unit<'a> {
    pub fn new(area: &'a mut [u8], code: Code, data: u64) -> Self {
        Unit {
            area: Fxsave::mut_from_bytes(area).expect("the FXSAVE area"),
            data,
            code,
        }
    }

    fn top(&self) -> usize {
        usize::from((self.area.fsw & TOP) >> TOP_SHIFT)
    }

    fn set_top(&mut self, top: usize) {
        self.area.fsw = self.area.fsw & !TOP | ((top & REGISTER_MASK) as u16) << TOP_SHIFT;
    }

    fn physical(&self, i: u8) -> usize {
        (self.top() + usize::from(i)) & REGISTER_MASK
    }

    fn empty(&self, i: u8) -> bool {
        self.area.ftw & 1 << self.physical(i) == 0
    }

    pub fn get(&self, i: u8) -> Extended {
        let slot = self.area.st[self.physical(i)];
        Extended::from_bits(u128::from_le_bytes(slot) & ((1 << crate::X87_REGISTER_BITS) - 1))
    }

    /// Write ST(i) and mark it valid; the slot's bytes past the 80 bits are zero.
    pub fn set(&mut self, i: u8, value: Extended) {
        let p = self.physical(i);
        self.area.st[p] = value.to_bits().to_le_bytes();
        self.area.ftw |= 1 << p;
    }

    fn free(&mut self, i: u8) {
        self.area.ftw &= !(1 << self.physical(i));
    }

    fn pop(&mut self) {
        self.free(0);
        self.set_top(self.top() + 1);
    }

    fn set_conditions(&mut self, set: u16) {
        self.area.fsw = self.area.fsw & !CONDITIONS | set;
    }

    fn set_c1(&mut self, up: bool) {
        self.area.fsw = self.area.fsw & !C1 | if up { C1 } else { 0 };
    }

    fn masked(&self, flags: u16) -> bool {
        flags & EXCEPTIONS & !self.area.fcw == 0
    }

    /// Raise `flags`; true when every one is masked, so the instruction goes on with its default response.
    /// An unmasked one clears C1 (seen on the reference), which a result may set again.
    fn raise(&mut self, flags: u16) -> bool {
        self.area.fsw |= flags;
        if self.masked(flags) {
            return true;
        }
        self.set_c1(false);
        self.area.fsw |= SUMMARY | BUSY;
        self.area.fop = self.code.opcode;
        self.area.fdp = self.data;
        false
    }

    /// A stack fault: invalid operation with the stack fault flag, C1 telling overflow from underflow.
    fn stack_fault(&mut self, overflow: bool) -> bool {
        let masked = self.raise(INVALID | STACK_FAULT);
        self.set_c1(overflow);
        masked
    }

    /// The physical register a push writes.
    fn below(&self) -> usize {
        (self.top() + REGISTER_MASK) & REGISTER_MASK
    }

    fn full_below(&self) -> bool {
        self.area.ftw & 1 << self.below() != 0
    }

    /// Push the indefinite after a masked stack fault, whatever is below.
    fn push_indefinite(&mut self) {
        self.set_top(self.below());
        self.set(0, Extended::INDEFINITE);
    }

    /// Push `value`, or the indefinite on a masked stack overflow.
    fn push(&mut self, value: Extended) {
        let below = self.below();
        if self.area.ftw & 1 << below != 0 {
            if !self.stack_fault(true) {
                return;
            }
            self.set_top(below);
            self.set(0, Extended::INDEFINITE);
            return;
        }
        self.set_top(below);
        self.set(0, value);
    }

    /// Pop `pops` times after an instruction that completed.
    fn pops(&mut self) {
        for _ in 0..self.code.pops {
            self.pop();
        }
    }

    /// Write `value` to ST(i) when it was delivered, and pop. An unmasked invalid operation, denormal operand
    /// or division by zero, met before the result, keeps the destination and the stack as they were.
    fn deliver(&mut self, i: u8, rounded: Rounded<Extended>) {
        let before = rounded.exceptions & BEFORE_RESULT;
        if !self.masked(before) {
            self.set_c1(false);
            self.raise(before);
            return;
        }
        if !self.raise(rounded.exceptions) && rounded.value.is_none() {
            return;
        }
        self.set_c1(rounded.up);
        if let Some(value) = rounded.value {
            self.set(i, value);
        }
        self.pops();
    }

    /// Run the code with the hook's arguments.
    pub fn run(&mut self, a: u64, b: u64) -> Results {
        let code = self.code;
        let i = code.register;
        match code.operation {
            Operation::Load => self.load(a, b),
            Operation::LoadConstant => {
                let (exponent, significand) = CONSTANTS[usize::from(i)];
                let value = constant(exponent, significand, rounding(self.area.fcw));
                self.set_c1(false);
                self.push(value);
            }
            Operation::Store => return self.store(),
            Operation::Arithmetic => self.arithmetic(a),
            Operation::Compare => return self.compare(a),
            Operation::Test => {
                let order = if self.empty(0) {
                    self.stack_fault(false);
                    Order::Unordered
                } else {
                    let (order, flags) = Self::order(self.get(0), Extended::ZERO, false, false);
                    self.set_c1(false);
                    self.raise(flags);
                    order
                };
                self.set_conditions(order.conditions());
            }
            Operation::Examine => {
                let value = self.get(0);
                let class = if self.empty(0) {
                    C3 | C0
                } else {
                    match value.class() {
                        Class::Unsupported => 0,
                        Class::Signaling | Class::Quiet => C0,
                        Class::Normal => C2,
                        Class::Infinity => C2 | C0,
                        Class::Zero => C3,
                        Class::Denormal => C3 | C2,
                    }
                };
                self.set_conditions(class | if value.sign { C1 } else { 0 });
            }
            Operation::ChangeSign | Operation::Absolute => {
                if self.empty(0) {
                    if self.stack_fault(false) {
                        self.set(0, Extended::INDEFINITE);
                    }
                    return [0; 2];
                }
                let value = self.get(0);
                let value = if code.operation == Operation::ChangeSign {
                    value.negate()
                } else {
                    value.with_sign(false)
                };
                self.set_c1(false);
                self.set(0, value);
            }
            Operation::Exchange => {
                let (a_empty, b_empty) = (self.empty(0), self.empty(i));
                let (mut x, mut y) = (self.get(0), self.get(i));
                if a_empty || b_empty {
                    if !self.stack_fault(false) {
                        return [0; 2];
                    }
                    if a_empty {
                        x = Extended::INDEFINITE;
                    }
                    if b_empty {
                        y = Extended::INDEFINITE;
                    }
                } else {
                    self.set_c1(false);
                }
                self.set(0, y);
                self.set(i, x);
            }
            Operation::ConditionalMove => {
                if self.empty(0) || self.empty(i) {
                    if self.stack_fault(false) {
                        self.set(0, Extended::INDEFINITE);
                    }
                    return [0; 2];
                }
                if a != 0 {
                    let value = self.get(i);
                    self.set(0, value);
                }
            }
            Operation::Free => {
                self.free(i);
                self.set_c1(false);
                self.pops();
            }
            Operation::Increment | Operation::Decrement => {
                let by = if code.operation == Operation::Increment {
                    1
                } else {
                    REGISTER_MASK
                };
                self.set_top(self.top() + by);
                self.set_c1(false);
            }
            Operation::Initialize => self.initialize(),
            Operation::ClearExceptions => self.area.fsw &= !CLEARED,
            Operation::LoadControl => {
                self.area.fcw = a as u16 & CONTROL_KEPT | CONTROL_ALWAYS;
                self.summarise();
            }
            Operation::RoundToInteger => self.unary(
                |_| false,
                |unit, x| {
                    let r = x.float().round_to_integral(rounding(unit.area.fcw));
                    let inexact = r.status.contains(Status::INEXACT);
                    Rounded {
                        value: Some(Extended::of(r.value)),
                        exceptions: if inexact { PRECISION } else { 0 },
                        up: r.value.abs() > x.float().abs(),
                    }
                },
            ),
            Operation::Scale => self.scale(),
            Operation::PartialRemainder => self.partial_remainder(),
            Operation::Extract => self.extract(),
            Operation::SquareRoot => self.unary(
                |x| x.sign && x.class() != Class::Zero,
                |unit, x| {
                    if x.class() != Class::Normal && x.class() != Class::Denormal {
                        return exact_value(x);
                    }
                    round_register(
                        crate::root::odd_root(x),
                        unit.area.fcw,
                        Precision::of(unit.area.fcw),
                    )
                },
            ),
            Operation::StoreEnvironment | Operation::Save => {
                let area = &*self.area;
                let words = u64::from(area.fcw)
                    | u64::from(area.fsw) << u16::BITS
                    | u64::from(self.tag_word()) << u32::BITS
                    | u64::from(area.fop) << FOP_SHIFT;
                let fdp = area.fdp;
                if code.operation == Operation::Save {
                    self.initialize();
                } else {
                    self.area.fcw |= EXCEPTIONS;
                    self.summarise();
                }
                return [words as i64, fdp as i64];
            }
            Operation::LoadEnvironment => {
                self.area.fcw = a as u16 & CONTROL_KEPT | CONTROL_ALWAYS;
                self.area.fsw = (a >> u16::BITS) as u16;
                let tags = (a >> u32::BITS) as u16;
                self.area.ftw = (0..REGISTERS)
                    .filter(|p| tags >> (TAG_BITS * *p as u32) & TAG_EMPTY != TAG_EMPTY)
                    .fold(0, |ftw, p| ftw | 1 << p);
                self.area.fop = (a >> FOP_SHIFT) as u16;
                self.area.fip = b;
                self.area.fdp = self.data;
                self.summarise();
            }
            Operation::Register => {
                let bits = self.get(i).to_bits();
                return [bits as u64 as i64, (bits >> u64::BITS) as u64 as i64];
            }
            Operation::SetRegister => {
                let p = self.physical(i);
                let bits = u128::from(a) | u128::from(b as u16) << u64::BITS;
                self.area.st[p] = bits.to_le_bytes();
            }
        }
        [0; 2]
    }

    /// The summary and busy bits as the flags and masks now say.
    fn summarise(&mut self) {
        if self.masked(self.area.fsw) {
            self.area.fsw &= !(SUMMARY | BUSY);
        } else {
            self.area.fsw |= SUMMARY | BUSY;
        }
    }

    fn initialize(&mut self) {
        let area = &mut *self.area;
        area.fcw = CONTROL_AT_START;
        area.fsw = 0;
        area.ftw = 0;
        area.fop = 0;
        area.fip = 0;
        area.fdp = 0;
    }

    /// The full tag word, two bits for each physical register.
    fn tag_word(&self) -> u16 {
        (0..REGISTERS).fold(0, |tags, p| {
            let i = ((p + REGISTERS - self.top()) & REGISTER_MASK) as u8;
            let tag = if self.empty(i) {
                TAG_EMPTY
            } else {
                self.get(i).tag()
            };
            tags | tag << (TAG_BITS * p as u32)
        })
    }

    fn load(&mut self, a: u64, b: u64) {
        let code = self.code;
        if code.format == Format::Register && self.empty(code.register) {
            if self.stack_fault(false) {
                self.push_indefinite();
            }
            return;
        }
        // A full stack is met before anything about the operand.
        if self.full_below() {
            self.push(Extended::INDEFINITE);
            return;
        }
        let value = match code.format {
            Format::Register => self.get(code.register),
            Format::Extended => Extended::from_bits(u128::from(a) | u128::from(b) << u64::BITS),
            Format::Single | Format::Double => {
                let narrow = Narrow::of(code.format, a);
                // An unmasked denormal operand is loaded all the same (seen on the reference).
                if narrow.value.class() == Class::Signaling {
                    if !self.raise(INVALID) {
                        return;
                    }
                } else if narrow.denormal {
                    self.raise(DENORMAL);
                }
                if narrow.value.is_nan() {
                    narrow.value.quieted()
                } else {
                    narrow.value
                }
            }
            Format::Decimal => decimal(u128::from(a) | u128::from(b) << u64::BITS),
            format => integer_value(format, a),
        };
        self.set_c1(false);
        self.push(value);
    }

    /// ST(0) to its destination. To memory, the value is given back with the status and tag words the store
    /// leaves, for the meaning to write once memory took the value (a fault there leaves the unit as it
    /// was); when an unmasked exception keeps it from memory, the unit is left as the exception leaves it and
    /// the second result says `NOT_STORED`.
    fn store(&mut self) -> Results {
        let code = self.code;
        let not_stored = [0, NOT_STORED as i64];
        if code.format == Format::Register {
            let value = self.get(0);
            if self.empty(0) {
                // `fstp` of the reserved encoding (iced's `fstpnce`) pops an empty ST(0) without a fault.
                if !code.quiet {
                    if !self.stack_fault(false) {
                        return not_stored;
                    }
                    self.set(code.register, Extended::INDEFINITE);
                }
            } else {
                self.set(code.register, value);
            }
            self.set_c1(false);
            self.pops();
            return not_stored;
        }
        let saved = (self.area.fsw, self.area.ftw);
        let stored = if self.empty(0) {
            if !self.stack_fault(false) {
                return not_stored;
            }
            indefinite(code.format)
        } else {
            let value = self.get(0);
            let rounded: Rounded<u128> = match code.format {
                Format::Extended => Rounded {
                    value: Some(value.to_bits()),
                    exceptions: 0,
                    up: false,
                },
                Format::Single => {
                    narrow::<rustc_apfloat::ieee::SingleS, SingleWideS>(value, self.area.fcw)
                }
                Format::Double => {
                    narrow::<rustc_apfloat::ieee::DoubleS, DoubleWideS>(value, self.area.fcw)
                }
                Format::Decimal => to_decimal(value, self.area.fcw),
                format => integer(value, format, self.area.fcw, code.truncate),
            };
            // An unmasked invalid operation, overflow or underflow keeps the value from memory, which is
            // then not looked at, and the stack stays.
            let kept = rounded.exceptions & (INVALID | OVERFLOW | UNDERFLOW);
            if !self.masked(kept) {
                self.set_c1(false);
                self.raise(kept);
                return not_stored;
            }
            self.raise(rounded.exceptions);
            self.set_c1(rounded.up);
            rounded.value.expect("a stored value")
        };
        self.pops();
        let after = (self.area.fsw, self.area.ftw);
        (self.area.fsw, self.area.ftw) = saved;
        let high = (stored >> u64::BITS) as u64 & u64::from(u16::MAX)
            | u64::from(after.0) << u16::BITS
            | u64::from(after.1) << u32::BITS;
        [stored as u64 as i64, high as i64]
    }

    /// Run `f` on ST(0) when it is a number; the special operands as Intel's manual says, then the operation's
    /// own invalid operands (`invalid`), then the denormal operand.
    fn unary(
        &mut self,
        invalid_operand: impl Fn(Extended) -> bool,
        f: impl Fn(&Self, Extended) -> Rounded<Extended>,
    ) {
        if self.empty(0) {
            if self.stack_fault(false) {
                self.set(0, Extended::INDEFINITE);
            }
            return;
        }
        let x = self.get(0);
        let rounded = match x.class() {
            Class::Unsupported => invalid(),
            Class::Signaling => Rounded {
                value: Some(x.quieted()),
                exceptions: INVALID,
                up: false,
            },
            Class::Quiet => exact_value(x),
            _ if invalid_operand(x) => invalid(),
            Class::Denormal => {
                if !self.raise(DENORMAL) {
                    return;
                }
                f(self, x)
            }
            _ => f(self, x),
        };
        self.deliver(0, rounded);
    }

    /// The order of two values for the compare instructions, and the exceptions it raises.
    fn order(a: Extended, b: Extended, quiet: bool, narrow_denormal: bool) -> (Order, u16) {
        let classes = [a.class(), b.class()];
        let signaling = classes
            .iter()
            .any(|c| matches!(c, Class::Signaling | Class::Unsupported));
        let nan = classes.contains(&Class::Quiet);
        if signaling || nan {
            let invalid = signaling || !quiet;
            return (Order::Unordered, if invalid { INVALID } else { 0 });
        }
        let denormal = narrow_denormal || classes.contains(&Class::Denormal);
        let order = match a.float().partial_cmp(&b.float()) {
            Some(core::cmp::Ordering::Greater) => Order::Greater,
            Some(core::cmp::Ordering::Less) => Order::Less,
            _ => Order::Equal,
        };
        (order, if denormal { DENORMAL } else { 0 })
    }

    /// A compare: its result is given even when an exception is unmasked, which keeps the stack from
    /// popping.
    fn compare(&mut self, a: u64) -> Results {
        let code = self.code;
        let other_empty = code.format == Format::Register && self.empty(code.register);
        let (order, masked) = if self.empty(0) || other_empty {
            (Order::Unordered, self.stack_fault(false))
        } else {
            let (other, narrow_denormal) = self.operand(a);
            let (order, flags) = Self::order(self.get(0), other, code.quiet, narrow_denormal);
            // `fcomi` and its kin leave C1 as it was.
            let c1 = self.area.fsw & C1 != 0;
            let masked = self.raise(flags);
            self.set_c1(c1 && code.flags);
            (order, masked)
        };
        let results = self.ordered(order);
        if masked {
            self.pops();
        }
        results
    }

    /// The other operand of an arithmetic or compare instruction: ST(`register`) or the memory value, and
    /// whether that was a denormal of a narrower format.
    fn operand(&self, a: u64) -> (Extended, bool) {
        let code = self.code;
        match code.format {
            Format::Register => {
                let i = if code.to_register { 0 } else { code.register };
                (self.get(i), false)
            }
            Format::Single | Format::Double => {
                let narrow = Narrow::of(code.format, a);
                (narrow.value, narrow.denormal)
            }
            format => (integer_value(format, a), false),
        }
    }

    /// Set the condition codes, or give rflags' bits for `fcomi`.
    fn ordered(&mut self, order: Order) -> Results {
        if self.code.flags {
            return [order.flags() as i64, 0];
        }
        self.set_conditions(order.conditions());
        [0; 2]
    }

    fn arithmetic(&mut self, a: u64) {
        let code = self.code;
        let memory = code.format != Format::Register;
        let destination = if code.to_register { code.register } else { 0 };
        let source_empty = !memory && self.empty(code.register);
        if self.empty(0) || source_empty {
            if self.stack_fault(false) {
                self.set(destination, Extended::INDEFINITE);
                self.pops();
            }
            return;
        }
        let (other, narrow_denormal) = self.operand(a);
        let this = self.get(destination);
        let (x, y) = match code.arithmetic {
            Arithmetic::SubtractReversed | Arithmetic::DivideReversed => (other, this),
            _ => (this, other),
        };
        let reversed = matches!(
            code.arithmetic,
            Arithmetic::SubtractReversed | Arithmetic::DivideReversed
        );
        let (narrow_x, narrow_y) = (reversed && narrow_denormal, !reversed && narrow_denormal);
        let rounded = binary(code.arithmetic, x, y, narrow_x, narrow_y, self.area.fcw);
        self.deliver(destination, rounded);
    }

    fn scale(&mut self) {
        if self.empty(0) || self.empty(1) {
            if self.stack_fault(false) {
                self.set(0, Extended::INDEFINITE);
            }
            return;
        }
        let (x, n) = (self.get(0), self.get(1));
        let fcw = self.area.fcw;
        let rounded = if let Some(r) = special_pair(x, n) {
            r
        } else {
            let (xc, nc) = (x.class(), n.class());
            let denormal = xc == Class::Denormal || nc == Class::Denormal;
            let flags = if denormal { DENORMAL } else { 0 };
            if !self.raise(flags) {
                return;
            }
            match (xc, nc) {
                (Class::Zero, Class::Infinity) if !n.sign => invalid(),
                (Class::Infinity, Class::Infinity) if n.sign => invalid(),
                (Class::Zero | Class::Infinity, _) => exact_value(x),
                (_, Class::Infinity) => exact_value(if n.sign {
                    Extended::ZERO.with_sign(x.sign)
                } else {
                    Extended::INFINITY.with_sign(x.sign)
                }),
                _ => {
                    let by = n.float().round_to_integral(Round::TowardZero).value;
                    let by = by
                        .to_i128(i32::BITS as usize)
                        .value
                        .clamp(-SCALE_LIMIT, SCALE_LIMIT) as i32;
                    round_register(
                        x.wide().scalbn_r(by, Round::TowardZero),
                        fcw,
                        Precision::Extended,
                    )
                }
            }
        };
        self.deliver(0, rounded);
    }

    /// `fprem` and `fprem1` (Intel SDM Vol. 2A): exact, by whole quotients when the exponents differ by less
    /// than 64, else by a quotient of `PARTIAL_BITS` plus the difference's low five bits (seen on the
    /// reference), with C2 set.
    fn partial_remainder(&mut self) {
        // Unless the result says otherwise, C2 is cleared and C0 and C3 are kept (seen on the reference).
        self.area.fsw &= !C2;
        if self.empty(0) || self.empty(1) {
            if self.stack_fault(false) {
                self.set(0, Extended::INDEFINITE);
            }
            return;
        }
        let (x, y) = (self.get(0), self.get(1));
        let (xc, yc) = (x.class(), y.class());
        let (rounded, conditions) = if let Some(r) = special_pair(x, y) {
            (r, None)
        } else if xc == Class::Infinity || yc == Class::Zero {
            (invalid(), None)
        } else {
            let denormal = xc == Class::Denormal || yc == Class::Denormal;
            if !self.raise(if denormal { DENORMAL } else { 0 }) {
                return;
            }
            // ST(0) itself, normalised (a pseudo-denormal is written as the normal it equals).
            if xc == Class::Zero || yc == Class::Infinity {
                let x = if xc == Class::Zero {
                    x
                } else {
                    Extended::of(x.float())
                };
                (exact_value(x), Some(0))
            } else {
                let (r, quotient, partial) = remainder(x, y, self.code.truncate, self.area.fcw);
                let bit = |n: u32| quotient >> n & 1 != 0;
                let conditions = if partial {
                    C2
                } else {
                    [(0, C1), (1, C3), (2, C0)]
                        .iter()
                        .filter(|(n, _)| bit(*n))
                        .fold(0, |c, (_, flag)| c | flag)
                };
                (r, Some(conditions))
            }
        };
        let kept = !self.masked(rounded.exceptions & BEFORE_RESULT);
        self.deliver(0, rounded);
        if let Some(conditions) = conditions.filter(|_| !kept) {
            self.set_conditions(conditions);
        }
    }

    fn extract(&mut self) {
        // A stack underflow, or else an overflow, leaves the indefinite in both.
        let (empty, full) = (self.empty(0), self.full_below());
        if empty || full {
            if self.stack_fault(!empty) {
                self.set(0, Extended::INDEFINITE);
                self.push_indefinite();
            }
            return;
        }
        let x = self.get(0);
        let (exponent, significand) = match x.class() {
            Class::Unsupported => (Extended::INDEFINITE, Extended::INDEFINITE),
            Class::Signaling | Class::Quiet => (x.quieted(), x.quieted()),
            Class::Infinity => (Extended::INFINITY, x),
            Class::Zero => (Extended::INFINITY.negate(), x),
            Class::Normal | Class::Denormal => {
                let f = x.float();
                let e = f.ilogb();
                let significand = Extended::of(f.scalbn(-e));
                (
                    Extended::of(X87DoubleExtended::from_i128(e.into()).value),
                    significand,
                )
            }
        };
        let flags = match x.class() {
            Class::Unsupported | Class::Signaling => INVALID,
            Class::Zero => DIVIDE_BY_ZERO,
            Class::Denormal => DENORMAL,
            _ => 0,
        };
        if !self.raise(flags) {
            return;
        }
        self.set_c1(false);
        self.set(0, exponent);
        self.push(significand);
    }
}

/// The quotient's bits a partial remainder takes at least: 32 plus the exponents' difference modulo 32.
const PARTIAL_BITS: i32 = 1 << PARTIAL_BITS_SHIFT;
const PARTIAL_BITS_SHIFT: u32 = 5;
/// Exponent differences from which a remainder is only partial.
const WHOLE_DIFFERENCES: i32 = u64::BITS as i32;

/// The exact remainder of finite non-zero `x` by `y` as a rounded result, the quotient's low bits, and
/// whether it is partial.
fn remainder(x: Extended, y: Extended, truncate: bool, fcw: u16) -> (Rounded<Extended>, u64, bool) {
    // Each operand normalised: its exponent, and its significand as a 64-bit integer.
    let normal = |v: Extended| {
        let f = v.float();
        let e = f.ilogb();
        let top = u64::BITS as i32 - 1;
        (e, f.abs().scalbn(top - e).to_u128(u64::BITS as usize).value)
    };
    let ((ex, a), (ey, b)) = (normal(x), normal(y));
    let difference = ex - ey;
    let partial = difference >= WHOLE_DIFFERENCES;
    let k = if partial {
        PARTIAL_BITS + (difference & (PARTIAL_BITS - 1))
    } else {
        difference
    };
    let bottom = ex - (u64::BITS as i32 - 1);
    let (mut quotient, mut rest, scale) = if k < 0 {
        (0, a, bottom)
    } else {
        let num = a << k;
        (num / b, num % b, bottom - k)
    };
    let mut sign = x.sign;
    // `fprem1` rounds the quotient to the nearest, ties to even.
    let shift = if k < 0 { -k } else { 0 };
    if !truncate && !partial && shift <= 1 {
        let doubled = rest << (1 - shift);
        if doubled > b || (doubled == b && quotient & 1 == 1) {
            quotient += 1;
            rest = (b << shift) - rest;
            sign = !sign;
        }
    }
    let value = X87DoubleExtended::from_u128(rest).value.scalbn(scale);
    let wide = exactly::<X87DoubleExtended, Wide>(value);
    let rounded = if rest == 0 {
        exact_value(Extended::ZERO.with_sign(x.sign))
    } else {
        round_register(if sign { -wide } else { wide }, fcw, Precision::Extended)
    };
    (rounded, quotient as u64, partial)
}

/// How far `fscale` moves an exponent at most: past it every result overflows or underflows alike.
const SCALE_LIMIT: i128 = 1 << (X87DoubleExtendedS::EXP_BITS + 1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Order {
    Greater,
    Less,
    Equal,
    Unordered,
}

impl Order {
    fn conditions(self) -> u16 {
        match self {
            Order::Greater => 0,
            Order::Less => C0,
            Order::Equal => C3,
            Order::Unordered => C3 | C2 | C0,
        }
    }

    fn flags(self) -> u64 {
        match self {
            Order::Greater => GREATER,
            Order::Less => LESS,
            Order::Equal => EQUAL,
            Order::Unordered => UNORDERED,
        }
    }
}

fn invalid() -> Rounded<Extended> {
    Rounded {
        value: Some(Extended::INDEFINITE),
        exceptions: INVALID,
        up: false,
    }
}

fn exact_value(value: Extended) -> Rounded<Extended> {
    Rounded {
        value: Some(value),
        exceptions: 0,
        up: false,
    }
}

/// The NaN two operands give (Intel SDM Vol. 1 table 4-7): a signaling one raises invalid; of two the quiet
/// one, else the one with the larger significand.
fn nan_of(a: Extended, b: Extended) -> Extended {
    match (a.is_nan(), b.is_nan()) {
        (true, false) => a.quieted(),
        (false, true) => b.quieted(),
        _ => {
            let (sa, sb) = (a.class() == Class::Signaling, b.class() == Class::Signaling);
            let pick = if sa != sb {
                if sa { b } else { a }
            } else if a.significand | QUIET_BIT != b.significand | QUIET_BIT {
                if a.significand | QUIET_BIT > b.significand | QUIET_BIT {
                    a
                } else {
                    b
                }
            } else if !a.sign {
                a
            } else {
                b
            };
            pick.quieted()
        }
    }
}

/// The result of two operands when either is not a number, or none.
fn special_pair(a: Extended, b: Extended) -> Option<Rounded<Extended>> {
    let classes = [a.class(), b.class()];
    if classes.contains(&Class::Unsupported) {
        return Some(invalid());
    }
    if a.is_nan() || b.is_nan() {
        let signaling = classes.contains(&Class::Signaling);
        return Some(Rounded {
            value: Some(nan_of(a, b)),
            exceptions: if signaling { INVALID } else { 0 },
            up: false,
        });
    }
    None
}

/// `x op y` with Intel's special cases and the precision control; `narrow_x` and `narrow_y` say an operand
/// came from a denormal of a narrower format (normal in 80 bits, but a denormal operand all the same).
fn binary(
    op: Arithmetic,
    x: Extended,
    y: Extended,
    narrow_x: bool,
    narrow_y: bool,
    fcw: u16,
) -> Rounded<Extended> {
    if let Some(r) = special_pair(x, y) {
        return r;
    }
    let (xc, yc) = (x.class(), y.class());
    let infinite = |c: Class| c == Class::Infinity;
    let zero = |c: Class| c == Class::Zero;
    let sign = x.sign != y.sign;
    let invalid_case = match op {
        Arithmetic::Add => infinite(xc) && infinite(yc) && x.sign != y.sign,
        Arithmetic::Subtract | Arithmetic::SubtractReversed => {
            infinite(xc) && infinite(yc) && x.sign == y.sign
        }
        Arithmetic::Multiply => (zero(xc) && infinite(yc)) || (infinite(xc) && zero(yc)),
        Arithmetic::Divide | Arithmetic::DivideReversed => {
            (zero(xc) && zero(yc)) || (infinite(xc) && infinite(yc))
        }
    };
    if invalid_case {
        return invalid();
    }
    let dividing = matches!(op, Arithmetic::Divide | Arithmetic::DivideReversed);
    if dividing && zero(yc) && !infinite(xc) {
        return Rounded {
            value: Some(Extended::INFINITY.with_sign(sign)),
            exceptions: DIVIDE_BY_ZERO,
            up: false,
        };
    }
    let denormal = xc == Class::Denormal || yc == Class::Denormal || narrow_x || narrow_y;
    let denormal = if denormal { DENORMAL } else { 0 };
    let with = |r: Rounded<Extended>| Rounded {
        exceptions: r.exceptions | denormal,
        ..r
    };
    let subtract = matches!(op, Arithmetic::Subtract | Arithmetic::SubtractReversed);
    let y_added = if subtract { y.negate() } else { y };
    match op {
        Arithmetic::Add | Arithmetic::Subtract | Arithmetic::SubtractReversed => {
            if infinite(xc) {
                return with(exact_value(x));
            }
            if infinite(yc) {
                return with(exact_value(y_added));
            }
        }
        Arithmetic::Multiply => {
            if infinite(xc) || infinite(yc) {
                return with(exact_value(Extended::INFINITY.with_sign(sign)));
            }
        }
        _ => {
            if infinite(xc) {
                return with(exact_value(Extended::INFINITY.with_sign(sign)));
            }
            if infinite(yc) {
                return with(exact_value(Extended::ZERO.with_sign(sign)));
            }
        }
    }
    let round = rounding(fcw);
    let (wx, wy) = (x.wide(), y_added.wide());
    let exact = match op {
        Arithmetic::Add | Arithmetic::Subtract | Arithmetic::SubtractReversed => {
            wx.add_r(wy, Round::TowardZero)
        }
        Arithmetic::Multiply => wx.mul_r(wy, Round::TowardZero),
        _ => wx.div_r(wy, Round::TowardZero),
    };
    if exact.value.is_zero() {
        // A sum of opposite values is +0, or -0 rounding down; of two zeros of one sign, that sign.
        let sign = match op {
            Arithmetic::Multiply | Arithmetic::Divide | Arithmetic::DivideReversed => sign,
            _ if zero(xc) && zero(yc) && x.sign == y_added.sign => x.sign,
            _ => round == Round::TowardNegative,
        };
        return with(exact_value(Extended::ZERO.with_sign(sign)));
    }
    with(round_register(odd(exact), fcw, Precision::of(fcw)))
}

/// A constant rounded to 64 bits by the rounding control (every constant is positive).
fn constant(exponent: u16, significand: u128, round: Round) -> Extended {
    if significand == 0 {
        return Extended::ZERO;
    }
    let kept = (significand >> u64::BITS) as u64;
    let rest = significand as u64;
    let half = 1 << (u64::BITS - 1);
    let up = match round {
        Round::NearestTiesToEven | Round::NearestTiesToAway => {
            rest > half || (rest == half && kept & 1 != 0)
        }
        Round::TowardPositive => rest != 0,
        Round::TowardNegative | Round::TowardZero => false,
    };
    Extended {
        sign: false,
        exponent,
        significand: kept + u64::from(up),
    }
}

/// An 80-bit value rounded into a narrower IEEE format `S` for memory (`U` its precision with a wide exponent).
fn narrow<S: Semantics, U: Semantics>(value: Extended, fcw: u16) -> Rounded<u128>
where
    Wide: FloatConvert<IeeeFloat<S>> + FloatConvert<IeeeFloat<U>>,
    IeeeFloat<U>: FloatConvert<IeeeFloat<S>> + FloatConvert<Wide>,
    IeeeFloat<S>: FloatConvert<Wide>,
{
    let bits = |f: IeeeFloat<S>| f.to_bits();
    match value.class() {
        Class::Unsupported => Rounded {
            value: Some(bits(-IeeeFloat::<S>::qnan(None))),
            exceptions: INVALID,
            up: false,
        },
        Class::Signaling | Class::Quiet => {
            // The payload's top bits, quieted.
            let quiet: IeeeFloat<S> = value
                .quieted()
                .float()
                .convert_r(Round::TowardZero, &mut false)
                .value;
            Rounded {
                value: Some(bits(quiet)),
                exceptions: if value.class() == Class::Signaling {
                    INVALID
                } else {
                    0
                },
                up: false,
            }
        }
        Class::Infinity | Class::Zero => Rounded {
            value: Some(bits(exactly(value.float()))),
            exceptions: 0,
            up: false,
        },
        // A store raises no denormal exception (seen on the reference).
        Class::Normal | Class::Denormal => {
            let r = round_into::<S, U>(value.wide(), fcw, false);
            Rounded {
                value: r.value.map(bits),
                exceptions: r.exceptions,
                up: r.up,
            }
        }
    }
}

/// A single or double from memory in 80 bits, a signaling one kept signaling, and whether it was a denormal
/// there.
struct Narrow {
    value: Extended,
    denormal: bool,
}

impl Narrow {
    fn of(format: Format, a: u64) -> Self {
        let (bits, fraction_bits) = if format == Format::Single {
            (u32::BITS, f32::MANTISSA_DIGITS - 1)
        } else {
            (u64::BITS, f64::MANTISSA_DIGITS - 1)
        };
        let fraction = a & ((1 << fraction_bits) - 1);
        let exponent = a >> fraction_bits & ((1 << (bits - 1 - fraction_bits)) - 1);
        let all = (1 << (bits - 1 - fraction_bits)) - 1;
        let sign = a >> (bits - 1) & 1 != 0;
        if exponent == all && fraction != 0 {
            let shift = u64::BITS - 1 - fraction_bits;
            return Narrow {
                value: Extended {
                    sign,
                    exponent: EXPONENT_ALL,
                    significand: INTEGER_BIT | fraction << shift,
                },
                denormal: false,
            };
        }
        let (value, denormal) = if format == Format::Single {
            let n = Single::from_bits(a.into());
            (exactly(n), n.is_denormal())
        } else {
            let n = Double::from_bits(a.into());
            (exactly(n), n.is_denormal())
        };
        Narrow {
            value: Extended::of(value),
            denormal,
        }
    }
}

/// An integer of `format` from the low bits of `a`, sign-extended, in 80 bits (always exact).
fn integer_value(format: Format, a: u64) -> Extended {
    let bits = format.integer_bits().expect("an integer format");
    let value = (a << (u64::BITS - bits)) as i64 >> (u64::BITS - bits);
    Extended::of(X87DoubleExtended::from_i128(value.into()).value)
}

/// The indefinite of a format, as stored on a masked invalid operation.
fn indefinite(format: Format) -> u128 {
    match format {
        Format::Register | Format::Extended => Extended::INDEFINITE.to_bits(),
        Format::Single => (-Single::qnan(None)).to_bits(),
        Format::Double => (-Double::qnan(None)).to_bits(),
        Format::Word => 1 << (u16::BITS - 1),
        Format::Integer => 1 << (u32::BITS - 1),
        Format::Long => 1 << (u64::BITS - 1),
        Format::Decimal => DECIMAL_INDEFINITE,
    }
}

/// An integer store: rounded by the rounding control or truncated; out of range or not a number is invalid.
fn integer(value: Extended, format: Format, fcw: u16, truncate: bool) -> Rounded<u128> {
    let bits = format.integer_bits().unwrap();
    let invalid = Rounded {
        value: Some(indefinite(format)),
        exceptions: INVALID,
        up: false,
    };
    match value.class() {
        Class::Normal | Class::Denormal | Class::Zero => {}
        _ => return invalid,
    }
    let round = if truncate {
        Round::TowardZero
    } else {
        rounding(fcw)
    };
    let f = value.float();
    let r = f.round_to_integral(round);
    let limit = 1i128 << (bits - 1);
    let n = r.value.to_i128(u128::BITS as usize).value;
    if n < -limit || n >= limit {
        return invalid;
    }
    let inexact = r.status.contains(Status::INEXACT);
    Rounded {
        value: Some(n as u128 & ((1 << bits) - 1)),
        exceptions: if inexact { PRECISION } else { 0 },
        up: r.value.abs() > f.abs(),
    }
}

/// A packed decimal's value; digits past 9 count at their place as they are.
fn decimal(bits: u128) -> Extended {
    let magnitude = (0..DECIMAL_DIGITS).rev().fold(0u128, |n, k| {
        n * DECIMAL_RADIX + (bits >> (k * DIGIT_BITS) & ((1 << DIGIT_BITS) - 1))
    });
    let value = Extended::of(X87DoubleExtended::from_u128(magnitude).value);
    value.with_sign(bits & DECIMAL_SIGN != 0)
}

fn to_decimal(value: Extended, fcw: u16) -> Rounded<u128> {
    let invalid = Rounded {
        value: Some(DECIMAL_INDEFINITE),
        exceptions: INVALID,
        up: false,
    };
    match value.class() {
        Class::Normal | Class::Denormal | Class::Zero => {}
        _ => return invalid,
    }
    let f = value.float();
    let r = f.round_to_integral(rounding(fcw));
    let n = r.value.abs().to_u128(u128::BITS as usize).value;
    if n >= DECIMAL_RADIX.pow(DECIMAL_DIGITS) {
        return invalid;
    }
    let mut packed = 0u128;
    let mut rest = n;
    for k in 0..DECIMAL_DIGITS {
        packed |= (rest % DECIMAL_RADIX) << (k * DIGIT_BITS);
        rest /= DECIMAL_RADIX;
    }
    if value.sign {
        packed |= DECIMAL_SIGN;
    }
    let inexact = r.status.contains(Status::INEXACT);
    Rounded {
        value: Some(packed),
        exceptions: if inexact { PRECISION } else { 0 },
        up: r.value.abs() > f.abs(),
    }
}
