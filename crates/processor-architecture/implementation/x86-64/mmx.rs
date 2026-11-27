//! The MMX forms: SSE's lane operations on the 64-bit MMX registers, the moves between them and the general
//! registers, memory and XMM, and EMMS; with SSE's and SSSE3's forms that name MMX registers.
//!
//! MMX registers are the low 64 bits of the x87 physical registers R0 to R7. Every MMX form but EMMS puts the x87
//! state in MMX's use, as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (vol. 1, 9.5) gives it: TOP becomes 0 and every
//! register valid, and a register MMX writes gets all ones in its bits 64 to 79. EMMS marks every register empty, and
//! on this chip also makes TOP 0.
//! Each raises #MF instead while an unmasked x87 exception is pending. The chip keeps FSW's ES and B equal to an
//! exception flag FCW does not mask: it sets or clears them so once it loads FSW, so a state holds them so.

use alloc::vec::Vec;

use iced_x86::Code;

use crate::behavior::StoreAcrossFault;
use crate::forms::Instruction;
use crate::integer::OperandPlace;
use crate::planted::{Bug, is_active};
use crate::vector::{Lane, LaneOperation, Signedness};
use crate::{PreciseInstruction, X87};

/// What an MMX form does with its operands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MmxOperation {
    /// SSE's lane operation on 64 bits: the destination's lanes and the source's, or a shift by the source's count.
    Lane(LaneOperation),
    /// MOVD and MOVQ into an MMX register, from a general register, memory or another MMX register; MOVD zero-extends.
    Load,
    /// MOVD, MOVQ and MOVNTQ from an MMX register; MOVD of a general register zero-extends.
    Store,
    /// PEXTRW: the word the immediate's low two bits name, zero-extended.
    ExtractWord,
    /// PINSRW: the source's low word into the word the immediate's low two bits name.
    InsertWord,
    /// PMOVMSKB: each byte's top bit.
    MoveMask,
    /// MOVQ2DQ: the MMX register zero-extended into the XMM register.
    MoveToVector,
    /// MOVDQ2Q: the XMM register's low quadword.
    MoveFromVector,
    /// EMMS.
    EmptyState,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MmxForm {
    pub instruction: Instruction,
    pub operation: MmxOperation,
    pub place: OperandPlace,
    pub code: Code,
}

/// An MMX form's behavior: what a store that faults part-way writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_x86_64::mmx)
)]
pub struct MmxBehavior {
    pub across_fault: StoreAcrossFault,
}

const TOP_MASK: u16 = 0b111 << 11;
const SUMMARY: u16 = 1 << 7;
/// The bits above the 64 MMX uses of the register an MMX form writes.
const EXPONENT_ONES: u128 = 0xFFFF << 64;
const ALL_VALID: u8 = 0xFF;

/// Whether an unmasked x87 exception is pending: FSW's ES, which a state keeps equal to an exception flag that FCW
/// does not mask, as the chip does.
pub const fn is_pending(x87: &X87) -> bool {
    x87.status & SUMMARY != 0
}

impl MmxForm {
    /// Whether the form writes its memory operand.
    pub fn writes_memory(&self) -> bool {
        self.place != OperandPlace::Register && self.operation == MmxOperation::Store
    }

    /// The bytes of the memory operand: 4 for MOVD and the unpacks of the low halves, 2 for PINSRW, else 8.
    pub fn memory_bytes(&self) -> u64 {
        match self.code {
            Code::Movd_mm_rm32 | Code::Movd_rm32_mm | Code::Punpcklbw_mm_mmm32 | Code::Punpcklwd_mm_mmm32
            | Code::Punpckldq_mm_mmm32 => 4,
            Code::Pinsrw_mm_r32m16_imm8 => 2,
            _ => 8,
        }
    }
}

/// Puts the x87 state in MMX's use, the register MMX writes, if any, holding the value.
pub(crate) fn enter_mmx(x87: &mut X87, written: Option<(usize, u64)>) {
    x87.status &= !TOP_MASK;
    x87.tags = ALL_VALID;
    if let Some((number, value)) = written {
        x87.registers[number] = EXPONENT_ONES | u128::from(value);
    }
}

