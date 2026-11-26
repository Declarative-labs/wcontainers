//! The Advanced SIMD instructions `simd.rs` does not take: each family reads its fields from the word and loops over
//! the elements as Arm's pseudocode for its encoding class does (the classes of the
//! [SIMD and floating-point instructions](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions)).

use core::convert::Infallible;

use crate::encoding::{Field, RD, RM, RN};
use crate::floating_point::lanes::{LaneComparison, LaneOperation, compute_lane};
use crate::floating_point::{FloatingPointOperation, Precision, Rounding};
use crate::operand::RegisterThirtyOne;
use crate::registers::{read_register, write_register};
use crate::{Exception, Instruction, Processor};

const Q: Field = Field::new(30, 1);
const SIZE: Field = Field::new(22, 2);
const SZ: Field = Field::new(22, 1);
/// Bit 28 sets a scalar encoding class apart from its vector one.
const SCALAR: Field = Field::new(28, 1);
const IMMH: Field = Field::new(19, 4);
const IMMH_IMMB: Field = Field::new(16, 7);
const IMM5: Field = Field::new(16, 5);
const IMM4: Field = Field::new(11, 4);
const ELEMENT_H: Field = Field::new(11, 1);
const ELEMENT_L: Field = Field::new(21, 1);
const ELEMENT_M: Field = Field::new(20, 1);
const ELEMENT_RM: Field = Field::new(16, 4);
const CMODE: Field = Field::new(12, 4);
const IMMEDIATE_OP: Field = Field::new(29, 1);
const O2: Field = Field::new(11, 1);
const ABC: Field = Field::new(16, 3);
const DEFGH: Field = Field::new(5, 5);
const FCMLA_ROTATION: Field = Field::new(11, 2);
const FCMLA_ELEMENT_ROTATION: Field = Field::new(13, 2);
const FCADD_ROTATION: Field = Field::new(12, 1);
const VECTOR_BITS: u32 = u128::BITS;
const HALF_VECTOR_BITS: u32 = u64::BITS;
/// FPSR.QC, the cumulative saturation flag.
const SATURATION: u64 = 1 << 27;
/// The bits of a byte of a dot product's group, and how many bytes one group holds.
const DOT_GROUP: u32 = 4;

/// Whether an integer element is read as signed or unsigned.
type Unsigned = bool;

/// The bitwise selects of three registers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Select {
    /// BSL: the first source where the destination has a one.
    ByDestination,
    /// BIT: the first source where the second has a one.
    WhereSecond,
    /// BIF: the first source where the second has a zero.
    WhereNotSecond,
}

/// Three registers of one arrangement, or their scalar form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SameOperation {
    SaturatingAdd(Unsigned),
    SaturatingSubtract(Unsigned),
    /// SQSHL, UQSHL, SQRSHL and UQRSHL by the signed low byte of the second element.
    SaturatingShift {
        unsigned: Unsigned,
        rounding: bool,
    },
    AbsoluteDifferenceAccumulate(Unsigned),
    MultiplyAccumulate {
        subtract: bool,
    },
    /// SQDMULH and SQRDMULH.
    DoublingMultiplyHigh {
        rounding: bool,
    },
    /// SQRDMLAH and SQRDMLSH.
    RoundingDoublingMultiplyAccumulate {
        subtract: bool,
    },
    BitwiseSelect(Select),
}

/// Three registers whose elements differ in size: long, wide and narrow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DifferentOperation {
    AddLong(Unsigned),
    SubtractLong(Unsigned),
    AddWide(Unsigned),
    SubtractWide(Unsigned),
    AddHighNarrow {
        rounding: bool,
    },
    SubtractHighNarrow {
        rounding: bool,
    },
    AbsoluteDifferenceLong {
        unsigned: Unsigned,
        accumulate: bool,
    },
    MultiplyLong {
        unsigned: Unsigned,
        accumulate: Accumulate,
    },
    SaturatingDoublingMultiplyLong(Accumulate),
    PolynomialMultiplyLong,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Accumulate {
    None,
    Add,
    Subtract,
}

/// Two registers of one arrangement, or their scalar form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MiscOperation {
    /// REV16, REV32 and REV64: the elements reversed inside containers of these bits.
    Reverse(u32),
    AddPairwiseLong {
        unsigned: Unsigned,
        accumulate: bool,
    },
    /// SUQADD (false) and USQADD (true): the source as the other signedness, added to the destination.
    SaturatingAccumulate(Unsigned),
    CountLeadingSignBits,
    CountLeadingZeros,
    CountBits,
    Not,
    ReverseBits,
    SaturatingAbsolute,
    SaturatingNegate,
    /// XTN, SQXTN, SQXTUN and UQXTN: from a signed or unsigned source, saturated to a signed or unsigned result.
    ExtractNarrow(Narrowing),
    ShiftLeftLongByElementSize,
    UnsignedReciprocalEstimate,
    UnsignedReciprocalSquareRootEstimate,
}

/// How a narrowing reads its source and saturates its result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Narrowing {
    Truncate,
    Signed,
    SignedToUnsigned,
    Unsigned,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AcrossOperation {
    AddLong(Unsigned),
    Maximum(Unsigned),
    Minimum(Unsigned),
    Add,
}

/// A register, the second operand an element of another register.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ElementOperation {
    Multiply(Accumulate),
    MultiplyLong {
        unsigned: Unsigned,
        accumulate: Accumulate,
    },
    SaturatingDoublingMultiplyLong(Accumulate),
    DoublingMultiplyHigh {
        rounding: bool,
    },
    RoundingDoublingMultiplyAccumulate {
        subtract: bool,
    },
    /// SDOT, UDOT, SUDOT and USDOT: whether the first and the second source are unsigned.
    Dot {
        first: Unsigned,
        second: Unsigned,
    },
}

/// A shift by an immediate: esize from the highest set bit of immh.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShiftOperation {
    ShiftRight {
        unsigned: Unsigned,
        rounding: bool,
        accumulate: bool,
    },
    ShiftLeft,
    /// SQSHL (signed to signed), SQSHLU (signed to unsigned) and UQSHL (unsigned to unsigned).
    SaturatingShiftLeft {
        source: Unsigned,
        result: Unsigned,
    },
    ShiftRightNarrow {
        rounding: bool,
        narrowing: Narrowing,
    },
    ShiftLeftLong(Unsigned),
    /// SRI (false) and SLI (true): the shifted element inserted over the destination's.
    Insert {
        left: bool,
    },
    /// FCVTZS and FCVTZU to fixed point.
    ToFixed(Unsigned),
    /// SCVTF and UCVTF from fixed point.
    FromFixed(Unsigned),
}

/// How a floating-point instruction takes its lanes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FloatingPointShape {
    /// Two sources of one arrangement, or their scalar form.
    Same,
    /// The destination as the third source, the addend.
    Accumulating,
    /// One source.
    Misc,
    /// Neighbouring pairs of the second source above the first.
    Pairwise,
    /// The two lanes of one source, to a scalar.
    ScalarPairwise,
    /// Every lane of one source, to a scalar, as a tree of halves.
    Across,
    /// The second source one element of a register, the destination the addend where it accumulates.
    Element { accumulating: bool },
}

/// The precision a floating-point encoding names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrecisionField {
    /// sz at bit 22: single or double.
    Size,
    Half,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AdvancedSimdOperation {
    Same(SameOperation),
    Different(DifferentOperation),
    Misc(MiscOperation),
    Across(AcrossOperation),
    /// ADDP (scalar): the two doublewords added.
    ScalarAddPairwise,
    Element(ElementOperation),
    Shift(ShiftOperation),
    /// TBL and TBX of one to four table registers.
    Table {
        registers: u32,
        extension: bool,
    },
    Extract,
    DuplicateElement,
    DuplicateGeneral,
    /// DUP (element) to a scalar, the alias MOV.
    DuplicateToScalar,
    /// SMOV (true) and UMOV (false).
    MoveToGeneral {
        signed: bool,
        doubleword: bool,
    },
    InsertGeneral,
    InsertElement,
    /// MOVI, MVNI, ORR, BIC and FMOV of an immediate, as op and cmode choose.
    ModifiedImmediate,
    Dot {
        first: Unsigned,
        second: Unsigned,
    },
    MatrixMultiply {
        first: Unsigned,
        second: Unsigned,
    },
    FloatingPoint {
        shape: FloatingPointShape,
        precision: PrecisionField,
        operation: LaneOperation,
    },
    /// FCVTL: to twice the precision of the lower or upper half.
    ConvertLonger,
    /// FCVTN (false) and FCVTXN (true): to half the precision, into the lower or upper half.
    ConvertNarrower {
        to_odd: bool,
    },
    /// FCMLA by a rotation.
    ComplexMultiplyAccumulate {
        element: bool,
    },
    /// FCADD by a rotation.
    ComplexAdd,
    /// BFCVTN: singles to BFloat16, into the lower or upper half.
    ConvertToBrainFloat,
    /// BFDOT: each pair of BFloat16 products added to a single.
    BrainFloatDot {
        element: bool,
    },
    /// BFMLALB and BFMLALT: the even or odd BFloat16 lanes widened, their product added to a single.
    BrainFloatMultiplyAccumulate {
        element: bool,
    },
    /// BFMMLA: a 2x4 by 4x2 BFloat16 matrix product added to a 2x2 matrix of singles.
    BrainFloatMatrixMultiply,
    /// FMLAL, FMLSL, FMLAL2 and FMLSL2: half-precision products added to singles.
    WideningMultiplyAccumulate {
        subtract: bool,
        upper: bool,
        element: bool,
    },
}

