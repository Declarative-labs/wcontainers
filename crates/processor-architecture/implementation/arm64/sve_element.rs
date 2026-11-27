//! The SVE integer and bitwise instructions that compute each element of a Z register from the same elements of
//! others, predicated or not, with an immediate or a shift: for example [SQADD (vectors, unpredicated)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/SQADD--vectors--unpredicated---Signed-saturating-add-vectors--unpredicated--).

use core::convert::Infallible;

use crate::encoding::Field;
use crate::floating_point::{Precision, expand_immediate};
use crate::integer::{BitmaskUse, decode_bit_masks};
use crate::operand::{RegisterWidth, Signedness};
use crate::sve::{
    PG3, PredicateValues, RD5, RM5, RN5, SIZE, ScalableInstruction, ScalableOperand,
    ScalableOperation, VectorValues, count_elements, is_active, read_element, saturate,
    to_element_bits, to_integer, to_mask, write_element,
};
use crate::{Exception, Instruction, Processor};

/// The governing predicate of CPY and FCPY, which may name any predicate register.
const COPY_PREDICATE: Field = Field::new(16, 4);
/// sh: whether an 8-bit immediate is shifted left by 8 bits.
const IMMEDIATE_SHIFT: Field = Field::new(13, 1);
const IMMEDIATE: Field = Field::new(5, 8);
/// M of CPY (immediate): whether inactive elements keep their value instead of being cleared.
const COPY_MERGING: Field = Field::new(14, 1);
/// M of MOVPRFX (predicated).
const PREFIX_MERGING: Field = Field::new(16, 1);
const BITMASK: Field = Field::new(5, 13);
/// tsz and imm3 of a shift by an immediate, the most significant first, predicated and not.
const PREDICATED_SHIFT: &[Field] = &[Field::new(22, 2), Field::new(5, 5)];
const UNPREDICATED_SHIFT: &[Field] = &[Field::new(22, 2), Field::new(16, 5)];
/// Bits of the imm3 below tsz in a shift by an immediate.
const SHIFT_FRACTION_BITS: u32 = 3;
/// Bits of the immr and imms of a bitmask immediate, above which it has N.
const BITMASK_PART_BITS: u32 = 6;
/// Bits of a fraction an unsigned estimate reads and gives.
const ESTIMATE_BITS: u32 = 9;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum BinaryOperation {
    Add,
    Subtract,
    Maximum(Signedness),
    Minimum(Signedness),
    AbsoluteDifference(Signedness),
    Multiply,
    MultiplyHigh(Signedness),
    /// Rounded toward zero; a division by zero gives zero.
    Divide(Signedness),
    Or,
    ExclusiveOr,
    And,
    AndNot,
    /// By the second element's whole unsigned value.
    Shift(ShiftDirection),
    SaturatingAdd(Signedness),
    SaturatingSubtract(Signedness),
    /// SUQADD: a signed element plus an unsigned one, saturated as signed.
    SignedAddUnsigned,
    /// USQADD: an unsigned element plus a signed one, saturated as unsigned.
    UnsignedAddSigned,
    HalvingAdd(Signedness),
    HalvingSubtract(Signedness),
    RoundingHalvingAdd(Signedness),
    /// By the second element as a signed number: left where it is positive, right where it is negative.
    RoundingShift(Signedness),
    SaturatingShift(Signedness),
    SaturatingRoundingShift(Signedness),
    PolynomialMultiply,
    /// SQDMULH and SQRDMULH: the high half of twice the product, saturated.
    DoublingMultiplyHigh {
        rounding: bool,
    },
    /// ADDP, SMAXP and their kin: an even element from a pair of the first operand, an odd one from the second's.
    Pairwise(Pairwise),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum Pairwise {
    Add,
    Maximum(Signedness),
    Minimum(Signedness),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum ShiftDirection {
    ArithmeticRight,
    LogicalRight,
    Left,
}

/// Where the second operand of a binary operation comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum BinaryForm {
    /// A second vector under a governing predicate, inactive elements keeping the first's value.
    Predicated,
    /// As predicated, the second's doubleword element holding the element.
    PredicatedWide,
    Unpredicated,
    UnpredicatedWide,
    /// An 8-bit immediate, unsigned and shifted left by 0 or 8 bits.
    ShiftedImmediate,
    SignedImmediate,
    UnsignedImmediate,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum UnaryOperation {
    Absolute,
    Negate,
    SignExtend(u32),
    ZeroExtend(u32),
    CountLeadingSigns,
    CountLeadingZeros,
    CountOnes,
    /// CNOT: one where the element is zero, zero elsewhere.
    LogicalNot,
    /// FABS and FNEG: the sign bit cleared or flipped.
    ClearSign,
    FlipSign,
    Not,
    /// REVB, REVH and REVW: the parts of these bits reversed within each element.
    ReverseParts(u32),
    ReverseBits,
    SaturatingAbsolute,
    SaturatingNegate,
    /// URECPE and URSQRTE of words.
    ReciprocalEstimate,
    ReciprocalSquareRootEstimate,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum ImmediateShift {
    ArithmeticRight,
    LogicalRight,
    Left,
    /// ASRD: an arithmetic shift right rounded toward zero, a division by a power of two.
    DivideRight,
    SaturatingLeft(Signedness),
    /// SQSHLU: a signed element shifted left, saturated as unsigned.
    SaturatingLeftUnsigned,
    RoundingRight(Signedness),
}

/// The bitwise operations of three registers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum TernaryLogic {
    ExclusiveOrOfThree,
    /// BCAX: the first exclusive-or the second and the third's complement.
    BitClearExclusiveOr,
    /// BSL: the first where the third is set, the second elsewhere.
    Select,
    SelectFirstInverted,
    SelectSecondInverted,
    SelectInverted,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum ElementOperation {
    Binary {
        operation: BinaryOperation,
        form: BinaryForm,
        /// The operands taken the other way round: SUBR, DIVR, LSLR and their kin.
        swapped: bool,
    },
    /// Under a governing predicate, inactive elements keeping the destination's value.
    Unary(UnaryOperation),
    ShiftImmediate {
        shift: ImmediateShift,
        predicated: bool,
    },
    Ternary(TernaryLogic),
    /// XAR: the exclusive-or of two vectors, each element rotated right.
    RotateExclusiveOr,
    /// ORR, EOR and AND of a bitmask immediate, or DUPM where none.
    Bitmask(Option<BinaryOperation>),
    DuplicateImmediate,
    DuplicateFloatingPoint,
    CopyImmediate,
    CopyFloatingPoint,
    /// MOVPRFX, under a governing predicate or not.
    Prefix {
        predicated: bool,
    },
    /// SADALP and UADALP: each element plus the sum of the two half-width elements of the source it covers.
    PairwiseAccumulate(Signedness),
    /// SSRA, USRA, SRSRA and URSRA: each element plus the source's shifted right.
    ShiftAccumulate(ImmediateShift),
    /// SRI and SLI: the source shifted into each element, which keeps its bits the shift leaves empty.
    ShiftInsert {
        left: bool,
    },
    /// SABA and UABA: each element plus the absolute difference of the two sources'.
    AbsoluteDifferenceAccumulate(Signedness),
    /// EORBT where `even`, the even elements from the first source's even and the second's odd; EORTB the odd ones.
    InterleavedExclusiveOr {
        even: bool,
    },
    /// MLA and MLS, which add to the destination, and MAD and MSB, which multiply it, under a governing predicate.
    MultiplyAdd {
        subtract: bool,
        multiplies_destination: bool,
    },
}

const fn vector(field: Field, values: VectorValues) -> ScalableOperand {
    ScalableOperand::Vector {
        field,
        values,
        pair: false,
    }
}

const GOVERNING: ScalableOperand = ScalableOperand::Predicate {
    field: PG3,
    values: PredicateValues::Governing,
};

use ScalableOperand::{ElementSize, Selector};
use VectorValues::{Data, ShiftAmounts, Written};

const PREDICATED: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
];
const PREDICATED_SHIFT_BY_VECTOR: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    vector(RN5, ShiftAmounts),
    ElementSize(SIZE),
];
const UNPREDICATED: &[ScalableOperand] = &[
    vector(RD5, Written),
    vector(RN5, Data),
    vector(RM5, Data),
    ElementSize(SIZE),
];
const UNPREDICATED_SHIFT_BY_VECTOR: &[ScalableOperand] = &[
    vector(RD5, Written),
    vector(RN5, Data),
    vector(RM5, ShiftAmounts),
    ElementSize(SIZE),
];
const UNPREDICATED_FIXED_SIZE: &[ScalableOperand] =
    &[vector(RD5, Written), vector(RN5, Data), vector(RM5, Data)];
const UNARY: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
];
const UNARY_FIXED_SIZE: &[ScalableOperand] = &[vector(RD5, Data), GOVERNING, vector(RN5, Data)];
const SHIFT_PREDICATED: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    ScalableOperand::SplitImmediate(PREDICATED_SHIFT),
];
const SHIFT_UNPREDICATED: &[ScalableOperand] = &[
    vector(RD5, Written),
    vector(RN5, Data),
    ScalableOperand::SplitImmediate(UNPREDICATED_SHIFT),
];
const TERNARY: &[ScalableOperand] = &[vector(RD5, Data), vector(RM5, Data), vector(RN5, Data)];
const ROTATE: &[ScalableOperand] = &[
    vector(RD5, Data),
    vector(RN5, Data),
    ScalableOperand::SplitImmediate(UNPREDICATED_SHIFT),
];
const SHIFTED_IMMEDIATE: &[ScalableOperand] = &[
    vector(RD5, Data),
    ElementSize(SIZE),
    Selector(IMMEDIATE_SHIFT),
    ScalableOperand::UnsignedImmediate(IMMEDIATE),
];
const SIGNED_IMMEDIATE: &[ScalableOperand] = &[
    vector(RD5, Data),
    ElementSize(SIZE),
    ScalableOperand::SignedImmediate(IMMEDIATE),
];
const UNSIGNED_IMMEDIATE: &[ScalableOperand] = &[
    vector(RD5, Data),
    ElementSize(SIZE),
    ScalableOperand::UnsignedImmediate(IMMEDIATE),
];
const BITMASK_OPERATION: &[ScalableOperand] =
    &[vector(RD5, Data), ScalableOperand::Bitmask(BITMASK)];
