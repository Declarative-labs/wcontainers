//! The AVX-512 forms of the second half that share one skeleton: each element of the destination computed from the
//! sources where the opmask register selects it, and merged or zeroed elsewhere, with a memory source read element
//! by element (or one element broadcast), embedded rounding and exception suppression.
//!
//! A form is a listed form whose sample encoding is an EVEX form of a mnemonic of [`EVEX_OPERATIONS`]; its iced-x86
//! `Code` is the sample's, and its place is the sample's: memory where the sample has a memory operand.

use alloc::vec::Vec;

use iced_x86::{Code, Decoder, DecoderOptions, EncodingKind, Mnemonic, OpCodeOperandKind, OpKind};

use crate::float::{Arrangement, FloatOperation, Precision};
use crate::forms::{INSTRUCTIONS, Instruction, SAMPLE_ENCODINGS};
use crate::integer::OperandPlace;
use crate::{OPMASK_REGISTERS, Processor, VECTOR_REGISTERS};

// The integer operations' formulas.
#[path = "evex_integer.rs"]
mod lanes;
// The fused multiply-add.
#[path = "evex_fused.rs"]
pub(crate) mod fused;
// The conversions.
#[path = "evex_convert.rs"]
pub(crate) mod convert;
// The operations on a number's parts.
#[path = "evex_special.rs"]
pub(crate) mod special;
// The compares into opmask registers.
#[path = "evex_compare.rs"]
pub(crate) mod compare;

pub use convert::{Conversion, Number};
pub use special::SpecialOperation;

pub use lanes::IntegerOperation;

/// Bytes of a ZMM register.
pub const ZMM_BYTES: usize = 64;

/// What a form computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvexOperation {
    /// VADDPS to VMAXSD: the element as the legacy operation computes a lane, under MXCSR or the embedded rounding.
    Float {
        operation: FloatOperation,
        precision: Precision,
        arrangement: Arrangement,
    },
    /// A form without floating-point exceptions, its destination's elements of `element_bytes` bytes.
    Integer { operation: IntegerOperation, element_bytes: usize },
    /// VCMPPS to VCMPSD: each selected element's predicate (the immediate's low five bits) into an opmask register.
    Compare { precision: Precision, arrangement: Arrangement },
    /// VCOMISS, VCOMISD (`signaling`) and VUCOMISS, VUCOMISD: ZF, PF and CF from the lowest elements' order.
    OrderedCompare { precision: Precision, signaling: bool },
    /// VCVTDQ2PS to VCVTSD2SS: each element of the source converted; a scalar form's upper elements are the first
    /// source's.
    Convert(Conversion),
    /// VCVTNEPS2BF16: each single to bfloat16; with `pair`, VCVTNE2PS2BF16: the second source's into the lower half
    /// of the destination's words, the first's into the upper.
    ToBfloat16 { pair: bool },
    /// VFMADD132PS to VFMSUBADD231PD: a product and an addend from the destination and the two sources, summed
    /// exactly and rounded once.
    Fused {
        kind: FusedKind,
        order: FusedOrder,
        precision: Precision,
        arrangement: Arrangement,
    },
    /// VGETEXP, VGETMANT, VFPCLASS, VRNDSCALE, VREDUCE and VSCALEF of each precision.
    Special { operation: SpecialOperation, precision: Precision, arrangement: Arrangement },
    /// VDPBF16PS: each single of the destination plus the products of the sources' bfloat16 pairs, the upper pair's
    /// first, each sum rounded to nearest with denormals read and written as zeros, MXCSR neither read nor written.
    DotBfloat16,
    /// VFMULCPH to VFCMADDCSH: complex numbers of two halves, the real part lower, multiplied (the second source
    /// conjugated where `conjugate`), added to the destination where `accumulate`, each step rounded.
    ComplexMultiply { conjugate: bool, accumulate: bool, arrangement: Arrangement },
    /// VMOVSH and VMOVW: the low word moved; a load or a move from a general-purpose register zeroes the bits above
    /// it, VMOVSH between registers takes them from the first source.
    MoveScalar,
    /// VCOMPRESSPS to VPCOMPRESSQ: the selected elements packed into the lowest, in order.
    Compress { element_bytes: usize },
    /// VEXPANDPS to VPEXPANDQ: the lowest elements, in order, into the selected ones.
    Expand { element_bytes: usize },
}

/// The signs a fused form gives its product and addend.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FusedKind {
    MultiplyAdd,
    MultiplySubtract,
    NegatedMultiplyAdd,
    NegatedMultiplySubtract,
    /// VFMADDSUB: the addend subtracted in even elements, added in odd ones.
    AlternatingSubtractAdd,
    /// VFMSUBADD: added in even elements, subtracted in odd ones.
    AlternatingAddSubtract,
}

impl FusedKind {
    /// Whether the product and the addend are negated in the element of the index.
    pub const fn to_signs(self, index: usize) -> (bool, bool) {
        let even = index & 1 == 0;
        match self {
            Self::MultiplyAdd => (false, false),
            Self::MultiplySubtract => (false, true),
            Self::NegatedMultiplyAdd => (true, false),
            Self::NegatedMultiplySubtract => (true, true),
            Self::AlternatingSubtractAdd => (false, even),
            Self::AlternatingAddSubtract => (false, !even),
        }
    }
}

/// Which operands are the multiplicands and which the addend: 132 multiplies the destination by the second source
/// and adds the first, 213 multiplies the first by the destination and adds the second, 231 multiplies the two
/// sources and adds the destination.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FusedOrder {
    Order132,
    Order213,
    Order231,
}

impl FusedOrder {
    /// The multiplicands and the addend from the destination, the first source and the second.
    pub const fn arrange<T: Copy>(self, destination: T, first: T, second: T) -> [T; 3] {
        match self {
            Self::Order132 => [destination, second, first],
            Self::Order213 => [first, destination, second],
            Self::Order231 => [first, second, destination],
        }
    }
}

/// Which codes of a mnemonic a row is for: those with an immediate, those without, or every one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Immediate {
    Any,
    With,
    Without,
}

