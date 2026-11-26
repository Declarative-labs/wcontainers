//! The AVX-512 opmask forms: KMOV, the logical forms, KADD, the shifts, KUNPCK, KORTEST and KTEST, each on the low
//! 8, 16, 32 or 64 bits of k0 to k7, as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (volume 2) defines them.
//!
//! A form that writes an opmask register zeroes it above the form's width; KUNPCK's width is its result's.

use iced_x86::Code;

use crate::forms::Instruction;

/// What an opmask form computes from its sources.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpmaskOperation {
    Add,
    And,
    /// The first source's complement and the second.
    AndNot,
    Or,
    /// The complement of the exclusive or.
    Xnor,
    Xor,
    Not,
    Move,
    /// KORTEST: ZF where the sources' or is zero, CF where it is all ones; the other status flags cleared.
    OrTest,
    /// KTEST: ZF where the sources share no bit, CF where the second has none the first lacks; the others cleared.
    Test,
    /// By the immediate; zero once it reaches the width.
    ShiftLeft,
    ShiftRight,
    /// KUNPCK: the second source's low half below the first source's low half.
    Unpack,
}

/// Where an opmask form's operands lie.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpmaskPlace {
    /// Opmask registers alone (and KSHIFT's immediate).
    Masks,
    /// KMOV into a general register, 32 bits wide but for KMOVQ's.
    ToGeneral,
    /// KMOV from a general register.
    FromGeneral,
    /// KMOV from memory of the form's width.
    Load,
    /// KMOV to memory of the form's width.
    Store,
}

/// One opmask form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OpmaskForm {
    pub instruction: Instruction,
    pub operation: OpmaskOperation,
    /// The bits of the registers the form reads and writes.
    pub bits: u32,
    pub place: OpmaskPlace,
    pub code: Code,
}

impl OpmaskOperation {
    /// Applies the operation to the two sources' low `bits` (the second ignored by a form of one source) and the
    /// immediate; for KORTEST and KTEST, ZF in bit 6 and CF in bit 0 of the result, as RFLAGS holds them.
    pub fn apply(self, bits: u32, first: u64, second: u64, immediate: u8) -> u64 {
        let result = self.compute(bits, first, second, immediate);
        crate::planted::mutate_opmask(self, bits, first, second, immediate, result)
    }

    pub(crate) fn compute(self, bits: u32, first: u64, second: u64, immediate: u8) -> u64 {
        let mask = to_mask(bits);
        let (first, second) = (first & mask, second & mask);
        let count = u32::from(immediate);
        let flag = |zero: bool, carry: bool| (u64::from(zero) << 6) | u64::from(carry);
        match self {
            Self::Add => first.wrapping_add(second) & mask,
            Self::And => first & second,
            Self::AndNot => !first & second,
            Self::Or => first | second,
            Self::Xnor => !(first ^ second) & mask,
            Self::Xor => first ^ second,
            Self::Not => !first & mask,
            Self::Move => first,
            Self::OrTest => flag(first | second == 0, first | second == mask),
            Self::Test => flag(first & second == 0, !first & second & mask == 0),
            Self::ShiftLeft if count >= bits => 0,
            Self::ShiftLeft => (first << count) & mask,
            Self::ShiftRight if count >= bits => 0,
            Self::ShiftRight => first >> count,
            Self::Unpack => {
                let half = to_mask(bits / 2);
                (second & half) | ((first & half) << (bits / 2))
            }
        }
    }

    /// Whether the operation writes RFLAGS rather than an opmask register.
    pub const fn writes_flags(self) -> bool {
        matches!(self, Self::OrTest | Self::Test)
    }

    /// Whether the operation reads one source.
    pub const fn reads_one_source(self) -> bool {
        matches!(self, Self::Not | Self::Move | Self::ShiftLeft | Self::ShiftRight)
    }
}

/// The low `bits` bits set.
pub const fn to_mask(bits: u32) -> u64 {
    if bits >= u64::BITS { u64::MAX } else { (1 << bits) - 1 }
}

impl OpmaskForm {
    /// The bits KUNPCK reads of each source, the width of every other form.
    pub const fn to_source_bits(&self) -> u32 {
        match self.operation {
            OpmaskOperation::Unpack => self.bits / 2,
            _ => self.bits,
        }
    }
}

/// The row of an opmask form, or `None` for any other form.
pub fn find_opmask_form(instruction: Instruction) -> Option<&'static OpmaskForm> {
    OPMASK_FORMS.iter().find(|form| form.instruction == instruction)
}

macro_rules! opmask_forms {
    ($($instruction:ident, $operation:ident, $bits:literal, $place:ident, $code:ident;)*) => {
        &[$(OpmaskForm {
            instruction: Instruction::$instruction,
            operation: OpmaskOperation::$operation,
            bits: $bits,
            place: OpmaskPlace::$place,
            code: Code::$code,
        },)*]
    };
}