const BITMASK_DUPLICATE: &[ScalableOperand] =
    &[vector(RD5, Written), ScalableOperand::Bitmask(BITMASK)];
const DUPLICATE_IMMEDIATE: &[ScalableOperand] = &[
    vector(RD5, Written),
    ElementSize(SIZE),
    Selector(IMMEDIATE_SHIFT),
    ScalableOperand::SignedImmediate(IMMEDIATE),
];
const DUPLICATE_FLOATING_POINT: &[ScalableOperand] = &[
    vector(RD5, Written),
    ElementSize(SIZE),
    ScalableOperand::UnsignedImmediate(IMMEDIATE),
];
const COPY_IMMEDIATE: &[ScalableOperand] = &[
    vector(RD5, Data),
    ScalableOperand::Predicate {
        field: COPY_PREDICATE,
        values: PredicateValues::Governing,
    },
    ElementSize(SIZE),
    Selector(IMMEDIATE_SHIFT),
    ScalableOperand::SignedImmediate(IMMEDIATE),
];
const COPY_FLOATING_POINT: &[ScalableOperand] = &[
    vector(RD5, Data),
    ScalableOperand::Predicate {
        field: COPY_PREDICATE,
        values: PredicateValues::Governing,
    },
    ElementSize(SIZE),
    ScalableOperand::UnsignedImmediate(IMMEDIATE),
];
const PREFIX: &[ScalableOperand] = &[vector(RD5, Written), vector(RN5, Data)];
const SHIFT_INTO: &[ScalableOperand] = &[
    vector(RD5, Data),
    vector(RN5, Data),
    ScalableOperand::SplitImmediate(UNPREDICATED_SHIFT),
];
const ACCUMULATE: &[ScalableOperand] = &[
    vector(RD5, Data),
    vector(RN5, Data),
    vector(RM5, Data),
    ElementSize(SIZE),
];
const MULTIPLY_ADD: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    vector(RN5, Data),
    vector(RM5, Data),
    ElementSize(SIZE),
];
const MULTIPLY_DESTINATION: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    vector(RM5, Data),
    vector(RN5, Data),
    ElementSize(SIZE),
];
const PREFIX_PREDICATED: &[ScalableOperand] = &[
    vector(RD5, Data),
    GOVERNING,
    vector(RN5, Data),
    ElementSize(SIZE),
    Selector(PREFIX_MERGING),
];