/// The mnemonic of each modeled EVEX operation.
pub static EVEX_OPERATIONS: &[(Mnemonic, Immediate, EvexOperation)] = &{
    use Arrangement::{Packed, Scalar};
    use FloatOperation::{Add, Divide, Maximum, Minimum, Multiply, SquareRoot, Subtract};
    use Immediate::{Any, With, Without};
    use IntegerOperation as I;
    use Precision::{Double, Half, Single};
    const fn float(operation: FloatOperation, precision: Precision, arrangement: Arrangement) -> EvexOperation {
        EvexOperation::Float { operation, precision, arrangement }
    }
    const fn integer(operation: IntegerOperation, element_bytes: usize) -> EvexOperation {
        EvexOperation::Integer { operation, element_bytes }
    }
    use FusedKind as F;
    use FusedOrder as O;
    use Number as N;
    const fn convert(from: Number, to: Number, truncate: bool) -> EvexOperation {
        EvexOperation::Convert(Conversion { from, to, truncate, scalar: false })
    }
    const fn scalar(from: Number, to: Number) -> EvexOperation {
        EvexOperation::Convert(Conversion { from, to, truncate: false, scalar: true })
    }
    const fn special(operation: SpecialOperation, precision: Precision, arrangement: Arrangement) -> EvexOperation {
        EvexOperation::Special { operation, precision, arrangement }
    }
    const fn fused(kind: FusedKind, order: FusedOrder, precision: Precision, arrangement: Arrangement) -> EvexOperation {
        EvexOperation::Fused { kind, order, precision, arrangement }
    }
    [
        (Mnemonic::Vaddps, Any, float(Add, Single, Packed)),
        (Mnemonic::Vaddpd, Any, float(Add, Double, Packed)),
        (Mnemonic::Vaddss, Any, float(Add, Single, Scalar)),
        (Mnemonic::Vaddsd, Any, float(Add, Double, Scalar)),
        (Mnemonic::Vsubps, Any, float(Subtract, Single, Packed)),
        (Mnemonic::Vsubpd, Any, float(Subtract, Double, Packed)),
        (Mnemonic::Vsubss, Any, float(Subtract, Single, Scalar)),
        (Mnemonic::Vsubsd, Any, float(Subtract, Double, Scalar)),
        (Mnemonic::Vmulps, Any, float(Multiply, Single, Packed)),
        (Mnemonic::Vmulpd, Any, float(Multiply, Double, Packed)),
        (Mnemonic::Vmulss, Any, float(Multiply, Single, Scalar)),
        (Mnemonic::Vmulsd, Any, float(Multiply, Double, Scalar)),
        (Mnemonic::Vdivps, Any, float(Divide, Single, Packed)),
        (Mnemonic::Vdivpd, Any, float(Divide, Double, Packed)),
        (Mnemonic::Vdivss, Any, float(Divide, Single, Scalar)),
        (Mnemonic::Vdivsd, Any, float(Divide, Double, Scalar)),
        (Mnemonic::Vsqrtps, Any, float(SquareRoot, Single, Packed)),
        (Mnemonic::Vsqrtpd, Any, float(SquareRoot, Double, Packed)),
        (Mnemonic::Vsqrtss, Any, float(SquareRoot, Single, Scalar)),
        (Mnemonic::Vsqrtsd, Any, float(SquareRoot, Double, Scalar)),
        (Mnemonic::Vminps, Any, float(Minimum, Single, Packed)),
        (Mnemonic::Vminpd, Any, float(Minimum, Double, Packed)),
        (Mnemonic::Vminss, Any, float(Minimum, Single, Scalar)),
        (Mnemonic::Vminsd, Any, float(Minimum, Double, Scalar)),
        (Mnemonic::Vmaxps, Any, float(Maximum, Single, Packed)),
        (Mnemonic::Vmaxpd, Any, float(Maximum, Double, Packed)),
        (Mnemonic::Vmaxss, Any, float(Maximum, Single, Scalar)),
        (Mnemonic::Vmaxsd, Any, float(Maximum, Double, Scalar)),
        (Mnemonic::Vpermb, Any, integer(I::Permute, 1)),
        (Mnemonic::Vpermw, Any, integer(I::Permute, 2)),
        (Mnemonic::Vpermd, Any, integer(I::Permute, 4)),
        (Mnemonic::Vpermps, Any, integer(I::Permute, 4)),
        (Mnemonic::Vpermq, Without, integer(I::Permute, 8)),
        (Mnemonic::Vpermpd, Without, integer(I::Permute, 8)),
        (Mnemonic::Vpermq, With, integer(I::PermuteQuartersByImmediate, 8)),
        (Mnemonic::Vpermpd, With, integer(I::PermuteQuartersByImmediate, 8)),
        (Mnemonic::Vpermi2b, Any, integer(I::PermuteTwoTablesIndexInDestination, 1)),
        (Mnemonic::Vpermi2w, Any, integer(I::PermuteTwoTablesIndexInDestination, 2)),
        (Mnemonic::Vpermi2d, Any, integer(I::PermuteTwoTablesIndexInDestination, 4)),
        (Mnemonic::Vpermi2ps, Any, integer(I::PermuteTwoTablesIndexInDestination, 4)),
        (Mnemonic::Vpermi2q, Any, integer(I::PermuteTwoTablesIndexInDestination, 8)),
        (Mnemonic::Vpermi2pd, Any, integer(I::PermuteTwoTablesIndexInDestination, 8)),
        (Mnemonic::Vpermt2b, Any, integer(I::PermuteTwoTablesIndexInFirst, 1)),
        (Mnemonic::Vpermt2w, Any, integer(I::PermuteTwoTablesIndexInFirst, 2)),
        (Mnemonic::Vpermt2d, Any, integer(I::PermuteTwoTablesIndexInFirst, 4)),
        (Mnemonic::Vpermt2ps, Any, integer(I::PermuteTwoTablesIndexInFirst, 4)),
        (Mnemonic::Vpermt2q, Any, integer(I::PermuteTwoTablesIndexInFirst, 8)),
        (Mnemonic::Vpermt2pd, Any, integer(I::PermuteTwoTablesIndexInFirst, 8)),
        (Mnemonic::Vpermilps, Without, integer(I::PermuteInLanes, 4)),
        (Mnemonic::Vpermilpd, Without, integer(I::PermuteInLanes, 8)),
        (Mnemonic::Vpermilps, With, integer(I::PermuteInLanesByImmediate, 4)),
        (Mnemonic::Vpermilpd, With, integer(I::PermuteInLanesByImmediate, 8)),
        (Mnemonic::Vpshufd, Any, integer(I::PermuteInLanesByImmediate, 4)),
        (Mnemonic::Vpshufhw, Any, integer(I::ShuffleHighWords, 2)),
        (Mnemonic::Vpshuflw, Any, integer(I::ShuffleLowWords, 2)),
        (Mnemonic::Vpshufb, Any, integer(I::ShuffleBytes, 1)),
        (Mnemonic::Vshufps, Any, integer(I::ShuffleSingles, 4)),
        (Mnemonic::Vshufpd, Any, integer(I::ShuffleDoubles, 8)),
        (Mnemonic::Vshuff32x4, Any, integer(I::ShuffleBlocks, 4)),
        (Mnemonic::Vshuff64x2, Any, integer(I::ShuffleBlocks, 8)),
        (Mnemonic::Vshufi32x4, Any, integer(I::ShuffleBlocks, 4)),
        (Mnemonic::Vshufi64x2, Any, integer(I::ShuffleBlocks, 8)),
        (Mnemonic::Valignd, Any, integer(I::AlignElements, 4)),
        (Mnemonic::Valignq, Any, integer(I::AlignElements, 8)),
        (Mnemonic::Vpalignr, Any, integer(I::AlignBytes, 1)),
        (Mnemonic::Vunpckhps, Any, integer(I::UnpackHigh, 4)),
        (Mnemonic::Vunpckhpd, Any, integer(I::UnpackHigh, 8)),
        (Mnemonic::Vunpcklps, Any, integer(I::UnpackLow, 4)),
        (Mnemonic::Vunpcklpd, Any, integer(I::UnpackLow, 8)),
        (Mnemonic::Vpdpbusd, Any, integer(I::DotBytes { saturating: false }, 4)),
        (Mnemonic::Vpdpbusds, Any, integer(I::DotBytes { saturating: true }, 4)),
        (Mnemonic::Vpdpwssd, Any, integer(I::DotWords { saturating: false }, 4)),
        (Mnemonic::Vpdpwssds, Any, integer(I::DotWords { saturating: true }, 4)),
        (Mnemonic::Vpmadd52luq, Any, integer(I::MultiplyAdd52 { high: false }, 8)),
        (Mnemonic::Vpmadd52huq, Any, integer(I::MultiplyAdd52 { high: true }, 8)),
        (Mnemonic::Vpmultishiftqb, Any, integer(I::MultiShiftBytes, 1)),
        (Mnemonic::Vpshldw, Any, integer(I::ShiftLeftDouble, 2)),
        (Mnemonic::Vpshldd, Any, integer(I::ShiftLeftDouble, 4)),
        (Mnemonic::Vpshldq, Any, integer(I::ShiftLeftDouble, 8)),
        (Mnemonic::Vpshrdw, Any, integer(I::ShiftRightDouble, 2)),
        (Mnemonic::Vpshrdd, Any, integer(I::ShiftRightDouble, 4)),
        (Mnemonic::Vpshrdq, Any, integer(I::ShiftRightDouble, 8)),
        (Mnemonic::Vpshldvw, Any, integer(I::ShiftLeftDoubleVariable, 2)),
        (Mnemonic::Vpshldvd, Any, integer(I::ShiftLeftDoubleVariable, 4)),
        (Mnemonic::Vpshldvq, Any, integer(I::ShiftLeftDoubleVariable, 8)),
        (Mnemonic::Vpshrdvw, Any, integer(I::ShiftRightDoubleVariable, 2)),
        (Mnemonic::Vpshrdvd, Any, integer(I::ShiftRightDoubleVariable, 4)),
        (Mnemonic::Vpshrdvq, Any, integer(I::ShiftRightDoubleVariable, 8)),
        (Mnemonic::Vpopcntb, Any, integer(I::PopulationCount, 1)),
        (Mnemonic::Vpopcntw, Any, integer(I::PopulationCount, 2)),
        (Mnemonic::Vpopcntd, Any, integer(I::PopulationCount, 4)),
        (Mnemonic::Vpopcntq, Any, integer(I::PopulationCount, 8)),
        (Mnemonic::Vplzcntd, Any, integer(I::LeadingZeros, 4)),
        (Mnemonic::Vplzcntq, Any, integer(I::LeadingZeros, 8)),
        (Mnemonic::Vpconflictd, Any, integer(I::Conflict, 4)),
        (Mnemonic::Vpconflictq, Any, integer(I::Conflict, 8)),
        (Mnemonic::Vgf2p8mulb, Any, integer(I::GaloisMultiply, 1)),
        (Mnemonic::Vgf2p8affineqb, Any, integer(I::GaloisAffine { inverse: false }, 1)),
        (Mnemonic::Vgf2p8affineinvqb, Any, integer(I::GaloisAffine { inverse: true }, 1)),
        (Mnemonic::Vpclmulqdq, Any, integer(I::CarryLessMultiply, 8)),
        (Mnemonic::Vaesenc, Any, integer(I::Aes { decrypt: false, last: false }, 1)),
        (Mnemonic::Vaesenclast, Any, integer(I::Aes { decrypt: false, last: true }, 1)),
        (Mnemonic::Vaesdec, Any, integer(I::Aes { decrypt: true, last: false }, 1)),
        (Mnemonic::Vaesdeclast, Any, integer(I::Aes { decrypt: true, last: true }, 1)),
        (Mnemonic::Vpshufbitqmb, Any, integer(I::ShuffleBitsToMask, 1)),
        (Mnemonic::Vpbroadcastmb2q, Any, integer(I::BroadcastMask, 8)),
        (Mnemonic::Vpbroadcastmw2d, Any, integer(I::BroadcastMask, 4)),
        (Mnemonic::Vcmpps, Any, EvexOperation::Compare { precision: Single, arrangement: Packed }),
        (Mnemonic::Vcmppd, Any, EvexOperation::Compare { precision: Double, arrangement: Packed }),
        (Mnemonic::Vcmpss, Any, EvexOperation::Compare { precision: Single, arrangement: Scalar }),
        (Mnemonic::Vcmpsd, Any, EvexOperation::Compare { precision: Double, arrangement: Scalar }),
        (Mnemonic::Vcomiss, Any, EvexOperation::OrderedCompare { precision: Single, signaling: true }),
        (Mnemonic::Vucomiss, Any, EvexOperation::OrderedCompare { precision: Single, signaling: false }),
        (Mnemonic::Vcomisd, Any, EvexOperation::OrderedCompare { precision: Double, signaling: true }),
        (Mnemonic::Vucomisd, Any, EvexOperation::OrderedCompare { precision: Double, signaling: false }),
        (Mnemonic::Vcvtdq2ps, Any, convert(N::Signed(4), N::Float(Single), false)),
        (Mnemonic::Vcvtps2dq, Any, convert(N::Float(Single), N::Signed(4), false)),
        (Mnemonic::Vcvttps2dq, Any, convert(N::Float(Single), N::Signed(4), true)),
        (Mnemonic::Vcvtdq2pd, Any, convert(N::Signed(4), N::Float(Double), false)),
        (Mnemonic::Vcvtpd2dq, Any, convert(N::Float(Double), N::Signed(4), false)),
        (Mnemonic::Vcvttpd2dq, Any, convert(N::Float(Double), N::Signed(4), true)),
        (Mnemonic::Vcvtudq2ps, Any, convert(N::Unsigned(4), N::Float(Single), false)),
        (Mnemonic::Vcvtps2udq, Any, convert(N::Float(Single), N::Unsigned(4), false)),
        (Mnemonic::Vcvttps2udq, Any, convert(N::Float(Single), N::Unsigned(4), true)),
        (Mnemonic::Vcvtudq2pd, Any, convert(N::Unsigned(4), N::Float(Double), false)),
        (Mnemonic::Vcvtpd2udq, Any, convert(N::Float(Double), N::Unsigned(4), false)),
        (Mnemonic::Vcvttpd2udq, Any, convert(N::Float(Double), N::Unsigned(4), true)),
        (Mnemonic::Vcvtqq2ps, Any, convert(N::Signed(8), N::Float(Single), false)),
        (Mnemonic::Vcvtps2qq, Any, convert(N::Float(Single), N::Signed(8), false)),
        (Mnemonic::Vcvttps2qq, Any, convert(N::Float(Single), N::Signed(8), true)),
        (Mnemonic::Vcvtqq2pd, Any, convert(N::Signed(8), N::Float(Double), false)),
        (Mnemonic::Vcvtpd2qq, Any, convert(N::Float(Double), N::Signed(8), false)),
        (Mnemonic::Vcvttpd2qq, Any, convert(N::Float(Double), N::Signed(8), true)),
        (Mnemonic::Vcvtuqq2ps, Any, convert(N::Unsigned(8), N::Float(Single), false)),
        (Mnemonic::Vcvtps2uqq, Any, convert(N::Float(Single), N::Unsigned(8), false)),
        (Mnemonic::Vcvttps2uqq, Any, convert(N::Float(Single), N::Unsigned(8), true)),
        (Mnemonic::Vcvtuqq2pd, Any, convert(N::Unsigned(8), N::Float(Double), false)),
        (Mnemonic::Vcvtpd2uqq, Any, convert(N::Float(Double), N::Unsigned(8), false)),
        (Mnemonic::Vcvttpd2uqq, Any, convert(N::Float(Double), N::Unsigned(8), true)),
        (Mnemonic::Vcvtps2pd, Any, convert(N::Float(Single), N::Float(Double), false)),
        (Mnemonic::Vcvtpd2ps, Any, convert(N::Float(Double), N::Float(Single), false)),
        (Mnemonic::Vcvtss2sd, Any, scalar(N::Float(Single), N::Float(Double))),
        (Mnemonic::Vcvtsd2ss, Any, scalar(N::Float(Double), N::Float(Single))),
        (Mnemonic::Vcvtneps2bf16, Any, EvexOperation::ToBfloat16 { pair: false }),
        (Mnemonic::Vcvtne2ps2bf16, Any, EvexOperation::ToBfloat16 { pair: true }),
        (Mnemonic::Vfmadd132ps, Any, fused(F::MultiplyAdd, O::Order132, Single, Packed)),
        (Mnemonic::Vfmadd132pd, Any, fused(F::MultiplyAdd, O::Order132, Double, Packed)),
        (Mnemonic::Vfmadd132ss, Any, fused(F::MultiplyAdd, O::Order132, Single, Scalar)),
        (Mnemonic::Vfmadd132sd, Any, fused(F::MultiplyAdd, O::Order132, Double, Scalar)),
        (Mnemonic::Vfmadd213ps, Any, fused(F::MultiplyAdd, O::Order213, Single, Packed)),
        (Mnemonic::Vfmadd213pd, Any, fused(F::MultiplyAdd, O::Order213, Double, Packed)),
        (Mnemonic::Vfmadd213ss, Any, fused(F::MultiplyAdd, O::Order213, Single, Scalar)),
        (Mnemonic::Vfmadd213sd, Any, fused(F::MultiplyAdd, O::Order213, Double, Scalar)),
        (Mnemonic::Vfmadd231ps, Any, fused(F::MultiplyAdd, O::Order231, Single, Packed)),
        (Mnemonic::Vfmadd231pd, Any, fused(F::MultiplyAdd, O::Order231, Double, Packed)),
        (Mnemonic::Vfmadd231ss, Any, fused(F::MultiplyAdd, O::Order231, Single, Scalar)),
        (Mnemonic::Vfmadd231sd, Any, fused(F::MultiplyAdd, O::Order231, Double, Scalar)),
        (Mnemonic::Vfmsub132ps, Any, fused(F::MultiplySubtract, O::Order132, Single, Packed)),
        (Mnemonic::Vfmsub132pd, Any, fused(F::MultiplySubtract, O::Order132, Double, Packed)),
        (Mnemonic::Vfmsub132ss, Any, fused(F::MultiplySubtract, O::Order132, Single, Scalar)),
        (Mnemonic::Vfmsub132sd, Any, fused(F::MultiplySubtract, O::Order132, Double, Scalar)),
        (Mnemonic::Vfmsub213ps, Any, fused(F::MultiplySubtract, O::Order213, Single, Packed)),
        (Mnemonic::Vfmsub213pd, Any, fused(F::MultiplySubtract, O::Order213, Double, Packed)),
        (Mnemonic::Vfmsub213ss, Any, fused(F::MultiplySubtract, O::Order213, Single, Scalar)),
        (Mnemonic::Vfmsub213sd, Any, fused(F::MultiplySubtract, O::Order213, Double, Scalar)),
        (Mnemonic::Vfmsub231ps, Any, fused(F::MultiplySubtract, O::Order231, Single, Packed)),
        (Mnemonic::Vfmsub231pd, Any, fused(F::MultiplySubtract, O::Order231, Double, Packed)),
        (Mnemonic::Vfmsub231ss, Any, fused(F::MultiplySubtract, O::Order231, Single, Scalar)),
        (Mnemonic::Vfmsub231sd, Any, fused(F::MultiplySubtract, O::Order231, Double, Scalar)),
        (Mnemonic::Vfnmadd132ps, Any, fused(F::NegatedMultiplyAdd, O::Order132, Single, Packed)),
        (Mnemonic::Vfnmadd132pd, Any, fused(F::NegatedMultiplyAdd, O::Order132, Double, Packed)),
        (Mnemonic::Vfnmadd132ss, Any, fused(F::NegatedMultiplyAdd, O::Order132, Single, Scalar)),
        (Mnemonic::Vfnmadd132sd, Any, fused(F::NegatedMultiplyAdd, O::Order132, Double, Scalar)),
        (Mnemonic::Vfnmadd213ps, Any, fused(F::NegatedMultiplyAdd, O::Order213, Single, Packed)),
        (Mnemonic::Vfnmadd213pd, Any, fused(F::NegatedMultiplyAdd, O::Order213, Double, Packed)),
        (Mnemonic::Vfnmadd213ss, Any, fused(F::NegatedMultiplyAdd, O::Order213, Single, Scalar)),
        (Mnemonic::Vfnmadd213sd, Any, fused(F::NegatedMultiplyAdd, O::Order213, Double, Scalar)),
        (Mnemonic::Vfnmadd231ps, Any, fused(F::NegatedMultiplyAdd, O::Order231, Single, Packed)),
        (Mnemonic::Vfnmadd231pd, Any, fused(F::NegatedMultiplyAdd, O::Order231, Double, Packed)),
        (Mnemonic::Vfnmadd231ss, Any, fused(F::NegatedMultiplyAdd, O::Order231, Single, Scalar)),
        (Mnemonic::Vfnmadd231sd, Any, fused(F::NegatedMultiplyAdd, O::Order231, Double, Scalar)),
        (Mnemonic::Vfnmsub132ps, Any, fused(F::NegatedMultiplySubtract, O::Order132, Single, Packed)),
        (Mnemonic::Vfnmsub132pd, Any, fused(F::NegatedMultiplySubtract, O::Order132, Double, Packed)),
        (Mnemonic::Vfnmsub132ss, Any, fused(F::NegatedMultiplySubtract, O::Order132, Single, Scalar)),
        (Mnemonic::Vfnmsub132sd, Any, fused(F::NegatedMultiplySubtract, O::Order132, Double, Scalar)),
        (Mnemonic::Vfnmsub213ps, Any, fused(F::NegatedMultiplySubtract, O::Order213, Single, Packed)),
        (Mnemonic::Vfnmsub213pd, Any, fused(F::NegatedMultiplySubtract, O::Order213, Double, Packed)),
        (Mnemonic::Vfnmsub213ss, Any, fused(F::NegatedMultiplySubtract, O::Order213, Single, Scalar)),
        (Mnemonic::Vfnmsub213sd, Any, fused(F::NegatedMultiplySubtract, O::Order213, Double, Scalar)),
        (Mnemonic::Vfnmsub231ps, Any, fused(F::NegatedMultiplySubtract, O::Order231, Single, Packed)),
        (Mnemonic::Vfnmsub231pd, Any, fused(F::NegatedMultiplySubtract, O::Order231, Double, Packed)),
        (Mnemonic::Vfnmsub231ss, Any, fused(F::NegatedMultiplySubtract, O::Order231, Single, Scalar)),
        (Mnemonic::Vfnmsub231sd, Any, fused(F::NegatedMultiplySubtract, O::Order231, Double, Scalar)),
        (Mnemonic::Vfmaddsub132ps, Any, fused(F::AlternatingSubtractAdd, O::Order132, Single, Packed)),
        (Mnemonic::Vfmaddsub132pd, Any, fused(F::AlternatingSubtractAdd, O::Order132, Double, Packed)),
        (Mnemonic::Vfmaddsub213ps, Any, fused(F::AlternatingSubtractAdd, O::Order213, Single, Packed)),
        (Mnemonic::Vfmaddsub213pd, Any, fused(F::AlternatingSubtractAdd, O::Order213, Double, Packed)),
        (Mnemonic::Vfmaddsub231ps, Any, fused(F::AlternatingSubtractAdd, O::Order231, Single, Packed)),
        (Mnemonic::Vfmaddsub231pd, Any, fused(F::AlternatingSubtractAdd, O::Order231, Double, Packed)),
        (Mnemonic::Vfmsubadd132ps, Any, fused(F::AlternatingAddSubtract, O::Order132, Single, Packed)),
        (Mnemonic::Vfmsubadd132pd, Any, fused(F::AlternatingAddSubtract, O::Order132, Double, Packed)),
        (Mnemonic::Vfmsubadd213ps, Any, fused(F::AlternatingAddSubtract, O::Order213, Single, Packed)),
        (Mnemonic::Vfmsubadd213pd, Any, fused(F::AlternatingAddSubtract, O::Order213, Double, Packed)),
        (Mnemonic::Vfmsubadd231ps, Any, fused(F::AlternatingAddSubtract, O::Order231, Single, Packed)),
        (Mnemonic::Vfmsubadd231pd, Any, fused(F::AlternatingAddSubtract, O::Order231, Double, Packed)),
        // AVX512-FP16.
        (Mnemonic::Vaddph, Any, float(Add, Half, Packed)),
        (Mnemonic::Vaddsh, Any, float(Add, Half, Scalar)),
        (Mnemonic::Vsubph, Any, float(Subtract, Half, Packed)),
        (Mnemonic::Vsubsh, Any, float(Subtract, Half, Scalar)),
        (Mnemonic::Vmulph, Any, float(Multiply, Half, Packed)),
        (Mnemonic::Vmulsh, Any, float(Multiply, Half, Scalar)),
        (Mnemonic::Vdivph, Any, float(Divide, Half, Packed)),
        (Mnemonic::Vdivsh, Any, float(Divide, Half, Scalar)),
        (Mnemonic::Vsqrtph, Any, float(SquareRoot, Half, Packed)),
        (Mnemonic::Vsqrtsh, Any, float(SquareRoot, Half, Scalar)),
        (Mnemonic::Vminph, Any, float(Minimum, Half, Packed)),
        (Mnemonic::Vminsh, Any, float(Minimum, Half, Scalar)),
        (Mnemonic::Vmaxph, Any, float(Maximum, Half, Packed)),
        (Mnemonic::Vmaxsh, Any, float(Maximum, Half, Scalar)),
        (Mnemonic::Vcmpph, Any, EvexOperation::Compare { precision: Half, arrangement: Packed }),
        (Mnemonic::Vcmpsh, Any, EvexOperation::Compare { precision: Half, arrangement: Scalar }),
        (Mnemonic::Vcomish, Any, EvexOperation::OrderedCompare { precision: Half, signaling: true }),
        (Mnemonic::Vucomish, Any, EvexOperation::OrderedCompare { precision: Half, signaling: false }),
        (Mnemonic::Vfmadd132ph, Any, fused(F::MultiplyAdd, O::Order132, Half, Packed)),
        (Mnemonic::Vfmadd132sh, Any, fused(F::MultiplyAdd, O::Order132, Half, Scalar)),
        (Mnemonic::Vfmadd213ph, Any, fused(F::MultiplyAdd, O::Order213, Half, Packed)),
        (Mnemonic::Vfmadd213sh, Any, fused(F::MultiplyAdd, O::Order213, Half, Scalar)),
        (Mnemonic::Vfmadd231ph, Any, fused(F::MultiplyAdd, O::Order231, Half, Packed)),
        (Mnemonic::Vfmadd231sh, Any, fused(F::MultiplyAdd, O::Order231, Half, Scalar)),
        (Mnemonic::Vfmsub132ph, Any, fused(F::MultiplySubtract, O::Order132, Half, Packed)),
        (Mnemonic::Vfmsub132sh, Any, fused(F::MultiplySubtract, O::Order132, Half, Scalar)),
        (Mnemonic::Vfmsub213ph, Any, fused(F::MultiplySubtract, O::Order213, Half, Packed)),
        (Mnemonic::Vfmsub213sh, Any, fused(F::MultiplySubtract, O::Order213, Half, Scalar)),
        (Mnemonic::Vfmsub231ph, Any, fused(F::MultiplySubtract, O::Order231, Half, Packed)),
        (Mnemonic::Vfmsub231sh, Any, fused(F::MultiplySubtract, O::Order231, Half, Scalar)),
        (Mnemonic::Vfnmadd132ph, Any, fused(F::NegatedMultiplyAdd, O::Order132, Half, Packed)),
        (Mnemonic::Vfnmadd132sh, Any, fused(F::NegatedMultiplyAdd, O::Order132, Half, Scalar)),
        (Mnemonic::Vfnmadd213ph, Any, fused(F::NegatedMultiplyAdd, O::Order213, Half, Packed)),
        (Mnemonic::Vfnmadd213sh, Any, fused(F::NegatedMultiplyAdd, O::Order213, Half, Scalar)),
        (Mnemonic::Vfnmadd231ph, Any, fused(F::NegatedMultiplyAdd, O::Order231, Half, Packed)),
        (Mnemonic::Vfnmadd231sh, Any, fused(F::NegatedMultiplyAdd, O::Order231, Half, Scalar)),
        (Mnemonic::Vfnmsub132ph, Any, fused(F::NegatedMultiplySubtract, O::Order132, Half, Packed)),
        (Mnemonic::Vfnmsub132sh, Any, fused(F::NegatedMultiplySubtract, O::Order132, Half, Scalar)),
        (Mnemonic::Vfnmsub213ph, Any, fused(F::NegatedMultiplySubtract, O::Order213, Half, Packed)),
        (Mnemonic::Vfnmsub213sh, Any, fused(F::NegatedMultiplySubtract, O::Order213, Half, Scalar)),
        (Mnemonic::Vfnmsub231ph, Any, fused(F::NegatedMultiplySubtract, O::Order231, Half, Packed)),
        (Mnemonic::Vfnmsub231sh, Any, fused(F::NegatedMultiplySubtract, O::Order231, Half, Scalar)),
        (Mnemonic::Vfmaddsub132ph, Any, fused(F::AlternatingSubtractAdd, O::Order132, Half, Packed)),
        (Mnemonic::Vfmaddsub213ph, Any, fused(F::AlternatingSubtractAdd, O::Order213, Half, Packed)),
        (Mnemonic::Vfmaddsub231ph, Any, fused(F::AlternatingSubtractAdd, O::Order231, Half, Packed)),
        (Mnemonic::Vfmsubadd132ph, Any, fused(F::AlternatingAddSubtract, O::Order132, Half, Packed)),
        (Mnemonic::Vfmsubadd213ph, Any, fused(F::AlternatingAddSubtract, O::Order213, Half, Packed)),
        (Mnemonic::Vfmsubadd231ph, Any, fused(F::AlternatingAddSubtract, O::Order231, Half, Packed)),
        (Mnemonic::Vcvtph2psx, Any, convert(N::Float(Half), N::Float(Single), false)),
        (Mnemonic::Vcvtph2pd, Any, convert(N::Float(Half), N::Float(Double), false)),
        (Mnemonic::Vcvtps2phx, Any, convert(N::Float(Single), N::Float(Half), false)),
        (Mnemonic::Vcvtpd2ph, Any, convert(N::Float(Double), N::Float(Half), false)),
        (Mnemonic::Vcvtph2dq, Any, convert(N::Float(Half), N::Signed(4), false)),
        (Mnemonic::Vcvttph2dq, Any, convert(N::Float(Half), N::Signed(4), true)),
        (Mnemonic::Vcvtdq2ph, Any, convert(N::Signed(4), N::Float(Half), false)),
        (Mnemonic::Vcvtph2udq, Any, convert(N::Float(Half), N::Unsigned(4), false)),
        (Mnemonic::Vcvttph2udq, Any, convert(N::Float(Half), N::Unsigned(4), true)),
        (Mnemonic::Vcvtudq2ph, Any, convert(N::Unsigned(4), N::Float(Half), false)),
        (Mnemonic::Vcvtph2qq, Any, convert(N::Float(Half), N::Signed(8), false)),
        (Mnemonic::Vcvttph2qq, Any, convert(N::Float(Half), N::Signed(8), true)),
        (Mnemonic::Vcvtqq2ph, Any, convert(N::Signed(8), N::Float(Half), false)),
        (Mnemonic::Vcvtph2uqq, Any, convert(N::Float(Half), N::Unsigned(8), false)),
        (Mnemonic::Vcvttph2uqq, Any, convert(N::Float(Half), N::Unsigned(8), true)),
        (Mnemonic::Vcvtuqq2ph, Any, convert(N::Unsigned(8), N::Float(Half), false)),
        (Mnemonic::Vcvtph2w, Any, convert(N::Float(Half), N::Signed(2), false)),
        (Mnemonic::Vcvttph2w, Any, convert(N::Float(Half), N::Signed(2), true)),
        (Mnemonic::Vcvtw2ph, Any, convert(N::Signed(2), N::Float(Half), false)),
        (Mnemonic::Vcvtph2uw, Any, convert(N::Float(Half), N::Unsigned(2), false)),
        (Mnemonic::Vcvttph2uw, Any, convert(N::Float(Half), N::Unsigned(2), true)),
        (Mnemonic::Vcvtuw2ph, Any, convert(N::Unsigned(2), N::Float(Half), false)),
        (Mnemonic::Vcvtph2ps, Any, convert(N::Float(Half), N::Float(Single), false)),
        (Mnemonic::Vcompressps, Any, EvexOperation::Compress { element_bytes: 4 }),
        (Mnemonic::Vcompresspd, Any, EvexOperation::Compress { element_bytes: 8 }),
        (Mnemonic::Vpcompressb, Any, EvexOperation::Compress { element_bytes: 1 }),
        (Mnemonic::Vpcompressw, Any, EvexOperation::Compress { element_bytes: 2 }),
        (Mnemonic::Vpcompressd, Any, EvexOperation::Compress { element_bytes: 4 }),
        (Mnemonic::Vpcompressq, Any, EvexOperation::Compress { element_bytes: 8 }),
        (Mnemonic::Vexpandps, Any, EvexOperation::Expand { element_bytes: 4 }),
        (Mnemonic::Vexpandpd, Any, EvexOperation::Expand { element_bytes: 8 }),
        (Mnemonic::Vpexpandb, Any, EvexOperation::Expand { element_bytes: 1 }),
        (Mnemonic::Vpexpandw, Any, EvexOperation::Expand { element_bytes: 2 }),
        (Mnemonic::Vpexpandd, Any, EvexOperation::Expand { element_bytes: 4 }),
        (Mnemonic::Vpexpandq, Any, EvexOperation::Expand { element_bytes: 8 }),
        (Mnemonic::Vfmulcph, Any, EvexOperation::ComplexMultiply { conjugate: false, accumulate: false, arrangement: Packed }),
        (Mnemonic::Vfmulcsh, Any, EvexOperation::ComplexMultiply { conjugate: false, accumulate: false, arrangement: Scalar }),
        (Mnemonic::Vfcmulcph, Any, EvexOperation::ComplexMultiply { conjugate: true, accumulate: false, arrangement: Packed }),
        (Mnemonic::Vfcmulcsh, Any, EvexOperation::ComplexMultiply { conjugate: true, accumulate: false, arrangement: Scalar }),
        (Mnemonic::Vfmaddcph, Any, EvexOperation::ComplexMultiply { conjugate: false, accumulate: true, arrangement: Packed }),
        (Mnemonic::Vfmaddcsh, Any, EvexOperation::ComplexMultiply { conjugate: false, accumulate: true, arrangement: Scalar }),
        (Mnemonic::Vfcmaddcph, Any, EvexOperation::ComplexMultiply { conjugate: true, accumulate: true, arrangement: Packed }),
        (Mnemonic::Vfcmaddcsh, Any, EvexOperation::ComplexMultiply { conjugate: true, accumulate: true, arrangement: Scalar }),
        (Mnemonic::Vmovsh, Any, EvexOperation::MoveScalar),
        (Mnemonic::Vmovw, Any, EvexOperation::MoveScalar),
        (Mnemonic::Vdpbf16ps, Any, EvexOperation::DotBfloat16),
        (Mnemonic::Vfixupimmps, Any, special(SpecialOperation::Fixup, Single, Packed)),
        (Mnemonic::Vfixupimmpd, Any, special(SpecialOperation::Fixup, Double, Packed)),
        (Mnemonic::Vfixupimmss, Any, special(SpecialOperation::Fixup, Single, Scalar)),
        (Mnemonic::Vfixupimmsd, Any, special(SpecialOperation::Fixup, Double, Scalar)),
        (Mnemonic::Vrangeps, Any, special(SpecialOperation::Range, Single, Packed)),
        (Mnemonic::Vrangepd, Any, special(SpecialOperation::Range, Double, Packed)),
        (Mnemonic::Vrangess, Any, special(SpecialOperation::Range, Single, Scalar)),
        (Mnemonic::Vrangesd, Any, special(SpecialOperation::Range, Double, Scalar)),
        (Mnemonic::Vgetexpps, Any, special(SpecialOperation::GetExponent, Single, Packed)),
        (Mnemonic::Vgetexppd, Any, special(SpecialOperation::GetExponent, Double, Packed)),
        (Mnemonic::Vgetexpph, Any, special(SpecialOperation::GetExponent, Half, Packed)),
        (Mnemonic::Vgetexpss, Any, special(SpecialOperation::GetExponent, Single, Scalar)),
        (Mnemonic::Vgetexpsd, Any, special(SpecialOperation::GetExponent, Double, Scalar)),
        (Mnemonic::Vgetexpsh, Any, special(SpecialOperation::GetExponent, Half, Scalar)),
        (Mnemonic::Vgetmantps, Any, special(SpecialOperation::GetMantissa, Single, Packed)),
        (Mnemonic::Vgetmantpd, Any, special(SpecialOperation::GetMantissa, Double, Packed)),
        (Mnemonic::Vgetmantph, Any, special(SpecialOperation::GetMantissa, Half, Packed)),
        (Mnemonic::Vgetmantss, Any, special(SpecialOperation::GetMantissa, Single, Scalar)),
        (Mnemonic::Vgetmantsd, Any, special(SpecialOperation::GetMantissa, Double, Scalar)),
        (Mnemonic::Vgetmantsh, Any, special(SpecialOperation::GetMantissa, Half, Scalar)),
        (Mnemonic::Vfpclassps, Any, special(SpecialOperation::Class, Single, Packed)),
        (Mnemonic::Vfpclasspd, Any, special(SpecialOperation::Class, Double, Packed)),
        (Mnemonic::Vfpclassph, Any, special(SpecialOperation::Class, Half, Packed)),
        (Mnemonic::Vfpclassss, Any, special(SpecialOperation::Class, Single, Scalar)),
        (Mnemonic::Vfpclasssd, Any, special(SpecialOperation::Class, Double, Scalar)),
        (Mnemonic::Vfpclasssh, Any, special(SpecialOperation::Class, Half, Scalar)),
        (Mnemonic::Vrndscaleps, Any, special(SpecialOperation::RoundScale, Single, Packed)),
        (Mnemonic::Vrndscalepd, Any, special(SpecialOperation::RoundScale, Double, Packed)),
        (Mnemonic::Vrndscaleph, Any, special(SpecialOperation::RoundScale, Half, Packed)),
        (Mnemonic::Vrndscaless, Any, special(SpecialOperation::RoundScale, Single, Scalar)),
        (Mnemonic::Vrndscalesd, Any, special(SpecialOperation::RoundScale, Double, Scalar)),
        (Mnemonic::Vrndscalesh, Any, special(SpecialOperation::RoundScale, Half, Scalar)),
        (Mnemonic::Vreduceps, Any, special(SpecialOperation::Reduce, Single, Packed)),
        (Mnemonic::Vreducepd, Any, special(SpecialOperation::Reduce, Double, Packed)),
        (Mnemonic::Vreduceph, Any, special(SpecialOperation::Reduce, Half, Packed)),
        (Mnemonic::Vreducess, Any, special(SpecialOperation::Reduce, Single, Scalar)),
        (Mnemonic::Vreducesd, Any, special(SpecialOperation::Reduce, Double, Scalar)),
        (Mnemonic::Vreducesh, Any, special(SpecialOperation::Reduce, Half, Scalar)),
        (Mnemonic::Vscalefps, Any, special(SpecialOperation::Scale, Single, Packed)),
        (Mnemonic::Vscalefpd, Any, special(SpecialOperation::Scale, Double, Packed)),
        (Mnemonic::Vscalefph, Any, special(SpecialOperation::Scale, Half, Packed)),
        (Mnemonic::Vscalefss, Any, special(SpecialOperation::Scale, Single, Scalar)),
        (Mnemonic::Vscalefsd, Any, special(SpecialOperation::Scale, Double, Scalar)),
        (Mnemonic::Vscalefsh, Any, special(SpecialOperation::Scale, Half, Scalar)),
        // A general-purpose register or its memory: an integer as wide as the register (0 here), the low element alone.
        (Mnemonic::Vcvtsd2si, Any, convert(N::Float(Double), N::Signed(0), false)),
        (Mnemonic::Vcvttsd2si, Any, convert(N::Float(Double), N::Signed(0), true)),
        (Mnemonic::Vcvtsd2usi, Any, convert(N::Float(Double), N::Unsigned(0), false)),
        (Mnemonic::Vcvttsd2usi, Any, convert(N::Float(Double), N::Unsigned(0), true)),
        (Mnemonic::Vcvtsi2sd, Any, scalar(N::Signed(0), N::Float(Double))),
        (Mnemonic::Vcvtusi2sd, Any, scalar(N::Unsigned(0), N::Float(Double))),
        (Mnemonic::Vcvtss2si, Any, convert(N::Float(Single), N::Signed(0), false)),
        (Mnemonic::Vcvttss2si, Any, convert(N::Float(Single), N::Signed(0), true)),
        (Mnemonic::Vcvtss2usi, Any, convert(N::Float(Single), N::Unsigned(0), false)),
        (Mnemonic::Vcvttss2usi, Any, convert(N::Float(Single), N::Unsigned(0), true)),
        (Mnemonic::Vcvtsi2ss, Any, scalar(N::Signed(0), N::Float(Single))),
        (Mnemonic::Vcvtusi2ss, Any, scalar(N::Unsigned(0), N::Float(Single))),
        (Mnemonic::Vcvtsh2si, Any, convert(N::Float(Half), N::Signed(0), false)),
        (Mnemonic::Vcvttsh2si, Any, convert(N::Float(Half), N::Signed(0), true)),
        (Mnemonic::Vcvtsh2usi, Any, convert(N::Float(Half), N::Unsigned(0), false)),
        (Mnemonic::Vcvttsh2usi, Any, convert(N::Float(Half), N::Unsigned(0), true)),
        (Mnemonic::Vcvtsi2sh, Any, scalar(N::Signed(0), N::Float(Half))),
        (Mnemonic::Vcvtusi2sh, Any, scalar(N::Unsigned(0), N::Float(Half))),
        (Mnemonic::Vcvtsh2ss, Any, scalar(N::Float(Half), N::Float(Single))),
        (Mnemonic::Vcvtsh2sd, Any, scalar(N::Float(Half), N::Float(Double))),
        (Mnemonic::Vcvtss2sh, Any, scalar(N::Float(Single), N::Float(Half))),
        (Mnemonic::Vcvtsd2sh, Any, scalar(N::Float(Double), N::Float(Half))),
    ]
};