/// The lane operation on the MMX register's 64 bits and the source's (for a shift, its count).
pub(crate) fn apply_lanes(operation: LaneOperation, destination: u64, source: u64, immediate: u8) -> u64 {
    let (first, second) = (u128::from(destination), u128::from(source));
    match operation {
        // The operations that join both operands' lanes take them as one 128-bit register.
        LaneOperation::Pack(..)
        | LaneOperation::HorizontalAdd(_)
        | LaneOperation::HorizontalAddSaturating
        | LaneOperation::HorizontalSubtract(_)
        | LaneOperation::HorizontalSubtractSaturating => operation.apply(first | second << 64, 0, immediate) as u64,
        LaneOperation::UnpackHigh(lane) if !is_active(Bug::MmxUnpackHighFromLowHalves) => {
            LaneOperation::UnpackLow(lane).apply(first >> 32, second >> 32, 0) as u64
        }
        LaneOperation::UnpackHigh(lane) => LaneOperation::UnpackLow(lane).apply(first, second, 0) as u64,
        LaneOperation::AlignRight => {
            let joined = first << 64 | second;
            if immediate >= 16 { 0 } else { (joined >> (8 * u32::from(immediate))) as u64 }
        }
        LaneOperation::ShuffleBytes => (0..8).fold(0, |result, position| {
            let selector = source >> (8 * position) & 0xFF;
            let chosen = if is_active(Bug::MmxShuffleBytesIndexFourBits) { selector & 0xF } else { selector & 7 };
            let byte = if selector & 0x80 != 0 || chosen >= 8 { 0 } else { destination >> (8 * chosen) & 0xFF };
            result | byte << (8 * position)
        }),
        _ => operation.apply(first, second, immediate) as u64,
    }
}

/// The row of an MMX form, or `None` for any other form.
pub fn find_mmx_form(instruction: Instruction) -> Option<&'static MmxForm> {
    MMX_FORMS.iter().find(|form| form.instruction == instruction)
}

const ACROSS_FAULT: [StoreAcrossFault; 2] = [StoreAcrossFault::WritesNothing, StoreAcrossFault::WritesBytesBelowFault];

/// For a store to memory both rules of a store that faults part-way, else one behavior.
pub(crate) fn list_mmx_behaviors(instruction: Instruction) -> Vec<PreciseInstruction> {
    let Some(form) = find_mmx_form(instruction) else {
        return Vec::new();
    };
    let across: &[StoreAcrossFault] = if form.writes_memory() { &ACROSS_FAULT } else { &ACROSS_FAULT[..1] };
    across.iter().map(|&across_fault| PreciseInstruction::Mmx { instruction, behavior: MmxBehavior { across_fault } }).collect()
}

const fn mmx(instruction: Instruction, operation: MmxOperation, place: OperandPlace, code: Code) -> MmxForm {
    MmxForm { instruction, operation, place, code }
}

use Lane::{Byte, Doubleword, Quadword, Word};
use LaneOperation::{
    Absolute, Add, AddSaturating, AlignRight, And, AndNot, Average, CompareEqual, CompareGreater, HorizontalAdd,
    HorizontalAddSaturating, HorizontalSubtract, HorizontalSubtractSaturating, Maximum, Minimum, MultiplyAddBytes,
    MultiplyAddWords, MultiplyDoublewords, MultiplyHighRoundScale, MultiplyHighWords, MultiplyLow, Or, Pack,
    ShiftLeft, ShiftRightArithmetic, ShiftRightLogical, ShuffleBytes, ShuffleLowWords, Sign, Subtract,
    SubtractSaturating, SumAbsoluteDifferences, UnpackHigh, UnpackLow, Xor,
};
use MmxOperation::{EmptyState, ExtractWord, InsertWord, Lane as Lanes, Load, MoveFromVector, MoveMask, MoveToVector, Store};
use OperandPlace::{Memory, Register};
use Signedness::{Signed, Unsigned};