/// The operation of each modeled instruction of this module, none for every other instruction.
pub const fn to_advanced_simd_operation(
    instruction: Instruction<Infallible>,
) -> Option<AdvancedSimdOperation> {
    use AdvancedSimdOperation as A;
    use FloatingPointShape as S;
    use Instruction as I;
    use LaneComparison as C;
    use LaneOperation as L;
    use PrecisionField::{Half, Size};
    use Rounding::{TiesAway, TiesToEven, TowardMinusInfinity, TowardPlusInfinity, TowardZero};
    const fn fp(
        shape: FloatingPointShape,
        precision: PrecisionField,
        operation: LaneOperation,
    ) -> AdvancedSimdOperation {
        AdvancedSimdOperation::FloatingPoint {
            shape,
            precision,
            operation,
        }
    }
    const fn scalar(operation: FloatingPointOperation) -> LaneOperation {
        LaneOperation::Scalar(operation)
    }
    const fn to_integer(rounding: Option<Rounding>, unsigned: bool) -> LaneOperation {
        LaneOperation::ToInteger {
            rounding,
            unsigned,
            fraction_bits: 0,
        }
    }
    const fn from_integer(unsigned: bool) -> LaneOperation {
        LaneOperation::FromInteger {
            unsigned,
            fraction_bits: 0,
        }
    }
    const fn round_to(rounding: Option<Rounding>, exact: bool) -> LaneOperation {
        LaneOperation::Scalar(FloatingPointOperation::RoundToIntegral { rounding, exact })
    }
    const fn within(bits: u32, rounding: Option<Rounding>) -> LaneOperation {
        LaneOperation::Scalar(FloatingPointOperation::RoundToIntegralWithin { bits, rounding })
    }
    const FMA: FloatingPointOperation = FloatingPointOperation::MultiplyAdd {
        negate_product: false,
        negate_addend: false,
    };
    const FMS: FloatingPointOperation = FloatingPointOperation::MultiplyAdd {
        negate_product: true,
        negate_addend: false,
    };
    use FloatingPointOperation as F;
    Some(match instruction {
        // Integer, three registers of one arrangement.
        I::SqaddVector | I::SqaddScalar => A::Same(SameOperation::SaturatingAdd(false)),
        I::UqaddVector | I::UqaddScalar => A::Same(SameOperation::SaturatingAdd(true)),
        I::SqsubVector | I::SqsubScalar => A::Same(SameOperation::SaturatingSubtract(false)),
        I::UqsubVector | I::UqsubScalar => A::Same(SameOperation::SaturatingSubtract(true)),
        I::SqshlVector | I::SqshlScalarRegister => A::Same(SameOperation::SaturatingShift {
            unsigned: false,
            rounding: false,
        }),
        I::UqshlVector | I::UqshlScalarRegister => A::Same(SameOperation::SaturatingShift {
            unsigned: true,
            rounding: false,
        }),
        I::SqrshlVector | I::SqrshlScalar => A::Same(SameOperation::SaturatingShift {
            unsigned: false,
            rounding: true,
        }),
        I::UqrshlVector | I::UqrshlScalar => A::Same(SameOperation::SaturatingShift {
            unsigned: true,
            rounding: true,
        }),
        I::SabaVector => A::Same(SameOperation::AbsoluteDifferenceAccumulate(false)),
        I::UabaVector => A::Same(SameOperation::AbsoluteDifferenceAccumulate(true)),
        I::MlaVector => A::Same(SameOperation::MultiplyAccumulate { subtract: false }),
        I::MlsVector => A::Same(SameOperation::MultiplyAccumulate { subtract: true }),
        I::SqdmulhVector | I::SqdmulhScalar => {
            A::Same(SameOperation::DoublingMultiplyHigh { rounding: false })
        }
        I::SqrdmulhVector | I::SqrdmulhScalar => {
            A::Same(SameOperation::DoublingMultiplyHigh { rounding: true })
        }
        I::SqrdmlahVector | I::SqrdmlahScalar => {
            A::Same(SameOperation::RoundingDoublingMultiplyAccumulate { subtract: false })
        }
        I::SqrdmlshVector | I::SqrdmlshScalar => {
            A::Same(SameOperation::RoundingDoublingMultiplyAccumulate { subtract: true })
        }
        I::BslVector => A::Same(SameOperation::BitwiseSelect(Select::ByDestination)),
        I::Bit => A::Same(SameOperation::BitwiseSelect(Select::WhereSecond)),
        I::Bif => A::Same(SameOperation::BitwiseSelect(Select::WhereNotSecond)),
        // Integer, three registers of different arrangements.
        I::Saddl => A::Different(DifferentOperation::AddLong(false)),
        I::Uaddl => A::Different(DifferentOperation::AddLong(true)),
        I::Ssubl => A::Different(DifferentOperation::SubtractLong(false)),
        I::Usubl => A::Different(DifferentOperation::SubtractLong(true)),
        I::Saddw => A::Different(DifferentOperation::AddWide(false)),
        I::Uaddw => A::Different(DifferentOperation::AddWide(true)),
        I::Ssubw => A::Different(DifferentOperation::SubtractWide(false)),
        I::Usubw => A::Different(DifferentOperation::SubtractWide(true)),
        I::Addhn => A::Different(DifferentOperation::AddHighNarrow { rounding: false }),
        I::Raddhn => A::Different(DifferentOperation::AddHighNarrow { rounding: true }),
        I::Subhn => A::Different(DifferentOperation::SubtractHighNarrow { rounding: false }),
        I::Rsubhn => A::Different(DifferentOperation::SubtractHighNarrow { rounding: true }),
        I::Sabdl => A::Different(DifferentOperation::AbsoluteDifferenceLong {
            unsigned: false,
            accumulate: false,
        }),
        I::Uabdl => A::Different(DifferentOperation::AbsoluteDifferenceLong {
            unsigned: true,
            accumulate: false,
        }),
        I::SabalVector => A::Different(DifferentOperation::AbsoluteDifferenceLong {
            unsigned: false,
            accumulate: true,
        }),
        I::UabalVector => A::Different(DifferentOperation::AbsoluteDifferenceLong {
            unsigned: true,
            accumulate: true,
        }),
        I::SmullVector => A::Different(DifferentOperation::MultiplyLong {
            unsigned: false,
            accumulate: Accumulate::None,
        }),
        I::UmullVector => A::Different(DifferentOperation::MultiplyLong {
            unsigned: true,
            accumulate: Accumulate::None,
        }),
        I::SmlalVector => A::Different(DifferentOperation::MultiplyLong {
            unsigned: false,
            accumulate: Accumulate::Add,
        }),
        I::UmlalVector => A::Different(DifferentOperation::MultiplyLong {
            unsigned: true,
            accumulate: Accumulate::Add,
        }),
        I::SmlslVector => A::Different(DifferentOperation::MultiplyLong {
            unsigned: false,
            accumulate: Accumulate::Subtract,
        }),
        I::UmlslVector => A::Different(DifferentOperation::MultiplyLong {
            unsigned: true,
            accumulate: Accumulate::Subtract,
        }),
        I::SqdmullVector | I::SqdmullScalar => A::Different(
            DifferentOperation::SaturatingDoublingMultiplyLong(Accumulate::None),
        ),
        I::SqdmlalVector | I::SqdmlalScalar => A::Different(
            DifferentOperation::SaturatingDoublingMultiplyLong(Accumulate::Add),
        ),
        I::SqdmlslVector | I::SqdmlslScalar => A::Different(
            DifferentOperation::SaturatingDoublingMultiplyLong(Accumulate::Subtract),
        ),
        I::PmullAsimddiffL => A::Different(DifferentOperation::PolynomialMultiplyLong),
        // Integer, two registers.
        I::Rev64VectorRegister => A::Misc(MiscOperation::Reverse(u64::BITS)),
        I::Rev32Vector => A::Misc(MiscOperation::Reverse(u32::BITS)),
        I::Rev16Vector => A::Misc(MiscOperation::Reverse(u16::BITS)),
        I::Saddlp => A::Misc(MiscOperation::AddPairwiseLong {
            unsigned: false,
            accumulate: false,
        }),
        I::Uaddlp => A::Misc(MiscOperation::AddPairwiseLong {
            unsigned: true,
            accumulate: false,
        }),
        I::SadalpVector => A::Misc(MiscOperation::AddPairwiseLong {
            unsigned: false,
            accumulate: true,
        }),
        I::UadalpVector => A::Misc(MiscOperation::AddPairwiseLong {
            unsigned: true,
            accumulate: true,
        }),
        I::SuqaddVector | I::SuqaddScalar => A::Misc(MiscOperation::SaturatingAccumulate(false)),
        I::UsqaddVector | I::UsqaddScalar => A::Misc(MiscOperation::SaturatingAccumulate(true)),
        I::ClsVector => A::Misc(MiscOperation::CountLeadingSignBits),
        I::ClzVector => A::Misc(MiscOperation::CountLeadingZeros),
        I::CntVector => A::Misc(MiscOperation::CountBits),
        I::NotVector => A::Misc(MiscOperation::Not),
        I::RbitVector => A::Misc(MiscOperation::ReverseBits),
        I::SqabsVector | I::SqabsScalar => A::Misc(MiscOperation::SaturatingAbsolute),
        I::SqnegVector | I::SqnegScalar => A::Misc(MiscOperation::SaturatingNegate),
        I::Xtn => A::Misc(MiscOperation::ExtractNarrow(Narrowing::Truncate)),
        I::SqxtnVector | I::SqxtnScalar => A::Misc(MiscOperation::ExtractNarrow(Narrowing::Signed)),
        I::SqxtunVector | I::SqxtunScalar => {
            A::Misc(MiscOperation::ExtractNarrow(Narrowing::SignedToUnsigned))
        }
        I::UqxtnVector | I::UqxtnScalar => {
            A::Misc(MiscOperation::ExtractNarrow(Narrowing::Unsigned))
        }
        I::ShllAsimdmiscS => A::Misc(MiscOperation::ShiftLeftLongByElementSize),
        I::UrecpeVector => A::Misc(MiscOperation::UnsignedReciprocalEstimate),
        I::UrsqrteVector => A::Misc(MiscOperation::UnsignedReciprocalSquareRootEstimate),
        // Integer, across lanes and pairwise to a scalar.
        I::Saddlv => A::Across(AcrossOperation::AddLong(false)),
        I::Uaddlv => A::Across(AcrossOperation::AddLong(true)),
        I::SmaxvScalarV => A::Across(AcrossOperation::Maximum(false)),
        I::UmaxvScalarV => A::Across(AcrossOperation::Maximum(true)),
        I::SminvScalarV => A::Across(AcrossOperation::Minimum(false)),
        I::UminvScalarV => A::Across(AcrossOperation::Minimum(true)),
        I::Addv => A::Across(AcrossOperation::Add),
        I::AddpScalarP => A::ScalarAddPairwise,
        // Integer, by element.
        I::MulAsimdelemR => A::Element(ElementOperation::Multiply(Accumulate::None)),
        I::MlaAsimdelemR => A::Element(ElementOperation::Multiply(Accumulate::Add)),
        I::MlsAsimdelemR => A::Element(ElementOperation::Multiply(Accumulate::Subtract)),
        I::SmullAsimdelemL => A::Element(ElementOperation::MultiplyLong {
            unsigned: false,
            accumulate: Accumulate::None,
        }),
        I::UmullAsimdelemL => A::Element(ElementOperation::MultiplyLong {
            unsigned: true,
            accumulate: Accumulate::None,
        }),
        I::SmlalAsimdelemL => A::Element(ElementOperation::MultiplyLong {
            unsigned: false,
            accumulate: Accumulate::Add,
        }),
        I::UmlalAsimdelemL => A::Element(ElementOperation::MultiplyLong {
            unsigned: true,
            accumulate: Accumulate::Add,
        }),
        I::SmlslAsimdelemL => A::Element(ElementOperation::MultiplyLong {
            unsigned: false,
            accumulate: Accumulate::Subtract,
        }),
        I::UmlslAsimdelemL => A::Element(ElementOperation::MultiplyLong {
            unsigned: true,
            accumulate: Accumulate::Subtract,
        }),
        I::SqdmullAsimdelemL | I::SqdmullAsisdelemL => A::Element(
            ElementOperation::SaturatingDoublingMultiplyLong(Accumulate::None),
        ),
        I::SqdmlalAsimdelemL | I::SqdmlalAsisdelemL => A::Element(
            ElementOperation::SaturatingDoublingMultiplyLong(Accumulate::Add),
        ),
        I::SqdmlslAsimdelemL | I::SqdmlslAsisdelemL => A::Element(
            ElementOperation::SaturatingDoublingMultiplyLong(Accumulate::Subtract),
        ),
        I::SqdmulhAsimdelemR | I::SqdmulhAsisdelemR => {
            A::Element(ElementOperation::DoublingMultiplyHigh { rounding: false })
        }
        I::SqrdmulhAsimdelemR | I::SqrdmulhAsisdelemR => {
            A::Element(ElementOperation::DoublingMultiplyHigh { rounding: true })
        }
        I::SqrdmlahAsimdelemR | I::SqrdmlahAsisdelemR => {
            A::Element(ElementOperation::RoundingDoublingMultiplyAccumulate { subtract: false })
        }
        I::SqrdmlshAsimdelemR | I::SqrdmlshAsisdelemR => {
            A::Element(ElementOperation::RoundingDoublingMultiplyAccumulate { subtract: true })
        }
        I::SdotVectorElement => A::Element(ElementOperation::Dot {
            first: false,
            second: false,
        }),
        I::UdotVectorElement => A::Element(ElementOperation::Dot {
            first: true,
            second: true,
        }),
        I::SudotVectorElement => A::Element(ElementOperation::Dot {
            first: false,
            second: true,
        }),
        I::UsdotVectorElement => A::Element(ElementOperation::Dot {
            first: true,
            second: false,
        }),
        I::SdotVector => A::Dot {
            first: false,
            second: false,
        },
        I::UdotVector => A::Dot {
            first: true,
            second: true,
        },
        I::UsdotVector => A::Dot {
            first: true,
            second: false,
        },
        I::SmmlaVector => A::MatrixMultiply {
            first: false,
            second: false,
        },
        I::UmmlaVector => A::MatrixMultiply {
            first: true,
            second: true,
        },
        I::UsmmlaVector => A::MatrixMultiply {
            first: true,
            second: false,
        },
        // Shifts by an immediate.
        I::SshrVectorImmediate | I::SshrScalarD => A::Shift(ShiftOperation::ShiftRight {
            unsigned: false,
            rounding: false,
            accumulate: false,
        }),
        I::UshrVectorImmediate | I::UshrScalarD => A::Shift(ShiftOperation::ShiftRight {
            unsigned: true,
            rounding: false,
            accumulate: false,
        }),
        I::SrshrVectorImmediate | I::SrshrScalarD => A::Shift(ShiftOperation::ShiftRight {
            unsigned: false,
            rounding: true,
            accumulate: false,
        }),
        I::UrshrVectorImmediate | I::UrshrScalarD => A::Shift(ShiftOperation::ShiftRight {
            unsigned: true,
            rounding: true,
            accumulate: false,
        }),
        I::SsraVectorImmediate | I::SsraScalarD => A::Shift(ShiftOperation::ShiftRight {
            unsigned: false,
            rounding: false,
            accumulate: true,
        }),
        I::UsraVectorImmediate | I::UsraScalarD => A::Shift(ShiftOperation::ShiftRight {
            unsigned: true,
            rounding: false,
            accumulate: true,
        }),
        I::SrsraVectorImmediate | I::SrsraScalarD => A::Shift(ShiftOperation::ShiftRight {
            unsigned: false,
            rounding: true,
            accumulate: true,
        }),
        I::UrsraVectorImmediate | I::UrsraScalarD => A::Shift(ShiftOperation::ShiftRight {
            unsigned: true,
            rounding: true,
            accumulate: true,
        }),
        I::ShlVectorImmediate | I::ShlScalarD => A::Shift(ShiftOperation::ShiftLeft),
        I::SqshlVectorImmediate | I::SqshlScalarImmediate => {
            A::Shift(ShiftOperation::SaturatingShiftLeft {
                source: false,
                result: false,
            })
        }
        I::SqshluVectorImmediate | I::SqshluScalar => {
            A::Shift(ShiftOperation::SaturatingShiftLeft {
                source: false,
                result: true,
            })
        }
        I::UqshlVectorImmediate | I::UqshlScalarImmediate => {
            A::Shift(ShiftOperation::SaturatingShiftLeft {
                source: true,
                result: true,
            })
        }
        I::Shrn => A::Shift(ShiftOperation::ShiftRightNarrow {
            rounding: false,
            narrowing: Narrowing::Truncate,
        }),
        I::Rshrn => A::Shift(ShiftOperation::ShiftRightNarrow {
            rounding: true,
            narrowing: Narrowing::Truncate,
        }),
        I::SqshrnVectorImmediate | I::SqshrnScalar => A::Shift(ShiftOperation::ShiftRightNarrow {
            rounding: false,
            narrowing: Narrowing::Signed,
        }),
        I::SqrshrnVectorImmediate | I::SqrshrnScalar => {
            A::Shift(ShiftOperation::ShiftRightNarrow {
                rounding: true,
                narrowing: Narrowing::Signed,
            })
        }
        I::SqshrunVectorImmediate | I::SqshrunScalar => {
            A::Shift(ShiftOperation::ShiftRightNarrow {
                rounding: false,
                narrowing: Narrowing::SignedToUnsigned,
            })
        }
        I::SqrshrunVectorImmediate | I::SqrshrunScalar => {
            A::Shift(ShiftOperation::ShiftRightNarrow {
                rounding: true,
                narrowing: Narrowing::SignedToUnsigned,
            })
        }
        I::UqshrnVectorImmediate | I::UqshrnScalar => A::Shift(ShiftOperation::ShiftRightNarrow {
            rounding: false,
            narrowing: Narrowing::Unsigned,
        }),
        I::UqrshrnVectorImmediate | I::UqrshrnScalar => {
            A::Shift(ShiftOperation::ShiftRightNarrow {
                rounding: true,
                narrowing: Narrowing::Unsigned,
            })
        }
        I::Sshll => A::Shift(ShiftOperation::ShiftLeftLong(false)),
        I::Ushll => A::Shift(ShiftOperation::ShiftLeftLong(true)),
        I::SriVectorImmediate | I::SriScalarD => A::Shift(ShiftOperation::Insert { left: false }),
        I::SliVectorImmediate | I::SliScalarD => A::Shift(ShiftOperation::Insert { left: true }),
        I::FcvtzsVectorImmediateImmediate | I::FcvtzsScalarImmediate => {
            A::Shift(ShiftOperation::ToFixed(false))
        }
        I::FcvtzuVectorImmediateImmediate | I::FcvtzuScalarImmediate => {
            A::Shift(ShiftOperation::ToFixed(true))
        }
        I::ScvtfVectorImmediateImmediate | I::ScvtfScalarImmediate => {
            A::Shift(ShiftOperation::FromFixed(false))
        }
        I::UcvtfVectorImmediateImmediate | I::UcvtfScalarImmediate => {
            A::Shift(ShiftOperation::FromFixed(true))
        }
        // Permutes, copies and immediates.
        I::TblVectorOne => A::Table {
            registers: 1,
            extension: false,
        },
        I::TblVectorTwo => A::Table {
            registers: 2,
            extension: false,
        },
        I::TblVectorThree => A::Table {
            registers: 3,
            extension: false,
        },
        I::TblVectorFour => A::Table {
            registers: 4,
            extension: false,
        },
        I::TbxVectorOne => A::Table {
            registers: 1,
            extension: true,
        },
        I::TbxVectorTwo => A::Table {
            registers: 2,
            extension: true,
        },
        I::TbxVectorThree => A::Table {
            registers: 3,
            extension: true,
        },
        I::TbxVectorFour => A::Table {
            registers: 4,
            extension: true,
        },
        I::ExtVector => A::Extract,
        I::DupVectorElement => A::DuplicateElement,
        I::DupVectorGeneral32 => A::DuplicateGeneral,
        I::Mov => A::DuplicateToScalar,
        I::SmovScalar32 => A::MoveToGeneral {
            signed: true,
            doubleword: false,
        },
        I::SmovScalar64 => A::MoveToGeneral {
            signed: true,
            doubleword: true,
        },
        I::UmovScalar32 => A::MoveToGeneral {
            signed: false,
            doubleword: false,
        },
        I::UmovScalarVi64_64 => A::MoveToGeneral {
            signed: false,
            doubleword: true,
        },
        I::InsVector32 => A::InsertGeneral,
        I::InsVector => A::InsertElement,
        I::MoviVectorImmediate32
        | I::OrrVectorImmediate32
        | I::MoviVectorImmediate16
        | I::OrrVectorImmediate16
        | I::MoviVectorSMsl
        | I::MoviVectorNs
        | I::FmovVectorImmediateNs32
        | I::MvniVectorImmediate32
        | I::BicVectorImmediate32
        | I::MvniVectorImmediate16
        | I::BicVectorImmediate16
        | I::MvniVectorSMsl
        | I::MoviScalar64
        | I::MoviVectorDNs
        | I::FmovVectorImmediateNs64
        | I::FmovVectorImmediateNs16 => A::ModifiedImmediate,
        // Floating point, two sources.
        I::FaddVectorRegister => fp(S::Same, Size, scalar(F::Add)),
        I::FaddVectorRegister16 => fp(S::Same, Half, scalar(F::Add)),
        I::FsubVectorRegister => fp(S::Same, Size, scalar(F::Subtract)),
        I::FsubVectorRegister16 => fp(S::Same, Half, scalar(F::Subtract)),
        I::FmulVectorRegister => fp(S::Same, Size, scalar(F::Multiply)),
        I::FmulVectorRegister16 => fp(S::Same, Half, scalar(F::Multiply)),
        I::FdivVectorRegister => fp(S::Same, Size, scalar(F::Divide)),
        I::FdivVectorRegister16 => fp(S::Same, Half, scalar(F::Divide)),
        I::FmaxVectorRegister => fp(S::Same, Size, scalar(F::Maximum)),
        I::FmaxVectorRegister16 => fp(S::Same, Half, scalar(F::Maximum)),
        I::FminVectorRegister => fp(S::Same, Size, scalar(F::Minimum)),
        I::FminVectorRegister16 => fp(S::Same, Half, scalar(F::Minimum)),
        I::FmaxnmVectorRegister => fp(S::Same, Size, scalar(F::MaximumNumber)),
        I::FmaxnmVectorRegister16 => fp(S::Same, Half, scalar(F::MaximumNumber)),
        I::FminnmVectorRegister => fp(S::Same, Size, scalar(F::MinimumNumber)),
        I::FminnmVectorRegister16 => fp(S::Same, Half, scalar(F::MinimumNumber)),
        I::FmulxVectorRegister | I::FmulxScalar => fp(S::Same, Size, L::MultiplyExtended),
        I::FmulxVectorRegister16 | I::FmulxScalar16 => fp(S::Same, Half, L::MultiplyExtended),
        I::FabdVectorRegister | I::FabdScalar => fp(S::Same, Size, L::AbsoluteDifference),
        I::FabdVectorRegister16 | I::FabdScalar16 => fp(S::Same, Half, L::AbsoluteDifference),
        I::FrecpsVectorRegister | I::FrecpsScalar => fp(S::Same, Size, L::ReciprocalStep),
        I::FrecpsVectorRegister16 | I::FrecpsScalar16 => fp(S::Same, Half, L::ReciprocalStep),
        I::FrsqrtsVectorRegister | I::FrsqrtsScalar => {
            fp(S::Same, Size, L::ReciprocalSquareRootStep)
        }
        I::FrsqrtsVectorRegister16 | I::FrsqrtsScalar16 => {
            fp(S::Same, Half, L::ReciprocalSquareRootStep)
        }
        I::FcmeqVectorRegister | I::FcmeqScalar => fp(S::Same, Size, L::Compare(C::Equal)),
        I::FcmeqVectorRegister16 | I::FcmeqScalar16 => fp(S::Same, Half, L::Compare(C::Equal)),
        I::FcmgeVectorRegister | I::FcmgeScalar => fp(S::Same, Size, L::Compare(C::GreaterOrEqual)),
        I::FcmgeVectorRegister16 | I::FcmgeScalar16 => {
            fp(S::Same, Half, L::Compare(C::GreaterOrEqual))
        }
        I::FcmgtVectorRegister | I::FcmgtScalar => fp(S::Same, Size, L::Compare(C::Greater)),
        I::FcmgtVectorRegister16 | I::FcmgtScalar16 => fp(S::Same, Half, L::Compare(C::Greater)),
        I::FacgeVectorRegister | I::FacgeScalar => {
            fp(S::Same, Size, L::Compare(C::AbsoluteGreaterOrEqual))
        }
        I::FacgeVectorRegister16 | I::FacgeScalar16 => {
            fp(S::Same, Half, L::Compare(C::AbsoluteGreaterOrEqual))
        }
        I::FacgtVectorRegister | I::FacgtScalar => {
            fp(S::Same, Size, L::Compare(C::AbsoluteGreater))
        }
        I::FacgtVectorRegister16 | I::FacgtScalar16 => {
            fp(S::Same, Half, L::Compare(C::AbsoluteGreater))
        }
        I::FmlaVectorRegister => fp(S::Accumulating, Size, scalar(FMA)),
        I::FmlaVectorRegister16 => fp(S::Accumulating, Half, scalar(FMA)),
        I::FmlsVectorRegister => fp(S::Accumulating, Size, scalar(FMS)),
        I::FmlsVectorRegister16 => fp(S::Accumulating, Half, scalar(FMS)),
        // Floating point, pairwise and across lanes.
        I::FaddpVectorRegister => fp(S::Pairwise, Size, scalar(F::Add)),
        I::FaddpVectorRegister16 => fp(S::Pairwise, Half, scalar(F::Add)),
        I::FmaxpVectorRegister => fp(S::Pairwise, Size, scalar(F::Maximum)),
        I::FmaxpVectorRegister16 => fp(S::Pairwise, Half, scalar(F::Maximum)),
        I::FminpVectorRegister => fp(S::Pairwise, Size, scalar(F::Minimum)),
        I::FminpVectorRegister16 => fp(S::Pairwise, Half, scalar(F::Minimum)),
        I::FmaxnmpVectorRegister => fp(S::Pairwise, Size, scalar(F::MaximumNumber)),
        I::FmaxnmpVectorRegister16 => fp(S::Pairwise, Half, scalar(F::MaximumNumber)),
        I::FminnmpVectorRegister => fp(S::Pairwise, Size, scalar(F::MinimumNumber)),
        I::FminnmpVectorRegister16 => fp(S::Pairwise, Half, scalar(F::MinimumNumber)),
        I::FaddpScalarRegisterP => fp(S::ScalarPairwise, Size, scalar(F::Add)),
        I::FaddpScalarRegisterP16 => fp(S::ScalarPairwise, Half, scalar(F::Add)),
        I::FmaxpScalarRegisterP => fp(S::ScalarPairwise, Size, scalar(F::Maximum)),
        I::FmaxpScalarRegisterP16 => fp(S::ScalarPairwise, Half, scalar(F::Maximum)),
        I::FminpScalarRegisterP => fp(S::ScalarPairwise, Size, scalar(F::Minimum)),
        I::FminpScalarRegisterP16 => fp(S::ScalarPairwise, Half, scalar(F::Minimum)),
        I::FmaxnmpScalarRegisterP => fp(S::ScalarPairwise, Size, scalar(F::MaximumNumber)),
        I::FmaxnmpScalarRegisterP16 => fp(S::ScalarPairwise, Half, scalar(F::MaximumNumber)),
        I::FminnmpScalarRegisterP => fp(S::ScalarPairwise, Size, scalar(F::MinimumNumber)),
        I::FminnmpScalarRegisterP16 => fp(S::ScalarPairwise, Half, scalar(F::MinimumNumber)),
        I::FmaxvScalarRegisterV32 => fp(S::Across, Size, scalar(F::Maximum)),
        I::FmaxvScalarRegisterV16 => fp(S::Across, Half, scalar(F::Maximum)),
        I::FminvScalarRegisterV32 => fp(S::Across, Size, scalar(F::Minimum)),
        I::FminvScalarRegisterV16 => fp(S::Across, Half, scalar(F::Minimum)),
        I::FmaxnmvScalarRegisterV32 => fp(S::Across, Size, scalar(F::MaximumNumber)),
        I::FmaxnmvScalarRegisterV16 => fp(S::Across, Half, scalar(F::MaximumNumber)),
        I::FminnmvScalarRegisterV32 => fp(S::Across, Size, scalar(F::MinimumNumber)),
        I::FminnmvScalarRegisterV16 => fp(S::Across, Half, scalar(F::MinimumNumber)),
        // Floating point, by element.
        I::FmulVectorRegisterElement | I::FmulScalarRegisterElement => fp(
            S::Element {
                accumulating: false,
            },
            Size,
            scalar(F::Multiply),
        ),
        I::FmulVectorRegisterElement16 | I::FmulScalarRegisterElement16 => fp(
            S::Element {
                accumulating: false,
            },
            Half,
            scalar(F::Multiply),
        ),
        I::FmulxVectorRegisterElement | I::FmulxScalarRegisterElement => fp(
            S::Element {
                accumulating: false,
            },
            Size,
            L::MultiplyExtended,
        ),
        I::FmulxVectorRegisterElement16 | I::FmulxScalarRegisterElement16 => fp(
            S::Element {
                accumulating: false,
            },
            Half,
            L::MultiplyExtended,
        ),
        I::FmlaVectorRegisterElement | I::FmlaScalarRegisterElement => {
            fp(S::Element { accumulating: true }, Size, scalar(FMA))
        }
        I::FmlaVectorRegisterElement16 | I::FmlaScalarRegisterElement16 => {
            fp(S::Element { accumulating: true }, Half, scalar(FMA))
        }
        I::FmlsVectorRegisterElement | I::FmlsScalarRegisterElement => {
            fp(S::Element { accumulating: true }, Size, scalar(FMS))
        }
        I::FmlsVectorRegisterElement16 | I::FmlsScalarRegisterElement16 => {
            fp(S::Element { accumulating: true }, Half, scalar(FMS))
        }
        // Floating point, one source.
        I::FabsVectorRegister => fp(S::Misc, Size, scalar(F::Absolute)),
        I::FabsVectorRegister16 => fp(S::Misc, Half, scalar(F::Absolute)),
        I::FnegVectorRegister => fp(S::Misc, Size, scalar(F::Negate)),
        I::FnegVectorRegister16 => fp(S::Misc, Half, scalar(F::Negate)),
        I::FsqrtVectorRegister => fp(S::Misc, Size, scalar(F::SquareRoot)),
        I::FsqrtVectorRegister16 => fp(S::Misc, Half, scalar(F::SquareRoot)),
        I::FrintnVectorRegister => fp(S::Misc, Size, round_to(Some(TiesToEven), false)),
        I::FrintnVectorRegister16 => fp(S::Misc, Half, round_to(Some(TiesToEven), false)),
        I::FrintpVectorRegister => fp(S::Misc, Size, round_to(Some(TowardPlusInfinity), false)),
        I::FrintpVectorRegister16 => fp(S::Misc, Half, round_to(Some(TowardPlusInfinity), false)),
        I::FrintmVectorRegister => fp(S::Misc, Size, round_to(Some(TowardMinusInfinity), false)),
        I::FrintmVectorRegister16 => fp(S::Misc, Half, round_to(Some(TowardMinusInfinity), false)),
        I::FrintzVectorRegister => fp(S::Misc, Size, round_to(Some(TowardZero), false)),
        I::FrintzVectorRegister16 => fp(S::Misc, Half, round_to(Some(TowardZero), false)),
        I::FrintaVectorRegister => fp(S::Misc, Size, round_to(Some(TiesAway), false)),
        I::FrintaVectorRegister16 => fp(S::Misc, Half, round_to(Some(TiesAway), false)),
        I::FrintxVectorRegister => fp(S::Misc, Size, round_to(None, true)),
        I::FrintxVectorRegister16 => fp(S::Misc, Half, round_to(None, true)),
        I::FrintiVectorRegister => fp(S::Misc, Size, round_to(None, false)),
        I::FrintiVectorRegister16 => fp(S::Misc, Half, round_to(None, false)),
        I::Frint32xVector => fp(S::Misc, Size, within(u32::BITS, None)),
        I::Frint32zVector => fp(S::Misc, Size, within(u32::BITS, Some(TowardZero))),
        I::Frint64xVector => fp(S::Misc, Size, within(u64::BITS, None)),
        I::Frint64zVector => fp(S::Misc, Size, within(u64::BITS, Some(TowardZero))),
        I::FcvtnsVectorRegister | I::FcvtnsScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TiesToEven), false))
        }
        I::FcvtnsVectorRegister16 | I::FcvtnsScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TiesToEven), false))
        }
        I::FcvtnuVectorRegister | I::FcvtnuScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TiesToEven), true))
        }
        I::FcvtnuVectorRegister16 | I::FcvtnuScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TiesToEven), true))
        }
        I::FcvtmsVectorRegister | I::FcvtmsScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TowardMinusInfinity), false))
        }
        I::FcvtmsVectorRegister16 | I::FcvtmsScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TowardMinusInfinity), false))
        }
        I::FcvtmuVectorRegister | I::FcvtmuScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TowardMinusInfinity), true))
        }
        I::FcvtmuVectorRegister16 | I::FcvtmuScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TowardMinusInfinity), true))
        }
        I::FcvtpsVectorRegister | I::FcvtpsScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TowardPlusInfinity), false))
        }
        I::FcvtpsVectorRegister16 | I::FcvtpsScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TowardPlusInfinity), false))
        }
        I::FcvtpuVectorRegister | I::FcvtpuScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TowardPlusInfinity), true))
        }
        I::FcvtpuVectorRegister16 | I::FcvtpuScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TowardPlusInfinity), true))
        }
        I::FcvtzsVectorRegister | I::FcvtzsScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TowardZero), false))
        }
        I::FcvtzsVectorRegister16 | I::FcvtzsScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TowardZero), false))
        }
        I::FcvtzuVectorRegister | I::FcvtzuScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TowardZero), true))
        }
        I::FcvtzuVectorRegister16 | I::FcvtzuScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TowardZero), true))
        }
        I::FcvtasVectorRegister | I::FcvtasScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TiesAway), false))
        }
        I::FcvtasVectorRegister16 | I::FcvtasScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TiesAway), false))
        }
        I::FcvtauVectorRegister | I::FcvtauScalarRegister => {
            fp(S::Misc, Size, to_integer(Some(TiesAway), true))
        }
        I::FcvtauVectorRegister16 | I::FcvtauScalarRegister16 => {
            fp(S::Misc, Half, to_integer(Some(TiesAway), true))
        }
        I::ScvtfVectorRegister | I::ScvtfScalarRegister => fp(S::Misc, Size, from_integer(false)),
        I::ScvtfVectorRegister16 | I::ScvtfScalarRegister16 => {
            fp(S::Misc, Half, from_integer(false))
        }
        I::UcvtfVectorRegister | I::UcvtfScalarRegister => fp(S::Misc, Size, from_integer(true)),
        I::UcvtfVectorRegister16 | I::UcvtfScalarRegister16 => {
            fp(S::Misc, Half, from_integer(true))
        }
        I::FcmeqVectorRegisterZero | I::FcmeqScalarRegisterZero => {
            fp(S::Misc, Size, L::Compare(C::Equal))
        }
        I::FcmeqVectorRegisterZero16 | I::FcmeqScalarRegisterZero16 => {
            fp(S::Misc, Half, L::Compare(C::Equal))
        }
        I::FcmgeVectorRegisterZero | I::FcmgeScalarRegisterZero => {
            fp(S::Misc, Size, L::Compare(C::GreaterOrEqual))
        }
        I::FcmgeVectorRegisterZero16 | I::FcmgeScalarRegisterZero16 => {
            fp(S::Misc, Half, L::Compare(C::GreaterOrEqual))
        }
        I::FcmgtVectorRegisterZero | I::FcmgtScalarRegisterZero => {
            fp(S::Misc, Size, L::Compare(C::Greater))
        }
        I::FcmgtVectorRegisterZero16 | I::FcmgtScalarRegisterZero16 => {
            fp(S::Misc, Half, L::Compare(C::Greater))
        }
        I::FcmleVectorRegisterZero | I::FcmleScalarRegisterZero => {
            fp(S::Misc, Size, L::CompareZeroWith(C::GreaterOrEqual))
        }
        I::FcmleVectorRegisterZero16 | I::FcmleScalarRegisterZero16 => {
            fp(S::Misc, Half, L::CompareZeroWith(C::GreaterOrEqual))
        }
        I::FcmltVectorRegisterZero | I::FcmltScalarRegisterZero => {
            fp(S::Misc, Size, L::CompareZeroWith(C::Greater))
        }
        I::FcmltVectorRegisterZero16 | I::FcmltScalarRegisterZero16 => {
            fp(S::Misc, Half, L::CompareZeroWith(C::Greater))
        }
        I::FrecpeVectorRegister | I::FrecpeScalarRegister => {
            fp(S::Misc, Size, L::ReciprocalEstimate)
        }
        I::FrecpeVectorRegister16 | I::FrecpeScalarRegister16 => {
            fp(S::Misc, Half, L::ReciprocalEstimate)
        }
        I::FrsqrteVectorRegister | I::FrsqrteScalarRegister => {
            fp(S::Misc, Size, L::ReciprocalSquareRootEstimate)
        }
        I::FrsqrteVectorRegister16 | I::FrsqrteScalarRegister16 => {
            fp(S::Misc, Half, L::ReciprocalSquareRootEstimate)
        }
        I::FrecpxScalarRegister => fp(S::Misc, Size, L::ReciprocalExponent),
        I::FrecpxScalarRegister16 => fp(S::Misc, Half, L::ReciprocalExponent),
        I::FcvtlVector => A::ConvertLonger,
        I::FcvtnVectorRegister => A::ConvertNarrower { to_odd: false },
        I::FcvtxnVector | I::FcvtxnScalar => A::ConvertNarrower { to_odd: true },
        I::FcmlaAsimdsame2C => A::ComplexMultiplyAccumulate { element: false },
        I::FcmlaAdvsimdElt => A::ComplexMultiplyAccumulate { element: true },
        I::FcaddAsimdsame2C => A::ComplexAdd,
        I::BfcvtnVector => A::ConvertToBrainFloat,
        I::BfdotVector => A::BrainFloatDot { element: false },
        I::BfdotVectorBF16DOTlane => A::BrainFloatDot { element: true },
        I::BfmlalAsimdsame2F => A::BrainFloatMultiplyAccumulate { element: false },
        I::BfmlalAsimdelemF => A::BrainFloatMultiplyAccumulate { element: true },
        I::BfmmlaVector => A::BrainFloatMatrixMultiply,
        I::FmlalVector => A::WideningMultiplyAccumulate {
            subtract: false,
            upper: false,
            element: false,
        },
        I::FmlslVector => A::WideningMultiplyAccumulate {
            subtract: true,
            upper: false,
            element: false,
        },
        I::Fmlal2Vector => A::WideningMultiplyAccumulate {
            subtract: false,
            upper: true,
            element: false,
        },
        I::Fmlsl2Vector => A::WideningMultiplyAccumulate {
            subtract: true,
            upper: true,
            element: false,
        },
        I::FmlalVectorElement => A::WideningMultiplyAccumulate {
            subtract: false,
            upper: false,
            element: true,
        },
        I::FmlslVectorElement => A::WideningMultiplyAccumulate {
            subtract: true,
            upper: false,
            element: true,
        },
        I::Fmlal2VectorElement => A::WideningMultiplyAccumulate {
            subtract: false,
            upper: true,
            element: true,
        },
        I::Fmlsl2VectorElement => A::WideningMultiplyAccumulate {
            subtract: true,
            upper: true,
            element: true,
        },
        _ => return None,
    })
}