/// One modeled EVEX form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexForm {
    pub instruction: Instruction,
    pub code: Code,
    pub operation: EvexOperation,
    pub place: OperandPlace,
    /// The bytes of the element a broadcast reads, or of one element of the memory operand where the form does not
    /// broadcast; 0 for a register form.
    pub memory_element_bytes: usize,
}

impl EvexForm {
    /// The bytes of the destination's vector: 16, 32 or 64; 16 for a scalar form.
    pub fn vector_bytes(&self) -> usize {
        match self.code.op_code().l() {
            0 => 16,
            1 => 32,
            _ => ZMM_BYTES,
        }
    }

    /// The bytes of one element of the destination, which the opmask selects.
    pub const fn element_bytes(&self) -> usize {
        match self.operation {
            EvexOperation::Float { precision, .. }
            | EvexOperation::Fused { precision, .. }
            | EvexOperation::Compare { precision, .. }
            | EvexOperation::Special { precision, .. }
            | EvexOperation::OrderedCompare { precision, .. } => (precision.to_bits() / u8::BITS) as usize,
            EvexOperation::Integer { element_bytes, .. }
            | EvexOperation::Compress { element_bytes }
            | EvexOperation::Expand { element_bytes } => element_bytes,
            EvexOperation::Convert(conversion) => conversion.to.to_bytes(),
            EvexOperation::ToBfloat16 { .. } => 2,
            EvexOperation::DotBfloat16 | EvexOperation::ComplexMultiply { .. } => 4,
            EvexOperation::MoveScalar => 2,
        }
    }

