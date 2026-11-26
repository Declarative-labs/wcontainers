//! Optional forms the earlier families left with "undefined instruction" alone though this chip runs them: the EVEX
//! moves and extractions to a general register or memory, EVEX VMOVSS and VMOVSD, VCVTPS2PH to memory, the VEX forms
//! of RCPPS to RSQRTSS, VRCP14 and VRSQRT14 with AVX512-FP16's VRCP and VRSQRT, MOVDIRI and MOVDIR64B, RDFSBASE to
//! WRGSBASE, UMONITOR and XBEGIN.
//!
//! The [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (vol. 2) bounds
//! VRCP14's and VRSQRT14's results by a relative error of 2 to the -14 and VRCPPH's and VRSQRTPH's by 2 to the -11, so
//! a chip's own tables decide them. On this chip VRCP14 gives a significand of 16 bits from a table read by the
//! source's top 16 significand bits, VRSQRT14 one read by its exponent's parity and top 15 bits; single and double
//! precision share the tables. DAZ reads the source and FTZ flushes VRCP14's tiny results; no form sets an MXCSR
//! flag. The half-precision forms read neither DAZ nor FTZ: their significand comes from tables of 10 bits, and
//! VRCPPH's tiny results from a table of their own.

use alloc::vec;
use alloc::vec::Vec;

use iced_x86::{Code, Decoder, DecoderOptions, OpCodeOperandKind};

use crate::behavior::StoreAcrossFault;
use crate::float::Precision;
use crate::forms::Instruction;
use crate::integer::OperandPlace;
use crate::planted::{Bug, is_active};
use crate::{PreciseInstruction, SAMPLE_ENCODINGS, X87};

/// What a form of this family does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionalOperation {
    /// EVEX VMOVD, VMOVQ, VPEXTRB to VPEXTRQ and VEXTRACTPS: the element of this many bytes the immediate's low bits
    /// number (VMOVD's and VMOVQ's lowest), to a general register zero-extended, or to memory.
    ExtractElement { bytes: u64 },
    /// EVEX VMOVHPS, VMOVHPD (`high`), VMOVLPS and VMOVLPD to memory: one quadword of the source.
    StoreQuadword { high: bool },
    /// EVEX VMOVSS and VMOVSD: the lowest element under the writemask; a load or a move of registers zeroes the rest
    /// of the destination up to its widest, a move of registers taking bits up to 127 from the first source.
    MoveScalar { bytes: u64 },
    /// EVEX VCVTPS2PH to memory: each selected single rounded to a half, as the register form rounds it, and stored.
    ConvertToHalves,
    /// The VEX forms of RCPPS, RCPSS, RSQRTPS and RSQRTSS: the legacy forms' tables.
    VexReciprocal { root: bool },
    /// VRCP14, VRSQRT14, VRCPPH, VRSQRTPH and their scalar forms.
    Reciprocal { root: bool, precision: Precision },
    /// MOVDIRI: the general register stored, as a MOV would.
    DirectStore,
    /// MOVDIR64B: 64 bytes from memory stored at the address the general register holds, which must lie on 64 bytes.
    DirectStoreBlock,
    /// RDFSBASE and RDGSBASE (`gs`): the segment base into the general register, its low half for a 32-bit form.
    ReadBase { gs: bool },
    /// WRFSBASE and WRGSBASE: the general register into the segment base, which must be canonical.
    WriteBase { gs: bool },
    /// UMONITOR: arms the monitor at the address the register holds; on this chip no address faults, a
    /// non-canonical one or one on a page that refuses a read included.
    Monitor,
    /// XBEGIN on a chip whose transactions always abort: EAX the abort status 0, and a jump to the fallback.
    TransactionBegin,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OptionalForm {
    pub instruction: Instruction,
    pub operation: OptionalOperation,
    pub place: OperandPlace,
}