impl AdvancedSimdOperation {
    /// Whether the operation reads FPCR.
    pub const fn reads_control(self) -> bool {
        match self {
            Self::FloatingPoint { operation, .. } => !matches!(
                operation,
                LaneOperation::Scalar(
                    FloatingPointOperation::Absolute | FloatingPointOperation::Negate
                )
            ),
            Self::Shift(ShiftOperation::ToFixed(_) | ShiftOperation::FromFixed(_))
            | Self::ConvertLonger
            | Self::ConvertNarrower { .. }
            | Self::ComplexMultiplyAccumulate { .. }
            | Self::ComplexAdd
            | Self::WideningMultiplyAccumulate { .. }
            | Self::ConvertToBrainFloat
            | Self::BrainFloatMultiplyAccumulate { .. } => true,
            _ => false,
        }
    }

    /// Whether the operation may set FPSR.QC, and so reads FPSR as the state it adds to.
    pub const fn saturates(self) -> bool {
        match self {
            Self::Same(operation) => matches!(
                operation,
                SameOperation::SaturatingAdd(_)
                    | SameOperation::SaturatingSubtract(_)
                    | SameOperation::SaturatingShift { .. }
                    | SameOperation::DoublingMultiplyHigh { .. }
                    | SameOperation::RoundingDoublingMultiplyAccumulate { .. }
            ),
            Self::Different(DifferentOperation::SaturatingDoublingMultiplyLong(_)) => true,
            Self::Misc(operation) => matches!(
                operation,
                MiscOperation::SaturatingAccumulate(_)
                    | MiscOperation::SaturatingAbsolute
                    | MiscOperation::SaturatingNegate
                    | MiscOperation::ExtractNarrow(
                        Narrowing::Signed | Narrowing::SignedToUnsigned | Narrowing::Unsigned
                    )
            ),
            Self::Element(operation) => matches!(
                operation,
                ElementOperation::SaturatingDoublingMultiplyLong(_)
                    | ElementOperation::DoublingMultiplyHigh { .. }
                    | ElementOperation::RoundingDoublingMultiplyAccumulate { .. }
            ),
            Self::Shift(operation) => matches!(
                operation,
                ShiftOperation::SaturatingShiftLeft { .. }
                    | ShiftOperation::ShiftRightNarrow {
                        narrowing: Narrowing::Signed
                            | Narrowing::SignedToUnsigned
                            | Narrowing::Unsigned,
                        ..
                    }
            ),
            _ => false,
        }
    }
}