    /// The bytes of one element of the source, which a memory source is read by.
    pub const fn source_element_bytes(&self) -> usize {
        match self.operation {
            EvexOperation::Convert(conversion) => conversion.from.to_bytes(),
            EvexOperation::ToBfloat16 { .. } => 4,
            _ => self.element_bytes(),
        }
    }

    /// The elements the form computes: one for a scalar form.
    pub fn elements(&self) -> usize {
        match self.operation {
            EvexOperation::Float { arrangement: Arrangement::Scalar, .. }
            | EvexOperation::Fused { arrangement: Arrangement::Scalar, .. }
            | EvexOperation::Compare { arrangement: Arrangement::Scalar, .. }
            | EvexOperation::Special { arrangement: Arrangement::Scalar, .. }
            | EvexOperation::MoveScalar
            | EvexOperation::ComplexMultiply { arrangement: Arrangement::Scalar, .. }
            | EvexOperation::OrderedCompare { .. } => 1,
            EvexOperation::Convert(Conversion { scalar: true, .. }) => 1,
            EvexOperation::ToBfloat16 { pair: true } => self.vector_bytes() / 2,
            _ => self.vector_bytes() / self.element_bytes().max(self.source_element_bytes()),
        }
    }

    /// The bytes a memory source reaches without broadcast: every element's.
    pub fn memory_bytes(&self) -> usize {
        match self.operation {
            EvexOperation::ToBfloat16 { pair: true } => self.vector_bytes(),
            _ => self.elements() * self.source_element_bytes(),
        }
    }