/// The model of each instruction of this kind, none for every other instruction.
pub(crate) fn to_element_instruction(
    instruction: Instruction<Infallible>,
) -> Option<ScalableInstruction> {
    use BinaryForm::{
        Predicated, PredicatedWide, ShiftedImmediate, Unpredicated, UnpredicatedWide,
    };
    use BinaryOperation as B;
    use ElementOperation as E;
    use Instruction as I;
    use ShiftDirection::{ArithmeticRight, Left, LogicalRight};
    use Signedness::{Signed, Unsigned};
    use UnaryOperation as U;
    let binary = |operation, form, swapped| {
        let operands = match (form, operation) {
            (
                Predicated | PredicatedWide,
                B::Shift(_)
                | B::RoundingShift(_)
                | B::SaturatingShift(_)
                | B::SaturatingRoundingShift(_),
            ) => PREDICATED_SHIFT_BY_VECTOR,
            (Predicated | PredicatedWide, _) => PREDICATED,
            (Unpredicated | UnpredicatedWide, B::Shift(_)) => UNPREDICATED_SHIFT_BY_VECTOR,
            (Unpredicated, B::And | B::Or | B::ExclusiveOr | B::AndNot | B::PolynomialMultiply) => {
                UNPREDICATED_FIXED_SIZE
            }
            (Unpredicated | UnpredicatedWide, _) => UNPREDICATED,
            (ShiftedImmediate, _) => SHIFTED_IMMEDIATE,
            (BinaryForm::SignedImmediate, _) => SIGNED_IMMEDIATE,
            (BinaryForm::UnsignedImmediate, _) => UNSIGNED_IMMEDIATE,
        };
        (
            E::Binary {
                operation,
                form,
                swapped,
            },
            operands,
        )
    };
    let unary = |operation| (E::Unary(operation), UNARY);
    let fixed = |operation| (E::Unary(operation), UNARY_FIXED_SIZE);
    let shift = |shift, predicated| {
        (
            E::ShiftImmediate { shift, predicated },
            if predicated {
                SHIFT_PREDICATED
            } else {
                SHIFT_UNPREDICATED
            },
        )
    };
    let ternary = |logic| (E::Ternary(logic), TERNARY);
    let doubling = |rounding| B::DoublingMultiplyHigh { rounding };
    let (operation, operands) = match instruction {
        I::AddZPmZ => binary(B::Add, Predicated, false),
        I::SubZPmZ => binary(B::Subtract, Predicated, false),
        I::SubrZPmZ => binary(B::Subtract, Predicated, true),
        I::SmaxZPmZ => binary(B::Maximum(Signed), Predicated, false),
        I::SminZPmZ => binary(B::Minimum(Signed), Predicated, false),
        I::SabdZPmZ => binary(B::AbsoluteDifference(Signed), Predicated, false),
        I::UmaxZPmZ => binary(B::Maximum(Unsigned), Predicated, false),
        I::UminZPmZ => binary(B::Minimum(Unsigned), Predicated, false),
        I::UabdZPmZ => binary(B::AbsoluteDifference(Unsigned), Predicated, false),
        I::MulZPmZ => binary(B::Multiply, Predicated, false),
        I::SmulhZPmZ => binary(B::MultiplyHigh(Signed), Predicated, false),
        I::UmulhZPmZ => binary(B::MultiplyHigh(Unsigned), Predicated, false),
        I::SdivZPmZ => binary(B::Divide(Signed), Predicated, false),
        I::Sdivr => binary(B::Divide(Signed), Predicated, true),
        I::UdivZPmZ => binary(B::Divide(Unsigned), Predicated, false),
        I::Udivr => binary(B::Divide(Unsigned), Predicated, true),
        I::OrrZPmZ => binary(B::Or, Predicated, false),
        I::EorZPmZ => binary(B::ExclusiveOr, Predicated, false),
        I::AndZPmZ => binary(B::And, Predicated, false),
        I::BicZPmZ => binary(B::AndNot, Predicated, false),
        I::AsrZPmZ => binary(B::Shift(ArithmeticRight), Predicated, false),
        I::LslZPmZ => binary(B::Shift(Left), Predicated, false),
        I::LsrZPmZ => binary(B::Shift(LogicalRight), Predicated, false),
        I::Asrr => binary(B::Shift(ArithmeticRight), Predicated, true),
        I::Lslr => binary(B::Shift(Left), Predicated, true),
        I::Lsrr => binary(B::Shift(LogicalRight), Predicated, true),
        I::AsrWIDEZPmZ => binary(B::Shift(ArithmeticRight), PredicatedWide, false),
        I::LslWIDEZPmZ => binary(B::Shift(Left), PredicatedWide, false),
        I::LsrWIDEZPmZ => binary(B::Shift(LogicalRight), PredicatedWide, false),
        I::SrshlZPmZ => binary(B::RoundingShift(Signed), Predicated, false),
        I::Srshlr => binary(B::RoundingShift(Signed), Predicated, true),
        I::UrshlZPmZ => binary(B::RoundingShift(Unsigned), Predicated, false),
        I::Urshlr => binary(B::RoundingShift(Unsigned), Predicated, true),
        I::SqshlZPmZ => binary(B::SaturatingShift(Signed), Predicated, false),
        I::Sqshlr => binary(B::SaturatingShift(Signed), Predicated, true),
        I::UqshlZPmZ => binary(B::SaturatingShift(Unsigned), Predicated, false),
        I::Uqshlr => binary(B::SaturatingShift(Unsigned), Predicated, true),
        I::SqrshlZPmZ => binary(B::SaturatingRoundingShift(Signed), Predicated, false),
        I::Sqrshlr => binary(B::SaturatingRoundingShift(Signed), Predicated, true),
        I::UqrshlZPmZ => binary(B::SaturatingRoundingShift(Unsigned), Predicated, false),
        I::Uqrshlr => binary(B::SaturatingRoundingShift(Unsigned), Predicated, true),
        I::ShaddZPmZ => binary(B::HalvingAdd(Signed), Predicated, false),
        I::UhaddZPmZ => binary(B::HalvingAdd(Unsigned), Predicated, false),
        I::ShsubZPmZ => binary(B::HalvingSubtract(Signed), Predicated, false),
        I::UhsubZPmZ => binary(B::HalvingSubtract(Unsigned), Predicated, false),
        I::Shsubr => binary(B::HalvingSubtract(Signed), Predicated, true),
        I::Uhsubr => binary(B::HalvingSubtract(Unsigned), Predicated, true),
        I::SrhaddZPmZ => binary(B::RoundingHalvingAdd(Signed), Predicated, false),
        I::UrhaddZPmZ => binary(B::RoundingHalvingAdd(Unsigned), Predicated, false),
        I::AddpZPmZ => binary(B::Pairwise(Pairwise::Add), Predicated, false),
        I::SmaxpZPmZ => binary(B::Pairwise(Pairwise::Maximum(Signed)), Predicated, false),
        I::SminpZPmZ => binary(B::Pairwise(Pairwise::Minimum(Signed)), Predicated, false),
        I::UmaxpZPmZ => binary(B::Pairwise(Pairwise::Maximum(Unsigned)), Predicated, false),
        I::UminpZPmZ => binary(B::Pairwise(Pairwise::Minimum(Unsigned)), Predicated, false),
        I::SqaddZPmZ => binary(B::SaturatingAdd(Signed), Predicated, false),
        I::UqaddZPmZ => binary(B::SaturatingAdd(Unsigned), Predicated, false),
        I::SqsubZPmZ => binary(B::SaturatingSubtract(Signed), Predicated, false),
        I::UqsubZPmZ => binary(B::SaturatingSubtract(Unsigned), Predicated, false),
        I::Sqsubr => binary(B::SaturatingSubtract(Signed), Predicated, true),
        I::Uqsubr => binary(B::SaturatingSubtract(Unsigned), Predicated, true),
        I::SuqaddZPmZ => binary(B::SignedAddUnsigned, Predicated, false),
        I::UsqaddZPmZ => binary(B::UnsignedAddSigned, Predicated, false),
        I::AddZZZ => binary(B::Add, Unpredicated, false),
        I::SubZZZ => binary(B::Subtract, Unpredicated, false),
        I::SqaddZZZ => binary(B::SaturatingAdd(Signed), Unpredicated, false),
        I::SqsubZZZ => binary(B::SaturatingSubtract(Signed), Unpredicated, false),
        I::UqaddZZZ => binary(B::SaturatingAdd(Unsigned), Unpredicated, false),
        I::UqsubZZZ => binary(B::SaturatingSubtract(Unsigned), Unpredicated, false),
        I::AndZZZ => binary(B::And, Unpredicated, false),
        I::OrrZZZ => binary(B::Or, Unpredicated, false),
        I::EorZZZ => binary(B::ExclusiveOr, Unpredicated, false),
        I::BicZZZ => binary(B::AndNot, Unpredicated, false),
        I::AsrWIDEZZZ => binary(B::Shift(ArithmeticRight), UnpredicatedWide, false),
        I::LslWIDEZZZ => binary(B::Shift(Left), UnpredicatedWide, false),
        I::LsrWIDEZZZ => binary(B::Shift(LogicalRight), UnpredicatedWide, false),
        I::MulZZZ => binary(B::Multiply, Unpredicated, false),
        I::PmulZZZ8 => binary(B::PolynomialMultiply, Unpredicated, false),
        I::SmulhZZZ => binary(B::MultiplyHigh(Signed), Unpredicated, false),
        I::UmulhZZZ => binary(B::MultiplyHigh(Unsigned), Unpredicated, false),
        I::SqdmulhZZZ => binary(doubling(false), Unpredicated, false),
        I::SqrdmulhZZZ => binary(doubling(true), Unpredicated, false),
        I::AddZI => binary(B::Add, ShiftedImmediate, false),
        I::SubZI => binary(B::Subtract, ShiftedImmediate, false),
        I::SubrZI => binary(B::Subtract, ShiftedImmediate, true),
        I::SqaddZI => binary(B::SaturatingAdd(Signed), ShiftedImmediate, false),
        I::SqsubZI => binary(B::SaturatingSubtract(Signed), ShiftedImmediate, false),
        I::UqaddZI => binary(B::SaturatingAdd(Unsigned), ShiftedImmediate, false),
        I::UqsubZI => binary(B::SaturatingSubtract(Unsigned), ShiftedImmediate, false),
        I::SmaxZI => binary(B::Maximum(Signed), BinaryForm::SignedImmediate, false),
        I::SminZI => binary(B::Minimum(Signed), BinaryForm::SignedImmediate, false),
        I::UmaxZI => binary(B::Maximum(Unsigned), BinaryForm::UnsignedImmediate, false),
        I::UminZI => binary(B::Minimum(Unsigned), BinaryForm::UnsignedImmediate, false),
        I::MulZI => binary(B::Multiply, BinaryForm::SignedImmediate, false),
        I::AbsZPmZ => unary(U::Absolute),
        I::NegZPmZ => unary(U::Negate),
        I::SxtbZPmZ => unary(U::SignExtend(u8::BITS)),
        I::SxthZPmZ => unary(U::SignExtend(u16::BITS)),
        I::SxtwZPmZ64 => fixed(U::SignExtend(u32::BITS)),
        I::UxtbZPmZ => unary(U::ZeroExtend(u8::BITS)),
        I::UxthZPmZ => unary(U::ZeroExtend(u16::BITS)),
        I::UxtwZPmZ64 => fixed(U::ZeroExtend(u32::BITS)),
        I::ClsZPmZ => unary(U::CountLeadingSigns),
        I::ClzZPmZ => unary(U::CountLeadingZeros),
        I::CntZPmZ => unary(U::CountOnes),
        I::CnotZPmZ => unary(U::LogicalNot),
        I::FabsZPmZ => unary(U::ClearSign),
        I::FnegZPmZ => unary(U::FlipSign),
        I::NotZPmZ => unary(U::Not),
        I::RevbZPmZ => unary(U::ReverseParts(u8::BITS)),
        I::RevhZPmZ => unary(U::ReverseParts(u16::BITS)),
        I::RevwZPmZ64 => fixed(U::ReverseParts(u32::BITS)),
        I::RbitZPmZ => unary(U::ReverseBits),
        I::SqabsZPmZ => unary(U::SaturatingAbsolute),
        I::SqnegZPmZ => unary(U::SaturatingNegate),
        I::UrecpeZPmZ32 => fixed(U::ReciprocalEstimate),
        I::UrsqrteZPmZ32 => fixed(U::ReciprocalSquareRootEstimate),
        I::AsrZPmI => shift(ImmediateShift::ArithmeticRight, true),
        I::LsrZPmI => shift(ImmediateShift::LogicalRight, true),
        I::LslZPmI => shift(ImmediateShift::Left, true),
        I::Asrd => shift(ImmediateShift::DivideRight, true),
        I::SqshlZPmI => shift(ImmediateShift::SaturatingLeft(Signed), true),
        I::UqshlZPmI => shift(ImmediateShift::SaturatingLeft(Unsigned), true),
        I::SqshluZPmI => shift(ImmediateShift::SaturatingLeftUnsigned, true),
        I::SrshrZPmI => shift(ImmediateShift::RoundingRight(Signed), true),
        I::UrshrZPmI => shift(ImmediateShift::RoundingRight(Unsigned), true),
        I::AsrZZI => shift(ImmediateShift::ArithmeticRight, false),
        I::LsrZZI => shift(ImmediateShift::LogicalRight, false),
        I::LslZZI => shift(ImmediateShift::Left, false),
        I::Eor3ZZZZ => ternary(TernaryLogic::ExclusiveOrOfThree),
        I::BcaxZZZZ => ternary(TernaryLogic::BitClearExclusiveOr),
        I::BslZZZZ => ternary(TernaryLogic::Select),
        I::Bsl1n => ternary(TernaryLogic::SelectFirstInverted),
        I::Bsl2n => ternary(TernaryLogic::SelectSecondInverted),
        I::Nbsl => ternary(TernaryLogic::SelectInverted),
        I::XarZZZI => (E::RotateExclusiveOr, ROTATE),
        I::OrrZI => (E::Bitmask(Some(B::Or)), BITMASK_OPERATION),
        I::EorZI => (E::Bitmask(Some(B::ExclusiveOr)), BITMASK_OPERATION),
        I::AndZI => (E::Bitmask(Some(B::And)), BITMASK_OPERATION),
        I::Dupm => (E::Bitmask(None), BITMASK_DUPLICATE),
        I::DupZI => (E::DuplicateImmediate, DUPLICATE_IMMEDIATE),
        I::Fdup => (E::DuplicateFloatingPoint, DUPLICATE_FLOATING_POINT),
        I::CpyZPzI | I::CpyZPmI => (E::CopyImmediate, COPY_IMMEDIATE),
        I::Fcpy => (E::CopyFloatingPoint, COPY_FLOATING_POINT),
        I::MovprfxZZ => (E::Prefix { predicated: false }, PREFIX),
        I::MovprfxZPZ => (E::Prefix { predicated: true }, PREFIX_PREDICATED),
        I::SadalpZPmZ => (E::PairwiseAccumulate(Signed), PREDICATED),
        I::UadalpZPmZ => (E::PairwiseAccumulate(Unsigned), PREDICATED),
        I::SsraZZI => (
            E::ShiftAccumulate(ImmediateShift::ArithmeticRight),
            SHIFT_INTO,
        ),
        I::UsraZZI => (E::ShiftAccumulate(ImmediateShift::LogicalRight), SHIFT_INTO),
        I::SrsraZZI => (
            E::ShiftAccumulate(ImmediateShift::RoundingRight(Signed)),
            SHIFT_INTO,
        ),
        I::UrsraZZI => (
            E::ShiftAccumulate(ImmediateShift::RoundingRight(Unsigned)),
            SHIFT_INTO,
        ),
        I::SriZZI => (E::ShiftInsert { left: false }, SHIFT_INTO),
        I::SliZZI => (E::ShiftInsert { left: true }, SHIFT_INTO),
        I::SabaZZZ => (E::AbsoluteDifferenceAccumulate(Signed), ACCUMULATE),
        I::UabaZZZ => (E::AbsoluteDifferenceAccumulate(Unsigned), ACCUMULATE),
        I::Eorbt => (E::InterleavedExclusiveOr { even: true }, ACCUMULATE),
        I::Eortb => (E::InterleavedExclusiveOr { even: false }, ACCUMULATE),
        I::MlaZPmZZ => (
            E::MultiplyAdd {
                subtract: false,
                multiplies_destination: false,
            },
            MULTIPLY_ADD,
        ),
        I::MlsZPmZZ => (
            E::MultiplyAdd {
                subtract: true,
                multiplies_destination: false,
            },
            MULTIPLY_ADD,
        ),
        I::Mad => (
            E::MultiplyAdd {
                subtract: false,
                multiplies_destination: true,
            },
            MULTIPLY_DESTINATION,
        ),
        I::Msb => (
            E::MultiplyAdd {
                subtract: true,
                multiplies_destination: true,
            },
            MULTIPLY_DESTINATION,
        ),
        _ => return None,
    };
    Some(ScalableInstruction {
        operation: ScalableOperation::Element(operation),
        operands,
    })
}