impl OptionalForm {
    /// The codes that encode the form: for a form whose operand size or address size picks the register, each
    /// size's, 32 bits first; else its sample encoding's.
    pub fn list_codes(&self) -> Vec<Code> {
        let codes: &[Code] = match self.operation {
            OptionalOperation::ReadBase { gs: false } => &[Code::Rdfsbase_r32, Code::Rdfsbase_r64],
            OptionalOperation::ReadBase { gs: true } => &[Code::Rdgsbase_r32, Code::Rdgsbase_r64],
            OptionalOperation::WriteBase { gs: false } => &[Code::Wrfsbase_r32, Code::Wrfsbase_r64],
            OptionalOperation::WriteBase { gs: true } => &[Code::Wrgsbase_r32, Code::Wrgsbase_r64],
            OptionalOperation::Monitor => &[Code::Umonitor_r32, Code::Umonitor_r64],
            OptionalOperation::DirectStoreBlock => &[Code::Movdir64b_r32_m512, Code::Movdir64b_r64_m512],
            OptionalOperation::TransactionBegin => &[Code::Xbegin_rel16, Code::Xbegin_rel32],
            _ => {
                let sample = SAMPLE_ENCODINGS[self.instruction as usize];
                return vec![Decoder::new(64, sample, DecoderOptions::NONE).decode().code()];
            }
        };
        codes.to_vec()
    }

    /// The form's code; the last of several, its 64-bit one.
    pub fn code(&self) -> Code {
        let codes = self.list_codes();
        codes[codes.len() - 1]
    }

    /// The bytes of the vector the form reads or writes: 16, 32 or 64; 16 for a scalar form.
    pub fn vector_bytes(&self) -> u64 {
        match self.code().op_code().l() {
            0 => 16,
            1 => 32,
            _ => 64,
        }
    }

    /// The bytes of one element.
    pub fn element_bytes(&self) -> u64 {
        match self.operation {
            OptionalOperation::ExtractElement { bytes } | OptionalOperation::MoveScalar { bytes } => bytes,
            OptionalOperation::StoreQuadword { .. } => 8,
            OptionalOperation::ConvertToHalves => 4,
            OptionalOperation::VexReciprocal { .. } => 4,
            OptionalOperation::Reciprocal { precision, .. } => u64::from(precision.to_bits() / u8::BITS),
            _ => 8,
        }
    }

    /// Whether a reciprocal form computes the lowest element alone, the rest of its first source kept.
    pub fn is_scalar(&self) -> bool {
        self.code().op_code().op_kinds().iter().any(|kind| matches!(kind, OpCodeOperandKind::xmm_vvvv))
    }

    /// Whether the form takes a writemask.
    pub fn can_mask(&self) -> bool {
        self.code().op_code().can_use_op_mask_register()
    }

    /// Whether a writemask of the form may zero.
    pub fn can_zero(&self) -> bool {
        self.code().op_code().can_use_zeroing_masking()
    }

    /// Whether the form's memory source may be one element broadcast.
    pub fn can_broadcast(&self) -> bool {
        self.place == OperandPlace::Memory && self.code().op_code().can_broadcast()
    }

    /// Whether the form ends with an 8-bit immediate.
    pub fn has_immediate(&self) -> bool {
        self.code().op_code().op_kinds().contains(&OpCodeOperandKind::imm8)
    }

    /// Whether the form writes its memory operand.
    pub fn writes_memory(&self) -> bool {
        self.place == OperandPlace::Memory
            && match self.operation {
                OptionalOperation::ExtractElement { .. }
                | OptionalOperation::StoreQuadword { .. }
                | OptionalOperation::ConvertToHalves
                | OptionalOperation::DirectStore => true,
                OptionalOperation::MoveScalar { .. } => {
                    matches!(self.code().op_code().op_kinds().first(), Some(OpCodeOperandKind::mem))
                }
                _ => false,
            }
    }

    /// The bytes of the memory operand: those a whole vector or element reaches, MOVDIR64B's 64.
    pub fn memory_bytes(&self, broadcast: bool) -> u64 {
        match self.operation {
            OptionalOperation::ConvertToHalves => self.vector_bytes() / 2,
            OptionalOperation::DirectStoreBlock => BLOCK_BYTES,
            OptionalOperation::DirectStore if self.instruction == Instruction::MovdiriMemu32Gpr32u32 => 4,
            OptionalOperation::Reciprocal { .. } | OptionalOperation::VexReciprocal { .. } if !self.is_scalar() && !broadcast => {
                self.vector_bytes()
            }
            _ => self.element_bytes(),
        }
    }