    /// Whether the form stores to memory: VCOMPRESS with a memory destination.
    pub fn stores(&self) -> bool {
        let to_memory = matches!(
            self.code.op_code().op_kinds().first(),
            Some(OpCodeOperandKind::mem | OpCodeOperandKind::r32_or_mem | OpCodeOperandKind::xmm_or_mem)
        );
        matches!(self.operation, EvexOperation::Compress { .. } | EvexOperation::MoveScalar)
            && self.place == OperandPlace::Memory
            && to_memory
    }

    /// Whether a denormal source is reported: not by VCVTPH2PS, which converts halves as F16C does.
    pub fn reports_denormal(&self) -> bool {
        self.code.mnemonic() != Mnemonic::Vcvtph2ps
    }

    /// Whether an operand the form reads is a half.
    pub const fn reads_half(&self) -> bool {
        match self.operation {
            EvexOperation::Convert(conversion) => matches!(conversion.from, Number::Float(Precision::Half)),
            _ => self.writes_half(),
        }
    }

    /// Whether the form computes or writes halves.
    pub const fn writes_half(&self) -> bool {
        match self.operation {
            EvexOperation::Float { precision, .. }
            | EvexOperation::Fused { precision, .. }
            | EvexOperation::Compare { precision, .. }
            | EvexOperation::Special { precision, .. }
            | EvexOperation::OrderedCompare { precision, .. } => matches!(precision, Precision::Half),
            EvexOperation::Convert(conversion) => matches!(conversion.to, Number::Float(Precision::Half)),
            EvexOperation::ComplexMultiply { .. } => true,
            EvexOperation::Integer { .. }
            | EvexOperation::ToBfloat16 { .. }
            | EvexOperation::DotBfloat16
            | EvexOperation::MoveScalar
            | EvexOperation::Compress { .. }
            | EvexOperation::Expand { .. } => false,
        }
    }