/// Ones in the lowest `bits` bits, up to all 128.
const fn ones(bits: u32) -> u128 {
    if bits >= u128::BITS {
        u128::MAX
    } else {
        (1 << bits) - 1
    }
}

/// The element of `esize` bits at the index.
const fn element(vector: u128, index: u32, esize: u32) -> u128 {
    (vector >> (index * esize)) & ones(esize)
}

/// The vector with the element at the index replaced.
const fn with_element(vector: u128, index: u32, esize: u32, value: u128) -> u128 {
    let mask = ones(esize) << (index * esize);
    (vector & !mask) | ((value << (index * esize)) & mask)
}

/// The element as an integer, signed or not.
const fn to_integer(value: u128, esize: u32, unsigned: Unsigned) -> i128 {
    if unsigned || esize >= u128::BITS {
        value as i128
    } else {
        ((value << (u128::BITS - esize)) as i128) >> (u128::BITS - esize)
    }
}

/// [SatQ](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.SatQ.3): the value clamped to `bits` bits, and whether it had to be.
fn saturate(value: i128, bits: u32, unsigned: Unsigned) -> (u128, bool) {
    let (minimum, maximum) = if unsigned {
        (0, (1_i128 << bits) - 1)
    } else {
        (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
    };
    let clamped = value.clamp(minimum, maximum);
    (clamped as u128 & ones(bits), clamped != value)
}

/// The value shifted left by a signed amount, right where it is negative, rounding first where asked; a shift past
/// every element's width is taken as far as the result shows it.
fn shift_signed(value: i128, shift: i32, rounding: bool) -> i128 {
    if shift >= 0 {
        if value == 0 {
            return 0;
        }
        // An element of up to 64 bits shifted 64 or more saturates whatever its value, as this far beyond does.
        if shift >= u64::BITS as i32 && !crate::planted::is(1003) {
            return value.signum() << (u64::BITS + u8::BITS);
        }
        if shift >= u64::BITS as i32 {
            return value.wrapping_shl(shift as u32);
        }
        return value << shift;
    }
    let right = shift.unsigned_abs();
    if right > u64::BITS + 1 {
        // A value below two to the 64 with half of two to `right` added is below two to `right`.
        return if rounding {
            0
        } else {
            value >> (i128::BITS - 1)
        };
    }
    let rounded = if rounding {
        value + (1 << (right - 1))
    } else {
        value
    };
    rounded >> right
}

/// What the instruction leaves undefined: the elements of an encoding the pseudocode rejects.
const fn undefined() -> Result<(), Exception> {
    Err(Exception::UndefinedInstruction)
}

/// The registers an instruction names, the arrangement its Q and size give, and the flags its lanes raise.
struct Decoded {
    word: u32,
    d: usize,
    n: usize,
    m: usize,
    q: bool,
    size: u32,
    scalar: bool,
}

impl Decoded {
    fn new(word: u32) -> Self {
        Self {
            word,
            d: RD.read(word) as usize,
            n: RN.read(word) as usize,
            m: RM.read(word) as usize,
            q: Q.read(word) != 0,
            size: SIZE.read(word),
            scalar: SCALAR.read(word) != 0,
        }
    }

    const fn datasize(&self) -> u32 {
        if self.q {
            VECTOR_BITS
        } else {
            HALF_VECTOR_BITS
        }
    }

    /// The lower (Q clear) or upper half of a register, as Vpart reads it.
    const fn part(&self, vector: u128) -> u128 {
        if self.q {
            vector >> HALF_VECTOR_BITS
        } else {
            vector & ones(HALF_VECTOR_BITS)
        }
    }
}

/// Writes the low `bits` bits of the value to the register and clears the rest, as V[] does.
fn write<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    number: usize,
    value: u128,
    bits: u32,
) {
    processor.vector_registers[number] = value & ones(bits);
}

/// Writes 64 bits to the lower half of the register, clearing the upper, or to the upper, keeping the lower, as
/// Vpart[] does.
fn write_part<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    number: usize,
    value: u128,
    upper: bool,
) {
    let value = value & ones(HALF_VECTOR_BITS);
    processor.vector_registers[number] = if upper && crate::planted::is(1010) {
        value << HALF_VECTOR_BITS
    } else if upper {
        (processor.vector_registers[number] & ones(HALF_VECTOR_BITS)) | (value << HALF_VECTOR_BITS)
    } else {
        value
    };
}