    /// The elements the form computes or moves.
    pub fn elements(&self) -> usize {
        match self.operation {
            OptionalOperation::ConvertToHalves => (self.vector_bytes() / 4) as usize,
            OptionalOperation::Reciprocal { .. } | OptionalOperation::VexReciprocal { .. } if !self.is_scalar() => {
                (self.vector_bytes() / self.element_bytes()) as usize
            }
            _ => 1,
        }
    }
}

/// MOVDIR64B's block, which its destination must be aligned to.
pub const BLOCK_BYTES: u64 = 64;

/// The row of a form of this family, or `None` for any other form.
pub fn find_optional_form(instruction: Instruction) -> Option<&'static OptionalForm> {
    OPTIONAL_FORMS.iter().find(|form| form.instruction == instruction)
}

const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];

/// For a store of more than one byte that can lie across pages both rules of a store that faults part-way, else one
/// behavior.
pub(crate) fn list_optional_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_optional_form(instruction) else {
        return Vec::new();
    };
    let splits = form.writes_memory() && form.memory_bytes(false) > 1;
    let across: &[StoreAcrossFault] = if splits { &ACROSS_FAULT } else { &ACROSS_FAULT[..1] };
    across.iter().map(|&across_fault| PreciseInstruction::Optional { instruction, across_fault }).collect()
}

/// Runs an integer store of the x87 stack's top with FCW's rounding control set to round toward zero, as FISTTP
/// does, and restores FCW.
pub(crate) fn store_truncated<T>(x87: &mut X87, round: impl FnOnce(&mut X87) -> T) -> T {
    const ROUNDING_CONTROL: u16 = 0b11 << 10;
    let control = x87.control;
    x87.control |= ROUNDING_CONTROL;
    let result = round(x87);
    x87.control = control;
    result
}

/// A floating-point format's widths.
#[derive(Clone, Copy)]
struct Format {
    fraction_bits: u32,
    exponent_bits: u32,
}

impl Format {
    const fn of(precision: Precision) -> Self {
        match precision {
            Precision::Half => Self { fraction_bits: 10, exponent_bits: 5 },
            Precision::Single => Self { fraction_bits: 23, exponent_bits: 8 },
            _ => Self { fraction_bits: 52, exponent_bits: 11 },
        }
    }

    const fn maximum_exponent(self) -> i64 {
        (1 << self.exponent_bits) - 1
    }

    const fn bias(self) -> i64 {
        self.maximum_exponent() >> 1
    }

    const fn infinity(self) -> u64 {
        (self.maximum_exponent() as u64) << self.fraction_bits
    }

    const fn quiet_bit(self) -> u64 {
        1 << (self.fraction_bits - 1)
    }

    const fn fraction_mask(self) -> u64 {
        (1 << self.fraction_bits) - 1
    }

    const fn sign_bit(self) -> u64 {
        1 << (self.fraction_bits + self.exponent_bits)
    }
}

/// A finite nonzero source, its significand normalized: the biased exponent, below 1 for a denormal, and the
/// fraction bits.
fn normalize(format: Format, exponent: i64, fraction: u64) -> (i64, u64) {
    if exponent != 0 {
        return (exponent, fraction);
    }
    let shift = fraction.leading_zeros() - (u64::BITS - 1 - format.fraction_bits);
    ((1 - i64::from(shift)), (fraction << shift) & format.fraction_mask())
}