    /// Whether the form reads a first source from EVEX.vvvv.
    pub fn reads_first_source(&self) -> bool {
        self.code.op_code().op_kinds().iter().any(|kind| {
            matches!(kind, OpCodeOperandKind::xmm_vvvv | OpCodeOperandKind::ymm_vvvv | OpCodeOperandKind::zmm_vvvv)
        })
    }

    /// Whether the form ends with an 8-bit immediate.
    pub fn has_immediate(&self) -> bool {
        self.code.op_code().op_kinds().contains(&OpCodeOperandKind::imm8)
    }

    /// Whether the form writes an opmask register rather than a vector.
    pub fn writes_opmask(&self) -> bool {
        self.code.op_code().op_kinds().first() == Some(&OpCodeOperandKind::k_reg)
    }

    /// Whether the form's source is an opmask register.
    pub fn reads_opmask(&self) -> bool {
        self.code.op_code().op_kinds().contains(&OpCodeOperandKind::k_rm)
    }

    /// Whether the form reads its destination's value besides merging into it.
    pub const fn reads_destination(&self) -> bool {
        match self.operation {
            EvexOperation::Float { .. }
            | EvexOperation::Convert(_)
            | EvexOperation::ToBfloat16 { .. }
            | EvexOperation::Compare { .. }
            | EvexOperation::MoveScalar
            | EvexOperation::OrderedCompare { .. } => false,
            EvexOperation::Special { operation, .. } => matches!(operation, SpecialOperation::Fixup),
            EvexOperation::Fused { .. } | EvexOperation::DotBfloat16 => true,
            EvexOperation::ComplexMultiply { accumulate, .. } => accumulate,
            EvexOperation::Integer { operation, .. } => operation.reads_destination(),
            EvexOperation::Compress { .. } | EvexOperation::Expand { .. } => false,
        }
    }