/// Every opmask form of the list.
pub static OPMASK_FORMS: &[OpmaskForm] = opmask_forms! {
    KaddbMaskmskwMaskmskwMaskmskwAvx512, Add, 8, Masks, VEX_Kaddb_kr_kr_kr;
    KadddMaskmskwMaskmskwMaskmskwAvx512, Add, 32, Masks, VEX_Kaddd_kr_kr_kr;
    KaddqMaskmskwMaskmskwMaskmskwAvx512, Add, 64, Masks, VEX_Kaddq_kr_kr_kr;
    KaddwMaskmskwMaskmskwMaskmskwAvx512, Add, 16, Masks, VEX_Kaddw_kr_kr_kr;
    KandbMaskmskwMaskmskwMaskmskwAvx512, And, 8, Masks, VEX_Kandb_kr_kr_kr;
    KanddMaskmskwMaskmskwMaskmskwAvx512, And, 32, Masks, VEX_Kandd_kr_kr_kr;
    KandnbMaskmskwMaskmskwMaskmskwAvx512, AndNot, 8, Masks, VEX_Kandnb_kr_kr_kr;
    KandndMaskmskwMaskmskwMaskmskwAvx512, AndNot, 32, Masks, VEX_Kandnd_kr_kr_kr;
    KandnqMaskmskwMaskmskwMaskmskwAvx512, AndNot, 64, Masks, VEX_Kandnq_kr_kr_kr;
    KandnwMaskmskwMaskmskwMaskmskwAvx512, AndNot, 16, Masks, VEX_Kandnw_kr_kr_kr;
    KandqMaskmskwMaskmskwMaskmskwAvx512, And, 64, Masks, VEX_Kandq_kr_kr_kr;
    KandwMaskmskwMaskmskwMaskmskwAvx512, And, 16, Masks, VEX_Kandw_kr_kr_kr;
    KmovbGpr32u32MaskmskwAvx512, Move, 8, ToGeneral, VEX_Kmovb_r32_kr;
    KmovbMaskmskwGpr32u32Avx512, Move, 8, FromGeneral, VEX_Kmovb_kr_r32;
    KmovbMaskmskwMasku8Avx512, Move, 8, Masks, VEX_Kmovb_kr_km8;
    KmovbMaskmskwMemu8Avx512, Move, 8, Load, VEX_Kmovb_kr_km8;
    KmovbMemu8MaskmskwAvx512, Move, 8, Store, VEX_Kmovb_m8_kr;
    KmovdGpr32u32MaskmskwAvx512, Move, 32, ToGeneral, VEX_Kmovd_r32_kr;
    KmovdMaskmskwGpr32u32Avx512, Move, 32, FromGeneral, VEX_Kmovd_kr_r32;
    KmovdMaskmskwMasku32Avx512, Move, 32, Masks, VEX_Kmovd_kr_km32;
    KmovdMaskmskwMemu32Avx512, Move, 32, Load, VEX_Kmovd_kr_km32;
    KmovdMemu32MaskmskwAvx512, Move, 32, Store, VEX_Kmovd_m32_kr;
    KmovqGpr64u64MaskmskwAvx512, Move, 64, ToGeneral, VEX_Kmovq_r64_kr;
    KmovqMaskmskwGpr64u64Avx512, Move, 64, FromGeneral, VEX_Kmovq_kr_r64;
    KmovqMaskmskwMasku64Avx512, Move, 64, Masks, VEX_Kmovq_kr_km64;
    KmovqMaskmskwMemu64Avx512, Move, 64, Load, VEX_Kmovq_kr_km64;
    KmovqMemu64MaskmskwAvx512, Move, 64, Store, VEX_Kmovq_m64_kr;
    KmovwGpr32u32MaskmskwAvx512, Move, 16, ToGeneral, VEX_Kmovw_r32_kr;
    KmovwMaskmskwGpr32u32Avx512, Move, 16, FromGeneral, VEX_Kmovw_kr_r32;
    KmovwMaskmskwMasku16Avx512, Move, 16, Masks, VEX_Kmovw_kr_km16;
    KmovwMaskmskwMemu16Avx512, Move, 16, Load, VEX_Kmovw_kr_km16;
    KmovwMemu16MaskmskwAvx512, Move, 16, Store, VEX_Kmovw_m16_kr;
    KnotbMaskmskwMaskmskwAvx512, Not, 8, Masks, VEX_Knotb_kr_kr;
    KnotdMaskmskwMaskmskwAvx512, Not, 32, Masks, VEX_Knotd_kr_kr;
    KnotqMaskmskwMaskmskwAvx512, Not, 64, Masks, VEX_Knotq_kr_kr;
    KnotwMaskmskwMaskmskwAvx512, Not, 16, Masks, VEX_Knotw_kr_kr;
    KorbMaskmskwMaskmskwMaskmskwAvx512, Or, 8, Masks, VEX_Korb_kr_kr_kr;
    KordMaskmskwMaskmskwMaskmskwAvx512, Or, 32, Masks, VEX_Kord_kr_kr_kr;
    KorqMaskmskwMaskmskwMaskmskwAvx512, Or, 64, Masks, VEX_Korq_kr_kr_kr;
    KortestbMaskmskwMaskmskwAvx512, OrTest, 8, Masks, VEX_Kortestb_kr_kr;
    KortestdMaskmskwMaskmskwAvx512, OrTest, 32, Masks, VEX_Kortestd_kr_kr;
    KortestqMaskmskwMaskmskwAvx512, OrTest, 64, Masks, VEX_Kortestq_kr_kr;
    KortestwMaskmskwMaskmskwAvx512, OrTest, 16, Masks, VEX_Kortestw_kr_kr;
    KorwMaskmskwMaskmskwMaskmskwAvx512, Or, 16, Masks, VEX_Korw_kr_kr_kr;
    KshiftlbMaskmskwMaskmskwImm8Avx512, ShiftLeft, 8, Masks, VEX_Kshiftlb_kr_kr_imm8;
    KshiftldMaskmskwMaskmskwImm8Avx512, ShiftLeft, 32, Masks, VEX_Kshiftld_kr_kr_imm8;
    KshiftlqMaskmskwMaskmskwImm8Avx512, ShiftLeft, 64, Masks, VEX_Kshiftlq_kr_kr_imm8;
    KshiftlwMaskmskwMaskmskwImm8Avx512, ShiftLeft, 16, Masks, VEX_Kshiftlw_kr_kr_imm8;
    KshiftrbMaskmskwMaskmskwImm8Avx512, ShiftRight, 8, Masks, VEX_Kshiftrb_kr_kr_imm8;
    KshiftrdMaskmskwMaskmskwImm8Avx512, ShiftRight, 32, Masks, VEX_Kshiftrd_kr_kr_imm8;
    KshiftrqMaskmskwMaskmskwImm8Avx512, ShiftRight, 64, Masks, VEX_Kshiftrq_kr_kr_imm8;
    KshiftrwMaskmskwMaskmskwImm8Avx512, ShiftRight, 16, Masks, VEX_Kshiftrw_kr_kr_imm8;
    KtestbMaskmskwMaskmskwAvx512, Test, 8, Masks, VEX_Ktestb_kr_kr;
    KtestdMaskmskwMaskmskwAvx512, Test, 32, Masks, VEX_Ktestd_kr_kr;
    KtestqMaskmskwMaskmskwAvx512, Test, 64, Masks, VEX_Ktestq_kr_kr;
    KtestwMaskmskwMaskmskwAvx512, Test, 16, Masks, VEX_Ktestw_kr_kr;
    KunpckbwMaskmskwMaskmskwMaskmskwAvx512, Unpack, 16, Masks, VEX_Kunpckbw_kr_kr_kr;
    KunpckdqMaskmskwMaskmskwMaskmskwAvx512, Unpack, 64, Masks, VEX_Kunpckdq_kr_kr_kr;
    KunpckwdMaskmskwMaskmskwMaskmskwAvx512, Unpack, 32, Masks, VEX_Kunpckwd_kr_kr_kr;
    KxnorbMaskmskwMaskmskwMaskmskwAvx512, Xnor, 8, Masks, VEX_Kxnorb_kr_kr_kr;
    KxnordMaskmskwMaskmskwMaskmskwAvx512, Xnor, 32, Masks, VEX_Kxnord_kr_kr_kr;
    KxnorqMaskmskwMaskmskwMaskmskwAvx512, Xnor, 64, Masks, VEX_Kxnorq_kr_kr_kr;
    KxnorwMaskmskwMaskmskwMaskmskwAvx512, Xnor, 16, Masks, VEX_Kxnorw_kr_kr_kr;
    KxorbMaskmskwMaskmskwMaskmskwAvx512, Xor, 8, Masks, VEX_Kxorb_kr_kr_kr;
    KxordMaskmskwMaskmskwMaskmskwAvx512, Xor, 32, Masks, VEX_Kxord_kr_kr_kr;
    KxorqMaskmskwMaskmskwMaskmskwAvx512, Xor, 64, Masks, VEX_Kxorq_kr_kr_kr;
    KxorwMaskmskwMaskmskwMaskmskwAvx512, Xor, 16, Masks, VEX_Kxorw_kr_kr_kr;
};
