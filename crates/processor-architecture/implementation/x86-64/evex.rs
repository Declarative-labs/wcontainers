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
    /// VFMADD132PS to VFMSUBADD231PD: a product and an addend from the destination and the two sources, summed
    /// exactly and rounded once.
    Fused {
        kind: FusedKind,
        order: FusedOrder,
        precision: Precision,
        arrangement: Arrangement,
    },
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
    use Precision::{Double, Single};
    const fn float(operation: FloatOperation, precision: Precision, arrangement: Arrangement) -> EvexOperation {
        EvexOperation::Float { operation, precision, arrangement }
    }
    const fn integer(operation: IntegerOperation, element_bytes: usize) -> EvexOperation {
        EvexOperation::Integer { operation, element_bytes }
    }
    use FusedKind as F;
    use FusedOrder as O;
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
            EvexOperation::Float { precision, .. } | EvexOperation::Fused { precision, .. } => {
                (precision.to_bits() / u8::BITS) as usize
            }
            EvexOperation::Integer { element_bytes, .. } => element_bytes,
        }
    }

    /// The elements the form computes: one for a scalar form.
    pub fn elements(&self) -> usize {
        match self.operation {
            EvexOperation::Float { arrangement: Arrangement::Scalar, .. }
            | EvexOperation::Fused { arrangement: Arrangement::Scalar, .. } => 1,
            _ => self.vector_bytes() / self.element_bytes(),
        }
    }

    /// The bytes a memory source reaches without broadcast: every element's.
    pub fn memory_bytes(&self) -> usize {
        self.elements() * self.element_bytes()
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
            EvexOperation::Float { .. } => false,
            EvexOperation::Fused { .. } => true,
            EvexOperation::Integer { operation, .. } => operation.reads_destination(),
        }
    }

    /// Whether a fault on an element of the memory operand the opmask leaves out is suppressed: for the forms whose
    /// memory elements are the destination's elements.
    pub const fn suppresses_faults(&self) -> bool {
        match self.operation {
            EvexOperation::Float { .. } | EvexOperation::Fused { .. } => true,
            EvexOperation::Integer { operation, .. } => operation.is_element_wise(),
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
    Some(EvexForm {
        instruction,
        code: decoded.code(),
        operation: *operation,
        place: if in_memory { OperandPlace::Memory } else { OperandPlace::Register },
        memory_element_bytes: if in_memory { to_broadcast_bytes(&decoded) } else { 0 },
    })
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