    /// Whether a fault on an element of the memory operand the opmask leaves out is suppressed: for the forms whose
    /// memory elements are the destination's elements.
    pub const fn suppresses_faults(&self) -> bool {
        match self.operation {
            EvexOperation::Float { .. }
            | EvexOperation::Fused { .. }
            | EvexOperation::Convert(_)
            | EvexOperation::Compare { .. }
            | EvexOperation::Special { .. }
            | EvexOperation::DotBfloat16
            | EvexOperation::MoveScalar
            | EvexOperation::ComplexMultiply { .. }
            | EvexOperation::OrderedCompare { .. } => true,
            EvexOperation::ToBfloat16 { pair } => !pair,
            EvexOperation::Integer { operation, .. } => operation.is_element_wise(),
            EvexOperation::Compress { .. } | EvexOperation::Expand { .. } => true,
        }
    }
}

/// The modeled EVEX form of the instruction, from its sample encoding.
pub fn find_evex_form(instruction: Instruction) -> Option<EvexForm> {
    let sample = SAMPLE_ENCODINGS[instruction as usize];
    if sample.is_empty() {
        return None;
    }
    let decoded = Decoder::new(64, sample, DecoderOptions::NONE).decode();
    if decoded.is_invalid() || decoded.encoding() != EncodingKind::EVEX {
        return None;
    }
    let immediate = decoded.code().op_code().op_kinds().contains(&OpCodeOperandKind::imm8);
    let (_, _, operation) = EVEX_OPERATIONS.iter().find(|(mnemonic, shape, _)| {
        *mnemonic == decoded.mnemonic()
            && match shape {
                Immediate::Any => true,
                Immediate::With => immediate,
                Immediate::Without => !immediate,
            }
    })?;
    let in_memory = (0..decoded.op_count()).any(|operand| decoded.op_kind(operand) == OpKind::Memory);
    let operation = match *operation {
        EvexOperation::Convert(conversion) => EvexOperation::Convert(Conversion {
            from: widen_to_register(conversion.from, &decoded),
            to: widen_to_register(conversion.to, &decoded),
            // A conversion to a general-purpose register reads the low element alone.
            scalar: conversion.scalar || to_register_bytes(&decoded).is_some(),
            ..conversion
        }),
        operation => operation,
    };
    Some(EvexForm {
        instruction,
        code: decoded.code(),
        operation,
        place: if in_memory { OperandPlace::Memory } else { OperandPlace::Register },
        memory_element_bytes: if in_memory { to_broadcast_bytes(&decoded) } else { 0 },
    })
}