/// The second operand of a binary operation on one element: an element's bits, or an immediate's value.
#[derive(Clone, Copy)]
enum Second {
    Element(u128),
    Immediate(i128),
}

impl Second {
    const fn to_integer(self, bits: u32, signedness: Signedness) -> i128 {
        match self {
            Self::Element(value) => to_integer(value, bits, signedness),
            Self::Immediate(value) => value,
        }
    }

    const fn to_bits(self, bits: u32) -> u128 {
        match self {
            Self::Element(value) => value,
            Self::Immediate(value) => value as u128 & to_mask(bits),
        }
    }
}

/// The element shifted by a signed amount, rounding where it shifts right and `rounding`, as an exact integer, or a
/// value past every element's range where a left shift overflows one.
fn shift_exactly(value: i128, amount: i128, rounding: bool) -> i128 {
    /// Past this a shift leaves any element zero, its sign, or overflowed.
    const FARTHEST: i128 = 100;
    /// A value of at most 64 bits shifted left by less than this stays within i128.
    const OVERFLOWING_LEFT: i128 = u64::BITS as i128;
    let amount = amount.clamp(-FARTHEST, FARTHEST);
    if amount >= 0 {
        if value == 0 {
            0
        } else if amount >= OVERFLOWING_LEFT {
            if value < 0 {
                i128::MIN / 2
            } else {
                i128::MAX / 2
            }
        } else {
            value << amount
        }
    } else {
        let right = (-amount) as u32;
        let round = if rounding { 1_i128 << (right - 1) } else { 0 };
        (value + round) >> right
    }
}