/// Every MMX form modeled.
pub static MMX_FORMS: &[MmxForm] = &[
    mmx(Instruction::Emms, EmptyState, Register, Code::Emms),
    mmx(Instruction::Movdq2qMmxqXmmq, MoveFromVector, Register, Code::Movdq2q_mm_xmm),
    mmx(Instruction::MovdGpr32Mmxd, Store, Register, Code::Movd_rm32_mm),
    mmx(Instruction::MovdMemdMmxd, Store, Memory, Code::Movd_rm32_mm),
    mmx(Instruction::MovdMmxqGpr32, Load, Register, Code::Movd_mm_rm32),
    mmx(Instruction::MovdMmxqMemd, Load, Memory, Code::Movd_mm_rm32),
    mmx(Instruction::MovntqMemqMmxq, Store, Memory, Code::Movntq_m64_mm),
    mmx(Instruction::Movq2dqXmmdqMmxq, MoveToVector, Register, Code::Movq2dq_xmm_mm),
    mmx(Instruction::MovqGpr64Mmxq, Store, Register, Code::Movq_rm64_mm),
    mmx(Instruction::MovqMemqMmxq0f7e, Store, Memory, Code::Movq_rm64_mm),
    mmx(Instruction::MovqMemqMmxq0f7f, Store, Memory, Code::Movq_mmm64_mm),
    mmx(Instruction::MovqMmxqGpr64, Load, Register, Code::Movq_mm_rm64),
    mmx(Instruction::MovqMmxqMemq0f6e, Load, Memory, Code::Movq_mm_rm64),
    mmx(Instruction::MovqMmxqMemq0f6f, Load, Memory, Code::Movq_mm_mmm64),
    mmx(Instruction::MovqMmxqMmxq0f6f, Load, Register, Code::Movq_mm_mmm64),
    mmx(Instruction::MovqMmxqMmxq0f7f, Load, Register, Code::Movq_mmm64_mm),
    mmx(Instruction::PabsbMmxqMmxq, Lanes(Absolute(Byte)), Register, Code::Pabsb_mm_mmm64),
    mmx(Instruction::PabsdMmxqMmxq, Lanes(Absolute(Doubleword)), Register, Code::Pabsd_mm_mmm64),
    mmx(Instruction::PabswMmxqMmxq, Lanes(Absolute(Word)), Register, Code::Pabsw_mm_mmm64),
    mmx(Instruction::PackssdwMmxqMemq, Lanes(Pack(Doubleword, Signed)), Memory, Code::Packssdw_mm_mmm64),
    mmx(Instruction::PackssdwMmxqMmxq, Lanes(Pack(Doubleword, Signed)), Register, Code::Packssdw_mm_mmm64),
    mmx(Instruction::PacksswbMmxqMemq, Lanes(Pack(Word, Signed)), Memory, Code::Packsswb_mm_mmm64),
    mmx(Instruction::PacksswbMmxqMmxq, Lanes(Pack(Word, Signed)), Register, Code::Packsswb_mm_mmm64),
    mmx(Instruction::PackuswbMmxqMemq, Lanes(Pack(Word, Unsigned)), Memory, Code::Packuswb_mm_mmm64),
    mmx(Instruction::PackuswbMmxqMmxq, Lanes(Pack(Word, Unsigned)), Register, Code::Packuswb_mm_mmm64),
    mmx(Instruction::PaddbMmxqMemq, Lanes(Add(Byte)), Memory, Code::Paddb_mm_mmm64),
    mmx(Instruction::PaddbMmxqMmxq, Lanes(Add(Byte)), Register, Code::Paddb_mm_mmm64),
    mmx(Instruction::PadddMmxqMemq, Lanes(Add(Doubleword)), Memory, Code::Paddd_mm_mmm64),
    mmx(Instruction::PadddMmxqMmxq, Lanes(Add(Doubleword)), Register, Code::Paddd_mm_mmm64),
    mmx(Instruction::PaddqMmxqMemq, Lanes(Add(Quadword)), Memory, Code::Paddq_mm_mmm64),
    mmx(Instruction::PaddqMmxqMmxq, Lanes(Add(Quadword)), Register, Code::Paddq_mm_mmm64),
    mmx(Instruction::PaddsbMmxqMemq, Lanes(AddSaturating(Byte,Signed)), Memory, Code::Paddsb_mm_mmm64),
    mmx(Instruction::PaddsbMmxqMmxq, Lanes(AddSaturating(Byte,Signed)), Register, Code::Paddsb_mm_mmm64),
    mmx(Instruction::PaddswMmxqMemq, Lanes(AddSaturating(Word,Signed)), Memory, Code::Paddsw_mm_mmm64),
    mmx(Instruction::PaddswMmxqMmxq, Lanes(AddSaturating(Word,Signed)), Register, Code::Paddsw_mm_mmm64),
    mmx(Instruction::PaddusbMmxqMemq, Lanes(AddSaturating(Byte,Unsigned)), Memory, Code::Paddusb_mm_mmm64),
    mmx(Instruction::PaddusbMmxqMmxq, Lanes(AddSaturating(Byte,Unsigned)), Register, Code::Paddusb_mm_mmm64),
    mmx(Instruction::PadduswMmxqMemq, Lanes(AddSaturating(Word,Unsigned)), Memory, Code::Paddusw_mm_mmm64),
    mmx(Instruction::PadduswMmxqMmxq, Lanes(AddSaturating(Word,Unsigned)), Register, Code::Paddusw_mm_mmm64),
    mmx(Instruction::PaddwMmxqMemq, Lanes(Add(Word)), Memory, Code::Paddw_mm_mmm64),
    mmx(Instruction::PaddwMmxqMmxq, Lanes(Add(Word)), Register, Code::Paddw_mm_mmm64),
    mmx(Instruction::PalignrMmxqMmxqImmb, Lanes(AlignRight), Register, Code::Palignr_mm_mmm64_imm8),
    mmx(Instruction::PandnMmxqMemq, Lanes(AndNot), Memory, Code::Pandn_mm_mmm64),
    mmx(Instruction::PandnMmxqMmxq, Lanes(AndNot), Register, Code::Pandn_mm_mmm64),
    mmx(Instruction::PandMmxqMemq, Lanes(And), Memory, Code::Pand_mm_mmm64),
    mmx(Instruction::PandMmxqMmxq, Lanes(And), Register, Code::Pand_mm_mmm64),
    mmx(Instruction::PavgbMmxqMemq, Lanes(Average(Byte)), Memory, Code::Pavgb_mm_mmm64),
    mmx(Instruction::PavgbMmxqMmxq, Lanes(Average(Byte)), Register, Code::Pavgb_mm_mmm64),
    mmx(Instruction::PavgwMmxqMemq, Lanes(Average(Word)), Memory, Code::Pavgw_mm_mmm64),
    mmx(Instruction::PavgwMmxqMmxq, Lanes(Average(Word)), Register, Code::Pavgw_mm_mmm64),
    mmx(Instruction::PcmpeqbMmxqMemq, Lanes(CompareEqual(Byte)), Memory, Code::Pcmpeqb_mm_mmm64),
    mmx(Instruction::PcmpeqbMmxqMmxq, Lanes(CompareEqual(Byte)), Register, Code::Pcmpeqb_mm_mmm64),
    mmx(Instruction::PcmpeqdMmxqMemq, Lanes(CompareEqual(Doubleword)), Memory, Code::Pcmpeqd_mm_mmm64),
    mmx(Instruction::PcmpeqdMmxqMmxq, Lanes(CompareEqual(Doubleword)), Register, Code::Pcmpeqd_mm_mmm64),
    mmx(Instruction::PcmpeqwMmxqMemq, Lanes(CompareEqual(Word)), Memory, Code::Pcmpeqw_mm_mmm64),
    mmx(Instruction::PcmpeqwMmxqMmxq, Lanes(CompareEqual(Word)), Register, Code::Pcmpeqw_mm_mmm64),
    mmx(Instruction::PcmpgtbMmxqMemq, Lanes(CompareGreater(Byte)), Memory, Code::Pcmpgtb_mm_mmm64),
    mmx(Instruction::PcmpgtbMmxqMmxq, Lanes(CompareGreater(Byte)), Register, Code::Pcmpgtb_mm_mmm64),
    mmx(Instruction::PcmpgtdMmxqMemq, Lanes(CompareGreater(Doubleword)), Memory, Code::Pcmpgtd_mm_mmm64),
    mmx(Instruction::PcmpgtdMmxqMmxq, Lanes(CompareGreater(Doubleword)), Register, Code::Pcmpgtd_mm_mmm64),
    mmx(Instruction::PcmpgtwMmxqMemq, Lanes(CompareGreater(Word)), Memory, Code::Pcmpgtw_mm_mmm64),
    mmx(Instruction::PcmpgtwMmxqMmxq, Lanes(CompareGreater(Word)), Register, Code::Pcmpgtw_mm_mmm64),
    mmx(Instruction::PextrwGpr32MmxqImmb, ExtractWord, Register, Code::Pextrw_r32_mm_imm8),
    mmx(Instruction::PhadddMmxqMmxq, Lanes(HorizontalAdd(Doubleword)), Register, Code::Phaddd_mm_mmm64),
    mmx(Instruction::PhaddswMmxqMmxq, Lanes(HorizontalAddSaturating), Register, Code::Phaddsw_mm_mmm64),
    mmx(Instruction::PhaddwMmxqMmxq, Lanes(HorizontalAdd(Word)), Register, Code::Phaddw_mm_mmm64),
    mmx(Instruction::PhsubdMmxqMmxq, Lanes(HorizontalSubtract(Doubleword)), Register, Code::Phsubd_mm_mmm64),
    mmx(Instruction::PhsubswMmxqMmxq, Lanes(HorizontalSubtractSaturating), Register, Code::Phsubsw_mm_mmm64),
    mmx(Instruction::PhsubwMmxqMmxq, Lanes(HorizontalSubtract(Word)), Register, Code::Phsubw_mm_mmm64),
    mmx(Instruction::PinsrwMmxqGpr32Immb, InsertWord, Register, Code::Pinsrw_mm_r32m16_imm8),
    mmx(Instruction::PinsrwMmxqMemwImmb, InsertWord, Memory, Code::Pinsrw_mm_r32m16_imm8),
    mmx(Instruction::PmaddubswMmxqMmxq, Lanes(MultiplyAddBytes), Register, Code::Pmaddubsw_mm_mmm64),
    mmx(Instruction::PmaddwdMmxqMemq, Lanes(MultiplyAddWords), Memory, Code::Pmaddwd_mm_mmm64),
    mmx(Instruction::PmaddwdMmxqMmxq, Lanes(MultiplyAddWords), Register, Code::Pmaddwd_mm_mmm64),
    mmx(Instruction::PmaxswMmxqMemq, Lanes(Maximum(Word, Signed)), Memory, Code::Pmaxsw_mm_mmm64),
    mmx(Instruction::PmaxswMmxqMmxq, Lanes(Maximum(Word, Signed)), Register, Code::Pmaxsw_mm_mmm64),
    mmx(Instruction::PmaxubMmxqMemq, Lanes(Maximum(Byte, Unsigned)), Memory, Code::Pmaxub_mm_mmm64),
    mmx(Instruction::PmaxubMmxqMmxq, Lanes(Maximum(Byte, Unsigned)), Register, Code::Pmaxub_mm_mmm64),
    mmx(Instruction::PminswMmxqMemq, Lanes(Minimum(Word, Signed)), Memory, Code::Pminsw_mm_mmm64),
    mmx(Instruction::PminswMmxqMmxq, Lanes(Minimum(Word, Signed)), Register, Code::Pminsw_mm_mmm64),
    mmx(Instruction::PminubMmxqMemq, Lanes(Minimum(Byte, Unsigned)), Memory, Code::Pminub_mm_mmm64),
    mmx(Instruction::PminubMmxqMmxq, Lanes(Minimum(Byte, Unsigned)), Register, Code::Pminub_mm_mmm64),
    mmx(Instruction::PmovmskbGpr32Mmxq, MoveMask, Register, Code::Pmovmskb_r32_mm),
    mmx(Instruction::PmulhrswMmxqMmxq, Lanes(MultiplyHighRoundScale), Register, Code::Pmulhrsw_mm_mmm64),
    mmx(Instruction::PmulhuwMmxqMemq, Lanes(MultiplyHighWords(Unsigned)), Memory, Code::Pmulhuw_mm_mmm64),
    mmx(Instruction::PmulhuwMmxqMmxq, Lanes(MultiplyHighWords(Unsigned)), Register, Code::Pmulhuw_mm_mmm64),
    mmx(Instruction::PmulhwMmxqMemq, Lanes(MultiplyHighWords(Signed)), Memory, Code::Pmulhw_mm_mmm64),
    mmx(Instruction::PmulhwMmxqMmxq, Lanes(MultiplyHighWords(Signed)), Register, Code::Pmulhw_mm_mmm64),
    mmx(Instruction::PmullwMmxqMemq, Lanes(MultiplyLow(Word)), Memory, Code::Pmullw_mm_mmm64),
    mmx(Instruction::PmullwMmxqMmxq, Lanes(MultiplyLow(Word)), Register, Code::Pmullw_mm_mmm64),
    mmx(Instruction::PmuludqMmxqMemq, Lanes(MultiplyDoublewords(Unsigned)), Memory, Code::Pmuludq_mm_mmm64),
    mmx(Instruction::PmuludqMmxqMmxq, Lanes(MultiplyDoublewords(Unsigned)), Register, Code::Pmuludq_mm_mmm64),
    mmx(Instruction::PorMmxqMemq, Lanes(Or), Memory, Code::Por_mm_mmm64),
    mmx(Instruction::PorMmxqMmxq, Lanes(Or), Register, Code::Por_mm_mmm64),
    mmx(Instruction::PsadbwMmxqMemq, Lanes(SumAbsoluteDifferences), Memory, Code::Psadbw_mm_mmm64),
    mmx(Instruction::PsadbwMmxqMmxq, Lanes(SumAbsoluteDifferences), Register, Code::Psadbw_mm_mmm64),
    mmx(Instruction::PshufbMmxqMmxq, Lanes(ShuffleBytes), Register, Code::Pshufb_mm_mmm64),
    mmx(Instruction::PshufwMmxqMemqImmb, Lanes(ShuffleLowWords), Memory, Code::Pshufw_mm_mmm64_imm8),
    mmx(Instruction::PshufwMmxqMmxqImmb, Lanes(ShuffleLowWords), Register, Code::Pshufw_mm_mmm64_imm8),
    mmx(Instruction::PsignbMmxqMmxq, Lanes(Sign(Byte)), Register, Code::Psignb_mm_mmm64),
    mmx(Instruction::PsigndMmxqMmxq, Lanes(Sign(Doubleword)), Register, Code::Psignd_mm_mmm64),
    mmx(Instruction::PsignwMmxqMmxq, Lanes(Sign(Word)), Register, Code::Psignw_mm_mmm64),
    mmx(Instruction::PslldMmxqImmb, Lanes(ShiftLeft(Doubleword)), Register, Code::Pslld_mm_imm8),
    mmx(Instruction::PslldMmxqMemq, Lanes(ShiftLeft(Doubleword)), Memory, Code::Pslld_mm_mmm64),
    mmx(Instruction::PslldMmxqMmxq, Lanes(ShiftLeft(Doubleword)), Register, Code::Pslld_mm_mmm64),
    mmx(Instruction::PsllqMmxqImmb, Lanes(ShiftLeft(Quadword)), Register, Code::Psllq_mm_imm8),
    mmx(Instruction::PsllqMmxqMemq, Lanes(ShiftLeft(Quadword)), Memory, Code::Psllq_mm_mmm64),
    mmx(Instruction::PsllqMmxqMmxq, Lanes(ShiftLeft(Quadword)), Register, Code::Psllq_mm_mmm64),
    mmx(Instruction::PsllwMmxqImmb, Lanes(ShiftLeft(Word)), Register, Code::Psllw_mm_imm8),
    mmx(Instruction::PsllwMmxqMemq, Lanes(ShiftLeft(Word)), Memory, Code::Psllw_mm_mmm64),
    mmx(Instruction::PsllwMmxqMmxq, Lanes(ShiftLeft(Word)), Register, Code::Psllw_mm_mmm64),
    mmx(Instruction::PsradMmxqImmb, Lanes(ShiftRightArithmetic(Doubleword)), Register, Code::Psrad_mm_imm8),
    mmx(Instruction::PsradMmxqMemq, Lanes(ShiftRightArithmetic(Doubleword)), Memory, Code::Psrad_mm_mmm64),
    mmx(Instruction::PsradMmxqMmxq, Lanes(ShiftRightArithmetic(Doubleword)), Register, Code::Psrad_mm_mmm64),
    mmx(Instruction::PsrawMmxqImmb, Lanes(ShiftRightArithmetic(Word)), Register, Code::Psraw_mm_imm8),
    mmx(Instruction::PsrawMmxqMemq, Lanes(ShiftRightArithmetic(Word)), Memory, Code::Psraw_mm_mmm64),
    mmx(Instruction::PsrawMmxqMmxq, Lanes(ShiftRightArithmetic(Word)), Register, Code::Psraw_mm_mmm64),
    mmx(Instruction::PsrldMmxqImmb, Lanes(ShiftRightLogical(Doubleword)), Register, Code::Psrld_mm_imm8),
    mmx(Instruction::PsrldMmxqMemq, Lanes(ShiftRightLogical(Doubleword)), Memory, Code::Psrld_mm_mmm64),
    mmx(Instruction::PsrldMmxqMmxq, Lanes(ShiftRightLogical(Doubleword)), Register, Code::Psrld_mm_mmm64),
    mmx(Instruction::PsrlqMmxqImmb, Lanes(ShiftRightLogical(Quadword)), Register, Code::Psrlq_mm_imm8),
    mmx(Instruction::PsrlqMmxqMemq, Lanes(ShiftRightLogical(Quadword)), Memory, Code::Psrlq_mm_mmm64),
    mmx(Instruction::PsrlqMmxqMmxq, Lanes(ShiftRightLogical(Quadword)), Register, Code::Psrlq_mm_mmm64),
    mmx(Instruction::PsrlwMmxqImmb, Lanes(ShiftRightLogical(Word)), Register, Code::Psrlw_mm_imm8),
    mmx(Instruction::PsrlwMmxqMemq, Lanes(ShiftRightLogical(Word)), Memory, Code::Psrlw_mm_mmm64),
    mmx(Instruction::PsrlwMmxqMmxq, Lanes(ShiftRightLogical(Word)), Register, Code::Psrlw_mm_mmm64),
    mmx(Instruction::PsubbMmxqMemq, Lanes(Subtract(Byte)), Memory, Code::Psubb_mm_mmm64),
    mmx(Instruction::PsubbMmxqMmxq, Lanes(Subtract(Byte)), Register, Code::Psubb_mm_mmm64),
    mmx(Instruction::PsubdMmxqMemq, Lanes(Subtract(Doubleword)), Memory, Code::Psubd_mm_mmm64),
    mmx(Instruction::PsubdMmxqMmxq, Lanes(Subtract(Doubleword)), Register, Code::Psubd_mm_mmm64),
    mmx(Instruction::PsubqMmxqMemq, Lanes(Subtract(Quadword)), Memory, Code::Psubq_mm_mmm64),
    mmx(Instruction::PsubqMmxqMmxq, Lanes(Subtract(Quadword)), Register, Code::Psubq_mm_mmm64),
    mmx(Instruction::PsubsbMmxqMemq, Lanes(SubtractSaturating(Byte,Signed)), Memory, Code::Psubsb_mm_mmm64),
    mmx(Instruction::PsubsbMmxqMmxq, Lanes(SubtractSaturating(Byte,Signed)), Register, Code::Psubsb_mm_mmm64),
    mmx(Instruction::PsubswMmxqMemq, Lanes(SubtractSaturating(Word,Signed)), Memory, Code::Psubsw_mm_mmm64),
    mmx(Instruction::PsubswMmxqMmxq, Lanes(SubtractSaturating(Word,Signed)), Register, Code::Psubsw_mm_mmm64),
    mmx(Instruction::PsubusbMmxqMemq, Lanes(SubtractSaturating(Byte,Unsigned)), Memory, Code::Psubusb_mm_mmm64),
    mmx(Instruction::PsubusbMmxqMmxq, Lanes(SubtractSaturating(Byte,Unsigned)), Register, Code::Psubusb_mm_mmm64),
    mmx(Instruction::PsubuswMmxqMemq, Lanes(SubtractSaturating(Word,Unsigned)), Memory, Code::Psubusw_mm_mmm64),
    mmx(Instruction::PsubuswMmxqMmxq, Lanes(SubtractSaturating(Word,Unsigned)), Register, Code::Psubusw_mm_mmm64),
    mmx(Instruction::PsubwMmxqMemq, Lanes(Subtract(Word)), Memory, Code::Psubw_mm_mmm64),
    mmx(Instruction::PsubwMmxqMmxq, Lanes(Subtract(Word)), Register, Code::Psubw_mm_mmm64),
    mmx(Instruction::PunpckhbwMmxqMemq, Lanes(UnpackHigh(Byte)), Memory, Code::Punpckhbw_mm_mmm64),
    mmx(Instruction::PunpckhbwMmxqMmxd, Lanes(UnpackHigh(Byte)), Register, Code::Punpckhbw_mm_mmm64),
    mmx(Instruction::PunpckhdqMmxqMemq, Lanes(UnpackHigh(Doubleword)), Memory, Code::Punpckhdq_mm_mmm64),
    mmx(Instruction::PunpckhdqMmxqMmxd, Lanes(UnpackHigh(Doubleword)), Register, Code::Punpckhdq_mm_mmm64),
    mmx(Instruction::PunpckhwdMmxqMemq, Lanes(UnpackHigh(Word)), Memory, Code::Punpckhwd_mm_mmm64),
    mmx(Instruction::PunpckhwdMmxqMmxd, Lanes(UnpackHigh(Word)), Register, Code::Punpckhwd_mm_mmm64),
    mmx(Instruction::PunpcklbwMmxqMemd, Lanes(UnpackLow(Byte)), Memory, Code::Punpcklbw_mm_mmm32),
    mmx(Instruction::PunpcklbwMmxqMmxd, Lanes(UnpackLow(Byte)), Register, Code::Punpcklbw_mm_mmm32),
    mmx(Instruction::PunpckldqMmxqMemd, Lanes(UnpackLow(Doubleword)), Memory, Code::Punpckldq_mm_mmm32),
    mmx(Instruction::PunpckldqMmxqMmxd, Lanes(UnpackLow(Doubleword)), Register, Code::Punpckldq_mm_mmm32),
    mmx(Instruction::PunpcklwdMmxqMemd, Lanes(UnpackLow(Word)), Memory, Code::Punpcklwd_mm_mmm32),
    mmx(Instruction::PunpcklwdMmxqMmxd, Lanes(UnpackLow(Word)), Register, Code::Punpcklwd_mm_mmm32),
    mmx(Instruction::PxorMmxqMemq, Lanes(Xor), Memory, Code::Pxor_mm_mmm64),
    mmx(Instruction::PxorMmxqMmxq, Lanes(Xor), Register, Code::Pxor_mm_mmm64),
    // SSSE3's forms with their source in memory.
    mmx(Instruction::PabsbMmxqMemq, Lanes(Absolute(Byte)), Memory, Code::Pabsb_mm_mmm64),
    mmx(Instruction::PabsdMmxqMemq, Lanes(Absolute(Doubleword)), Memory, Code::Pabsd_mm_mmm64),
    mmx(Instruction::PabswMmxqMemq, Lanes(Absolute(Word)), Memory, Code::Pabsw_mm_mmm64),
    mmx(Instruction::PalignrMmxqMemqImmb, Lanes(AlignRight), Memory, Code::Palignr_mm_mmm64_imm8),
    mmx(Instruction::PhadddMmxqMemq, Lanes(HorizontalAdd(Doubleword)), Memory, Code::Phaddd_mm_mmm64),
    mmx(Instruction::PhaddswMmxqMemq, Lanes(HorizontalAddSaturating), Memory, Code::Phaddsw_mm_mmm64),
    mmx(Instruction::PhaddwMmxqMemq, Lanes(HorizontalAdd(Word)), Memory, Code::Phaddw_mm_mmm64),
    mmx(Instruction::PhsubdMmxqMemq, Lanes(HorizontalSubtract(Doubleword)), Memory, Code::Phsubd_mm_mmm64),
    mmx(Instruction::PhsubswMmxqMemq, Lanes(HorizontalSubtractSaturating), Memory, Code::Phsubsw_mm_mmm64),
    mmx(Instruction::PhsubwMmxqMemq, Lanes(HorizontalSubtract(Word)), Memory, Code::Phsubw_mm_mmm64),
    mmx(Instruction::PmaddubswMmxqMemq, Lanes(MultiplyAddBytes), Memory, Code::Pmaddubsw_mm_mmm64),
    mmx(Instruction::PmulhrswMmxqMemq, Lanes(MultiplyHighRoundScale), Memory, Code::Pmulhrsw_mm_mmm64),
    mmx(Instruction::PshufbMmxqMemq, Lanes(ShuffleBytes), Memory, Code::Pshufb_mm_mmm64),
    mmx(Instruction::PsignbMmxqMemq, Lanes(Sign(Byte)), Memory, Code::Psignb_mm_mmm64),
    mmx(Instruction::PsigndMmxqMemq, Lanes(Sign(Doubleword)), Memory, Code::Psignd_mm_mmm64),
    mmx(Instruction::PsignwMmxqMemq, Lanes(Sign(Word)), Memory, Code::Psignw_mm_mmm64),
];