/// The bytes of the general-purpose register (or its memory) the form names.
fn to_register_bytes(decoded: &iced_x86::Instruction) -> Option<usize> {
    decoded.code().op_code().op_kinds().iter().find_map(|kind| match kind {
        OpCodeOperandKind::r32_reg | OpCodeOperandKind::r32_or_mem => Some(4),
        OpCodeOperandKind::r64_reg | OpCodeOperandKind::r64_or_mem => Some(8),
        _ => None,
    })
}

/// An integer of the row's width 0 as wide as the form's general-purpose register.
fn widen_to_register(number: Number, decoded: &iced_x86::Instruction) -> Number {
    let bytes = to_register_bytes(decoded).unwrap_or(0);
    match number {
        Number::Signed(0) => Number::Signed(bytes),
        Number::Unsigned(0) => Number::Unsigned(bytes),
        number => number,
    }
}

/// Whether the form's destination is a general-purpose register.
pub fn writes_register(form: &EvexForm) -> bool {
    match form.code.op_code().op_kinds().first() {
        Some(OpCodeOperandKind::r32_reg | OpCodeOperandKind::r64_reg) => true,
        // VMOVW's destination is a register or memory.
        Some(OpCodeOperandKind::r32_or_mem) => form.place == OperandPlace::Register,
        _ => false,
    }
}

/// The bytes of the element the form's broadcast reads, which may differ from its vector's element.
fn to_broadcast_bytes(decoded: &iced_x86::Instruction) -> usize {
    let mut broadcast = *decoded;
    broadcast.set_is_broadcast(decoded.code().op_code().can_broadcast());
    broadcast.memory_size().element_size()
}

/// Every modeled EVEX form, in the list's order.
pub fn list_evex_forms() -> Vec<EvexForm> {
    INSTRUCTIONS.iter().filter_map(|&instruction| find_evex_form(instruction)).collect()
}

impl Processor {
    /// ZMM0 to ZMM31 by number, lowest 128 bits first.
    pub fn read_zmm(&self, number: usize) -> [u128; 4] {
        match number.checked_sub(VECTOR_REGISTERS) {
            None => {
                let [third, fourth] = self.zmm_upper_halves[number];
                [self.vector_registers[number], self.upper_vector_halves[number], third, fourth]
            }
            Some(high) => self.high_vector_registers[high],
        }
    }

    pub fn write_zmm(&mut self, number: usize, value: [u128; 4]) {
        match number.checked_sub(VECTOR_REGISTERS) {
            None => {
                self.vector_registers[number] = value[0];
                self.upper_vector_halves[number] = value[1];
                self.zmm_upper_halves[number] = [value[2], value[3]];
            }
            Some(high) => self.high_vector_registers[high] = value,
        }
    }

    /// The opmask register's value; k0 in a mask's place selects every element.
    pub const fn read_opmask(&self, number: usize) -> u64 {
        assert!(number < OPMASK_REGISTERS, "k0 to k7");
        self.opmask_registers[number]
    }
}

/// The element of `bytes` bytes at the index of a vector held as 128-bit parts.
pub fn read_element(vector: &[u128; 4], index: usize, bytes: usize) -> u64 {
    let bits = bytes * 8;
    let start = index * bits;
    let part = vector[start / 128] >> (start % 128);
    if bits == 64 { part as u64 } else { part as u64 & ((1 << bits) - 1) }
}

/// The vector with the element of `bytes` bytes at the index replaced.
pub fn write_element(vector: &mut [u128; 4], index: usize, bytes: usize, value: u64) {
    let bits = bytes * 8;
    let start = index * bits;
    let mask = if bits == 64 { u128::from(u64::MAX) } else { (1u128 << bits) - 1 };
    let part = &mut vector[start / 128];
    *part = (*part & !(mask << (start % 128))) | ((u128::from(value) & mask) << (start % 128));
}