/// One element of the result from one element of each operand, as its bits.
fn apply_binary(operation: BinaryOperation, first: u128, second: Second, bits: u32) -> u128 {
    use BinaryOperation as B;
    use Signedness::{Signed, Unsigned};
    let integers = |signedness| {
        (
            to_integer(first, bits, signedness),
            second.to_integer(bits, signedness),
        )
    };
    let second_bits = second.to_bits(bits);
    let value = match operation {
        B::Add if crate::planted::is(1000) => first.wrapping_add(second_bits) & !1,
        B::Add => first.wrapping_add(second_bits),
        B::Subtract => first.wrapping_sub(second_bits),
        B::Maximum(signedness) => {
            let (first, second) = integers(signedness);
            first.max(second) as u128
        }
        B::Minimum(signedness) => {
            let (first, second) = integers(signedness);
            first.min(second) as u128
        }
        B::AbsoluteDifference(signedness) => {
            let (first, second) = integers(signedness);
            first.abs_diff(second)
        }
        B::Multiply => first.wrapping_mul(second_bits),
        B::MultiplyHigh(Unsigned) if crate::planted::is(1017) => {
            let (first, second) = integers(Signed);
            ((first * second) >> bits) as u128
        }
        B::MultiplyHigh(Unsigned) => (first * second_bits) >> bits,
        B::MultiplyHigh(Signed) => {
            let (first, second) = integers(Signed);
            ((first * second) >> bits) as u128
        }
        B::Divide(signedness) => {
            let (first, second) = integers(signedness);
            if second == 0 {
                0
            } else if crate::planted::is(1016) && second == -1 {
                saturate(-first, bits, signedness)
            } else {
                (first / second) as u128
            }
        }
        B::Or => first | second_bits,
        B::ExclusiveOr => first ^ second_bits,
        B::And => first & second_bits,
        B::AndNot => first & !second_bits,
        B::Shift(direction) => {
            let amount = second_bits.min(u128::from(bits)) as u32;
            match direction {
                ShiftDirection::Left => first << amount,
                ShiftDirection::LogicalRight => first >> amount,
                ShiftDirection::ArithmeticRight if crate::planted::is(1018) && amount >= bits => 0,
                ShiftDirection::ArithmeticRight => {
                    (to_integer(first, bits, Signed) >> amount.min(bits - 1)) as u128
                }
            }
        }
        B::SaturatingAdd(signedness) => {
            let (first, second) = integers(signedness);
            saturate(first + second, bits, signedness)
        }
        B::SaturatingSubtract(Unsigned) if crate::planted::is(1022) => {
            first.wrapping_sub(second_bits)
        }
        B::SaturatingSubtract(signedness) => {
            let (first, second) = integers(signedness);
            saturate(first - second, bits, signedness)
        }
        B::SignedAddUnsigned => saturate(
            to_integer(first, bits, Signed) + second.to_integer(bits, Unsigned),
            bits,
            Signed,
        ),
        B::UnsignedAddSigned => saturate(
            to_integer(first, bits, Unsigned) + second.to_integer(bits, Signed),
            bits,
            Unsigned,
        ),
        B::HalvingAdd(signedness) => {
            let (first, second) = integers(signedness);
            ((first + second) >> 1) as u128
        }
        B::HalvingSubtract(signedness) => {
            let (first, second) = integers(signedness);
            ((first - second) >> 1) as u128
        }
        B::RoundingHalvingAdd(signedness) => {
            let (first, second) = integers(signedness);
            ((first + second + 1) >> 1) as u128
        }
        B::RoundingShift(signedness) => {
            let amount_bits = if crate::planted::is(1019) {
                u8::BITS
            } else {
                bits
            };
            let amount = to_integer(second_bits, amount_bits, Signed);
            if amount >= i128::from(bits) {
                0
            } else {
                shift_exactly(to_integer(first, bits, signedness), amount, true) as u128
            }
        }
        B::SaturatingShift(signedness) | B::SaturatingRoundingShift(signedness) => {
            let amount = to_integer(second_bits, bits, Signed);
            let rounding =
                matches!(operation, B::SaturatingRoundingShift(_)) || crate::planted::is(1020);
            let shifted = shift_exactly(to_integer(first, bits, signedness), amount, rounding);
            saturate(shifted, bits, signedness)
        }
        B::PolynomialMultiply => (0..bits)
            .filter(|bit| second_bits >> bit & 1 != 0)
            .fold(0, |product, bit| product ^ (first << bit)),
        B::DoublingMultiplyHigh { rounding } => {
            // Twice the product shifted right by the bits is the product shifted by one less, which cannot overflow.
            let (first, second) = integers(Signed);
            let round = if rounding { 1_i128 << (bits - 2) } else { 0 };
            let product = (first * second + round) >> (bits - 1);
            if crate::planted::is(1021) {
                product as u128
            } else {
                saturate(product, bits, Signed)
            }
        }
        B::Pairwise(pairwise) => match pairwise {
            Pairwise::Add => first.wrapping_add(second_bits),
            Pairwise::Maximum(signedness) => {
                let (first, second) = integers(signedness);
                first.max(second) as u128
            }
            Pairwise::Minimum(signedness) => {
                let (first, second) = integers(signedness);
                first.min(second) as u128
            }
        },
    };
    value & to_mask(bits)
}