/// Applies the operation to the registers the word names, setting FPSR.QC where it saturates and the cumulative
/// floating-point flags its lanes raise.
pub fn simulate_advanced_simd<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: AdvancedSimdOperation,
) -> Result<(), Exception> {
    let decoded = Decoded::new(word);
    let mut saturated = false;
    let mut flags = 0;
    let outcome = match operation {
        AdvancedSimdOperation::Same(operation) => {
            simulate_same(processor, &decoded, operation, &mut saturated)
        }
        AdvancedSimdOperation::Different(operation) => {
            simulate_different(processor, &decoded, operation, &mut saturated)
        }
        AdvancedSimdOperation::Misc(operation) => {
            simulate_misc(processor, &decoded, operation, &mut saturated)
        }
        AdvancedSimdOperation::Across(operation) => simulate_across(processor, &decoded, operation),
        AdvancedSimdOperation::ScalarAddPairwise => {
            let source = processor.vector_registers[decoded.n];
            let sum = element(source, 0, u64::BITS).wrapping_add(element(source, 1, u64::BITS));
            write(processor, decoded.d, sum, u64::BITS);
            Ok(())
        }
        AdvancedSimdOperation::Element(operation) => {
            simulate_element(processor, &decoded, operation, &mut saturated)
        }
        AdvancedSimdOperation::Shift(operation) => {
            simulate_shift(processor, &decoded, operation, &mut saturated, &mut flags)
        }
        AdvancedSimdOperation::Table {
            registers,
            extension,
        } => {
            let indices = processor.vector_registers[decoded.m];
            let mut result = if extension {
                processor.vector_registers[decoded.d]
            } else {
                0
            };
            for byte in 0..decoded.datasize() / u8::BITS {
                let index = element(indices, byte, u8::BITS) as u32;
                if index >= registers * VECTOR_BITS / u8::BITS && crate::planted::is(1022) {
                    result = with_element(result, byte, u8::BITS, 0);
                }
                if index < registers * VECTOR_BITS / u8::BITS {
                    let register_offset = if crate::planted::is(1021) {
                        0
                    } else {
                        index / (VECTOR_BITS / u8::BITS)
                    };
                    let register = (decoded.n + register_offset as usize) % 32;
                    let value = element(
                        processor.vector_registers[register],
                        index % (VECTOR_BITS / u8::BITS),
                        u8::BITS,
                    );
                    result = with_element(result, byte, u8::BITS, value);
                }
            }
            write(processor, decoded.d, result, decoded.datasize());
            Ok(())
        }
        AdvancedSimdOperation::Extract => {
            let position = IMM4.read(word);
            if !decoded.q && position >= u64::BITS / u8::BITS {
                return undefined();
            }
            let datasize = decoded.datasize();
            let low = processor.vector_registers[decoded.n] & ones(datasize);
            let high = processor.vector_registers[decoded.m] & ones(datasize);
            let position = if crate::planted::is(1023) && (position + 1) * u8::BITS < datasize {
                position + 1
            } else {
                position
            };
            let shift = position * u8::BITS;
            let result = if shift == 0 {
                low
            } else {
                (low >> shift) | (high << (datasize - shift))
            };
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
        AdvancedSimdOperation::DuplicateElement
        | AdvancedSimdOperation::DuplicateToScalar
        | AdvancedSimdOperation::InsertElement => {
            let imm5 = IMM5.read(word);
            let Some((esize, index)) = to_imm5_element(imm5) else {
                return undefined();
            };
            match operation {
                AdvancedSimdOperation::InsertElement => {
                    let source_index = if crate::planted::is(1026) {
                        IMM4.read(word) % (VECTOR_BITS / esize)
                    } else {
                        IMM4.read(word) >> esize.trailing_zeros().saturating_sub(3)
                    };
                    let value = element(processor.vector_registers[decoded.n], source_index, esize);
                    let updated =
                        with_element(processor.vector_registers[decoded.d], index, esize, value);
                    write(processor, decoded.d, updated, VECTOR_BITS);
                }
                AdvancedSimdOperation::DuplicateToScalar => {
                    let value = element(processor.vector_registers[decoded.n], index, esize);
                    write(processor, decoded.d, value, esize);
                }
                _ => {
                    if esize == u64::BITS && !decoded.q {
                        return undefined();
                    }
                    let index = if crate::planted::is(1024) {
                        index.saturating_sub(1)
                    } else {
                        index
                    };
                    let value = element(processor.vector_registers[decoded.n], index, esize);
                    let result = (0..decoded.datasize() / esize)
                        .fold(0, |result, lane| with_element(result, lane, esize, value));
                    write(processor, decoded.d, result, decoded.datasize());
                }
            }
            Ok(())
        }
        AdvancedSimdOperation::DuplicateGeneral | AdvancedSimdOperation::InsertGeneral => {
            let Some((esize, index)) = to_imm5_element(IMM5.read(word)) else {
                return undefined();
            };
            let value = u128::from(read_register(
                processor,
                decoded.n as u32,
                RegisterThirtyOne::ZeroRegister,
            )) & ones(esize);
            if operation == AdvancedSimdOperation::InsertGeneral {
                let updated =
                    with_element(processor.vector_registers[decoded.d], index, esize, value);
                write(processor, decoded.d, updated, VECTOR_BITS);
            } else {
                if esize == u64::BITS && !decoded.q {
                    return undefined();
                }
                let result = (0..decoded.datasize() / esize)
                    .fold(0, |result, lane| with_element(result, lane, esize, value));
                write(processor, decoded.d, result, decoded.datasize());
            }
            Ok(())
        }
        AdvancedSimdOperation::MoveToGeneral { signed, doubleword } => {
            let Some((esize, index)) = to_imm5_element(IMM5.read(word)) else {
                return undefined();
            };
            let width = if doubleword { u64::BITS } else { u32::BITS };
            let allowed = match (signed, doubleword) {
                (true, false) => esize <= u16::BITS,
                (true, true) => esize <= u32::BITS,
                (false, false) => esize <= u32::BITS,
                (false, true) => esize == u64::BITS,
            };
            if !allowed {
                return undefined();
            }
            let value = element(processor.vector_registers[decoded.n], index, esize);
            let value = if signed && !crate::planted::is(1025) {
                to_integer(value, esize, false) as u128 & ones(width)
            } else {
                value
            };
            write_register(
                processor,
                decoded.d as u32,
                value as u64,
                RegisterThirtyOne::ZeroRegister,
            );
            Ok(())
        }
        AdvancedSimdOperation::ModifiedImmediate => {
            simulate_modified_immediate(processor, &decoded)
        }
        AdvancedSimdOperation::Dot { first, second } => {
            let (n, m) = (
                processor.vector_registers[decoded.n],
                processor.vector_registers[decoded.m],
            );
            let result = (0..decoded.datasize() / u32::BITS).fold(0, |result, lane| {
                let sum = dot(n, m, lane, lane, first, second);
                let accumulated = element(processor.vector_registers[decoded.d], lane, u32::BITS)
                    .wrapping_add(sum as u128);
                with_element(result, lane, u32::BITS, accumulated)
            });
            write(processor, decoded.d, result, decoded.datasize());
            Ok(())
        }
        AdvancedSimdOperation::MatrixMultiply { first, second } => {
            let (n, m) = (
                processor.vector_registers[decoded.n],
                processor.vector_registers[decoded.m],
            );
            let addend = processor.vector_registers[decoded.d];
            let mut result = 0;
            for row in 0..2 {
                for column in 0..2 {
                    let mut sum = to_integer(
                        element(addend, 2 * row + column, u32::BITS),
                        u32::BITS,
                        false,
                    );
                    for k in 0..8 {
                        sum += to_integer(element(n, 8 * row + k, u8::BITS), u8::BITS, first)
                            * to_integer(
                                element(m, 8 * column + k, u8::BITS),
                                u8::BITS,
                                second || crate::planted::is(1050),
                            );
                    }
                    result = with_element(result, 2 * row + column, u32::BITS, sum as u128);
                }
            }
            write(processor, decoded.d, result, VECTOR_BITS);
            Ok(())
        }
        AdvancedSimdOperation::FloatingPoint {
            shape,
            precision,
            operation,
        } => simulate_floating_point_lanes(
            processor, &decoded, shape, precision, operation, &mut flags,
        ),
        AdvancedSimdOperation::ConvertLonger => {
            let (from, to) = if SZ.read(word) == 0 {
                (Precision::Half, Precision::Single)
            } else {
                (Precision::Single, Precision::Double)
            };
            let source = if crate::planted::is(1039) {
                processor.vector_registers[decoded.n] >> HALF_VECTOR_BITS
            } else {
                decoded.part(processor.vector_registers[decoded.n])
            };
            let fpcr = processor.floating_point_control;
            let result = (0..VECTOR_BITS / to.to_bits()).fold(0, |result, lane| {
                let value = element(source, lane, from.to_bits()) as u64;
                let converted = compute_lane(
                    LaneOperation::ConvertTo(to),
                    &[value],
                    from,
                    fpcr,
                    &mut flags,
                );
                with_element(result, lane, to.to_bits(), u128::from(converted))
            });
            write(processor, decoded.d, result, VECTOR_BITS);
            Ok(())
        }
        AdvancedSimdOperation::ConvertNarrower { to_odd } => {
            let (from, to) = if SZ.read(word) == 0 {
                if to_odd {
                    return undefined();
                }
                (Precision::Single, Precision::Half)
            } else {
                (Precision::Double, Precision::Single)
            };
            let lane_operation = if to_odd {
                LaneOperation::ConvertToOdd
            } else {
                LaneOperation::ConvertTo(to)
            };
            let source = processor.vector_registers[decoded.n];
            let fpcr = processor.floating_point_control;
            let lanes = if decoded.scalar {
                1
            } else {
                VECTOR_BITS / from.to_bits()
            };
            let result = (0..lanes).fold(0, |result, lane| {
                let value = element(source, lane, from.to_bits()) as u64;
                let converted = compute_lane(lane_operation, &[value], from, fpcr, &mut flags);
                with_element(result, lane, to.to_bits(), u128::from(converted))
            });
            if decoded.scalar {
                write(processor, decoded.d, result, to.to_bits());
            } else {
                write_part(
                    processor,
                    decoded.d,
                    result,
                    decoded.q && !crate::planted::is(1046),
                );
            }
            Ok(())
        }
        AdvancedSimdOperation::ComplexMultiplyAccumulate {
            element: by_element,
        } => simulate_complex_multiply_accumulate(processor, &decoded, by_element, &mut flags),
        AdvancedSimdOperation::ComplexAdd => simulate_complex_add(processor, &decoded, &mut flags),
        AdvancedSimdOperation::ConvertToBrainFloat => {
            let source = processor.vector_registers[decoded.n];
            let fpcr = processor.floating_point_control;
            let result = (0..VECTOR_BITS / u32::BITS).fold(0, |result, lane| {
                let value = element(source, lane, u32::BITS) as u64;
                let converted =
                    crate::floating_point::lanes::convert_to_brain_float(value, fpcr, &mut flags);
                with_element(result, lane, u16::BITS, u128::from(converted))
            });
            write_part(processor, decoded.d, result, decoded.q);
            Ok(())
        }
        AdvancedSimdOperation::BrainFloatDot {
            element: by_element,
        } => {
            let n = processor.vector_registers[decoded.n];
            let d = processor.vector_registers[decoded.d];
            let (m, index) = if by_element {
                let (register, index) = to_element_operand(word, u32::BITS);
                (processor.vector_registers[register], Some(index))
            } else {
                (processor.vector_registers[decoded.m], None)
            };
            let datasize = decoded.datasize();
            let result = (0..datasize / u32::BITS).fold(0, |result, lane| {
                let pair = index.unwrap_or(lane);
                let first = [
                    element(n, 2 * lane, u16::BITS) as u64,
                    element(n, 2 * lane + 1, u16::BITS) as u64,
                ];
                let second = [
                    element(m, 2 * pair, u16::BITS) as u64,
                    element(m, 2 * pair + 1, u16::BITS) as u64,
                ];
                let sum = crate::floating_point::lanes::dot_add_brain(
                    element(d, lane, u32::BITS) as u64,
                    first,
                    second,
                );
                with_element(result, lane, u32::BITS, u128::from(sum))
            });
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
        AdvancedSimdOperation::BrainFloatMultiplyAccumulate {
            element: by_element,
        } => {
            let n = processor.vector_registers[decoded.n];
            let d = processor.vector_registers[decoded.d];
            let odd = u32::from(decoded.q && !crate::planted::is(1062));
            let second_element = by_element.then(|| {
                let index = (ELEMENT_H.read(word) << 2)
                    | (ELEMENT_L.read(word) << 1)
                    | ELEMENT_M.read(word);
                element(
                    processor.vector_registers[ELEMENT_RM.read(word) as usize],
                    index,
                    u16::BITS,
                )
            });
            let m = processor.vector_registers[decoded.m];
            let fpcr = processor.floating_point_control;
            let result = (0..VECTOR_BITS / u32::BITS).fold(0, |result, lane| {
                let first = (element(n, 2 * lane + odd, u16::BITS) as u64) << u16::BITS;
                let second = (second_element
                    .unwrap_or_else(|| element(m, 2 * lane + odd, u16::BITS))
                    as u64)
                    << u16::BITS;
                let addend = element(d, lane, u32::BITS) as u64;
                let sum = compute_lane(
                    LaneOperation::Scalar(FloatingPointOperation::MultiplyAdd {
                        negate_product: false,
                        negate_addend: false,
                    }),
                    &[first, second, addend],
                    Precision::Single,
                    fpcr,
                    &mut flags,
                );
                with_element(result, lane, u32::BITS, u128::from(sum))
            });
            write(processor, decoded.d, result, VECTOR_BITS);
            Ok(())
        }
        AdvancedSimdOperation::BrainFloatMatrixMultiply => {
            let (n, m) = (
                processor.vector_registers[decoded.n],
                processor.vector_registers[decoded.m],
            );
            let addend = processor.vector_registers[decoded.d];
            let mut result = 0;
            for row in 0..2 {
                for column in 0..2 {
                    let mut sum = element(addend, 2 * row + column, u32::BITS) as u64;
                    for k in 0..2 {
                        let first = [
                            element(n, 4 * row + 2 * k, u16::BITS) as u64,
                            element(n, 4 * row + 2 * k + 1, u16::BITS) as u64,
                        ];
                        let second = [
                            element(m, to_matrix_lane(column, k), u16::BITS) as u64,
                            element(m, to_matrix_lane(column, k) + 1, u16::BITS) as u64,
                        ];
                        sum = crate::floating_point::lanes::dot_add_brain(sum, first, second);
                    }
                    result = with_element(result, 2 * row + column, u32::BITS, u128::from(sum));
                }
            }
            write(processor, decoded.d, result, VECTOR_BITS);
            Ok(())
        }
        AdvancedSimdOperation::WideningMultiplyAccumulate {
            subtract,
            upper,
            element: by_element,
        } => simulate_widening_multiply_accumulate(
            processor, &decoded, subtract, upper, by_element, &mut flags,
        ),
    };
    if outcome.is_ok() {
        if saturated && !crate::planted::is(1001) {
            processor.floating_point_status |= SATURATION;
        }
        processor.floating_point_status |= flags;
    }
    outcome
}

/// The first BFloat16 lane of row `row`, pair `pair` of BFMMLA's second matrix.
fn to_matrix_lane(row: u32, pair: u32) -> u32 {
    if crate::planted::is(1063) {
        2 * row + 4 * pair
    } else {
        4 * row + 2 * pair
    }
}

/// The element size and index imm5 names: its lowest set bit gives the size, the bits above it the index.
const fn to_imm5_element(imm5: u32) -> Option<(u32, u32)> {
    let size = imm5.trailing_zeros();
    if size > 3 {
        return None;
    }
    Some((u8::BITS << size, imm5 >> (size + 1)))
}

/// The sum of the products of the four bytes of lane `first_lane` of `n` and of lane `second_lane` of `m`.
fn dot(
    n: u128,
    m: u128,
    first_lane: u32,
    second_lane: u32,
    first: Unsigned,
    second: Unsigned,
) -> i128 {
    (0..DOT_GROUP)
        .map(|byte| {
            to_integer(
                element(n, DOT_GROUP * first_lane + byte, u8::BITS),
                u8::BITS,
                first || crate::planted::is(1049),
            ) * to_integer(
                element(m, DOT_GROUP * second_lane + byte, u8::BITS),
                u8::BITS,
                second || crate::planted::is(1049),
            )
        })
        .sum()
}

/// The element size, the number of elements and the width of the result of a form of one arrangement.
fn to_same_shape(decoded: &Decoded, esize: u32) -> (u32, u32) {
    let datasize = if decoded.scalar {
        esize
    } else {
        decoded.datasize()
    };
    (datasize, datasize / esize)
}

fn simulate_same<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    operation: SameOperation,
    saturated: &mut bool,
) -> Result<(), Exception> {
    let esize = u8::BITS << decoded.size;
    let (datasize, elements) = to_same_shape(decoded, esize);
    let n = processor.vector_registers[decoded.n];
    let m = processor.vector_registers[decoded.m];
    let d = processor.vector_registers[decoded.d];
    if let SameOperation::BitwiseSelect(select) = operation {
        let result = match select {
            Select::ByDestination => m ^ ((m ^ n) & d),
            Select::WhereSecond => d ^ ((d ^ n) & m),
            Select::WhereNotSecond if crate::planted::is(1051) => d ^ ((d ^ n) & m),
            Select::WhereNotSecond => d ^ ((d ^ n) & !m),
        };
        write(processor, decoded.d, result, datasize);
        return Ok(());
    }
    let doubling = matches!(
        operation,
        SameOperation::DoublingMultiplyHigh { .. }
            | SameOperation::RoundingDoublingMultiplyAccumulate { .. }
    );
    if doubling && (decoded.size == 0 || decoded.size == 3) {
        return undefined();
    }
    if matches!(
        operation,
        SameOperation::MultiplyAccumulate { .. } | SameOperation::AbsoluteDifferenceAccumulate(_)
    ) && decoded.size == 3
    {
        return undefined();
    }
    let mut result = 0;
    for index in 0..elements {
        let (first, second, third) = (
            element(n, index, esize),
            element(m, index, esize),
            element(d, index, esize),
        );
        let value = match operation {
            SameOperation::SaturatingAdd(unsigned)
            | SameOperation::SaturatingSubtract(unsigned) => {
                let (a, b) = (
                    to_integer(first, esize, unsigned),
                    to_integer(second, esize, unsigned),
                );
                let sum = if matches!(operation, SameOperation::SaturatingAdd(_)) {
                    a + b
                } else {
                    a - b
                };
                let (value, sat) = if crate::planted::is(1002) {
                    (sum as u128 & ones(esize), false)
                } else {
                    saturate(sum, esize, unsigned)
                };
                *saturated |= sat;
                value
            }
            SameOperation::SaturatingShift { unsigned, rounding } => {
                let shift = to_integer(second & 0xFF, u8::BITS, false) as i32;
                let shifted = shift_signed(to_integer(first, esize, unsigned), shift, rounding);
                let (value, sat) = saturate(shifted, esize, unsigned);
                *saturated |= sat;
                value
            }
            SameOperation::AbsoluteDifferenceAccumulate(unsigned) => {
                let difference = (to_integer(first, esize, unsigned)
                    - to_integer(second, esize, unsigned))
                .unsigned_abs();
                third.wrapping_add(difference)
            }
            SameOperation::MultiplyAccumulate { subtract } => {
                let product = first.wrapping_mul(second);
                if subtract {
                    third.wrapping_sub(product)
                } else {
                    third.wrapping_add(product)
                }
            }
            SameOperation::DoublingMultiplyHigh { rounding } => {
                let (value, sat) = doubling_multiply_high(first, second, esize, rounding);
                *saturated |= sat;
                value
            }
            SameOperation::RoundingDoublingMultiplyAccumulate { subtract } => {
                let (value, sat) =
                    rounding_doubling_multiply_accumulate(first, second, third, esize, subtract);
                *saturated |= sat;
                value
            }
            SameOperation::BitwiseSelect(_) => unreachable!("taken above"),
        };
        result = with_element(result, index, esize, value);
    }
    write(processor, decoded.d, result, datasize);
    Ok(())
}

/// SQDMULH and SQRDMULH of one element: twice the product's high half, rounded where asked.
fn doubling_multiply_high(first: u128, second: u128, esize: u32, rounding: bool) -> (u128, bool) {
    let product = 2 * to_integer(first, esize, false) * to_integer(second, esize, false);
    let rounded = if rounding && !crate::planted::is(1004) {
        product + (1 << (esize - 1))
    } else {
        product
    };
    saturate(rounded >> esize, esize, false)
}

/// SQRDMLAH and SQRDMLSH of one element.
fn rounding_doubling_multiply_accumulate(
    first: u128,
    second: u128,
    accumulator: u128,
    esize: u32,
    subtract: bool,
) -> (u128, bool) {
    let product = 2 * to_integer(first, esize, false) * to_integer(second, esize, false);
    let accumulator = to_integer(accumulator, esize, false) << esize;
    let sum = if subtract {
        accumulator - product
    } else {
        accumulator + product
    };
    let rounding = if crate::planted::is(1057) {
        0
    } else {
        1 << (esize - 1)
    };
    saturate((sum + rounding) >> esize, esize, false)
}

/// SQDMULL, and SQDMLAL and SQDMLSL with their accumulator, of one element: each step saturated to twice the size.
fn saturating_doubling_multiply_long(
    first: u128,
    second: u128,
    accumulator: u128,
    esize: u32,
    accumulate: Accumulate,
) -> (u128, bool) {
    let product = 2 * to_integer(first, esize, false) * to_integer(second, esize, false);
    let (product, first_saturated) = if crate::planted::is(1005) {
        (product as u128 & ones(2 * esize), false)
    } else {
        saturate(product, 2 * esize, false)
    };
    let product = to_integer(product, 2 * esize, false);
    let accumulator = to_integer(accumulator, 2 * esize, false);
    let (value, second_saturated) = match accumulate {
        Accumulate::None => (product as u128 & ones(2 * esize), false),
        Accumulate::Add => saturate(accumulator + product, 2 * esize, false),
        Accumulate::Subtract => saturate(accumulator - product, 2 * esize, false),
    };
    (value, first_saturated || second_saturated)
}

/// SMULL, UMULL and their accumulating forms, of one element, to twice its size.
fn multiply_long(
    first: u128,
    second: u128,
    accumulator: u128,
    esize: u32,
    unsigned: Unsigned,
    accumulate: Accumulate,
) -> u128 {
    let product =
        (to_integer(first, esize, unsigned) * to_integer(second, esize, unsigned)) as u128;
    match accumulate {
        Accumulate::None => product,
        Accumulate::Add => accumulator.wrapping_add(product),
        Accumulate::Subtract => accumulator.wrapping_sub(product),
    }
}

/// The carry-less product of two elements.
fn polynomial_multiply(first: u128, second: u128, esize: u32) -> u128 {
    (0..esize)
        .filter(|bit| second >> bit & 1 != 0)
        .fold(0, |product, bit| product ^ (first << bit))
}

fn simulate_different<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    operation: DifferentOperation,
    saturated: &mut bool,
) -> Result<(), Exception> {
    let esize = u8::BITS << decoded.size;
    let n = processor.vector_registers[decoded.n];
    let m = processor.vector_registers[decoded.m];
    let d = processor.vector_registers[decoded.d];
    match operation {
        DifferentOperation::PolynomialMultiplyLong => {
            if decoded.size == 1 || decoded.size == 2 {
                return undefined();
            }
            let (first, second) = (decoded.part(n), decoded.part(m));
            let result = (0..HALF_VECTOR_BITS / esize).fold(0, |result, index| {
                let product = polynomial_multiply(
                    element(first, index, esize),
                    element(second, index, esize),
                    esize,
                );
                let product = if crate::planted::is(1056) && esize == u64::BITS {
                    product & ones(u64::BITS)
                } else {
                    product
                };
                with_element(result, index, 2 * esize, product)
            });
            write(processor, decoded.d, result, VECTOR_BITS);
            Ok(())
        }
        DifferentOperation::AddHighNarrow { rounding }
        | DifferentOperation::SubtractHighNarrow { rounding } => {
            if decoded.size == 3 {
                return undefined();
            }
            let subtract = matches!(operation, DifferentOperation::SubtractHighNarrow { .. });
            let result = (0..HALF_VECTOR_BITS / esize).fold(0, |result, index| {
                let (first, second) = (element(n, index, 2 * esize), element(m, index, 2 * esize));
                let sum = if subtract {
                    first.wrapping_sub(second)
                } else {
                    first.wrapping_add(second)
                };
                let rounded = if rounding && !crate::planted::is(1007) {
                    sum.wrapping_add(1 << (esize - 1))
                } else {
                    sum
                };
                let high = if crate::planted::is(1006) {
                    rounded & ones(esize)
                } else {
                    (rounded & ones(2 * esize)) >> esize
                };
                with_element(result, index, esize, high)
            });
            write_part(processor, decoded.d, result, decoded.q);
            Ok(())
        }
        _ => {
            if decoded.size == 3 {
                return undefined();
            }
            let doubling = matches!(
                operation,
                DifferentOperation::SaturatingDoublingMultiplyLong(_)
            );
            if doubling && decoded.size == 0 {
                return undefined();
            }
            let wide = matches!(
                operation,
                DifferentOperation::AddWide(_) | DifferentOperation::SubtractWide(_)
            );
            let (first_source, second_source, elements) = if decoded.scalar {
                (n, m, 1)
            } else {
                let first = if wide { n } else { decoded.part(n) };
                let second = if crate::planted::is(1008) {
                    m & ones(HALF_VECTOR_BITS)
                } else {
                    decoded.part(m)
                };
                (first, second, HALF_VECTOR_BITS / esize)
            };
            let mut result = 0;
            for index in 0..elements {
                let first_size = if wide { 2 * esize } else { esize };
                let first = element(first_source, index, first_size);
                let second = element(second_source, index, esize);
                let accumulator = element(d, index, 2 * esize);
                let value = match operation {
                    DifferentOperation::AddLong(unsigned)
                    | DifferentOperation::SubtractLong(unsigned)
                    | DifferentOperation::AddWide(unsigned)
                    | DifferentOperation::SubtractWide(unsigned) => {
                        let a = to_integer(first, first_size, unsigned);
                        let b = to_integer(second, esize, unsigned);
                        let sum = if matches!(
                            operation,
                            DifferentOperation::AddLong(_) | DifferentOperation::AddWide(_)
                        ) {
                            a + b
                        } else {
                            a - b
                        };
                        sum as u128
                    }
                    DifferentOperation::AbsoluteDifferenceLong {
                        unsigned,
                        accumulate,
                    } => {
                        let difference = (to_integer(first, esize, unsigned)
                            - to_integer(second, esize, unsigned))
                        .unsigned_abs();
                        if accumulate {
                            accumulator.wrapping_add(difference)
                        } else {
                            difference
                        }
                    }
                    DifferentOperation::MultiplyLong {
                        unsigned,
                        accumulate,
                    } => multiply_long(first, second, accumulator, esize, unsigned, accumulate),
                    DifferentOperation::SaturatingDoublingMultiplyLong(accumulate) => {
                        let (value, sat) = saturating_doubling_multiply_long(
                            first,
                            second,
                            accumulator,
                            esize,
                            accumulate,
                        );
                        *saturated |= sat;
                        value
                    }
                    DifferentOperation::PolynomialMultiplyLong
                    | DifferentOperation::AddHighNarrow { .. }
                    | DifferentOperation::SubtractHighNarrow { .. } => unreachable!("taken above"),
                };
                result = with_element(result, index, 2 * esize, value);
            }
            let datasize = if decoded.scalar {
                2 * esize
            } else {
                VECTOR_BITS
            };
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
    }
}

fn simulate_misc<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    operation: MiscOperation,
    saturated: &mut bool,
) -> Result<(), Exception> {
    let esize = u8::BITS << decoded.size;
    let n = processor.vector_registers[decoded.n];
    let d = processor.vector_registers[decoded.d];
    match operation {
        MiscOperation::Reverse(container) => {
            let container = if crate::planted::is(1018) {
                u64::BITS
            } else {
                container
            };
            if esize >= container {
                return undefined();
            }
            let datasize = decoded.datasize();
            let per_container = container / esize;
            let result = (0..datasize / esize).fold(0, |result, index| {
                let reversed =
                    index - index % per_container + (per_container - 1 - index % per_container);
                with_element(result, index, esize, element(n, reversed, esize))
            });
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
        MiscOperation::AddPairwiseLong {
            unsigned,
            accumulate,
        } => {
            if decoded.size == 3 {
                return undefined();
            }
            let datasize = decoded.datasize();
            let result = (0..datasize / (2 * esize)).fold(0, |result, index| {
                let sum = to_integer(element(n, 2 * index, esize), esize, unsigned)
                    + to_integer(element(n, 2 * index + 1, esize), esize, unsigned);
                let sum = if accumulate && !crate::planted::is(1054) {
                    element(d, index, 2 * esize).wrapping_add(sum as u128)
                } else {
                    sum as u128
                };
                with_element(result, index, 2 * esize, sum)
            });
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
        MiscOperation::ExtractNarrow(narrowing) => {
            if decoded.size == 3 {
                return undefined();
            }
            let elements = if decoded.scalar {
                1
            } else {
                HALF_VECTOR_BITS / esize
            };
            let result = (0..elements).fold(0, |result, index| {
                let (value, sat) = narrow(
                    element(n, index, 2 * esize) as i128,
                    2 * esize,
                    esize,
                    narrowing,
                );
                *saturated |= sat;
                with_element(result, index, esize, value)
            });
            if decoded.scalar {
                write(processor, decoded.d, result, esize);
            } else {
                write_part(processor, decoded.d, result, decoded.q);
            }
            Ok(())
        }
        MiscOperation::ShiftLeftLongByElementSize => {
            if decoded.size == 3 {
                return undefined();
            }
            let source = decoded.part(n);
            let result = (0..HALF_VECTOR_BITS / esize).fold(0, |result, index| {
                let shift = esize - u32::from(crate::planted::is(1058));
                with_element(
                    result,
                    index,
                    2 * esize,
                    element(source, index, esize) << shift,
                )
            });
            write(processor, decoded.d, result, VECTOR_BITS);
            Ok(())
        }
        MiscOperation::UnsignedReciprocalEstimate
        | MiscOperation::UnsignedReciprocalSquareRootEstimate => {
            if SZ.read(decoded.word) != 0 {
                return undefined();
            }
            let datasize = decoded.datasize();
            let result = (0..datasize / u32::BITS).fold(0, |result, index| {
                let value = element(n, index, u32::BITS) as u64;
                let estimate = if operation == MiscOperation::UnsignedReciprocalEstimate {
                    if value >> 31 == 0 && !(crate::planted::is(1047) && value == 0x7FFF_FFFF) {
                        u64::from(u32::MAX)
                    } else {
                        crate::floating_point::lanes::estimate_reciprocal_fraction(
                            (value >> 23) & 0x1FF,
                        ) << 23
                    }
                } else if value >> 30 == 0 && !(crate::planted::is(1048) && value == 0x3FFF_FFFF) {
                    u64::from(u32::MAX)
                } else {
                    crate::floating_point::lanes::estimate_reciprocal_square_root_fraction(
                        (value >> 23) & 0x1FF,
                    ) << 23
                };
                with_element(result, index, u32::BITS, u128::from(estimate))
            });
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
        _ => {
            // CNT, NOT and RBIT take bytes whatever their size field holds.
            let esize = if matches!(
                operation,
                MiscOperation::CountBits | MiscOperation::Not | MiscOperation::ReverseBits
            ) {
                u8::BITS
            } else {
                esize
            };
            let (datasize, elements) = to_same_shape(decoded, esize);
            let mut result = 0;
            for index in 0..elements {
                let value = element(n, index, esize);
                let signed = to_integer(value, esize, false);
                let lane = match operation {
                    MiscOperation::SaturatingAccumulate(unsigned) => {
                        let source =
                            to_integer(value, esize, !unsigned && !crate::planted::is(1053));
                        let accumulator = to_integer(element(d, index, esize), esize, unsigned);
                        let (value, sat) = saturate(source + accumulator, esize, unsigned);
                        *saturated |= sat;
                        value
                    }
                    MiscOperation::CountLeadingSignBits => {
                        let leading = if signed < 0 {
                            !value & ones(esize)
                        } else {
                            value
                        };
                        let count = u128::from(leading.leading_zeros() - (u128::BITS - esize));
                        if crate::planted::is(1017) {
                            count
                        } else {
                            count.saturating_sub(1)
                        }
                    }
                    MiscOperation::CountLeadingZeros => {
                        u128::from(value.leading_zeros() - (u128::BITS - esize))
                    }
                    MiscOperation::CountBits if crate::planted::is(1055) => {
                        u128::from(u8::BITS - value.count_ones())
                    }
                    MiscOperation::CountBits => u128::from(value.count_ones()),
                    MiscOperation::Not => !value,
                    MiscOperation::ReverseBits => value.reverse_bits() >> (u128::BITS - esize),
                    MiscOperation::SaturatingAbsolute | MiscOperation::SaturatingNegate => {
                        let changed = if operation == MiscOperation::SaturatingAbsolute {
                            signed.abs()
                        } else {
                            -signed
                        };
                        let (value, sat) = if crate::planted::is(1052) {
                            (changed as u128 & ones(esize), false)
                        } else {
                            saturate(changed, esize, false)
                        };
                        *saturated |= sat;
                        value
                    }
                    _ => unreachable!("taken above"),
                };
                result = with_element(result, index, esize, lane);
            }
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
    }
}

/// One element narrowed from `from` bits to `to`, read and saturated as the narrowing says.
fn narrow(value: i128, from: u32, to: u32, narrowing: Narrowing) -> (u128, bool) {
    let value = value as u128 & ones(from);
    match narrowing {
        Narrowing::Truncate => (value & ones(to), false),
        Narrowing::Signed => saturate(to_integer(value, from, false), to, false),
        Narrowing::SignedToUnsigned if crate::planted::is(1011) => {
            saturate(to_integer(value, from, false).abs(), to, true)
        }
        Narrowing::SignedToUnsigned => saturate(to_integer(value, from, false), to, true),
        Narrowing::Unsigned => saturate(to_integer(value, from, true), to, true),
    }
}

fn simulate_across<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    operation: AcrossOperation,
) -> Result<(), Exception> {
    let esize = u8::BITS << decoded.size;
    if decoded.size == 3 || (!decoded.q && decoded.size == 2) {
        return undefined();
    }
    let n = processor.vector_registers[decoded.n];
    let elements = decoded.datasize() / esize;
    let values = (0..elements).map(|index| element(n, index, esize));
    let (value, bits) = match operation {
        AcrossOperation::AddLong(unsigned) => (
            values
                .map(|value| to_integer(value, esize, unsigned))
                .sum::<i128>() as u128,
            if crate::planted::is(1019) {
                esize
            } else {
                2 * esize
            },
        ),
        AcrossOperation::Add => (values.fold(0_u128, u128::wrapping_add), esize),
        AcrossOperation::Maximum(unsigned) => (
            values
                .map(|value| to_integer(value, esize, unsigned || crate::planted::is(1020)))
                .max()
                .unwrap_or(0) as u128,
            esize,
        ),
        AcrossOperation::Minimum(unsigned) => (
            values
                .map(|value| to_integer(value, esize, unsigned))
                .min()
                .unwrap_or(0) as u128,
            esize,
        ),
    };
    write(processor, decoded.d, value, bits);
    Ok(())
}

/// The register and index of the element a by-element instruction of this element size names: H:L:M and a 4-bit Rm
/// for halfwords, H:L and M:Rm for words, H and M:Rm for doublewords.
fn to_element_operand(word: u32, esize: u32) -> (usize, u32) {
    let (h, l, m) = (
        ELEMENT_H.read(word),
        ELEMENT_L.read(word),
        ELEMENT_M.read(word),
    );
    let rm = ELEMENT_RM.read(word) as usize;
    match esize {
        16 if crate::planted::is(1009) => (rm, (l << 1) | m),
        16 => (rm, (h << 2) | (l << 1) | m),
        32 if crate::planted::is(1041) => (rm | ((m as usize) << 4), h << 1),
        32 => (rm | ((m as usize) << 4), (h << 1) | l),
        _ => (rm | ((m as usize) << 4), h),
    }
}

fn simulate_element<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    operation: ElementOperation,
    saturated: &mut bool,
) -> Result<(), Exception> {
    let word = decoded.word;
    if let ElementOperation::Dot { first, second } = operation {
        let (m, index) = to_element_operand(word, u32::BITS);
        let (n_value, m_value) = (
            processor.vector_registers[decoded.n],
            processor.vector_registers[m],
        );
        let d_value = processor.vector_registers[decoded.d];
        let result = (0..decoded.datasize() / u32::BITS).fold(0, |result, lane| {
            let sum = dot(n_value, m_value, lane, index, first, second);
            with_element(
                result,
                lane,
                u32::BITS,
                element(d_value, lane, u32::BITS).wrapping_add(sum as u128),
            )
        });
        write(processor, decoded.d, result, decoded.datasize());
        return Ok(());
    }
    if decoded.size == 0 || decoded.size == 3 {
        return undefined();
    }
    let esize = u8::BITS << decoded.size;
    let (m, index) = to_element_operand(word, esize);
    let second = element(processor.vector_registers[m], index, esize);
    let n = processor.vector_registers[decoded.n];
    let d = processor.vector_registers[decoded.d];
    let long = matches!(
        operation,
        ElementOperation::MultiplyLong { .. } | ElementOperation::SaturatingDoublingMultiplyLong(_)
    );
    let (source, elements) = if decoded.scalar {
        (n, 1)
    } else if long {
        (decoded.part(n), HALF_VECTOR_BITS / esize)
    } else {
        (n, decoded.datasize() / esize)
    };
    let result_size = if long { 2 * esize } else { esize };
    let mut result = 0;
    for lane in 0..elements {
        let first = element(source, lane, esize);
        let accumulator = element(d, lane, result_size);
        let value = match operation {
            ElementOperation::Multiply(accumulate) => {
                let product = first.wrapping_mul(second);
                match accumulate {
                    Accumulate::None => product,
                    Accumulate::Add => accumulator.wrapping_add(product),
                    Accumulate::Subtract => accumulator.wrapping_sub(product),
                }
            }
            ElementOperation::MultiplyLong {
                unsigned,
                accumulate,
            } => multiply_long(first, second, accumulator, esize, unsigned, accumulate),
            ElementOperation::SaturatingDoublingMultiplyLong(accumulate) => {
                let (value, sat) = saturating_doubling_multiply_long(
                    first,
                    second,
                    accumulator,
                    esize,
                    accumulate,
                );
                *saturated |= sat;
                value
            }
            ElementOperation::DoublingMultiplyHigh { rounding } => {
                let (value, sat) = doubling_multiply_high(first, second, esize, rounding);
                *saturated |= sat;
                value
            }
            ElementOperation::RoundingDoublingMultiplyAccumulate { subtract } => {
                let (value, sat) = rounding_doubling_multiply_accumulate(
                    first,
                    second,
                    accumulator,
                    esize,
                    subtract,
                );
                *saturated |= sat;
                value
            }
            ElementOperation::Dot { .. } => unreachable!("taken above"),
        };
        result = with_element(result, lane, result_size, value);
    }
    let datasize = if decoded.scalar {
        result_size
    } else if long {
        VECTOR_BITS
    } else {
        decoded.datasize()
    };
    write(processor, decoded.d, result, datasize);
    Ok(())
}

fn simulate_shift<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    operation: ShiftOperation,
    saturated: &mut bool,
    flags: &mut u64,
) -> Result<(), Exception> {
    let word = decoded.word;
    let immh = IMMH.read(word);
    if immh == 0 {
        return undefined();
    }
    let narrow_or_long = matches!(
        operation,
        ShiftOperation::ShiftRightNarrow { .. } | ShiftOperation::ShiftLeftLong(_)
    );
    if narrow_or_long && immh >= 8 {
        return undefined();
    }
    let esize = u8::BITS << (u32::BITS - 1 - immh.leading_zeros());
    if !decoded.scalar && !decoded.q && esize == u64::BITS && !narrow_or_long {
        return undefined();
    }
    let immediate = IMMH_IMMB.read(word);
    let right_shift = 2 * esize - immediate;
    let left_shift = immediate - esize;
    let n = processor.vector_registers[decoded.n];
    let d = processor.vector_registers[decoded.d];
    match operation {
        ShiftOperation::ShiftRightNarrow {
            rounding,
            narrowing,
        } => {
            let elements = if decoded.scalar {
                1
            } else {
                HALF_VECTOR_BITS / esize
            };
            let result = (0..elements).fold(0, |result, index| {
                let wide = to_integer(
                    element(n, index, 2 * esize),
                    2 * esize,
                    matches!(narrowing, Narrowing::Unsigned | Narrowing::Truncate),
                );
                let shifted = shift_signed(
                    wide,
                    -(right_shift as i32),
                    rounding || crate::planted::is(1015),
                );
                let (value, sat) = match narrowing {
                    Narrowing::Truncate => (shifted as u128 & ones(esize), false),
                    Narrowing::Signed => saturate(shifted, esize, false),
                    Narrowing::SignedToUnsigned | Narrowing::Unsigned => {
                        saturate(shifted, esize, true)
                    }
                };
                *saturated |= sat;
                with_element(result, index, esize, value)
            });
            if decoded.scalar {
                write(processor, decoded.d, result, esize);
            } else {
                write_part(processor, decoded.d, result, decoded.q);
            }
            Ok(())
        }
        ShiftOperation::ShiftLeftLong(unsigned) => {
            let source = decoded.part(n);
            let result = (0..HALF_VECTOR_BITS / esize).fold(0, |result, index| {
                let value = to_integer(
                    element(source, index, esize),
                    esize,
                    unsigned || crate::planted::is(1016),
                ) << left_shift;
                with_element(result, index, 2 * esize, value as u128)
            });
            write(processor, decoded.d, result, VECTOR_BITS);
            Ok(())
        }
        ShiftOperation::ToFixed(unsigned) | ShiftOperation::FromFixed(unsigned) => {
            if esize == u8::BITS {
                return undefined();
            }
            let precision = match esize {
                16 => Precision::Half,
                32 => Precision::Single,
                _ => Precision::Double,
            };
            let lane_operation = if matches!(operation, ShiftOperation::ToFixed(_)) {
                LaneOperation::ToInteger {
                    rounding: Some(Rounding::TowardZero),
                    unsigned,
                    fraction_bits: if crate::planted::is(1059) {
                        left_shift
                    } else {
                        right_shift
                    },
                }
            } else {
                LaneOperation::FromInteger {
                    unsigned,
                    fraction_bits: right_shift,
                }
            };
            let (datasize, elements) = to_same_shape(decoded, esize);
            let fpcr = processor.floating_point_control;
            let result = (0..elements).fold(0, |result, index| {
                let value = element(n, index, esize) as u64;
                let converted = compute_lane(lane_operation, &[value], precision, fpcr, flags);
                with_element(result, index, esize, u128::from(converted))
            });
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
        _ => {
            let (datasize, elements) = to_same_shape(decoded, esize);
            let mut result = 0;
            for index in 0..elements {
                let value = element(n, index, esize);
                let accumulator = element(d, index, esize);
                let lane = match operation {
                    ShiftOperation::ShiftRight {
                        unsigned,
                        rounding,
                        accumulate,
                    } => {
                        let shifted = if crate::planted::is(1012) && right_shift == esize {
                            0
                        } else {
                            shift_signed(
                                to_integer(value, esize, unsigned),
                                -(right_shift as i32),
                                rounding,
                            )
                        } as u128;
                        if accumulate && !crate::planted::is(1013) {
                            accumulator.wrapping_add(shifted)
                        } else {
                            shifted
                        }
                    }
                    ShiftOperation::ShiftLeft => value << left_shift,
                    ShiftOperation::SaturatingShiftLeft { source, result } => {
                        let (value, sat) = saturate(
                            to_integer(value, esize, source) << left_shift,
                            esize,
                            result,
                        );
                        *saturated |= sat;
                        value
                    }
                    ShiftOperation::Insert { left } => {
                        let (mask, shifted) = if left {
                            (ones(esize) << left_shift, value << left_shift)
                        } else if right_shift >= esize && crate::planted::is(1014) {
                            (ones(esize), 0)
                        } else if right_shift >= esize {
                            (0, 0)
                        } else {
                            (ones(esize) >> right_shift, value >> right_shift)
                        };
                        (accumulator & !mask) | (shifted & mask)
                    }
                    _ => unreachable!("taken above"),
                };
                result = with_element(result, index, esize, lane);
            }
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
    }
}

/// [AdvSIMDExpandImm](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-vector#impl-aarch64.AdvSIMDExpandImm.3), and the operation op and cmode give it.
fn simulate_modified_immediate<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    let word = decoded.word;
    let op = IMMEDIATE_OP.read(word);
    let cmode = CMODE.read(word);
    let imm8 = u64::from((ABC.read(word) << 5) | DEFGH.read(word));
    let replicate = |value: u64, bits: u32| {
        (0..u64::BITS / bits).fold(0, |all, place| all | (value << (place * bits)))
    };
    let bit = |index: u32| (imm8 >> index) & 1;
    let imm64 = match cmode >> 1 {
        0b000 => replicate(imm8, 32),
        0b001 => replicate(imm8 << 8, 32),
        0b010 => replicate(imm8 << 16, 32),
        0b011 => replicate(imm8 << 24, 32),
        0b100 => replicate(imm8, 16),
        0b101 => replicate(imm8 << 8, 16),
        0b110 if cmode & 1 == 0 && crate::planted::is(1027) => replicate(imm8 << 8, 32),
        0b110 if cmode & 1 == 0 => replicate((imm8 << 8) | 0xFF, 32),
        0b110 => replicate((imm8 << 16) | 0xFFFF, 32),
        _ => match (cmode & 1, op) {
            (0, 0) => replicate(imm8, 8),
            (0, _) => (0..8).fold(0, |all, index| all | ((bit(index) * 0xFF) << (index * 8))),
            (_, 0) if O2.read(word) == 1 => {
                let imm16 = (bit(7) << 15)
                    | ((bit(6) ^ 1) << 14)
                    | ((bit(6) * 0b11) << 12)
                    | ((imm8 & 0x3F) << 6);
                replicate(imm16, 16)
            }
            (_, 0) => {
                let imm32 = (bit(7) << 31)
                    | ((bit(6) ^ 1) << 30)
                    | ((bit(6) * 0b1_1111) << 25)
                    | ((imm8 & 0x3F) << 19);
                replicate(imm32, 32)
            }
            _ => {
                if !decoded.q {
                    return undefined();
                }
                (bit(7) << 63)
                    | ((bit(6) ^ 1) << 62)
                    | ((bit(6) * 0xFF) << 54)
                    | ((imm8 & 0x3F) << 48)
            }
        },
    };
    let imm = u128::from(imm64) | (u128::from(imm64) << HALF_VECTOR_BITS);
    let logical = cmode < 0b1100 && cmode & 1 == 1;
    let datasize = if cmode == 0b1110 && op == 1 && !decoded.q {
        u64::BITS
    } else {
        decoded.datasize()
    };
    let d = processor.vector_registers[decoded.d];
    let result = match (logical, op) {
        (true, 0) => d | imm,
        (true, _) if crate::planted::is(1028) => d | imm,
        (true, _) => d & !imm,
        (false, 1) if cmode != 0b1110 && cmode != 0b1111 => !imm,
        _ => imm,
    };
    write(processor, decoded.d, result, datasize);
    Ok(())
}

/// The precision a floating-point encoding names: half, or single or double as sz gives it.
fn to_precision(word: u32, field: PrecisionField) -> Precision {
    match field {
        PrecisionField::Half => Precision::Half,
        PrecisionField::Size if SZ.read(word) == 0 => Precision::Single,
        PrecisionField::Size => Precision::Double,
    }
}

fn simulate_floating_point_lanes<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    shape: FloatingPointShape,
    precision: PrecisionField,
    operation: LaneOperation,
    flags: &mut u64,
) -> Result<(), Exception> {
    let word = decoded.word;
    let precision = to_precision(word, precision);
    let esize = precision.to_bits();
    let fpcr = processor.floating_point_control;
    let n = processor.vector_registers[decoded.n];
    let m = processor.vector_registers[decoded.m];
    let d = processor.vector_registers[decoded.d];
    let mut lane = |sources: &[u128]| -> u128 {
        let bits: [u64; 3] =
            core::array::from_fn(|index| sources.get(index).copied().unwrap_or(0) as u64);
        u128::from(compute_lane(
            operation,
            &bits[..sources.len()],
            precision,
            fpcr,
            flags,
        ))
    };
    let vector_double = !decoded.scalar && !decoded.q && esize == u64::BITS;
    match shape {
        FloatingPointShape::ScalarPairwise => {
            let second = if crate::planted::is(1064) { 2 } else { 1 };
            let value = lane(&[element(n, 0, esize), element(n, second, esize)]);
            write(processor, decoded.d, value, esize);
            Ok(())
        }
        FloatingPointShape::Across => {
            if esize == u64::BITS || (esize == u32::BITS && !decoded.q) {
                return undefined();
            }
            let elements = decoded.datasize() / esize;
            let mut values: [u128; 8] =
                core::array::from_fn(|index| element(n, index as u32, esize));
            let mut count = if crate::planted::is(1030) {
                elements as usize / 2
            } else {
                elements as usize
            };
            // Reduce: the operation of the reductions of the lower and the upper half, down to one element.
            while count > 1 {
                count /= 2;
                for index in 0..count {
                    values[index] = lane(&[values[2 * index], values[2 * index + 1]]);
                }
            }
            write(processor, decoded.d, values[0], esize);
            Ok(())
        }
        FloatingPointShape::Pairwise => {
            if vector_double {
                return undefined();
            }
            let datasize = decoded.datasize();
            let elements = datasize / esize;
            let joined = |index: u32| {
                if crate::planted::is(1029) {
                    element(n, index % elements, esize)
                } else if index < elements {
                    element(n, index, esize)
                } else {
                    element(m, index - elements, esize)
                }
            };
            let mut result = 0;
            for index in 0..elements {
                let value = lane(&[joined(2 * index), joined(2 * index + 1)]);
                result = with_element(result, index, esize, value);
            }
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
        _ => {
            if vector_double {
                return undefined();
            }
            let (datasize, elements) = to_same_shape(decoded, esize);
            let by_element = match shape {
                FloatingPointShape::Element { .. } => {
                    if esize == u64::BITS && ELEMENT_L.read(word) == 1 {
                        return undefined();
                    }
                    let (register, index) = to_element_operand(word, esize);
                    Some(element(processor.vector_registers[register], index, esize))
                }
                _ => None,
            };
            let mut result = 0;
            for index in 0..elements {
                let first = element(n, index, esize);
                let second = by_element.unwrap_or_else(|| element(m, index, esize));
                let addend = element(d, index, esize);
                let value = match shape {
                    FloatingPointShape::Misc => lane(&[first]),
                    FloatingPointShape::Same
                    | FloatingPointShape::Element {
                        accumulating: false,
                    } => lane(&[first, second]),
                    _ if crate::planted::is(1042) => {
                        let sign = 1 << (esize - 1);
                        lane(&[first ^ sign, second, addend ^ sign])
                    }
                    _ => lane(&[first, second, addend]),
                };
                result = with_element(result, index, esize, value);
            }
            write(processor, decoded.d, result, datasize);
            Ok(())
        }
    }
}

/// [FCMLA](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/FCMLA--Floating-point-complex-multiply-accumulate-): each pair of
/// lanes a complex number, the product of one part of the first by the second, rotated, added to the destination.
fn simulate_complex_multiply_accumulate<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    by_element: bool,
    flags: &mut u64,
) -> Result<(), Exception> {
    let word = decoded.word;
    let (precision, esize) = match decoded.size {
        1 => (Precision::Half, u16::BITS),
        2 => (Precision::Single, u32::BITS),
        3 if !by_element => (Precision::Double, u64::BITS),
        _ => return undefined(),
    };
    let datasize = decoded.datasize();
    if !decoded.q && esize == u64::BITS {
        return undefined();
    }
    let rotation = if by_element {
        FCMLA_ELEMENT_ROTATION.read(word)
    } else {
        FCMLA_ROTATION.read(word)
    };
    let n = processor.vector_registers[decoded.n];
    let d = processor.vector_registers[decoded.d];
    // By element, the second source is one complex pair: H:L for halves, H for singles, of a register of 4 or 5 bits.
    let element_pair = if by_element {
        let (h, l, m) = (
            ELEMENT_H.read(word),
            ELEMENT_L.read(word),
            ELEMENT_M.read(word),
        );
        let rm = ELEMENT_RM.read(word) as usize;
        let (register, index) = if esize == u16::BITS {
            (rm | ((m as usize) << 4), (h << 1) | l)
        } else {
            if l == 1 || !decoded.q {
                return undefined();
            }
            (rm | ((m as usize) << 4), h)
        };
        if esize == u16::BITS && !decoded.q && h == 1 {
            return undefined();
        }
        Some((processor.vector_registers[register], index))
    } else {
        None
    };
    let m = processor.vector_registers[decoded.m];
    let fpcr = processor.floating_point_control;
    let negate = |value: u128| value ^ (1 << (esize - 1));
    let mut result = 0;
    for pair in 0..datasize / (2 * esize) {
        let (second_real, second_imaginary) = match element_pair {
            Some((register, index)) => (
                element(register, 2 * index, esize),
                element(register, 2 * index + 1, esize),
            ),
            None => (element(m, 2 * pair, esize), element(m, 2 * pair + 1, esize)),
        };
        let first_real = element(n, 2 * pair, esize);
        let first_imaginary = element(n, 2 * pair + 1, esize);
        let (element1, element2, element3, element4) = match rotation {
            0 => (second_real, first_real, second_imaginary, first_real),
            1 if crate::planted::is(1043) => (
                negate(second_imaginary),
                first_real,
                second_real,
                first_real,
            ),
            1 => (
                negate(second_imaginary),
                first_imaginary,
                second_real,
                first_imaginary,
            ),
            2 => (
                negate(second_real),
                first_real,
                negate(second_imaginary),
                first_real,
            ),
            _ => (
                second_imaginary,
                first_imaginary,
                negate(second_real),
                first_imaginary,
            ),
        };
        let fma = |addend: u128, first: u128, second: u128, flags: &mut u64| {
            u128::from(compute_lane(
                LaneOperation::Scalar(FloatingPointOperation::MultiplyAdd {
                    negate_product: false,
                    negate_addend: false,
                }),
                &[first as u64, second as u64, addend as u64],
                precision,
                fpcr,
                flags,
            ))
        };
        let real = fma(element(d, 2 * pair, esize), element2, element1, flags);
        let imaginary = fma(element(d, 2 * pair + 1, esize), element4, element3, flags);
        result = with_element(result, 2 * pair, esize, real);
        result = with_element(result, 2 * pair + 1, esize, imaginary);
    }
    write(processor, decoded.d, result, datasize);
    Ok(())
}

/// [FCADD](https://developer.arm.com/documentation/ddi0602/latest/SIMD-FP-Instructions/FCADD--Floating-point-complex-add-): the second source rotated by 90 or 270
/// degrees, added to the first.
fn simulate_complex_add<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    flags: &mut u64,
) -> Result<(), Exception> {
    let (precision, esize) = match decoded.size {
        1 => (Precision::Half, u16::BITS),
        2 => (Precision::Single, u32::BITS),
        3 => (Precision::Double, u64::BITS),
        _ => return undefined(),
    };
    if !decoded.q && esize == u64::BITS {
        return undefined();
    }
    let datasize = decoded.datasize();
    let rotate_270 = FCADD_ROTATION.read(decoded.word) == 1 && !crate::planted::is(1044);
    let n = processor.vector_registers[decoded.n];
    let m = processor.vector_registers[decoded.m];
    let fpcr = processor.floating_point_control;
    let negate = |value: u128| value ^ (1 << (esize - 1));
    let add = |first: u128, second: u128, flags: &mut u64| {
        u128::from(compute_lane(
            LaneOperation::Scalar(FloatingPointOperation::Add),
            &[first as u64, second as u64],
            precision,
            fpcr,
            flags,
        ))
    };
    let mut result = 0;
    for pair in 0..datasize / (2 * esize) {
        let (second_real, second_imaginary) =
            (element(m, 2 * pair, esize), element(m, 2 * pair + 1, esize));
        let (element1, element3) = if rotate_270 {
            (second_imaginary, negate(second_real))
        } else {
            (negate(second_imaginary), second_real)
        };
        let real = add(element(n, 2 * pair, esize), element1, flags);
        let imaginary = add(element(n, 2 * pair + 1, esize), element3, flags);
        result = with_element(result, 2 * pair, esize, real);
        result = with_element(result, 2 * pair + 1, esize, imaginary);
    }
    write(processor, decoded.d, result, datasize);
    Ok(())
}