/// One element of VRCP14 or VRCPPH under MXCSR's DAZ and FTZ (which a half ignores).
pub fn compute_reciprocal(precision: Precision, source: u64, mxcsr: u32) -> u64 {
    let format = Format::of(precision);
    let half = precision == Precision::Half;
    let daz = !half && mxcsr & crate::float::DENORMALS_ARE_ZEROS != 0;
    let ftz = !half && mxcsr & crate::float::FLUSH_TO_ZERO != 0;
    let sign = source & format.sign_bit();
    let exponent = (source >> format.fraction_bits) as i64 & format.maximum_exponent();
    let fraction = source & format.fraction_mask();
    if exponent == format.maximum_exponent() {
        return if fraction != 0 { source | format.quiet_bit() } else { sign };
    }
    if exponent == 0 && (fraction == 0 || daz || is_active(Bug::ReciprocalDenormalsAreZeros)) {
        return sign | format.infinity();
    }
    let (exponent, fraction) = normalize(format, exponent, fraction);
    let biased = 2 * format.bias() - exponent - i64::from(fraction != 0);
    if biased >= format.maximum_exponent() {
        return sign | format.infinity();
    }
    let significand = match (fraction, half) {
        (0, _) => 0,
        (_, true) => u64::from(crate::optional_recording::RECIPROCAL_HALF[fraction as usize]),
        (_, false) => {
            let index = (fraction >> (format.fraction_bits - 16)) as usize;
            let index = if is_active(Bug::ReciprocalFourteenIndexFifteenBits) { index & !1 } else { index };
            u64::from(crate::optional_recording::RECIPROCAL_FOURTEEN[index]) << (format.fraction_bits - 16)
        }
    };
    if biased >= 1 {
        return sign | (biased as u64) << format.fraction_bits | significand;
    }
    // A tiny result; the largest source gives one at most two places below the smallest normal.
    if ftz {
        return sign;
    }
    if half {
        let index = ((exponent - (2 * format.bias() - 1)) as usize) << format.fraction_bits | fraction as usize;
        return sign | u64::from(crate::optional_recording::RECIPROCAL_HALF_TINY[index]);
    }
    let shift = 1 - biased;
    sign | ((1 << format.fraction_bits | significand) >> shift)
}

/// One element of VRSQRT14 or VRSQRTPH under MXCSR's DAZ (which a half ignores).
pub fn compute_root_reciprocal(precision: Precision, source: u64, mxcsr: u32) -> u64 {
    let format = Format::of(precision);
    let half = precision == Precision::Half;
    let daz = !half && mxcsr & crate::float::DENORMALS_ARE_ZEROS != 0;
    let sign = source & format.sign_bit();
    let exponent = (source >> format.fraction_bits) as i64 & format.maximum_exponent();
    let fraction = source & format.fraction_mask();
    let indefinite = format.sign_bit() | format.infinity() | format.quiet_bit();
    if exponent == format.maximum_exponent() {
        if fraction != 0 {
            return source | format.quiet_bit();
        }
        return if sign != 0 { indefinite } else { 0 };
    }
    if exponent == 0 && (fraction == 0 || daz) {
        return sign | format.infinity();
    }
    if sign != 0 {
        return indefinite;
    }
    let (exponent, fraction) = normalize(format, exponent, fraction);
    let unbiased = exponent - format.bias();
    let odd = unbiased & 1 != 0;
    let odd = if is_active(Bug::RootReciprocalParitySwapped) { !odd } else { odd };
    if !odd && fraction == 0 {
        return ((format.bias() - unbiased / 2) as u64) << format.fraction_bits;
    }
    // The source is 2 to an even power times a significand of 1 to 4, whose root reciprocal lies in 1/2 to 1.
    let even = if odd { unbiased - 1 } else { unbiased };
    let biased = format.bias() - even / 2 - 1;
    let parity = usize::from(odd);
    let significand = if half {
        u64::from(crate::optional_recording::ROOT_RECIPROCAL_HALF[parity << 10 | fraction as usize])
    } else {
        let index = parity << 15 | (fraction >> (format.fraction_bits - 15)) as usize;
        u64::from(crate::optional_recording::ROOT_RECIPROCAL_FOURTEEN[index]) << (format.fraction_bits - 16)
    };
    (biased as u64) << format.fraction_bits | significand
}

use OperandPlace::{Memory, Register};
use OptionalOperation::{
    ConvertToHalves, DirectStore, DirectStoreBlock, ExtractElement, Monitor, MoveScalar, ReadBase, Reciprocal,
    StoreQuadword, TransactionBegin, VexReciprocal, WriteBase,
};

const fn optional(instruction: Instruction, operation: OptionalOperation, place: OperandPlace) -> OptionalForm {
    OptionalForm { instruction, operation, place }
}