/// The element and tsz:imm3 of a shift by an immediate, or none where tsz is zero, which is unallocated.
fn decode_shift(word: u32, fields: &[Field]) -> Option<(u32, u32)> {
    let value = fields
        .iter()
        .fold(0, |value, field| (value << field.width) | field.read(word));
    let highest = (value >> SHIFT_FRACTION_BITS).checked_ilog2()?;
    Some((u8::BITS << highest, value))
}

pub(crate) fn simulate_element<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: ElementOperation,
) -> Result<(), Exception> {
    let size = SIZE.read(word);
    let bits = to_element_bits(size);
    let destination = RD5.read(word) as usize;
    let governing = processor.predicate_registers[PG3.read(word) as usize];
    let source = processor.vector_registers[RN5.read(word) as usize];
    let old = processor.vector_registers[destination];
    let result = match operation {
        ElementOperation::Binary {
            operation,
            form,
            swapped,
        } => simulate_binary(processor, word, operation, form, swapped)?,
        ElementOperation::Unary(operation) => {
            let bits = match operation {
                UnaryOperation::SignExtend(u32::BITS)
                | UnaryOperation::ZeroExtend(u32::BITS)
                | UnaryOperation::ReverseParts(u32::BITS) => u64::BITS,
                UnaryOperation::ReciprocalEstimate
                | UnaryOperation::ReciprocalSquareRootEstimate => u32::BITS,
                _ => bits,
            };
            let mut result = old;
            for index in 0..count_elements(bits) {
                if is_active(governing, index, bits) {
                    let element = apply_unary(operation, read_element(source, index, bits), bits);
                    write_element(&mut result, index, bits, element);
                }
            }
            result
        }
        ElementOperation::ShiftImmediate { shift, predicated } => {
            let fields = if predicated {
                PREDICATED_SHIFT
            } else {
                UNPREDICATED_SHIFT
            };
            let Some((bits, encoded)) = decode_shift(word, fields) else {
                return Err(Exception::UndefinedInstruction);
            };
            let amount = match shift {
                ImmediateShift::Left
                | ImmediateShift::SaturatingLeft(_)
                | ImmediateShift::SaturatingLeftUnsigned => encoded - bits,
                _ => 2 * bits - encoded,
            };
            let shifted = if predicated { old } else { source };
            let mut result = old;
            for index in 0..count_elements(bits) {
                if predicated && !is_active(governing, index, bits) {
                    continue;
                }
                let element = read_element(shifted, index, bits);
                let value = shift_by_immediate(shift, element, amount, bits);
                write_element(&mut result, index, bits, value);
            }
            result
        }
        ElementOperation::Ternary(logic) => {
            let (first, second, third) = (
                old,
                processor.vector_registers[RM5.read(word) as usize],
                source,
            );
            match logic {
                TernaryLogic::ExclusiveOrOfThree => first ^ second ^ third,
                TernaryLogic::BitClearExclusiveOr => first ^ (second & !third),
                TernaryLogic::Select => (first & third) | (second & !third),
                TernaryLogic::SelectFirstInverted => (!first & third) | (second & !third),
                TernaryLogic::SelectSecondInverted if crate::planted::is(1025) => {
                    (!first & third) | (second & !third)
                }
                TernaryLogic::SelectSecondInverted => (first & third) | (!second & !third),
                TernaryLogic::SelectInverted => !((first & third) | (second & !third)),
            }
        }
        ElementOperation::RotateExclusiveOr => {
            let Some((bits, encoded)) = decode_shift(word, UNPREDICATED_SHIFT) else {
                return Err(Exception::UndefinedInstruction);
            };
            let amount = 2 * bits - encoded;
            let joined = old ^ source;
            let mut result = 0;
            for index in 0..count_elements(bits) {
                let element = read_element(joined, index, bits);
                let rotated = if crate::planted::is(1026) {
                    (element << amount) | (element >> (bits - amount))
                } else {
                    (element >> amount) | (element << (bits - amount))
                } & to_mask(bits);
                write_element(&mut result, index, bits, rotated);
            }
            result
        }
        ElementOperation::Bitmask(operation) => {
            let field = BITMASK.read(word);
            let n = field >> (2 * BITMASK_PART_BITS) != 0 && !crate::planted::is(1027);
            let immr = (field >> BITMASK_PART_BITS) & to_mask(BITMASK_PART_BITS) as u32;
            let imms = field & to_mask(BITMASK_PART_BITS) as u32;
            let Some((mask, _)) =
                decode_bit_masks(n, imms, immr, BitmaskUse::Immediate, RegisterWidth::Bits64)
            else {
                return Err(Exception::UndefinedInstruction);
            };
            let immediate = replicate(u128::from(mask), u64::BITS);
            match operation {
                Some(operation) => {
                    apply_binary(operation, old, Second::Element(immediate), u128::BITS)
                }
                None => immediate,
            }
        }
        ElementOperation::DuplicateImmediate => {
            let immediate = to_shifted_signed_immediate(word, size)?;
            replicate(immediate as u128 & to_mask(bits), bits)
        }
        ElementOperation::DuplicateFloatingPoint => {
            replicate(to_floating_point_immediate(word, size)?, bits)
        }
        ElementOperation::CopyImmediate => {
            let immediate = to_shifted_signed_immediate(word, size)? as u128 & to_mask(bits);
            let predicate = processor.predicate_registers[COPY_PREDICATE.read(word) as usize];
            let merging = COPY_MERGING.read(word) != 0 && !crate::planted::is(1028);
            copy_into(old, predicate, immediate, bits, merging)
        }
        ElementOperation::CopyFloatingPoint => {
            let immediate = to_floating_point_immediate(word, size)?;
            let predicate = processor.predicate_registers[COPY_PREDICATE.read(word) as usize];
            copy_into(old, predicate, immediate, bits, true)
        }
        ElementOperation::Prefix { predicated: false } => source,
        ElementOperation::Prefix { predicated: true } => {
            let merging = PREFIX_MERGING.read(word) != 0 && !crate::planted::is(1029);
            let mut result = if merging { old } else { 0 };
            for index in 0..count_elements(bits) {
                if is_active(governing, index, bits) {
                    write_element(&mut result, index, bits, read_element(source, index, bits));
                }
            }
            result
        }
        ElementOperation::PairwiseAccumulate(signedness) => {
            if size == 0 {
                return Err(Exception::UndefinedInstruction);
            }
            let half = bits / 2;
            let mut result = old;
            for index in 0..count_elements(bits) {
                if !is_active(governing, index, bits) {
                    continue;
                }
                let signedness = if crate::planted::is(1033) {
                    Signedness::Unsigned
                } else {
                    signedness
                };
                let sum = to_integer(read_element(source, 2 * index, half), half, signedness)
                    + to_integer(read_element(source, 2 * index + 1, half), half, signedness);
                let total = read_element(old, index, bits).wrapping_add(sum as u128);
                write_element(&mut result, index, bits, total);
            }
            result
        }
        ElementOperation::ShiftAccumulate(shift) => {
            let Some((bits, encoded)) = decode_shift(word, UNPREDICATED_SHIFT) else {
                return Err(Exception::UndefinedInstruction);
            };
            let amount = 2 * bits - encoded;
            let mut result = 0;
            for index in 0..count_elements(bits) {
                let shifted =
                    shift_by_immediate(shift, read_element(source, index, bits), amount, bits);
                let sum = read_element(old, index, bits).wrapping_add(shifted);
                write_element(&mut result, index, bits, sum);
            }
            result
        }
        ElementOperation::ShiftInsert { left } => {
            let Some((bits, encoded)) = decode_shift(word, UNPREDICATED_SHIFT) else {
                return Err(Exception::UndefinedInstruction);
            };
            let mut result = 0;
            for index in 0..count_elements(bits) {
                let element = read_element(source, index, bits);
                let (shifted, kept) = if left {
                    let amount = encoded - bits;
                    (element << amount, !(to_mask(bits) << amount))
                } else {
                    let amount = 2 * bits - encoded;
                    (element >> amount, !(to_mask(bits) >> amount))
                };
                let kept = if crate::planted::is(1035) { 0 } else { kept };
                let inserted = (read_element(old, index, bits) & kept) | shifted;
                write_element(&mut result, index, bits, inserted & to_mask(bits));
            }
            result
        }
        ElementOperation::AbsoluteDifferenceAccumulate(signedness) => {
            let signedness = if crate::planted::is(1036) {
                Signedness::Unsigned
            } else {
                signedness
            };
            let second = processor.vector_registers[RM5.read(word) as usize];
            let mut result = 0;
            for index in 0..count_elements(bits) {
                let difference =
                    to_integer(read_element(source, index, bits), bits, signedness).abs_diff(
                        to_integer(read_element(second, index, bits), bits, signedness),
                    );
                let sum = read_element(old, index, bits).wrapping_add(difference);
                write_element(&mut result, index, bits, sum);
            }
            result
        }
        ElementOperation::InterleavedExclusiveOr { even } => {
            let second = processor.vector_registers[RM5.read(word) as usize];
            let mut result = old;
            for pair in 0..count_elements(bits) / 2 {
                let (written, other) = if even || crate::planted::is(1037) {
                    (2 * pair, 2 * pair + 1)
                } else {
                    (2 * pair + 1, 2 * pair)
                };
                let value = read_element(source, written, bits) ^ read_element(second, other, bits);
                write_element(&mut result, written, bits, value);
            }
            result
        }
        ElementOperation::MultiplyAdd {
            subtract,
            multiplies_destination,
        } => {
            let second = processor.vector_registers[RM5.read(word) as usize];
            let (addend, first) = if multiplies_destination {
                (source, old)
            } else {
                (old, source)
            };
            let mut result = old;
            for index in 0..count_elements(bits) {
                if !is_active(governing, index, bits) {
                    if crate::planted::is(1038) {
                        write_element(&mut result, index, bits, 0);
                    }
                    continue;
                }
                let product = read_element(first, index, bits)
                    .wrapping_mul(read_element(second, index, bits));
                let accumulator = read_element(addend, index, bits);
                let value = if subtract {
                    accumulator.wrapping_sub(product)
                } else {
                    accumulator.wrapping_add(product)
                };
                write_element(&mut result, index, bits, value);
            }
            result
        }
    };
    processor.vector_registers[destination] = result;
    Ok(())
}