/// FMLAL and its forms: each half-precision product, of the lower or upper half of the first source's lanes, added to
/// the single-precision destination with one rounding, as
/// [FPMulAddH](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#:~:text=FPMulAddH) does.
fn simulate_widening_multiply_accumulate<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    decoded: &Decoded,
    subtract: bool,
    upper: bool,
    by_element: bool,
    flags: &mut u64,
) -> Result<(), Exception> {
    let datasize = decoded.datasize();
    let elements = datasize / u32::BITS;
    let n = processor.vector_registers[decoded.n];
    let m = processor.vector_registers[decoded.m];
    let d = processor.vector_registers[decoded.d];
    let offset = if upper || crate::planted::is(1045) {
        elements
    } else {
        0
    };
    let element_value = if by_element {
        let (h, l, mm) = (
            ELEMENT_H.read(decoded.word),
            ELEMENT_L.read(decoded.word),
            ELEMENT_M.read(decoded.word),
        );
        let register = ELEMENT_RM.read(decoded.word) as usize;
        Some(element(
            processor.vector_registers[register],
            (h << 2) | (l << 1) | mm,
            u16::BITS,
        ))
    } else {
        None
    };
    let fpcr = processor.floating_point_control;
    let mut result = 0;
    for index in 0..elements {
        let first = element(n, index + offset, u16::BITS) as u64;
        let first = if subtract { first ^ (1 << 15) } else { first };
        let second = element_value.unwrap_or_else(|| element(m, index + offset, u16::BITS)) as u64;
        let addend = element(d, index, u32::BITS) as u64;
        let value =
            crate::floating_point::lanes::multiply_add_halves(addend, first, second, fpcr, flags);
        result = with_element(result, index, u32::BITS, u128::from(value));
    }
    write(processor, decoded.d, result, datasize);
    Ok(())
}