const fn reciprocal(instruction: Instruction, root: bool, precision: Precision, place: OperandPlace) -> OptionalForm {
    optional(instruction, Reciprocal { root, precision }, place)
}

use Precision::{Double, Half, Single};

/// Every form of this family.
pub static OPTIONAL_FORMS: &[OptionalForm] = &[
    optional(Instruction::VmovdGpr32u32Xmmu32Avx512, ExtractElement { bytes: 4 }, Register),
    optional(Instruction::VmovdMemu32Xmmu32Avx512, ExtractElement { bytes: 4 }, Memory),
    optional(Instruction::VmovqGpr64u64Xmmu64Avx512, ExtractElement { bytes: 8 }, Register),
    optional(Instruction::VmovqMemu64Xmmu64Avx512, ExtractElement { bytes: 8 }, Memory),
    optional(Instruction::VpextrbGpr32u8Xmmu8Imm8Avx512, ExtractElement { bytes: 1 }, Register),
    optional(Instruction::VpextrbMemu8Xmmu8Imm8Avx512, ExtractElement { bytes: 1 }, Memory),
    optional(Instruction::VpextrwGpr32u16Xmmu16Imm8Avx512, ExtractElement { bytes: 2 }, Register),
    optional(Instruction::VpextrwGpr32u16Xmmu16Imm8Avx512C5, ExtractElement { bytes: 2 }, Register),
    optional(Instruction::VpextrwMemu16Xmmu16Imm8Avx512, ExtractElement { bytes: 2 }, Memory),
    optional(Instruction::VpextrdGpr32u32Xmmu32Imm8Avx512, ExtractElement { bytes: 4 }, Register),
    optional(Instruction::VpextrdMemu32Xmmu32Imm8Avx512, ExtractElement { bytes: 4 }, Memory),
    optional(Instruction::VpextrqGpr64u64Xmmu64Imm8Avx512, ExtractElement { bytes: 8 }, Register),
    optional(Instruction::VpextrqMemu64Xmmu64Imm8Avx512, ExtractElement { bytes: 8 }, Memory),
    optional(Instruction::VextractpsGpr32f32Xmmf32Imm8Avx512, ExtractElement { bytes: 4 }, Register),
    optional(Instruction::VextractpsMemf32Xmmf32Imm8Avx512, ExtractElement { bytes: 4 }, Memory),
    optional(Instruction::VmovhpdMemf64Xmmf64Avx512, StoreQuadword { high: true }, Memory),
    optional(Instruction::VmovhpsMemf32Xmmf32Avx512, StoreQuadword { high: true }, Memory),
    optional(Instruction::VmovlpdMemf64Xmmf64Avx512, StoreQuadword { high: false }, Memory),
    optional(Instruction::VmovlpsMemf32Xmmf32Avx512, StoreQuadword { high: false }, Memory),
    optional(Instruction::VmovssXmmf32MaskmskwXmmf32Xmmf32Avx512, MoveScalar { bytes: 4 }, Register),
    optional(Instruction::VmovssXmmf32MaskmskwMemf32Avx512, MoveScalar { bytes: 4 }, Memory),
    optional(Instruction::VmovssMemf32MaskmskwXmmf32Avx512, MoveScalar { bytes: 4 }, Memory),
    optional(Instruction::VmovsdXmmf64MaskmskwXmmf64Xmmf64Avx512, MoveScalar { bytes: 8 }, Register),
    optional(Instruction::VmovsdXmmf64MaskmskwMemf64Avx512, MoveScalar { bytes: 8 }, Memory),
    optional(Instruction::VmovsdMemf64MaskmskwXmmf64Avx512, MoveScalar { bytes: 8 }, Memory),
    optional(Instruction::Vcvtps2phMemf16MaskmskwXmmf32Imm8Avx512, ConvertToHalves, Memory),
    optional(Instruction::Vcvtps2phMemf16MaskmskwYmmf32Imm8Avx512, ConvertToHalves, Memory),
    optional(Instruction::Vcvtps2phMemf16MaskmskwZmmf32Imm8Avx512, ConvertToHalves, Memory),
    optional(Instruction::VrcppsXmmdqXmmdq, VexReciprocal { root: false }, Register),
    optional(Instruction::VrcppsXmmdqMemdq, VexReciprocal { root: false }, Memory),
    optional(Instruction::VrcppsYmmqqYmmqq, VexReciprocal { root: false }, Register),
    optional(Instruction::VrcppsYmmqqMemqq, VexReciprocal { root: false }, Memory),
    optional(Instruction::VrcpssXmmdqXmmdqXmmd, VexReciprocal { root: false }, Register),
    optional(Instruction::VrcpssXmmdqXmmdqMemd, VexReciprocal { root: false }, Memory),
    optional(Instruction::VrsqrtpsXmmdqXmmdq, VexReciprocal { root: true }, Register),
    optional(Instruction::VrsqrtpsXmmdqMemdq, VexReciprocal { root: true }, Memory),
    optional(Instruction::VrsqrtpsYmmqqYmmqq, VexReciprocal { root: true }, Register),
    optional(Instruction::VrsqrtpsYmmqqMemqq, VexReciprocal { root: true }, Memory),
    optional(Instruction::VrsqrtssXmmdqXmmdqXmmd, VexReciprocal { root: true }, Register),
    optional(Instruction::VrsqrtssXmmdqXmmdqMemd, VexReciprocal { root: true }, Memory),
    reciprocal(Instruction::Vrcp14psXmmf32MaskmskwXmmf32Avx512, false, Single, Register),
    reciprocal(Instruction::Vrcp14psXmmf32MaskmskwMemf32Avx512, false, Single, Memory),
    reciprocal(Instruction::Vrcp14psYmmf32MaskmskwYmmf32Avx512, false, Single, Register),
    reciprocal(Instruction::Vrcp14psYmmf32MaskmskwMemf32Avx512, false, Single, Memory),
    reciprocal(Instruction::Vrcp14psZmmf32MaskmskwZmmf32Avx512, false, Single, Register),
    reciprocal(Instruction::Vrcp14psZmmf32MaskmskwMemf32Avx512, false, Single, Memory),
    reciprocal(Instruction::Vrcp14pdXmmf64MaskmskwXmmf64Avx512, false, Double, Register),
    reciprocal(Instruction::Vrcp14pdXmmf64MaskmskwMemf64Avx512, false, Double, Memory),
    reciprocal(Instruction::Vrcp14pdYmmf64MaskmskwYmmf64Avx512, false, Double, Register),
    reciprocal(Instruction::Vrcp14pdYmmf64MaskmskwMemf64Avx512, false, Double, Memory),
    reciprocal(Instruction::Vrcp14pdZmmf64MaskmskwZmmf64Avx512, false, Double, Register),
    reciprocal(Instruction::Vrcp14pdZmmf64MaskmskwMemf64Avx512, false, Double, Memory),
    reciprocal(Instruction::Vrcp14ssXmmf32MaskmskwXmmf32Xmmf32Avx512, false, Single, Register),
    reciprocal(Instruction::Vrcp14ssXmmf32MaskmskwXmmf32Memf32Avx512, false, Single, Memory),
    reciprocal(Instruction::Vrcp14sdXmmf64MaskmskwXmmf64Xmmf64Avx512, false, Double, Register),
    reciprocal(Instruction::Vrcp14sdXmmf64MaskmskwXmmf64Memf64Avx512, false, Double, Memory),
    reciprocal(Instruction::Vrsqrt14psXmmf32MaskmskwXmmf32Avx512, true, Single, Register),
    reciprocal(Instruction::Vrsqrt14psXmmf32MaskmskwMemf32Avx512, true, Single, Memory),
    reciprocal(Instruction::Vrsqrt14psYmmf32MaskmskwYmmf32Avx512, true, Single, Register),
    reciprocal(Instruction::Vrsqrt14psYmmf32MaskmskwMemf32Avx512, true, Single, Memory),
    reciprocal(Instruction::Vrsqrt14psZmmf32MaskmskwZmmf32Avx512, true, Single, Register),
    reciprocal(Instruction::Vrsqrt14psZmmf32MaskmskwMemf32Avx512, true, Single, Memory),
    reciprocal(Instruction::Vrsqrt14pdXmmf64MaskmskwXmmf64Avx512, true, Double, Register),
    reciprocal(Instruction::Vrsqrt14pdXmmf64MaskmskwMemf64Avx512, true, Double, Memory),
    reciprocal(Instruction::Vrsqrt14pdYmmf64MaskmskwYmmf64Avx512, true, Double, Register),
    reciprocal(Instruction::Vrsqrt14pdYmmf64MaskmskwMemf64Avx512, true, Double, Memory),
    reciprocal(Instruction::Vrsqrt14pdZmmf64MaskmskwZmmf64Avx512, true, Double, Register),
    reciprocal(Instruction::Vrsqrt14pdZmmf64MaskmskwMemf64Avx512, true, Double, Memory),
    reciprocal(Instruction::Vrsqrt14ssXmmf32MaskmskwXmmf32Xmmf32Avx512, true, Single, Register),
    reciprocal(Instruction::Vrsqrt14ssXmmf32MaskmskwXmmf32Memf32Avx512, true, Single, Memory),
    reciprocal(Instruction::Vrsqrt14sdXmmf64MaskmskwXmmf64Xmmf64Avx512, true, Double, Register),
    reciprocal(Instruction::Vrsqrt14sdXmmf64MaskmskwXmmf64Memf64Avx512, true, Double, Memory),
    reciprocal(Instruction::VrcpphXmmf16MaskmskwXmmf16Avx512, false, Half, Register),
    reciprocal(Instruction::VrcpphXmmf16MaskmskwMemf16Avx512, false, Half, Memory),
    reciprocal(Instruction::VrcpphYmmf16MaskmskwYmmf16Avx512, false, Half, Register),
    reciprocal(Instruction::VrcpphYmmf16MaskmskwMemf16Avx512, false, Half, Memory),
    reciprocal(Instruction::VrcpphZmmf16MaskmskwZmmf16Avx512, false, Half, Register),
    reciprocal(Instruction::VrcpphZmmf16MaskmskwMemf16Avx512, false, Half, Memory),
    reciprocal(Instruction::VrcpshXmmf16MaskmskwXmmf16Xmmf16Avx512, false, Half, Register),
    reciprocal(Instruction::VrcpshXmmf16MaskmskwXmmf16Memf16Avx512, false, Half, Memory),
    reciprocal(Instruction::VrsqrtphXmmf16MaskmskwXmmf16Avx512, true, Half, Register),
    reciprocal(Instruction::VrsqrtphXmmf16MaskmskwMemf16Avx512, true, Half, Memory),
    reciprocal(Instruction::VrsqrtphYmmf16MaskmskwYmmf16Avx512, true, Half, Register),
    reciprocal(Instruction::VrsqrtphYmmf16MaskmskwMemf16Avx512, true, Half, Memory),
    reciprocal(Instruction::VrsqrtphZmmf16MaskmskwZmmf16Avx512, true, Half, Register),
    reciprocal(Instruction::VrsqrtphZmmf16MaskmskwMemf16Avx512, true, Half, Memory),
    reciprocal(Instruction::VrsqrtshXmmf16MaskmskwXmmf16Xmmf16Avx512, true, Half, Register),
    reciprocal(Instruction::VrsqrtshXmmf16MaskmskwXmmf16Memf16Avx512, true, Half, Memory),
    optional(Instruction::MovdiriMemu32Gpr32u32, DirectStore, Memory),
    optional(Instruction::MovdiriMemu64Gpr64u64, DirectStore, Memory),
    optional(Instruction::Movdir64bGpraMem, DirectStoreBlock, Memory),
    optional(Instruction::RdfsbaseGpry, ReadBase { gs: false }, Register),
    optional(Instruction::RdgsbaseGpry, ReadBase { gs: true }, Register),
    optional(Instruction::WrfsbaseGpry, WriteBase { gs: false }, Register),
    optional(Instruction::WrgsbaseGpry, WriteBase { gs: true }, Register),
    optional(Instruction::UmonitorGpra, Monitor, Register),
    optional(Instruction::XbeginRelbrz, TransactionBegin, Register),
];