/// The vector a binary operation leaves in its destination.
fn simulate_binary<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    operation: BinaryOperation,
    form: BinaryForm,
    swapped: bool,
) -> Result<u128, Exception> {
    let size = SIZE.read(word);
    let bitwise_doublewords = form == BinaryForm::Unpredicated
        && matches!(
            operation,
            BinaryOperation::And
                | BinaryOperation::Or
                | BinaryOperation::ExclusiveOr
                | BinaryOperation::AndNot
        );
    let bits = if bitwise_doublewords {
        u64::BITS
    } else {
        to_element_bits(size)
    };
    let destination = processor.vector_registers[RD5.read(word) as usize];
    let (first, second, governing) = match form {
        BinaryForm::Predicated | BinaryForm::PredicatedWide => (
            destination,
            processor.vector_registers[RN5.read(word) as usize],
            Some(processor.predicate_registers[PG3.read(word) as usize]),
        ),
        BinaryForm::Unpredicated | BinaryForm::UnpredicatedWide => (
            processor.vector_registers[RN5.read(word) as usize],
            processor.vector_registers[RM5.read(word) as usize],
            None,
        ),
        BinaryForm::ShiftedImmediate
        | BinaryForm::SignedImmediate
        | BinaryForm::UnsignedImmediate => (destination, 0, None),
    };
    let immediate = match form {
        BinaryForm::ShiftedImmediate => {
            let shift = IMMEDIATE_SHIFT.read(word);
            if size == 0 && shift != 0 {
                return Err(Exception::UndefinedInstruction);
            }
            Some(i128::from(IMMEDIATE.read(word)) << (shift * u8::BITS))
        }
        BinaryForm::SignedImmediate => Some(i128::from(IMMEDIATE.read_signed(word))),
        BinaryForm::UnsignedImmediate => Some(i128::from(IMMEDIATE.read(word))),
        _ => None,
    };
    let wide = matches!(
        form,
        BinaryForm::PredicatedWide | BinaryForm::UnpredicatedWide
    );
    let mut result = destination;
    for index in 0..count_elements(bits) {
        if governing.is_some_and(|governing| !is_active(governing, index, bits)) {
            continue;
        }
        let (element, other) = if matches!(operation, BinaryOperation::Pairwise(_)) {
            let pair = if index % 2 == 0 || crate::planted::is(1034) {
                first
            } else {
                second
            };
            let low = index & !1;
            (
                read_element(pair, low, bits),
                Second::Element(read_element(pair, low + 1, bits)),
            )
        } else {
            let other = match immediate {
                Some(immediate) => Second::Immediate(immediate),
                None if wide => {
                    Second::Element(read_element(second, index * bits / u64::BITS, u64::BITS))
                }
                None => Second::Element(read_element(second, index, bits)),
            };
            (read_element(first, index, bits), other)
        };
        let value = match (swapped, other) {
            (false, other) => apply_binary(operation, element, other, bits),
            (true, Second::Element(other)) => {
                apply_binary(operation, other, Second::Element(element), bits)
            }
            (true, Second::Immediate(other)) => apply_binary(
                operation,
                other as u128 & to_mask(bits),
                Second::Element(element),
                bits,
            ),
        };
        write_element(&mut result, index, bits, value);
    }
    Ok(result)
}

