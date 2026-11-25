//! The AVX-512 forms of the second half that share one skeleton: each element of the destination computed from the
//! sources' elements of the same index where the opmask register selects it, and merged or zeroed elsewhere, with
//! a memory source read element by element (or one element broadcast), embedded rounding and exception suppression.
//!
//! A form is a listed form whose sample encoding is an EVEX form of a mnemonic of [`EVEX_OPERATIONS`]; its iced-x86
//! `Code` is the sample's, and its place is the sample's: memory where the sample has a memory operand.

use alloc::vec::Vec;

use iced_x86::{Code, Decoder, DecoderOptions, EncodingKind, Mnemonic, OpKind};

use crate::float::{Arrangement, FloatOperation, Precision};
use crate::forms::{INSTRUCTIONS, Instruction, SAMPLE_ENCODINGS};
use crate::integer::OperandPlace;
use crate::{OPMASK_REGISTERS, Processor, VECTOR_REGISTERS};

/// Bytes of a ZMM register.
pub const ZMM_BYTES: usize = 64;

/// What a form computes in each element it writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvexOperation {
    /// VADDPS to VMAXSD: the element as the legacy operation computes a lane, under MXCSR or the embedded rounding.
    Float {
        operation: FloatOperation,
        precision: Precision,
        arrangement: Arrangement,
    },
}

/// The mnemonic of each modeled EVEX operation.
pub static EVEX_OPERATIONS: &[(Mnemonic, EvexOperation)] = &{
    use Arrangement::{Packed, Scalar};
    use FloatOperation::{Add, Divide, Maximum, Minimum, Multiply, SquareRoot, Subtract};
    use Precision::{Double, Single};
    const fn float(operation: FloatOperation, precision: Precision, arrangement: Arrangement) -> EvexOperation {
        EvexOperation::Float { operation, precision, arrangement }
    }
    [
        (Mnemonic::Vaddps, float(Add, Single, Packed)),
        (Mnemonic::Vaddpd, float(Add, Double, Packed)),
        (Mnemonic::Vaddss, float(Add, Single, Scalar)),
        (Mnemonic::Vaddsd, float(Add, Double, Scalar)),
        (Mnemonic::Vsubps, float(Subtract, Single, Packed)),
        (Mnemonic::Vsubpd, float(Subtract, Double, Packed)),
        (Mnemonic::Vsubss, float(Subtract, Single, Scalar)),
        (Mnemonic::Vsubsd, float(Subtract, Double, Scalar)),
        (Mnemonic::Vmulps, float(Multiply, Single, Packed)),
        (Mnemonic::Vmulpd, float(Multiply, Double, Packed)),
        (Mnemonic::Vmulss, float(Multiply, Single, Scalar)),
        (Mnemonic::Vmulsd, float(Multiply, Double, Scalar)),
        (Mnemonic::Vdivps, float(Divide, Single, Packed)),
        (Mnemonic::Vdivpd, float(Divide, Double, Packed)),
        (Mnemonic::Vdivss, float(Divide, Single, Scalar)),
        (Mnemonic::Vdivsd, float(Divide, Double, Scalar)),
        (Mnemonic::Vsqrtps, float(SquareRoot, Single, Packed)),
        (Mnemonic::Vsqrtpd, float(SquareRoot, Double, Packed)),
        (Mnemonic::Vsqrtss, float(SquareRoot, Single, Scalar)),
        (Mnemonic::Vsqrtsd, float(SquareRoot, Double, Scalar)),
        (Mnemonic::Vminps, float(Minimum, Single, Packed)),
        (Mnemonic::Vminpd, float(Minimum, Double, Packed)),
        (Mnemonic::Vminss, float(Minimum, Single, Scalar)),
        (Mnemonic::Vminsd, float(Minimum, Double, Scalar)),
        (Mnemonic::Vmaxps, float(Maximum, Single, Packed)),
        (Mnemonic::Vmaxpd, float(Maximum, Double, Packed)),
        (Mnemonic::Vmaxss, float(Maximum, Single, Scalar)),
        (Mnemonic::Vmaxsd, float(Maximum, Double, Scalar)),
    ]
};

/// One modeled EVEX form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexForm {
    pub instruction: Instruction,
    pub code: Code,
    pub operation: EvexOperation,
    pub place: OperandPlace,
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

    /// The bytes of one element.
    pub const fn element_bytes(&self) -> usize {
        match self.operation {
            EvexOperation::Float { precision, .. } => (precision.to_bits() / u8::BITS) as usize,
        }
    }

    /// The elements the form computes: one for a scalar form.
    pub fn elements(&self) -> usize {
        match self.operation {
            EvexOperation::Float { arrangement: Arrangement::Scalar, .. } => 1,
            EvexOperation::Float { .. } => self.vector_bytes() / self.element_bytes(),
        }
    }

    /// The bytes a memory source reaches without broadcast: every element's.
    pub fn memory_bytes(&self) -> usize {
        self.elements() * self.element_bytes()
    }

    /// Whether the form reads a first source from EVEX.vvvv: every form but a square root's packed one.
    pub fn reads_first_source(&self) -> bool {
        !matches!(
            self.operation,
            EvexOperation::Float { operation: FloatOperation::SquareRoot, arrangement: Arrangement::Packed, .. }
        )
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
    let (_, operation) = EVEX_OPERATIONS.iter().find(|(mnemonic, _)| *mnemonic == decoded.mnemonic())?;
    let in_memory = (0..decoded.op_count()).any(|operand| decoded.op_kind(operand) == OpKind::Memory);
    Some(EvexForm {
        instruction,
        code: decoded.code(),
        operation: *operation,
        place: if in_memory { OperandPlace::Memory } else { OperandPlace::Register },
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