/// The bugs planted in this module's models, each caught where no state of its instruction agrees with the chip.
pub const ADVANCED_SIMD_PLANTED_BUGS: [crate::planted::PlantedBug; 65] = [
    crate::planted::PlantedBug {
        number: 1001,
        instruction: Instruction::SqaddVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a saturated element leaves QC clear",
    },
    crate::planted::PlantedBug {
        number: 1002,
        instruction: Instruction::UqsubVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a difference below zero wraps instead of saturating",
    },
    crate::planted::PlantedBug {
        number: 1003,
        instruction: Instruction::SqshlVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a left shift of 64 or more wraps instead of saturating",
    },
    crate::planted::PlantedBug {
        number: 1004,
        instruction: Instruction::SqrdmulhVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "no rounding constant",
    },
    crate::planted::PlantedBug {
        number: 1005,
        instruction: Instruction::SqdmlalVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the doubled product is not saturated before it is added",
    },
    crate::planted::PlantedBug {
        number: 1006,
        instruction: Instruction::Addhn,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the low half of each sum",
    },
    crate::planted::PlantedBug {
        number: 1007,
        instruction: Instruction::Raddhn,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "no rounding constant",
    },
    crate::planted::PlantedBug {
        number: 1008,
        instruction: Instruction::Saddw,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the 2 form reads the lower half of the second source",
    },
    crate::planted::PlantedBug {
        number: 1009,
        instruction: Instruction::SmlalAsimdelemL,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a halfword element's index leaves out H",
    },
    crate::planted::PlantedBug {
        number: 1010,
        instruction: Instruction::Xtn,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the 2 form clears the lower half",
    },
    crate::planted::PlantedBug {
        number: 1011,
        instruction: Instruction::SqxtunVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a negative element saturates as its absolute value",
    },
    crate::planted::PlantedBug {
        number: 1012,
        instruction: Instruction::SshrVectorImmediate,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a shift by the element size gives zero for a negative element",
    },
    crate::planted::PlantedBug {
        number: 1013,
        instruction: Instruction::UrsraVectorImmediate,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the destination is not added",
    },
    crate::planted::PlantedBug {
        number: 1014,
        instruction: Instruction::SriVectorImmediate,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a shift by the element size clears the destination",
    },
    crate::planted::PlantedBug {
        number: 1015,
        instruction: Instruction::SqshrnVectorImmediate,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "rounds as SQRSHRN",
    },
    crate::planted::PlantedBug {
        number: 1016,
        instruction: Instruction::Sshll,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the element is extended with zeros",
    },
    crate::planted::PlantedBug {
        number: 1017,
        instruction: Instruction::ClsVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the sign bit is counted too",
    },
    crate::planted::PlantedBug {
        number: 1018,
        instruction: Instruction::Rev32Vector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "reverses inside doublewords",
    },
    crate::planted::PlantedBug {
        number: 1019,
        instruction: Instruction::Uaddlv,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the sum wraps at the element size",
    },
    crate::planted::PlantedBug {
        number: 1020,
        instruction: Instruction::SmaxvScalarV,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "compares the elements unsigned",
    },
    crate::planted::PlantedBug {
        number: 1021,
        instruction: Instruction::TblVectorTwo,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "an index into the second register reads the first",
    },
    crate::planted::PlantedBug {
        number: 1022,
        instruction: Instruction::TbxVectorOne,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "an index past the table writes zero",
    },
    crate::planted::PlantedBug {
        number: 1023,
        instruction: Instruction::ExtVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the position is one byte further",
    },
    crate::planted::PlantedBug {
        number: 1024,
        instruction: Instruction::DupVectorElement,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the element one below the index",
    },
    crate::planted::PlantedBug {
        number: 1025,
        instruction: Instruction::SmovScalar32,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the element is extended with zeros",
    },
    crate::planted::PlantedBug {
        number: 1026,
        instruction: Instruction::InsVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the source index is imm4 whatever the element size",
    },
    crate::planted::PlantedBug {
        number: 1027,
        instruction: Instruction::MoviVectorSMsl,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the bits shifted in are zeros",
    },
    crate::planted::PlantedBug {
        number: 1028,
        instruction: Instruction::BicVectorImmediate32,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "sets the immediate's bits as ORR does",
    },
    crate::planted::PlantedBug {
        number: 1029,
        instruction: Instruction::FaddpVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "every pair is taken from the first source",
    },
    crate::planted::PlantedBug {
        number: 1030,
        instruction: Instruction::FmaxnmvScalarRegisterV32,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "only the lower half is reduced",
    },
    crate::planted::PlantedBug {
        number: 1031,
        instruction: Instruction::FmulxVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "an infinity times a zero is the default NaN",
    },
    crate::planted::PlantedBug {
        number: 1032,
        instruction: Instruction::FrecpsVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the first source is not negated",
    },
    crate::planted::PlantedBug {
        number: 1033,
        instruction: Instruction::FcmgeVectorRegisterZero,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a quiet NaN raises no Invalid Operation",
    },
    crate::planted::PlantedBug {
        number: 1034,
        instruction: Instruction::FacgtVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "compares the signed values",
    },
    crate::planted::PlantedBug {
        number: 1035,
        instruction: Instruction::FrecpeVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the estimate is not rounded",
    },
    crate::planted::PlantedBug {
        number: 1036,
        instruction: Instruction::FrsqrteVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the exponent's parity is taken the other way round",
    },
    crate::planted::PlantedBug {
        number: 1037,
        instruction: Instruction::FcvtzsVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "an out-of-range value raises no Invalid Operation",
    },
    crate::planted::PlantedBug {
        number: 1038,
        instruction: Instruction::ScvtfVectorRegister16,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the integer is read unsigned",
    },
    crate::planted::PlantedBug {
        number: 1039,
        instruction: Instruction::FcvtlVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the lower form reads the upper half",
    },
    crate::planted::PlantedBug {
        number: 1040,
        instruction: Instruction::FcvtxnVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "rounds to nearest instead of to odd",
    },
    crate::planted::PlantedBug {
        number: 1041,
        instruction: Instruction::FmlaVectorRegisterElement,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a single element's index leaves out L",
    },
    crate::planted::PlantedBug {
        number: 1042,
        instruction: Instruction::FmlsVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "negates the addend instead of the first source",
    },
    crate::planted::PlantedBug {
        number: 1043,
        instruction: Instruction::FcmlaAsimdsame2C,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a rotation of 90 degrees multiplies the real part of the first source",
    },
    crate::planted::PlantedBug {
        number: 1044,
        instruction: Instruction::FcaddAsimdsame2C,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the rotation is always 90 degrees",
    },
    crate::planted::PlantedBug {
        number: 1045,
        instruction: Instruction::FmlalVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "every form takes the upper lanes",
    },
    crate::planted::PlantedBug {
        number: 1046,
        instruction: Instruction::FcvtnVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the 2 form writes the lower half",
    },
    crate::planted::PlantedBug {
        number: 1047,
        instruction: Instruction::UrecpeVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "an element of 0x7FFFFFFF takes an estimate",
    },
    crate::planted::PlantedBug {
        number: 1048,
        instruction: Instruction::UrsqrteVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "an element of 0x3FFFFFFF takes an estimate",
    },
    crate::planted::PlantedBug {
        number: 1049,
        instruction: Instruction::SdotVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the products are unsigned",
    },
    crate::planted::PlantedBug {
        number: 1050,
        instruction: Instruction::UsmmlaVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the second matrix is read unsigned",
    },
    crate::planted::PlantedBug {
        number: 1051,
        instruction: Instruction::Bif,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "selects as BIT does",
    },
    crate::planted::PlantedBug {
        number: 1052,
        instruction: Instruction::SqabsVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the most negative element wraps",
    },
    crate::planted::PlantedBug {
        number: 1053,
        instruction: Instruction::SuqaddVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the source is read signed",
    },
    crate::planted::PlantedBug {
        number: 1054,
        instruction: Instruction::SadalpVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the destination is not added",
    },
    crate::planted::PlantedBug {
        number: 1055,
        instruction: Instruction::CntVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "counts the clear bits",
    },
    crate::planted::PlantedBug {
        number: 1056,
        instruction: Instruction::PmullAsimddiffL,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the 128-bit product keeps its low half",
    },
    crate::planted::PlantedBug {
        number: 1057,
        instruction: Instruction::SqrdmlahVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "no rounding constant",
    },
    crate::planted::PlantedBug {
        number: 1058,
        instruction: Instruction::ShllAsimdmiscS,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "shifts by one less than the element size",
    },
    crate::planted::PlantedBug {
        number: 1059,
        instruction: Instruction::FcvtzsVectorImmediateImmediate,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the fraction bits are immh:immb less the element size",
    },
    crate::planted::PlantedBug {
        number: 1060,
        instruction: Instruction::BfdotVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the sums round toward zero, not to odd",
    },
    crate::planted::PlantedBug {
        number: 1061,
        instruction: Instruction::BfcvtnVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "rounds toward zero whatever FPCR says",
    },
    crate::planted::PlantedBug {
        number: 1062,
        instruction: Instruction::BfmlalAsimdsame2F,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "BFMLALT takes the even lanes",
    },
    crate::planted::PlantedBug {
        number: 1063,
        instruction: Instruction::BfmmlaVector,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the second matrix is read by columns",
    },
    crate::planted::PlantedBug {
        number: 1064,
        instruction: Instruction::FmaxpScalarRegisterP16,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "the second lane is lane 2",
    },
    crate::planted::PlantedBug {
        number: 1065,
        instruction: Instruction::FabdVectorRegister,
        planted_by: crate::planted::PlantedBy::AdvancedSimd,
        what: "a NaN result keeps its sign",
    },
];