/// One element of a unary operation.
fn apply_unary(operation: UnaryOperation, element: u128, bits: u32) -> u128 {
    use Signedness::Signed;
    let signed = to_integer(element, bits, Signed);
    let unused = u128::BITS - bits;
    let value = match operation {
        UnaryOperation::Absolute => signed.unsigned_abs(),
        UnaryOperation::Negate => element.wrapping_neg(),
        UnaryOperation::SignExtend(from) => to_integer(element, from, Signed) as u128,
        UnaryOperation::ZeroExtend(from) => element & to_mask(from),
        UnaryOperation::CountLeadingSigns => {
            let leading = if signed < 0 { !element } else { element };
            u128::from(
                (leading & to_mask(bits)).leading_zeros()
                    - unused
                    - u32::from(!crate::planted::is(1030)),
            )
        }
        UnaryOperation::CountLeadingZeros => u128::from(element.leading_zeros() - unused),
        UnaryOperation::CountOnes => u128::from(element.count_ones()),
        UnaryOperation::LogicalNot => u128::from(element == 0),
        UnaryOperation::ClearSign => element & !(1 << (bits - 1)),
        UnaryOperation::FlipSign => element ^ (1 << (bits - 1)),
        UnaryOperation::Not => !element,
        UnaryOperation::ReverseParts(part) => {
            let part = if crate::planted::is(1031) && part == u8::BITS {
                u16::BITS
            } else {
                part
            };
            let parts = (bits / part).max(1);
            (0..parts).fold(0, |value, index| {
                value | (read_element(element, index, part) << ((parts - 1 - index) * part))
            })
        }
        UnaryOperation::ReverseBits => element.reverse_bits() >> unused,
        UnaryOperation::SaturatingAbsolute => saturate(signed.abs(), bits, Signed),
        UnaryOperation::SaturatingNegate => saturate(-signed, bits, Signed),
        UnaryOperation::ReciprocalEstimate | UnaryOperation::ReciprocalSquareRootEstimate => {
            let square_root = operation == UnaryOperation::ReciprocalSquareRootEstimate;
            let out_of_range = if square_root {
                element >> (bits - 2) == 0
            } else {
                element >> (bits - 1) == 0
            };
            if out_of_range {
                to_mask(bits)
            } else {
                let fraction = (element >> (bits - ESTIMATE_BITS)) as u32;
                let estimate = if square_root {
                    estimate_reciprocal_square_root(fraction)
                } else {
                    estimate_reciprocal(fraction)
                };
                (u128::from(estimate) & to_mask(ESTIMATE_BITS)) << (bits - ESTIMATE_BITS)
            }
        }
    };
    value & to_mask(bits)
}

/// [RecipEstimate](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.RecipEstimate.2) of a 9-bit fraction from 256 up.
fn estimate_reciprocal(fraction: u32) -> u32 {
    let scaled = fraction * 2 + u32::from(!crate::planted::is(1032));
    let estimate = (1 << 19) / scaled;
    estimate.div_ceil(2)
}

/// [RecipSqrtEstimate](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-float#impl-shared.RecipSqrtEstimate.2) of a 9-bit fraction from 128 up.
fn estimate_reciprocal_square_root(fraction: u32) -> u32 {
    let scaled = if fraction < 256 {
        u64::from(fraction) * 2 + 1
    } else {
        ((u64::from(fraction >> 1) << 1) + 1) * 2
    };
    let mut estimate = 512_u64;
    while scaled * (estimate + 1) * (estimate + 1) < 1 << 28 {
        estimate += 1;
    }
    estimate.div_ceil(2) as u32
}

/// One element shifted by an immediate amount.
fn shift_by_immediate(shift: ImmediateShift, element: u128, amount: u32, bits: u32) -> u128 {
    use Signedness::{Signed, Unsigned};
    let signed = to_integer(element, bits, Signed);
    let value = match shift {
        ImmediateShift::ArithmeticRight => (signed >> amount.min(bits - 1)) as u128,
        ImmediateShift::LogicalRight => element >> amount,
        ImmediateShift::Left => element << amount,
        ImmediateShift::DivideRight => {
            let bias = if signed < 0 && !crate::planted::is(1023) {
                (1 << amount) - 1
            } else {
                0
            };
            ((signed + bias) >> amount) as u128
        }
        ImmediateShift::SaturatingLeft(signedness) => saturate(
            to_integer(element, bits, signedness) << amount,
            bits,
            signedness,
        ),
        ImmediateShift::SaturatingLeftUnsigned => saturate(
            signed << amount,
            bits,
            if crate::planted::is(1024) {
                Signed
            } else {
                Unsigned
            },
        ),
        ImmediateShift::RoundingRight(signedness) => shift_exactly(
            to_integer(element, bits, signedness),
            -i128::from(amount),
            true,
        ) as u128,
    };
    value & to_mask(bits)
}

/// The element's bits in every element of a Z register.
pub(crate) fn replicate(element: u128, bits: u32) -> u128 {
    (0..count_elements(bits)).fold(0, |vector, index| vector | (element << (index * bits)))
}

/// The value active elements get, and inactive ones their own or zero.
fn copy_into(old: u128, predicate: u16, value: u128, bits: u32, merging: bool) -> u128 {
    let mut result = if merging { old } else { 0 };
    for index in 0..count_elements(bits) {
        if is_active(predicate, index, bits) {
            write_element(&mut result, index, bits, value);
        }
    }
    result
}

/// The signed 8-bit immediate shifted left by sh bytes; sh with byte elements is unallocated.
fn to_shifted_signed_immediate(word: u32, size: u32) -> Result<i128, Exception> {
    let shift = IMMEDIATE_SHIFT.read(word);
    if size == 0 && shift != 0 {
        return Err(Exception::UndefinedInstruction);
    }
    Ok(i128::from(IMMEDIATE.read_signed(word)) << (shift * u8::BITS))
}

/// The 8-bit floating-point immediate expanded to the element's precision; byte elements are unallocated.
fn to_floating_point_immediate(word: u32, size: u32) -> Result<u128, Exception> {
    let precision = match size {
        1 => Precision::Half,
        2 => Precision::Single,
        3 => Precision::Double,
        _ => return Err(Exception::UndefinedInstruction),
    };
    Ok(u128::from(expand_immediate(
        u64::from(IMMEDIATE.read(word)),
        precision,
    )))
}
