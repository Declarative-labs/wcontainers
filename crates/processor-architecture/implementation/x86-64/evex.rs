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
    ]
};

/// One modeled EVEX form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexForm {
    pub instruction: Instruction,
    pub code: Code,
    pub operation: EvexOperation,
    pub place: OperandPlace,
    /// The bytes of one element of the memory operand, which a broadcast reads; 0 for a register form.
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
            EvexOperation::Float { precision, .. } => (precision.to_bits() / u8::BITS) as usize,
            EvexOperation::Integer { element_bytes, .. } => element_bytes,
        }
    }

    /// The elements the form computes: one for a scalar form.
    pub fn elements(&self) -> usize {
        match self.operation {
            EvexOperation::Float { arrangement: Arrangement::Scalar, .. } => 1,
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

    /// Whether the form reads its destination's value besides merging into it.
    pub const fn reads_destination(&self) -> bool {
        match self.operation {
            EvexOperation::Float { .. } => false,
            EvexOperation::Integer { operation, .. } => operation.reads_destination(),
        }
    }

    /// Whether a fault on an element of the memory operand the opmask leaves out is suppressed: for the forms whose
    /// memory elements are the destination's elements.
    pub const fn suppresses_faults(&self) -> bool {
        match self.operation {
            EvexOperation::Float { .. } => true,
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
        memory_element_bytes: if in_memory { decoded.memory_size().element_size() } else { 0 },
    })
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
