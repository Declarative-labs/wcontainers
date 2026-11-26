//! The legacy SSE integer forms that share one skeleton: an XMM destination combined lane by lane with an XMM
//! register, or with 16 bytes of memory aligned to 16, and written back; and the VEX forms of those that combine two
//! sources, whose 128-bit halves each combine as the legacy form's lanes do.
//!
//! A form is one row of [`VECTOR_FORMS`] or [`VEX_FORMS`]: its operation, where its source lies, its iced-x86 `Code`
//! and its encoding.

use alloc::vec::Vec;

use iced_x86::Code;
use crate::forms::Instruction;

use crate::integer::OperandPlace;

/// The width of the lanes an operation works on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lane {
    Byte,
    Word,
    Doubleword,
    Quadword,
}

impl Lane {
    pub const fn to_bits(self) -> u32 {
        match self {
            Self::Byte => u8::BITS,
            Self::Word => u16::BITS,
            Self::Doubleword => u32::BITS,
            Self::Quadword => u64::BITS,
        }
    }
}

/// Whether lanes hold signed or unsigned integers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Signedness {
    Signed,
    Unsigned,
}

/// What a form does with the destination's and the source's lanes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneOperation {
    /// PADDB to PADDQ: sums that wrap.
    Add(Lane),
    /// PADDSB, PADDSW, PADDUSB, PADDUSW.
    AddSaturating(Lane, Signedness),
    /// PSUBB to PSUBQ.
    Subtract(Lane),
    /// PSUBSB, PSUBSW, PSUBUSB, PSUBUSW.
    SubtractSaturating(Lane, Signedness),
    And,
    /// PANDN: the destination's complement and the source.
    AndNot,
    Or,
    Xor,
    /// PAVGB, PAVGW: the unsigned sum plus one, halved.
    Average(Lane),
    /// PCMPEQB to PCMPEQD: all ones where equal.
    CompareEqual(Lane),
    /// PCMPGTB to PCMPGTD: all ones where the destination is greater, signed.
    CompareGreater(Lane),
    /// PMADDWD: the signed products of word pairs, each pair summed into a doubleword that wraps.
    MultiplyAddWords,
    /// PMAXSW, PMAXUB.
    Maximum(Lane, Signedness),
    /// PMINSW, PMINUB.
    Minimum(Lane, Signedness),
    /// PMULHW, PMULHUW: the high half of each word product.
    MultiplyHighWords(Signedness),
    /// PMULLW, PMULLD: the low half of each product.
    MultiplyLow(Lane),
    /// PMULUDQ, PMULDQ: the low doublewords of each quadword multiplied into the whole quadword.
    MultiplyDoublewords(Signedness),
    /// PABSB to PABSD: the source's magnitude, unsigned, so that the lowest value stays.
    Absolute(Lane),
    /// PSIGNB to PSIGND: the destination negated where the source is negative and zeroed where it is zero.
    Sign(Lane),
    /// PSHUFB: each byte the destination's byte the source's byte indexes by its low four bits, or zero where its
    /// top bit is set.
    ShuffleBytes,
    /// PHADDW, PHADDD: the sums of adjacent lanes, the destination's then the source's, wrapping.
    HorizontalAdd(Lane),
    /// PHADDSW: as PHADDW, saturated, signed.
    HorizontalAddSaturating,
    /// PHSUBW, PHSUBD: each even lane less the odd lane above it, the destination's then the source's.
    HorizontalSubtract(Lane),
    /// PHSUBSW: as PHSUBW, saturated, signed.
    HorizontalSubtractSaturating,
    /// PMADDUBSW: the destination's unsigned bytes by the source's signed ones, each pair summed into a word,
    /// saturated, signed.
    MultiplyAddBytes,
    /// PMULHRSW: each signed word product shifted right by 14, plus one, shifted right by one more.
    MultiplyHighRoundScale,
    /// PSHUFD: each doubleword the source's the immediate's two bits for it select.
    ShuffleDoublewords,
    /// PSHUFLW: the low four words so selected from the source's low four, the high quadword the source's.
    ShuffleLowWords,
    /// PSHUFHW: the high four words so selected from the source's high four, the low quadword the source's.
    ShuffleHighWords,
    /// SHUFPS: the low two singles selected from the destination, the high two from the source.
    ShuffleSingles,
    /// SHUFPD: the low double selected from the destination by bit 0, the high from the source by bit 1.
    ShuffleDoubles,
    /// PALIGNR: the destination above the source, shifted right by the immediate's bytes, the low 16 kept.
    AlignRight,
    /// PBLENDW, BLENDPS, BLENDPD: the source's lane where the immediate's bit for it is set, else the
    /// destination's.
    Blend(Lane),
    /// PSLLDQ: the destination shifted left by the immediate's bytes, zero from 16.
    ShiftBytesLeft,
    /// PSRLDQ: the destination shifted right by the immediate's bytes, zero from 16.
    ShiftBytesRight,
    /// MOVAPS, MOVUPS, MOVAPD, MOVUPD, MOVDQA, MOVDQU: the source.
    Move,
    /// MOVSLDUP: the source's even singles, each twice.
    DuplicateEvenSingles,
    /// MOVSHDUP: the source's odd singles, each twice.
    DuplicateOddSingles,
    /// MOVDDUP: the source's low double twice.
    DuplicateLowDouble,
    /// MOVSS and MOVSD between registers, MOVLPS and MOVLPD: the source's low lane into the destination's, the
    /// others kept.
    MoveLow(Lane),
    /// MOVSS and MOVSD from memory, MOVQ: the source's low lane, the others zeroed.
    MoveLowZeroing(Lane),
    /// MOVHLPS: the source's high quadword into the destination's low one.
    MoveHighToLow,
    /// MOVLHPS, MOVHPS and MOVHPD from memory: the source's low quadword into the destination's high one.
    MoveLowToHigh,
    /// PMOVSXBW to PMOVZXDQ: the source's low lanes extended, signed or not, to the wider lanes.
    Extend { from: Lane, to: Lane, signedness: Signedness },
    /// PTEST: ZF where the destination and the source share no bit, CF where the source has none the destination
    /// lacks, every other status flag cleared; no lane written.
    Test,
    /// PHMINPOSUW: the source's least unsigned word, its lowest index in bits 16 to 18, every other bit zero.
    MinimumWordPosition,
    /// INSERTPS: a single (from a register, the one the immediate's bits 7 and 6 select; from memory, the one read)
    /// into the destination's lane its bits 5 and 4 name, then the lanes its low four bits name zeroed.
    InsertSingle { from_memory: bool },
    /// PSADBW: in each quadword, the sum of the bytes' absolute differences.
    SumAbsoluteDifferences,
    /// PACKSSWB, PACKSSDW, PACKUSWB: the destination's lanes then the source's, each narrowed to half its width and
    /// saturated.
    Pack(Lane, Signedness),
    /// PUNPCKHBW to PUNPCKHQDQ: the high halves' lanes interleaved, the destination's first.
    UnpackHigh(Lane),
    /// PUNPCKLBW to PUNPCKLQDQ: the low halves' lanes interleaved.
    UnpackLow(Lane),
    /// PSLLW to PSLLQ by the source's low quadword: zero once the count reaches the width.
    ShiftLeft(Lane),
    /// PSRLW to PSRLQ.
    ShiftRightLogical(Lane),
    /// PSRAW, PSRAD: the sign in every bit once the count reaches the width.
    ShiftRightArithmetic(Lane),
}

/// How a vector form is encoded, which decides the bits of the register it reads and writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorEncoding {
    /// Legacy SSE: the destination is the first source, and bits 128 to 255 are kept.
    Legacy,
    /// VEX.128: the first source is VEX.vvvv, and bits 128 to 255 of the destination are zeroed.
    L128,
    /// VEX.256: as VEX.128 on each 128-bit half of YMM registers or 32 bytes of memory.
    L256,
}

/// One SSE integer form, legacy or VEX.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VectorForm {
    pub instruction: Instruction,
    pub operation: LaneOperation,
    pub place: OperandPlace,
    pub code: Code,
    pub encoding: VectorEncoding,
}

impl LaneOperation {
    /// Whether the operation reads its destination: PABSB to PABSD and PSHUFD, PSHUFLW and PSHUFHW do not.
    pub const fn reads_destination(self) -> bool {
        !matches!(
            self,
            Self::Absolute(_)
                | Self::ShuffleDoublewords
                | Self::ShuffleLowWords
                | Self::ShuffleHighWords
                | Self::Move
                | Self::DuplicateEvenSingles
                | Self::DuplicateOddSingles
                | Self::DuplicateLowDouble
                | Self::MoveLowZeroing(_)
                | Self::Extend { .. }
                | Self::MinimumWordPosition
        )
    }

    /// Whether the operation writes RFLAGS rather than its destination: PTEST does.
    pub const fn writes_flags(self) -> bool {
        matches!(self, Self::Test)
    }

    /// Whether the source is a shift count rather than lanes.
    pub const fn shifts(self) -> bool {
        matches!(self, Self::ShiftLeft(_) | Self::ShiftRightLogical(_) | Self::ShiftRightArithmetic(_))
    }

    /// The width of the lanes the operation reads.
    pub const fn to_lane(self) -> Lane {
        match self {
            Self::Add(lane)
            | Self::AddSaturating(lane, _)
            | Self::Subtract(lane)
            | Self::SubtractSaturating(lane, _)
            | Self::Average(lane)
            | Self::CompareEqual(lane)
            | Self::CompareGreater(lane)
            | Self::Maximum(lane, _)
            | Self::Minimum(lane, _)
            | Self::Pack(lane, _)
            | Self::UnpackHigh(lane)
            | Self::UnpackLow(lane)
            | Self::ShiftLeft(lane)
            | Self::ShiftRightLogical(lane)
            | Self::ShiftRightArithmetic(lane) => lane,
            Self::MultiplyLow(lane)
            | Self::Blend(lane)
            | Self::Absolute(lane)
            | Self::Sign(lane)
            | Self::HorizontalAdd(lane)
            | Self::HorizontalSubtract(lane) => lane,
            Self::MultiplyAddWords
            | Self::MultiplyHighWords(_)
            | Self::HorizontalAddSaturating
            | Self::HorizontalSubtractSaturating
            | Self::MultiplyHighRoundScale => Lane::Word,
            Self::MultiplyDoublewords(_)
            | Self::ShuffleDoublewords
            | Self::ShuffleSingles
            | Self::DuplicateEvenSingles
            | Self::DuplicateOddSingles => Lane::Doubleword,
            Self::Move | Self::DuplicateLowDouble | Self::MoveHighToLow | Self::MoveLowToHigh => Lane::Quadword,
            Self::MoveLow(lane) | Self::MoveLowZeroing(lane) | Self::Extend { from: lane, .. } => lane,
            Self::Test => Lane::Quadword,
            Self::MinimumWordPosition => Lane::Word,
            Self::InsertSingle { .. } => Lane::Doubleword,
            Self::ShuffleLowWords | Self::ShuffleHighWords => Lane::Word,
            Self::ShuffleDoubles => Lane::Quadword,
            Self::SumAbsoluteDifferences
            | Self::ShuffleBytes
            | Self::MultiplyAddBytes
            | Self::AlignRight
            | Self::ShiftBytesLeft
            | Self::ShiftBytesRight => Lane::Byte,
            Self::And | Self::AndNot | Self::Or | Self::Xor => Lane::Quadword,
        }
    }

    /// Applies the operation to the destination's 128 bits and the source's (for a shift by an immediate, the
    /// immediate), with the form's immediate, 0 for a form without one.
    pub fn apply(self, destination: u128, source: u128, immediate: u8) -> u128 {
        let result = self.compute(destination, source, immediate);
        crate::planted::mutate_lanes(self, destination, source, immediate, result)
    }

    /// The operation's result as the manuals define it.
    pub(crate) fn compute(self, destination: u128, source: u128, immediate: u8) -> u128 {
        let lane = self.to_lane();
        let bits = lane.to_bits();
        let each = |combine: &dyn Fn(u64, u64) -> u64| {
            join(lane, split(lane, destination).iter().zip(split(lane, source)).map(|(&first, second)| combine(first, second)))
        };
        let mask = to_mask(bits);
        let signed = |value: u64| to_signed(value, bits);
        match self {
            Self::Add(_) => each(&|first, second| first.wrapping_add(second) & mask),
            Self::Subtract(_) => each(&|first, second| first.wrapping_sub(second) & mask),
            Self::AddSaturating(_, signedness) => {
                each(&|first, second| saturate(lane, signedness, widen(first, bits, signedness) + widen(second, bits, signedness)))
            }
            Self::SubtractSaturating(_, signedness) => {
                each(&|first, second| saturate(lane, signedness, widen(first, bits, signedness) - widen(second, bits, signedness)))
            }
            Self::And => destination & source,
            Self::AndNot => !destination & source,
            Self::Or => destination | source,
            Self::Xor => destination ^ source,
            Self::Average(_) => each(&|first, second| (first + second + 1) >> 1),
            Self::CompareEqual(_) => each(&|first, second| if first == second { mask } else { 0 }),
            Self::CompareGreater(_) => each(&|first, second| if signed(first) > signed(second) { mask } else { 0 }),
            Self::Maximum(_, Signedness::Signed) => each(&|first, second| if signed(first) >= signed(second) { first } else { second }),
            Self::Maximum(_, Signedness::Unsigned) => each(&|first, second| first.max(second)),
            Self::Minimum(_, Signedness::Signed) => each(&|first, second| if signed(first) <= signed(second) { first } else { second }),
            Self::Minimum(_, Signedness::Unsigned) => each(&|first, second| first.min(second)),
            Self::MultiplyHighWords(signedness) => each(&|first, second| {
                let product = widen(first, bits, signedness) * widen(second, bits, signedness);
                (product >> bits) as u64 & mask
            }),
            Self::MultiplyLow(_) => each(&|first, second| first.wrapping_mul(second) & mask),
            Self::MultiplyAddWords => {
                let (first, second) = (split(Lane::Word, destination), split(Lane::Word, source));
                let products: Vec<i64> = first.iter().zip(&second).map(|(&a, &b)| signed(a) * signed(b)).collect();
                join(Lane::Doubleword, products.chunks(2).map(|pair| (pair[0] + pair[1]) as u64 & to_mask(u32::BITS)))
            }
            Self::MultiplyDoublewords(signedness) => {
                let low = |value: u64| widen(value & to_mask(u32::BITS), u32::BITS, signedness);
                let (first, second) = (split(Lane::Quadword, destination), split(Lane::Quadword, source));
                join(Lane::Quadword, first.iter().zip(second).map(|(&a, b)| (low(a) * low(b)) as u64))
            }
            Self::Absolute(_) => each(&|_, second| signed(second).unsigned_abs() & mask),
            Self::Sign(_) => each(&|first, second| match signed(second) {
                0 => 0,
                negative if negative < 0 => first.wrapping_neg() & mask,
                _ => first,
            }),
            Self::ShuffleBytes => {
                let bytes = split(Lane::Byte, destination);
                each(&|_, index| if index & 0x80 != 0 { 0 } else { bytes[(index & 0xF) as usize] })
            }
            Self::HorizontalAdd(_) | Self::HorizontalAddSaturating | Self::HorizontalSubtract(_) | Self::HorizontalSubtractSaturating => {
                let lanes = split(lane, destination).into_iter().chain(split(lane, source)).collect::<Vec<u64>>();
                let combine = |pair: &[u64]| -> u64 {
                    let (even, odd) = (i128::from(signed(pair[0])), i128::from(signed(pair[1])));
                    match self {
                        Self::HorizontalAdd(_) => pair[0].wrapping_add(pair[1]) & mask,
                        Self::HorizontalSubtract(_) => pair[0].wrapping_sub(pair[1]) & mask,
                        Self::HorizontalAddSaturating => saturate(lane, Signedness::Signed, even + odd),
                        _ => saturate(lane, Signedness::Signed, even - odd),
                    }
                };
                join(lane, lanes.chunks(2).map(combine))
            }
            Self::MultiplyAddBytes => {
                let (first, second) = (split(Lane::Byte, destination), split(Lane::Byte, source));
                let products: Vec<i128> =
                    first.iter().zip(&second).map(|(&a, &b)| i128::from(a) * i128::from(to_signed(b, u8::BITS))).collect();
                join(Lane::Word, products.chunks(2).map(|pair| saturate(Lane::Word, Signedness::Signed, pair[0] + pair[1])))
            }
            Self::ShuffleDoublewords | Self::ShuffleLowWords | Self::ShuffleHighWords => {
                let lanes = split(lane, source);
                let select = |index: usize| (u32::from(immediate) >> (2 * (index % 4))) & 3;
                join(
                    lane,
                    (0..lanes.len()).map(|index| match (self, index) {
                        (Self::ShuffleLowWords, 4..) | (Self::ShuffleHighWords, ..4) => lanes[index],
                        (Self::ShuffleHighWords, _) => lanes[4 + select(index) as usize],
                        _ => lanes[select(index) as usize],
                    }),
                )
            }
            Self::ShuffleSingles => {
                let (first, second) = (split(lane, destination), split(lane, source));
                let select = |index: u32| ((immediate >> (2 * index)) & 3) as usize;
                join(lane, [first[select(0)], first[select(1)], second[select(2)], second[select(3)]].into_iter())
            }
            Self::ShuffleDoubles => {
                let (first, second) = (split(lane, destination), split(lane, source));
                join(lane, [first[usize::from(immediate & 1)], second[usize::from((immediate >> 1) & 1)]].into_iter())
            }
            Self::AlignRight => {
                let bytes: Vec<u64> = split(Lane::Byte, source).into_iter().chain(split(Lane::Byte, destination)).collect();
                join(Lane::Byte, (0..16).map(|index| bytes.get(index + usize::from(immediate)).copied().unwrap_or(0)))
            }
            Self::Blend(_) => {
                let (first, second) = (split(lane, destination), split(lane, source));
                join(lane, (0..first.len()).map(|index| if immediate >> index & 1 != 0 { second[index] } else { first[index] }))
            }
            Self::Move => source,
            Self::DuplicateEvenSingles | Self::DuplicateOddSingles => {
                let lanes = split(lane, source);
                let odd = usize::from(self == Self::DuplicateOddSingles);
                join(lane, (0..4).map(|index| lanes[index & !1 | odd]))
            }
            Self::DuplicateLowDouble => join(lane, [source as u64, source as u64].into_iter()),
            Self::Test => {
                let zero = if destination & source == 0 { 1 << 6 } else { 0 };
                let carry = u128::from(!destination & source == 0);
                zero | carry
            }
            Self::MinimumWordPosition => {
                let words = split(Lane::Word, source);
                let (index, least) = words.iter().enumerate().min_by_key(|(index, word)| (**word, *index)).expect("eight words");
                u128::from(*least) | ((index as u128) << 16)
            }
            Self::InsertSingle { from_memory } => {
                let singles = split(lane, source);
                let chosen = if from_memory { singles[0] } else { singles[usize::from(immediate >> 6)] };
                let mut lanes = split(lane, destination);
                lanes[usize::from((immediate >> 4) & 3)] = chosen;
                join(lane, lanes.iter().enumerate().map(|(index, &value)| if immediate >> index & 1 != 0 { 0 } else { value }))
            }
            Self::MoveLow(_) => (destination & !u128::from(mask)) | (source & u128::from(mask)),
            Self::MoveLowZeroing(_) => source & u128::from(mask),
            Self::MoveHighToLow => (destination & !u128::from(u64::MAX)) | (source >> 64),
            Self::MoveLowToHigh => (destination & u128::from(u64::MAX)) | (source << 64),
            Self::Extend { from, to, signedness } => {
                let lanes = split(from, source);
                let wide = u128::BITS / to.to_bits();
                join(to, (0..wide as usize).map(|index| widen(lanes[index], from.to_bits(), signedness) as u64))
            }
            Self::ShiftBytesLeft if source >= 16 => 0,
            Self::ShiftBytesLeft => destination << (8 * source),
            Self::ShiftBytesRight if source >= 16 => 0,
            Self::ShiftBytesRight => destination >> (8 * source),
            Self::MultiplyHighRoundScale => each(&|first, second| {
                let product = signed(first) * signed(second);
                (((product >> 14) + 1) >> 1) as u64 & mask
            }),
            Self::SumAbsoluteDifferences => {
                let differences: Vec<u64> = split(Lane::Byte, destination)
                    .iter()
                    .zip(split(Lane::Byte, source))
                    .map(|(&a, b)| a.abs_diff(b))
                    .collect();
                join(Lane::Quadword, differences.chunks(8).map(|bytes| bytes.iter().sum()))
            }
            Self::Pack(_, signedness) => {
                let narrow = narrower(lane);
                let lanes = split(lane, destination).into_iter().chain(split(lane, source));
                join(narrow, lanes.map(|value| saturate(narrow, signedness, i128::from(signed(value)))))
            }
            Self::UnpackLow(_) | Self::UnpackHigh(_) => {
                let half = 128 / bits as usize / 2;
                let start = if matches!(self, Self::UnpackHigh(_)) { half } else { 0 };
                let (first, second) = (split(lane, destination), split(lane, source));
                join(lane, (start..start + half).flat_map(|index| [first[index], second[index]]))
            }
            Self::ShiftLeft(_) | Self::ShiftRightLogical(_) | Self::ShiftRightArithmetic(_) => {
                let count = source as u64;
                each(&|value, _| match self {
                    Self::ShiftLeft(_) if count >= u64::from(bits) => 0,
                    Self::ShiftLeft(_) => (value << count) & mask,
                    Self::ShiftRightLogical(_) if count >= u64::from(bits) => 0,
                    Self::ShiftRightLogical(_) => value >> count,
                    _ => (signed(value) >> count.min(u64::from(bits) - 1)) as u64 & mask,
                })
            }
        }
    }
}

/// The lane half as wide, which PACKSSWB, PACKSSDW and PACKUSWB narrow to.
const fn narrower(lane: Lane) -> Lane {
    match lane {
        Lane::Word => Lane::Byte,
        Lane::Doubleword => Lane::Word,
        Lane::Quadword => Lane::Doubleword,
        Lane::Byte => panic!("no lane is narrower than a byte"),
    }
}

/// The lanes of 128 bits, lowest first.
pub fn split(lane: Lane, value: u128) -> Vec<u64> {
    let bits = lane.to_bits();
    (0..u128::BITS / bits).map(|index| (value >> (index * bits)) as u64 & to_mask(bits)).collect()
}

/// 128 bits of lanes, lowest first.
pub fn join(lane: Lane, lanes: impl Iterator<Item = u64>) -> u128 {
    let bits = lane.to_bits();
    lanes.enumerate().fold(0, |all, (index, value)| all | (u128::from(value & to_mask(bits)) << (index as u32 * bits)))
}

const fn to_mask(bits: u32) -> u64 {
    if bits >= u64::BITS { u64::MAX } else { (1 << bits) - 1 }
}

pub(crate) const fn to_signed(value: u64, bits: u32) -> i64 {
    let unused = u64::BITS - bits;
    ((value << unused) as i64) >> unused
}

fn widen(value: u64, bits: u32, signedness: Signedness) -> i128 {
    match signedness {
        Signedness::Signed => i128::from(to_signed(value, bits)),
        Signedness::Unsigned => i128::from(value),
    }
}

/// The value clamped to the lane's range.
pub(crate) fn saturate(lane: Lane, signedness: Signedness, value: i128) -> u64 {
    let bits = lane.to_bits();
    let (lowest, highest) = match signedness {
        Signedness::Signed => (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1),
        Signedness::Unsigned => (0, (1i128 << bits) - 1),
    };
    value.clamp(lowest, highest) as u64 & to_mask(bits)
}

impl VectorForm {
    /// Whether a memory source must be aligned to 16 bytes: every 16-byte one but MOVUPS, MOVUPD and MOVDQU's.
    pub fn requires_alignment(&self) -> bool {
        self.encoding == VectorEncoding::Legacy
            && !matches!(self.code, Code::Movups_xmm_xmmm128 | Code::Movupd_xmm_xmmm128 | Code::Movdqu_xmm_xmmm128)
            && self.memory_bytes() == 16
    }

    /// The bytes a memory source reaches: 16, 32 for VEX.256, or as many as the form's code names.
    pub fn memory_bytes(&self) -> u64 {
        match self.encoding {
            VectorEncoding::L128 => return 16,
            VectorEncoding::L256 => return 32,
            VectorEncoding::Legacy => {}
        }
        match self.code {
            Code::Pmovsxbq_xmm_xmmm16 | Code::Pmovzxbq_xmm_xmmm16 => 2,
            Code::Movss_xmm_xmmm32
            | Code::Insertps_xmm_xmmm32_imm8
            | Code::Pmovsxbd_xmm_xmmm32
            | Code::Pmovsxwq_xmm_xmmm32
            | Code::Pmovzxbd_xmm_xmmm32
            | Code::Pmovzxwq_xmm_xmmm32 => 4,
            Code::Movddup_xmm_xmmm64
            | Code::Movsd_xmm_xmmm64
            | Code::Movq_xmm_xmmm64
            | Code::Movq_xmm_rm64
            | Code::Movhps_xmm_m64
            | Code::Movlps_xmm_m64
            | Code::Movhpd_xmm_m64
            | Code::Movlpd_xmm_m64
            | Code::Pmovsxbw_xmm_xmmm64
            | Code::Pmovsxwd_xmm_xmmm64
            | Code::Pmovsxdq_xmm_xmmm64
            | Code::Pmovzxbw_xmm_xmmm64
            | Code::Pmovzxwd_xmm_xmmm64
            | Code::Pmovzxdq_xmm_xmmm64 => 8,
            _ => 16,
        }
    }
}

/// The row of a legacy SSE integer form, or `None` for any other form.
pub fn find_vector_form(instruction: Instruction) -> Option<&'static VectorForm> {
    VECTOR_FORMS.iter().chain(VEX_FORMS).find(|form| form.instruction == instruction)
}

const fn form(instruction: Instruction, operation: LaneOperation, place: OperandPlace, code: Code) -> VectorForm {
    VectorForm {
        instruction,
        operation,
        place,
        code,
        encoding: VectorEncoding::Legacy,
    }
}

use Lane::{Byte, Doubleword, Quadword, Word};
use LaneOperation::{
    Add, AddSaturating, And, AndNot, Average, CompareEqual, CompareGreater, Maximum, Minimum, MultiplyAddWords,
    MultiplyHighWords, MultiplyLow, MultiplyDoublewords, Or, Pack, ShiftLeft, ShiftRightArithmetic, Absolute, Sign,
    ShuffleBytes, HorizontalAdd, HorizontalAddSaturating, HorizontalSubtract, HorizontalSubtractSaturating,
    MultiplyAddBytes, MultiplyHighRoundScale, ShuffleDoublewords, ShuffleLowWords, ShuffleHighWords, ShuffleSingles,
    ShuffleDoubles, AlignRight, Blend, ShiftBytesLeft, ShiftBytesRight, Move, DuplicateEvenSingles, DuplicateOddSingles,
    DuplicateLowDouble, MoveLow, MoveLowZeroing, MoveHighToLow, MoveLowToHigh, Extend, Test, MinimumWordPosition,
    InsertSingle,
    ShiftRightLogical, Subtract, SubtractSaturating, SumAbsoluteDifferences, UnpackHigh, UnpackLow, Xor,
};
use OperandPlace::{Memory, Register};
use Signedness::{Signed, Unsigned};

/// The forms of each operation: the source a register, then, after a slash, memory.
macro_rules! forms {
    ($($($register:ident)? $(/ $memory:ident)?, $operation:expr, $code:ident;)*) => {
        &[$(
            $(form(Instruction::$register, $operation, Register, Code::$code),)?
            $(form(Instruction::$memory, $operation, Memory, Code::$code),)?
        )*]
    };
}

/// Every legacy SSE integer form modeled so far.
pub static VECTOR_FORMS: &[VectorForm] = forms! {
    PaddbXmmdqXmmdq / PaddbXmmdqMemdq, Add(Byte), Paddb_xmm_xmmm128;
    PaddwXmmdqXmmdq / PaddwXmmdqMemdq, Add(Word), Paddw_xmm_xmmm128;
    PadddXmmdqXmmdq / PadddXmmdqMemdq, Add(Doubleword), Paddd_xmm_xmmm128;
    PaddqXmmdqXmmdq / PaddqXmmdqMemdq, Add(Quadword), Paddq_xmm_xmmm128;
    PaddsbXmmdqXmmdq / PaddsbXmmdqMemdq, AddSaturating(Byte, Signed), Paddsb_xmm_xmmm128;
    PaddswXmmdqXmmdq / PaddswXmmdqMemdq, AddSaturating(Word, Signed), Paddsw_xmm_xmmm128;
    PaddusbXmmdqXmmdq / PaddusbXmmdqMemdq, AddSaturating(Byte, Unsigned), Paddusb_xmm_xmmm128;
    PadduswXmmdqXmmdq / PadduswXmmdqMemdq, AddSaturating(Word, Unsigned), Paddusw_xmm_xmmm128;
    PsubbXmmdqXmmdq / PsubbXmmdqMemdq, Subtract(Byte), Psubb_xmm_xmmm128;
    PsubwXmmdqXmmdq / PsubwXmmdqMemdq, Subtract(Word), Psubw_xmm_xmmm128;
    PsubdXmmdqXmmdq / PsubdXmmdqMemdq, Subtract(Doubleword), Psubd_xmm_xmmm128;
    PsubqXmmdqXmmdq / PsubqXmmdqMemdq, Subtract(Quadword), Psubq_xmm_xmmm128;
    PsubsbXmmdqXmmdq / PsubsbXmmdqMemdq, SubtractSaturating(Byte, Signed), Psubsb_xmm_xmmm128;
    PsubswXmmdqXmmdq / PsubswXmmdqMemdq, SubtractSaturating(Word, Signed), Psubsw_xmm_xmmm128;
    PsubusbXmmdqXmmdq / PsubusbXmmdqMemdq, SubtractSaturating(Byte, Unsigned), Psubusb_xmm_xmmm128;
    PsubuswXmmdqXmmdq / PsubuswXmmdqMemdq, SubtractSaturating(Word, Unsigned), Psubusw_xmm_xmmm128;
    PandXmmdqXmmdq / PandXmmdqMemdq, And, Pand_xmm_xmmm128;
    PandnXmmdqXmmdq / PandnXmmdqMemdq, AndNot, Pandn_xmm_xmmm128;
    PorXmmdqXmmdq / PorXmmdqMemdq, Or, Por_xmm_xmmm128;
    PxorXmmdqXmmdq / PxorXmmdqMemdq, Xor, Pxor_xmm_xmmm128;
    PavgbXmmdqXmmdq / PavgbXmmdqMemdq, Average(Byte), Pavgb_xmm_xmmm128;
    PavgwXmmdqXmmdq / PavgwXmmdqMemdq, Average(Word), Pavgw_xmm_xmmm128;
    PcmpeqbXmmdqXmmdq / PcmpeqbXmmdqMemdq, CompareEqual(Byte), Pcmpeqb_xmm_xmmm128;
    PcmpeqwXmmdqXmmdq / PcmpeqwXmmdqMemdq, CompareEqual(Word), Pcmpeqw_xmm_xmmm128;
    PcmpeqdXmmdqXmmdq / PcmpeqdXmmdqMemdq, CompareEqual(Doubleword), Pcmpeqd_xmm_xmmm128;
    PcmpgtbXmmdqXmmdq / PcmpgtbXmmdqMemdq, CompareGreater(Byte), Pcmpgtb_xmm_xmmm128;
    PcmpgtwXmmdqXmmdq / PcmpgtwXmmdqMemdq, CompareGreater(Word), Pcmpgtw_xmm_xmmm128;
    PcmpgtdXmmdqXmmdq / PcmpgtdXmmdqMemdq, CompareGreater(Doubleword), Pcmpgtd_xmm_xmmm128;
    PmaddwdXmmdqXmmdq / PmaddwdXmmdqMemdq, MultiplyAddWords, Pmaddwd_xmm_xmmm128;
    PmaxswXmmdqXmmdq / PmaxswXmmdqMemdq, Maximum(Word, Signed), Pmaxsw_xmm_xmmm128;
    PmaxubXmmdqXmmdq / PmaxubXmmdqMemdq, Maximum(Byte, Unsigned), Pmaxub_xmm_xmmm128;
    PminswXmmdqXmmdq / PminswXmmdqMemdq, Minimum(Word, Signed), Pminsw_xmm_xmmm128;
    PminubXmmdqXmmdq / PminubXmmdqMemdq, Minimum(Byte, Unsigned), Pminub_xmm_xmmm128;
    PmulhuwXmmdqXmmdq / PmulhuwXmmdqMemdq, MultiplyHighWords(Unsigned), Pmulhuw_xmm_xmmm128;
    PmulhwXmmdqXmmdq / PmulhwXmmdqMemdq, MultiplyHighWords(Signed), Pmulhw_xmm_xmmm128;
    PmullwXmmdqXmmdq / PmullwXmmdqMemdq, MultiplyLow(Word), Pmullw_xmm_xmmm128;
    PmuludqXmmdqXmmdq / PmuludqXmmdqMemdq, MultiplyDoublewords(Unsigned), Pmuludq_xmm_xmmm128;
    PsadbwXmmdqXmmdq / PsadbwXmmdqMemdq, SumAbsoluteDifferences, Psadbw_xmm_xmmm128;
    PacksswbXmmdqXmmdq / PacksswbXmmdqMemdq, Pack(Word, Signed), Packsswb_xmm_xmmm128;
    PackssdwXmmdqXmmdq / PackssdwXmmdqMemdq, Pack(Doubleword, Signed), Packssdw_xmm_xmmm128;
    PackuswbXmmdqXmmdq / PackuswbXmmdqMemdq, Pack(Word, Unsigned), Packuswb_xmm_xmmm128;
    PunpckhbwXmmdqXmmq / PunpckhbwXmmdqMemdq, UnpackHigh(Byte), Punpckhbw_xmm_xmmm128;
    PunpckhwdXmmdqXmmq / PunpckhwdXmmdqMemdq, UnpackHigh(Word), Punpckhwd_xmm_xmmm128;
    PunpckhdqXmmdqXmmq / PunpckhdqXmmdqMemdq, UnpackHigh(Doubleword), Punpckhdq_xmm_xmmm128;
    PunpckhqdqXmmdqXmmq / PunpckhqdqXmmdqMemdq, UnpackHigh(Quadword), Punpckhqdq_xmm_xmmm128;
    PunpcklbwXmmdqXmmq / PunpcklbwXmmdqMemdq, UnpackLow(Byte), Punpcklbw_xmm_xmmm128;
    PunpcklwdXmmdqXmmq / PunpcklwdXmmdqMemdq, UnpackLow(Word), Punpcklwd_xmm_xmmm128;
    PunpckldqXmmdqXmmq / PunpckldqXmmdqMemdq, UnpackLow(Doubleword), Punpckldq_xmm_xmmm128;
    PunpcklqdqXmmdqXmmq / PunpcklqdqXmmdqMemdq, UnpackLow(Quadword), Punpcklqdq_xmm_xmmm128;
    PsllwXmmdqXmmdq / PsllwXmmdqMemdq, ShiftLeft(Word), Psllw_xmm_xmmm128;
    PslldXmmdqXmmdq / PslldXmmdqMemdq, ShiftLeft(Doubleword), Pslld_xmm_xmmm128;
    PsllqXmmdqXmmdq / PsllqXmmdqMemdq, ShiftLeft(Quadword), Psllq_xmm_xmmm128;
    PsrlwXmmdqXmmdq / PsrlwXmmdqMemdq, ShiftRightLogical(Word), Psrlw_xmm_xmmm128;
    PsrldXmmdqXmmdq / PsrldXmmdqMemdq, ShiftRightLogical(Doubleword), Psrld_xmm_xmmm128;
    PsrlqXmmdqXmmdq / PsrlqXmmdqMemdq, ShiftRightLogical(Quadword), Psrlq_xmm_xmmm128;
    PsrawXmmdqXmmdq / PsrawXmmdqMemdq, ShiftRightArithmetic(Word), Psraw_xmm_xmmm128;
    PsradXmmdqXmmdq / PsradXmmdqMemdq, ShiftRightArithmetic(Doubleword), Psrad_xmm_xmmm128;
    // The floating-point logic and unpacks, which work on the bits alone.
    AndpsXmmxudXmmxud / AndpsXmmxudMemxud, And, Andps_xmm_xmmm128;
    AndpdXmmxuqXmmxuq / AndpdXmmxuqMemxuq, And, Andpd_xmm_xmmm128;
    AndnpsXmmxudXmmxud / AndnpsXmmxudMemxud, AndNot, Andnps_xmm_xmmm128;
    AndnpdXmmxuqXmmxuq / AndnpdXmmxuqMemxuq, AndNot, Andnpd_xmm_xmmm128;
    OrpsXmmxudXmmxud / OrpsXmmxudMemxud, Or, Orps_xmm_xmmm128;
    OrpdXmmxuqXmmxuq / OrpdXmmxuqMemxuq, Or, Orpd_xmm_xmmm128;
    XorpsXmmxudXmmxud / XorpsXmmxudMemxud, Xor, Xorps_xmm_xmmm128;
    XorpdXmmxuqXmmxuq / XorpdXmmxuqMemxuq, Xor, Xorpd_xmm_xmmm128;
    UnpcklpsXmmpsXmmq / UnpcklpsXmmpsMemdq, UnpackLow(Doubleword), Unpcklps_xmm_xmmm128;
    UnpckhpsXmmpsXmmdq / UnpckhpsXmmpsMemdq, UnpackHigh(Doubleword), Unpckhps_xmm_xmmm128;
    UnpcklpdXmmpdXmmq / UnpcklpdXmmpdMemdq, UnpackLow(Quadword), Unpcklpd_xmm_xmmm128;
    UnpckhpdXmmpdXmmq / UnpckhpdXmmpdMemdq, UnpackHigh(Quadword), Unpckhpd_xmm_xmmm128;
    // SSE4.1 and SSE4.2 forms of operations SSE2 has at other widths.
    PmaxsbXmmdqXmmdq / PmaxsbXmmdqMemdq, Maximum(Byte, Signed), Pmaxsb_xmm_xmmm128;
    PmaxsdXmmdqXmmdq / PmaxsdXmmdqMemdq, Maximum(Doubleword, Signed), Pmaxsd_xmm_xmmm128;
    PmaxuwXmmdqXmmdq / PmaxuwXmmdqMemdq, Maximum(Word, Unsigned), Pmaxuw_xmm_xmmm128;
    PmaxudXmmdqXmmdq / PmaxudXmmdqMemdq, Maximum(Doubleword, Unsigned), Pmaxud_xmm_xmmm128;
    PminsbXmmdqXmmdq / PminsbXmmdqMemdq, Minimum(Byte, Signed), Pminsb_xmm_xmmm128;
    PminsdXmmdqXmmdq / PminsdXmmdqMemdq, Minimum(Doubleword, Signed), Pminsd_xmm_xmmm128;
    PminuwXmmdqXmmdq / PminuwXmmdqMemdq, Minimum(Word, Unsigned), Pminuw_xmm_xmmm128;
    PminudXmmdqXmmdq / PminudXmmdqMemdq, Minimum(Doubleword, Unsigned), Pminud_xmm_xmmm128;
    PcmpeqqXmmdqXmmdq / PcmpeqqXmmdqMemdq, CompareEqual(Quadword), Pcmpeqq_xmm_xmmm128;
    PcmpgtqXmmdqXmmdq / PcmpgtqXmmdqMemdq, CompareGreater(Quadword), Pcmpgtq_xmm_xmmm128;
    PackusdwXmmdqXmmdq / PackusdwXmmdqMemdq, Pack(Doubleword, Unsigned), Packusdw_xmm_xmmm128;
    PmulldXmmdqXmmdq / PmulldXmmdqMemdq, MultiplyLow(Doubleword), Pmulld_xmm_xmmm128;
    PmuldqXmmdqXmmdq / PmuldqXmmdqMemdq, MultiplyDoublewords(Signed), Pmuldq_xmm_xmmm128;
    // SSSE3.
    PabsbXmmdqXmmdq / PabsbXmmdqMemdq, Absolute(Byte), Pabsb_xmm_xmmm128;
    PabswXmmdqXmmdq / PabswXmmdqMemdq, Absolute(Word), Pabsw_xmm_xmmm128;
    PabsdXmmdqXmmdq / PabsdXmmdqMemdq, Absolute(Doubleword), Pabsd_xmm_xmmm128;
    PsignbXmmdqXmmdq / PsignbXmmdqMemdq, Sign(Byte), Psignb_xmm_xmmm128;
    PsignwXmmdqXmmdq / PsignwXmmdqMemdq, Sign(Word), Psignw_xmm_xmmm128;
    PsigndXmmdqXmmdq / PsigndXmmdqMemdq, Sign(Doubleword), Psignd_xmm_xmmm128;
    PshufbXmmdqXmmdq / PshufbXmmdqMemdq, ShuffleBytes, Pshufb_xmm_xmmm128;
    PhaddwXmmdqXmmdq / PhaddwXmmdqMemdq, HorizontalAdd(Word), Phaddw_xmm_xmmm128;
    PhadddXmmdqXmmdq / PhadddXmmdqMemdq, HorizontalAdd(Doubleword), Phaddd_xmm_xmmm128;
    PhaddswXmmdqXmmdq / PhaddswXmmdqMemdq, HorizontalAddSaturating, Phaddsw_xmm_xmmm128;
    PhsubwXmmdqXmmdq / PhsubwXmmdqMemdq, HorizontalSubtract(Word), Phsubw_xmm_xmmm128;
    PhsubdXmmdqXmmdq / PhsubdXmmdqMemdq, HorizontalSubtract(Doubleword), Phsubd_xmm_xmmm128;
    PhsubswXmmdqXmmdq / PhsubswXmmdqMemdq, HorizontalSubtractSaturating, Phsubsw_xmm_xmmm128;
    PmaddubswXmmdqXmmdq / PmaddubswXmmdqMemdq, MultiplyAddBytes, Pmaddubsw_xmm_xmmm128;
    PmulhrswXmmdqXmmdq / PmulhrswXmmdqMemdq, MultiplyHighRoundScale, Pmulhrsw_xmm_xmmm128;
    // Shifts by an immediate, which name the destination alone, and the forms with a control immediate.
    PsllwXmmdqImmb, ShiftLeft(Word), Psllw_xmm_imm8;
    PslldXmmdqImmb, ShiftLeft(Doubleword), Pslld_xmm_imm8;
    PsllqXmmdqImmb, ShiftLeft(Quadword), Psllq_xmm_imm8;
    PsrlwXmmdqImmb, ShiftRightLogical(Word), Psrlw_xmm_imm8;
    PsrldXmmdqImmb, ShiftRightLogical(Doubleword), Psrld_xmm_imm8;
    PsrlqXmmdqImmb, ShiftRightLogical(Quadword), Psrlq_xmm_imm8;
    PsrawXmmdqImmb, ShiftRightArithmetic(Word), Psraw_xmm_imm8;
    PsradXmmdqImmb, ShiftRightArithmetic(Doubleword), Psrad_xmm_imm8;
    PslldqXmmdqImmb, ShiftBytesLeft, Pslldq_xmm_imm8;
    PsrldqXmmdqImmb, ShiftBytesRight, Psrldq_xmm_imm8;
    PshufdXmmdqXmmdqImmb / PshufdXmmdqMemdqImmb, ShuffleDoublewords, Pshufd_xmm_xmmm128_imm8;
    PshuflwXmmdqXmmdqImmb / PshuflwXmmdqMemdqImmb, ShuffleLowWords, Pshuflw_xmm_xmmm128_imm8;
    PshufhwXmmdqXmmdqImmb / PshufhwXmmdqMemdqImmb, ShuffleHighWords, Pshufhw_xmm_xmmm128_imm8;
    ShufpsXmmpsXmmpsImmb / ShufpsXmmpsMempsImmb, ShuffleSingles, Shufps_xmm_xmmm128_imm8;
    ShufpdXmmpdXmmpdImmb / ShufpdXmmpdMempdImmb, ShuffleDoubles, Shufpd_xmm_xmmm128_imm8;
    PalignrXmmdqXmmdqImmb / PalignrXmmdqMemdqImmb, AlignRight, Palignr_xmm_xmmm128_imm8;
    PblendwXmmdqXmmdqImmb / PblendwXmmdqMemdqImmb, Blend(Word), Pblendw_xmm_xmmm128_imm8;
    BlendpsXmmdqXmmdqImmb / BlendpsXmmdqMemdqImmb, Blend(Doubleword), Blendps_xmm_xmmm128_imm8;
    BlendpdXmmdqXmmdqImmb / BlendpdXmmdqMemdqImmb, Blend(Quadword), Blendpd_xmm_xmmm128_imm8;
    // Moves into a register, from a register or memory, and register to register the other way.
    MovapsXmmpsXmmps0f28 / MovapsXmmpsMemps, Move, Movaps_xmm_xmmm128;
    MovapsXmmpsXmmps0f29, Move, Movaps_xmmm128_xmm;
    MovupsXmmpsXmmps0f10 / MovupsXmmpsMemps, Move, Movups_xmm_xmmm128;
    MovupsXmmpsXmmps0f11, Move, Movups_xmmm128_xmm;
    MovapdXmmpdXmmpd0f28 / MovapdXmmpdMempd, Move, Movapd_xmm_xmmm128;
    MovapdXmmpdXmmpd0f29, Move, Movapd_xmmm128_xmm;
    MovupdXmmpdXmmpd0f10 / MovupdXmmpdMempd, Move, Movupd_xmm_xmmm128;
    MovupdXmmpdXmmpd0f11, Move, Movupd_xmmm128_xmm;
    MovdqaXmmdqXmmdq0f6f / MovdqaXmmdqMemdq, Move, Movdqa_xmm_xmmm128;
    MovdqaXmmdqXmmdq0f7f, Move, Movdqa_xmmm128_xmm;
    MovdquXmmdqXmmdq0f6f / MovdquXmmdqMemdq, Move, Movdqu_xmm_xmmm128;
    MovdquXmmdqXmmdq0f7f, Move, Movdqu_xmmm128_xmm;
    MovsldupXmmpsXmmps / MovsldupXmmpsMemps, DuplicateEvenSingles, Movsldup_xmm_xmmm128;
    MovshdupXmmpsXmmps / MovshdupXmmpsMemps, DuplicateOddSingles, Movshdup_xmm_xmmm128;
    MovddupXmmdqXmmq / MovddupXmmdqMemq, DuplicateLowDouble, Movddup_xmm_xmmm64;
    MovssXmmssXmmss0f10, MoveLow(Doubleword), Movss_xmm_xmmm32;
    / MovssXmmdqMemss, MoveLowZeroing(Doubleword), Movss_xmm_xmmm32;
    MovssXmmssXmmss0f11, MoveLow(Doubleword), Movss_xmmm32_xmm;
    MovsdXmmXmmsdXmmsd0f10, MoveLow(Quadword), Movsd_xmm_xmmm64;
    / MovsdXmmXmmdqMemsd, MoveLowZeroing(Quadword), Movsd_xmm_xmmm64;
    MovsdXmmXmmsdXmmsd0f11, MoveLow(Quadword), Movsd_xmmm64_xmm;
    MovqXmmdqXmmq0f7e / MovqXmmdqMemq0f7e, MoveLowZeroing(Quadword), Movq_xmm_xmmm64;
    MovqXmmdqXmmq0fd6, MoveLowZeroing(Quadword), Movq_xmmm64_xmm;
    / MovqXmmdqMemq0f6e, MoveLowZeroing(Quadword), Movq_xmm_rm64;
    MovhlpsXmmqXmmq, MoveHighToLow, Movhlps_xmm_xmm;
    MovlhpsXmmqXmmq, MoveLowToHigh, Movlhps_xmm_xmm;
    / MovhpsXmmqMemq, MoveLowToHigh, Movhps_xmm_m64;
    / MovhpdXmmsdMemq, MoveLowToHigh, Movhpd_xmm_m64;
    / MovlpsXmmqMemq, MoveLow(Quadword), Movlps_xmm_m64;
    / MovlpdXmmsdMemq, MoveLow(Quadword), Movlpd_xmm_m64;
    PtestXmmdqXmmdq / PtestXmmdqMemdq, Test, Ptest_xmm_xmmm128;
    PhminposuwXmmdqXmmdq / PhminposuwXmmdqMemdq, MinimumWordPosition, Phminposuw_xmm_xmmm128;
    InsertpsXmmpsXmmpsImmb, InsertSingle { from_memory: false }, Insertps_xmm_xmmm32_imm8;
    / InsertpsXmmpsMemdImmb, InsertSingle { from_memory: true }, Insertps_xmm_xmmm32_imm8;
    // SSE4.1's extensions.
    PmovsxbwXmmdqXmmq / PmovsxbwXmmdqMemq, Extend { from: Byte, to: Word, signedness: Signed }, Pmovsxbw_xmm_xmmm64;
    PmovsxbdXmmdqXmmd / PmovsxbdXmmdqMemd, Extend { from: Byte, to: Doubleword, signedness: Signed }, Pmovsxbd_xmm_xmmm32;
    PmovsxbqXmmdqXmmw / PmovsxbqXmmdqMemw, Extend { from: Byte, to: Quadword, signedness: Signed }, Pmovsxbq_xmm_xmmm16;
    PmovsxwdXmmdqXmmq / PmovsxwdXmmdqMemq, Extend { from: Word, to: Doubleword, signedness: Signed }, Pmovsxwd_xmm_xmmm64;
    PmovsxwqXmmdqXmmd / PmovsxwqXmmdqMemd, Extend { from: Word, to: Quadword, signedness: Signed }, Pmovsxwq_xmm_xmmm32;
    PmovsxdqXmmdqXmmq / PmovsxdqXmmdqMemq, Extend { from: Doubleword, to: Quadword, signedness: Signed }, Pmovsxdq_xmm_xmmm64;
    PmovzxbwXmmdqXmmq / PmovzxbwXmmdqMemq, Extend { from: Byte, to: Word, signedness: Unsigned }, Pmovzxbw_xmm_xmmm64;
    PmovzxbdXmmdqXmmd / PmovzxbdXmmdqMemd, Extend { from: Byte, to: Doubleword, signedness: Unsigned }, Pmovzxbd_xmm_xmmm32;
    PmovzxbqXmmdqXmmw / PmovzxbqXmmdqMemw, Extend { from: Byte, to: Quadword, signedness: Unsigned }, Pmovzxbq_xmm_xmmm16;
    PmovzxwdXmmdqXmmq / PmovzxwdXmmdqMemq, Extend { from: Word, to: Doubleword, signedness: Unsigned }, Pmovzxwd_xmm_xmmm64;
    PmovzxwqXmmdqXmmd / PmovzxwqXmmdqMemd, Extend { from: Word, to: Quadword, signedness: Unsigned }, Pmovzxwq_xmm_xmmm32;
    PmovzxdqXmmdqXmmq / PmovzxdqXmmdqMemq, Extend { from: Doubleword, to: Quadword, signedness: Unsigned }, Pmovzxdq_xmm_xmmm64;
};

/// Whether the critical values of the operation's lanes need the edges of the lane half as wide, which a pack
/// saturates to.
pub const fn narrows(operation: LaneOperation) -> bool {
    matches!(operation, LaneOperation::Pack(..))
}

/// The VEX forms of each operation: the second source a register, then, after a slash, memory.
macro_rules! vex_forms {
    ($($register:ident / $memory:ident, $operation:expr, $code:ident, $encoding:ident;)*) => {
        &[$(
            VectorForm {
                instruction: Instruction::$register,
                operation: $operation,
                place: Register,
                code: Code::$code,
                encoding: VectorEncoding::$encoding,
            },
            VectorForm {
                instruction: Instruction::$memory,
                operation: $operation,
                place: Memory,
                code: Code::$code,
                encoding: VectorEncoding::$encoding,
            },
        )*]
    };
}

/// The VEX.128 (AVX) and VEX.256 (AVX2, and AVX for the floating-point logic) forms of the legacy forms that combine
/// two sources, each row found from a row of [`VECTOR_FORMS`] by name in the list and in iced-x86's codes.
pub static VEX_FORMS: &[VectorForm] = vex_forms! {
    VpaddbXmmdqXmmdqXmmdq / VpaddbXmmdqXmmdqMemdq, Add(Byte), VEX_Vpaddb_xmm_xmm_xmmm128, L128;
    VpaddbYmmqqYmmqqYmmqq / VpaddbYmmqqYmmqqMemqq, Add(Byte), VEX_Vpaddb_ymm_ymm_ymmm256, L256;
    VpaddwXmmdqXmmdqXmmdq / VpaddwXmmdqXmmdqMemdq, Add(Word), VEX_Vpaddw_xmm_xmm_xmmm128, L128;
    VpaddwYmmqqYmmqqYmmqq / VpaddwYmmqqYmmqqMemqq, Add(Word), VEX_Vpaddw_ymm_ymm_ymmm256, L256;
    VpadddXmmdqXmmdqXmmdq / VpadddXmmdqXmmdqMemdq, Add(Doubleword), VEX_Vpaddd_xmm_xmm_xmmm128, L128;
    VpadddYmmqqYmmqqYmmqq / VpadddYmmqqYmmqqMemqq, Add(Doubleword), VEX_Vpaddd_ymm_ymm_ymmm256, L256;
    VpaddqXmmdqXmmdqXmmdq / VpaddqXmmdqXmmdqMemdq, Add(Quadword), VEX_Vpaddq_xmm_xmm_xmmm128, L128;
    VpaddqYmmqqYmmqqYmmqq / VpaddqYmmqqYmmqqMemqq, Add(Quadword), VEX_Vpaddq_ymm_ymm_ymmm256, L256;
    VpaddsbXmmdqXmmdqXmmdq / VpaddsbXmmdqXmmdqMemdq, AddSaturating(Byte, Signed), VEX_Vpaddsb_xmm_xmm_xmmm128, L128;
    VpaddsbYmmqqYmmqqYmmqq / VpaddsbYmmqqYmmqqMemqq, AddSaturating(Byte, Signed), VEX_Vpaddsb_ymm_ymm_ymmm256, L256;
    VpaddswXmmdqXmmdqXmmdq / VpaddswXmmdqXmmdqMemdq, AddSaturating(Word, Signed), VEX_Vpaddsw_xmm_xmm_xmmm128, L128;
    VpaddswYmmqqYmmqqYmmqq / VpaddswYmmqqYmmqqMemqq, AddSaturating(Word, Signed), VEX_Vpaddsw_ymm_ymm_ymmm256, L256;
    VpaddusbXmmdqXmmdqXmmdq / VpaddusbXmmdqXmmdqMemdq, AddSaturating(Byte, Unsigned), VEX_Vpaddusb_xmm_xmm_xmmm128, L128;
    VpaddusbYmmqqYmmqqYmmqq / VpaddusbYmmqqYmmqqMemqq, AddSaturating(Byte, Unsigned), VEX_Vpaddusb_ymm_ymm_ymmm256, L256;
    VpadduswXmmdqXmmdqXmmdq / VpadduswXmmdqXmmdqMemdq, AddSaturating(Word, Unsigned), VEX_Vpaddusw_xmm_xmm_xmmm128, L128;
    VpadduswYmmqqYmmqqYmmqq / VpadduswYmmqqYmmqqMemqq, AddSaturating(Word, Unsigned), VEX_Vpaddusw_ymm_ymm_ymmm256, L256;
    VpsubbXmmdqXmmdqXmmdq / VpsubbXmmdqXmmdqMemdq, Subtract(Byte), VEX_Vpsubb_xmm_xmm_xmmm128, L128;
    VpsubbYmmqqYmmqqYmmqq / VpsubbYmmqqYmmqqMemqq, Subtract(Byte), VEX_Vpsubb_ymm_ymm_ymmm256, L256;
    VpsubwXmmdqXmmdqXmmdq / VpsubwXmmdqXmmdqMemdq, Subtract(Word), VEX_Vpsubw_xmm_xmm_xmmm128, L128;
    VpsubwYmmqqYmmqqYmmqq / VpsubwYmmqqYmmqqMemqq, Subtract(Word), VEX_Vpsubw_ymm_ymm_ymmm256, L256;
    VpsubdXmmdqXmmdqXmmdq / VpsubdXmmdqXmmdqMemdq, Subtract(Doubleword), VEX_Vpsubd_xmm_xmm_xmmm128, L128;
    VpsubdYmmqqYmmqqYmmqq / VpsubdYmmqqYmmqqMemqq, Subtract(Doubleword), VEX_Vpsubd_ymm_ymm_ymmm256, L256;
    VpsubqXmmdqXmmdqXmmdq / VpsubqXmmdqXmmdqMemdq, Subtract(Quadword), VEX_Vpsubq_xmm_xmm_xmmm128, L128;
    VpsubqYmmqqYmmqqYmmqq / VpsubqYmmqqYmmqqMemqq, Subtract(Quadword), VEX_Vpsubq_ymm_ymm_ymmm256, L256;
    VpsubsbXmmdqXmmdqXmmdq / VpsubsbXmmdqXmmdqMemdq, SubtractSaturating(Byte, Signed), VEX_Vpsubsb_xmm_xmm_xmmm128, L128;
    VpsubsbYmmqqYmmqqYmmqq / VpsubsbYmmqqYmmqqMemqq, SubtractSaturating(Byte, Signed), VEX_Vpsubsb_ymm_ymm_ymmm256, L256;
    VpsubswXmmdqXmmdqXmmdq / VpsubswXmmdqXmmdqMemdq, SubtractSaturating(Word, Signed), VEX_Vpsubsw_xmm_xmm_xmmm128, L128;
    VpsubswYmmqqYmmqqYmmqq / VpsubswYmmqqYmmqqMemqq, SubtractSaturating(Word, Signed), VEX_Vpsubsw_ymm_ymm_ymmm256, L256;
    VpsubusbXmmdqXmmdqXmmdq / VpsubusbXmmdqXmmdqMemdq, SubtractSaturating(Byte, Unsigned), VEX_Vpsubusb_xmm_xmm_xmmm128, L128;
    VpsubusbYmmqqYmmqqYmmqq / VpsubusbYmmqqYmmqqMemqq, SubtractSaturating(Byte, Unsigned), VEX_Vpsubusb_ymm_ymm_ymmm256, L256;
    VpsubuswXmmdqXmmdqXmmdq / VpsubuswXmmdqXmmdqMemdq, SubtractSaturating(Word, Unsigned), VEX_Vpsubusw_xmm_xmm_xmmm128, L128;
    VpsubuswYmmqqYmmqqYmmqq / VpsubuswYmmqqYmmqqMemqq, SubtractSaturating(Word, Unsigned), VEX_Vpsubusw_ymm_ymm_ymmm256, L256;
    VpandXmmdqXmmdqXmmdq / VpandXmmdqXmmdqMemdq, And, VEX_Vpand_xmm_xmm_xmmm128, L128;
    VpandYmmqqYmmqqYmmqq / VpandYmmqqYmmqqMemqq, And, VEX_Vpand_ymm_ymm_ymmm256, L256;
    VpandnXmmdqXmmdqXmmdq / VpandnXmmdqXmmdqMemdq, AndNot, VEX_Vpandn_xmm_xmm_xmmm128, L128;
    VpandnYmmqqYmmqqYmmqq / VpandnYmmqqYmmqqMemqq, AndNot, VEX_Vpandn_ymm_ymm_ymmm256, L256;
    VporXmmdqXmmdqXmmdq / VporXmmdqXmmdqMemdq, Or, VEX_Vpor_xmm_xmm_xmmm128, L128;
    VporYmmqqYmmqqYmmqq / VporYmmqqYmmqqMemqq, Or, VEX_Vpor_ymm_ymm_ymmm256, L256;
    VpxorXmmdqXmmdqXmmdq / VpxorXmmdqXmmdqMemdq, Xor, VEX_Vpxor_xmm_xmm_xmmm128, L128;
    VpxorYmmqqYmmqqYmmqq / VpxorYmmqqYmmqqMemqq, Xor, VEX_Vpxor_ymm_ymm_ymmm256, L256;
    VpavgbXmmdqXmmdqXmmdq / VpavgbXmmdqXmmdqMemdq, Average(Byte), VEX_Vpavgb_xmm_xmm_xmmm128, L128;
    VpavgbYmmqqYmmqqYmmqq / VpavgbYmmqqYmmqqMemqq, Average(Byte), VEX_Vpavgb_ymm_ymm_ymmm256, L256;
    VpavgwXmmdqXmmdqXmmdq / VpavgwXmmdqXmmdqMemdq, Average(Word), VEX_Vpavgw_xmm_xmm_xmmm128, L128;
    VpavgwYmmqqYmmqqYmmqq / VpavgwYmmqqYmmqqMemqq, Average(Word), VEX_Vpavgw_ymm_ymm_ymmm256, L256;
    VpcmpeqbXmmdqXmmdqXmmdq / VpcmpeqbXmmdqXmmdqMemdq, CompareEqual(Byte), VEX_Vpcmpeqb_xmm_xmm_xmmm128, L128;
    VpcmpeqbYmmqqYmmqqYmmqq / VpcmpeqbYmmqqYmmqqMemqq, CompareEqual(Byte), VEX_Vpcmpeqb_ymm_ymm_ymmm256, L256;
    VpcmpeqwXmmdqXmmdqXmmdq / VpcmpeqwXmmdqXmmdqMemdq, CompareEqual(Word), VEX_Vpcmpeqw_xmm_xmm_xmmm128, L128;
    VpcmpeqwYmmqqYmmqqYmmqq / VpcmpeqwYmmqqYmmqqMemqq, CompareEqual(Word), VEX_Vpcmpeqw_ymm_ymm_ymmm256, L256;
    VpcmpeqdXmmdqXmmdqXmmdq / VpcmpeqdXmmdqXmmdqMemdq, CompareEqual(Doubleword), VEX_Vpcmpeqd_xmm_xmm_xmmm128, L128;
    VpcmpeqdYmmqqYmmqqYmmqq / VpcmpeqdYmmqqYmmqqMemqq, CompareEqual(Doubleword), VEX_Vpcmpeqd_ymm_ymm_ymmm256, L256;
    VpcmpgtbXmmdqXmmdqXmmdq / VpcmpgtbXmmdqXmmdqMemdq, CompareGreater(Byte), VEX_Vpcmpgtb_xmm_xmm_xmmm128, L128;
    VpcmpgtbYmmqqYmmqqYmmqq / VpcmpgtbYmmqqYmmqqMemqq, CompareGreater(Byte), VEX_Vpcmpgtb_ymm_ymm_ymmm256, L256;
    VpcmpgtwXmmdqXmmdqXmmdq / VpcmpgtwXmmdqXmmdqMemdq, CompareGreater(Word), VEX_Vpcmpgtw_xmm_xmm_xmmm128, L128;
    VpcmpgtwYmmqqYmmqqYmmqq / VpcmpgtwYmmqqYmmqqMemqq, CompareGreater(Word), VEX_Vpcmpgtw_ymm_ymm_ymmm256, L256;
    VpcmpgtdXmmdqXmmdqXmmdq / VpcmpgtdXmmdqXmmdqMemdq, CompareGreater(Doubleword), VEX_Vpcmpgtd_xmm_xmm_xmmm128, L128;
    VpcmpgtdYmmqqYmmqqYmmqq / VpcmpgtdYmmqqYmmqqMemqq, CompareGreater(Doubleword), VEX_Vpcmpgtd_ymm_ymm_ymmm256, L256;
    VpmaddwdXmmdqXmmdqXmmdq / VpmaddwdXmmdqXmmdqMemdq, MultiplyAddWords, VEX_Vpmaddwd_xmm_xmm_xmmm128, L128;
    VpmaddwdYmmqqYmmqqYmmqq / VpmaddwdYmmqqYmmqqMemqq, MultiplyAddWords, VEX_Vpmaddwd_ymm_ymm_ymmm256, L256;
    VpmaxswXmmdqXmmdqXmmdq / VpmaxswXmmdqXmmdqMemdq, Maximum(Word, Signed), VEX_Vpmaxsw_xmm_xmm_xmmm128, L128;
    VpmaxswYmmqqYmmqqYmmqq / VpmaxswYmmqqYmmqqMemqq, Maximum(Word, Signed), VEX_Vpmaxsw_ymm_ymm_ymmm256, L256;
    VpmaxubXmmdqXmmdqXmmdq / VpmaxubXmmdqXmmdqMemdq, Maximum(Byte, Unsigned), VEX_Vpmaxub_xmm_xmm_xmmm128, L128;
    VpmaxubYmmqqYmmqqYmmqq / VpmaxubYmmqqYmmqqMemqq, Maximum(Byte, Unsigned), VEX_Vpmaxub_ymm_ymm_ymmm256, L256;
    VpminswXmmdqXmmdqXmmdq / VpminswXmmdqXmmdqMemdq, Minimum(Word, Signed), VEX_Vpminsw_xmm_xmm_xmmm128, L128;
    VpminswYmmqqYmmqqYmmqq / VpminswYmmqqYmmqqMemqq, Minimum(Word, Signed), VEX_Vpminsw_ymm_ymm_ymmm256, L256;
    VpminubXmmdqXmmdqXmmdq / VpminubXmmdqXmmdqMemdq, Minimum(Byte, Unsigned), VEX_Vpminub_xmm_xmm_xmmm128, L128;
    VpminubYmmqqYmmqqYmmqq / VpminubYmmqqYmmqqMemqq, Minimum(Byte, Unsigned), VEX_Vpminub_ymm_ymm_ymmm256, L256;
    VpmulhuwXmmdqXmmdqXmmdq / VpmulhuwXmmdqXmmdqMemdq, MultiplyHighWords(Unsigned), VEX_Vpmulhuw_xmm_xmm_xmmm128, L128;
    VpmulhuwYmmqqYmmqqYmmqq / VpmulhuwYmmqqYmmqqMemqq, MultiplyHighWords(Unsigned), VEX_Vpmulhuw_ymm_ymm_ymmm256, L256;
    VpmulhwXmmdqXmmdqXmmdq / VpmulhwXmmdqXmmdqMemdq, MultiplyHighWords(Signed), VEX_Vpmulhw_xmm_xmm_xmmm128, L128;
    VpmulhwYmmqqYmmqqYmmqq / VpmulhwYmmqqYmmqqMemqq, MultiplyHighWords(Signed), VEX_Vpmulhw_ymm_ymm_ymmm256, L256;
    VpmullwXmmdqXmmdqXmmdq / VpmullwXmmdqXmmdqMemdq, MultiplyLow(Word), VEX_Vpmullw_xmm_xmm_xmmm128, L128;
    VpmullwYmmqqYmmqqYmmqq / VpmullwYmmqqYmmqqMemqq, MultiplyLow(Word), VEX_Vpmullw_ymm_ymm_ymmm256, L256;
    VpmuludqXmmdqXmmdqXmmdq / VpmuludqXmmdqXmmdqMemdq, MultiplyDoublewords(Unsigned), VEX_Vpmuludq_xmm_xmm_xmmm128, L128;
    VpmuludqYmmqqYmmqqYmmqq / VpmuludqYmmqqYmmqqMemqq, MultiplyDoublewords(Unsigned), VEX_Vpmuludq_ymm_ymm_ymmm256, L256;
    VpsadbwXmmdqXmmdqXmmdq / VpsadbwXmmdqXmmdqMemdq, SumAbsoluteDifferences, VEX_Vpsadbw_xmm_xmm_xmmm128, L128;
    VpsadbwYmmqqYmmqqYmmqq / VpsadbwYmmqqYmmqqMemqq, SumAbsoluteDifferences, VEX_Vpsadbw_ymm_ymm_ymmm256, L256;
    VpacksswbXmmdqXmmdqXmmdq / VpacksswbXmmdqXmmdqMemdq, Pack(Word, Signed), VEX_Vpacksswb_xmm_xmm_xmmm128, L128;
    VpacksswbYmmqqYmmqqYmmqq / VpacksswbYmmqqYmmqqMemqq, Pack(Word, Signed), VEX_Vpacksswb_ymm_ymm_ymmm256, L256;
    VpackssdwXmmdqXmmdqXmmdq / VpackssdwXmmdqXmmdqMemdq, Pack(Doubleword, Signed), VEX_Vpackssdw_xmm_xmm_xmmm128, L128;
    VpackssdwYmmqqYmmqqYmmqq / VpackssdwYmmqqYmmqqMemqq, Pack(Doubleword, Signed), VEX_Vpackssdw_ymm_ymm_ymmm256, L256;
    VpackuswbXmmdqXmmdqXmmdq / VpackuswbXmmdqXmmdqMemdq, Pack(Word, Unsigned), VEX_Vpackuswb_xmm_xmm_xmmm128, L128;
    VpackuswbYmmqqYmmqqYmmqq / VpackuswbYmmqqYmmqqMemqq, Pack(Word, Unsigned), VEX_Vpackuswb_ymm_ymm_ymmm256, L256;
    VpunpckhbwXmmdqXmmdqXmmdq / VpunpckhbwXmmdqXmmdqMemdq, UnpackHigh(Byte), VEX_Vpunpckhbw_xmm_xmm_xmmm128, L128;
    VpunpckhbwYmmqqYmmqqYmmqq / VpunpckhbwYmmqqYmmqqMemqq, UnpackHigh(Byte), VEX_Vpunpckhbw_ymm_ymm_ymmm256, L256;
    VpunpckhwdXmmdqXmmdqXmmdq / VpunpckhwdXmmdqXmmdqMemdq, UnpackHigh(Word), VEX_Vpunpckhwd_xmm_xmm_xmmm128, L128;
    VpunpckhwdYmmqqYmmqqYmmqq / VpunpckhwdYmmqqYmmqqMemqq, UnpackHigh(Word), VEX_Vpunpckhwd_ymm_ymm_ymmm256, L256;
    VpunpckhdqXmmdqXmmdqXmmdq / VpunpckhdqXmmdqXmmdqMemdq, UnpackHigh(Doubleword), VEX_Vpunpckhdq_xmm_xmm_xmmm128, L128;
    VpunpckhdqYmmqqYmmqqYmmqq / VpunpckhdqYmmqqYmmqqMemqq, UnpackHigh(Doubleword), VEX_Vpunpckhdq_ymm_ymm_ymmm256, L256;
    VpunpckhqdqXmmdqXmmdqXmmdq / VpunpckhqdqXmmdqXmmdqMemdq, UnpackHigh(Quadword), VEX_Vpunpckhqdq_xmm_xmm_xmmm128, L128;
    VpunpckhqdqYmmqqYmmqqYmmqq / VpunpckhqdqYmmqqYmmqqMemqq, UnpackHigh(Quadword), VEX_Vpunpckhqdq_ymm_ymm_ymmm256, L256;
    VpunpcklbwXmmdqXmmdqXmmdq / VpunpcklbwXmmdqXmmdqMemdq, UnpackLow(Byte), VEX_Vpunpcklbw_xmm_xmm_xmmm128, L128;
    VpunpcklbwYmmqqYmmqqYmmqq / VpunpcklbwYmmqqYmmqqMemqq, UnpackLow(Byte), VEX_Vpunpcklbw_ymm_ymm_ymmm256, L256;
    VpunpcklwdXmmdqXmmdqXmmdq / VpunpcklwdXmmdqXmmdqMemdq, UnpackLow(Word), VEX_Vpunpcklwd_xmm_xmm_xmmm128, L128;
    VpunpcklwdYmmqqYmmqqYmmqq / VpunpcklwdYmmqqYmmqqMemqq, UnpackLow(Word), VEX_Vpunpcklwd_ymm_ymm_ymmm256, L256;
    VpunpckldqXmmdqXmmdqXmmdq / VpunpckldqXmmdqXmmdqMemdq, UnpackLow(Doubleword), VEX_Vpunpckldq_xmm_xmm_xmmm128, L128;
    VpunpckldqYmmqqYmmqqYmmqq / VpunpckldqYmmqqYmmqqMemqq, UnpackLow(Doubleword), VEX_Vpunpckldq_ymm_ymm_ymmm256, L256;
    VpunpcklqdqXmmdqXmmdqXmmdq / VpunpcklqdqXmmdqXmmdqMemdq, UnpackLow(Quadword), VEX_Vpunpcklqdq_xmm_xmm_xmmm128, L128;
    VpunpcklqdqYmmqqYmmqqYmmqq / VpunpcklqdqYmmqqYmmqqMemqq, UnpackLow(Quadword), VEX_Vpunpcklqdq_ymm_ymm_ymmm256, L256;
    VandpsXmmdqXmmdqXmmdq / VandpsXmmdqXmmdqMemdq, And, VEX_Vandps_xmm_xmm_xmmm128, L128;
    VandpsYmmqqYmmqqYmmqq / VandpsYmmqqYmmqqMemqq, And, VEX_Vandps_ymm_ymm_ymmm256, L256;
    VandpdXmmdqXmmdqXmmdq / VandpdXmmdqXmmdqMemdq, And, VEX_Vandpd_xmm_xmm_xmmm128, L128;
    VandpdYmmqqYmmqqYmmqq / VandpdYmmqqYmmqqMemqq, And, VEX_Vandpd_ymm_ymm_ymmm256, L256;
    VandnpsXmmdqXmmdqXmmdq / VandnpsXmmdqXmmdqMemdq, AndNot, VEX_Vandnps_xmm_xmm_xmmm128, L128;
    VandnpsYmmqqYmmqqYmmqq / VandnpsYmmqqYmmqqMemqq, AndNot, VEX_Vandnps_ymm_ymm_ymmm256, L256;
    VandnpdXmmdqXmmdqXmmdq / VandnpdXmmdqXmmdqMemdq, AndNot, VEX_Vandnpd_xmm_xmm_xmmm128, L128;
    VandnpdYmmqqYmmqqYmmqq / VandnpdYmmqqYmmqqMemqq, AndNot, VEX_Vandnpd_ymm_ymm_ymmm256, L256;
    VorpsXmmdqXmmdqXmmdq / VorpsXmmdqXmmdqMemdq, Or, VEX_Vorps_xmm_xmm_xmmm128, L128;
    VorpsYmmqqYmmqqYmmqq / VorpsYmmqqYmmqqMemqq, Or, VEX_Vorps_ymm_ymm_ymmm256, L256;
    VorpdXmmdqXmmdqXmmdq / VorpdXmmdqXmmdqMemdq, Or, VEX_Vorpd_xmm_xmm_xmmm128, L128;
    VorpdYmmqqYmmqqYmmqq / VorpdYmmqqYmmqqMemqq, Or, VEX_Vorpd_ymm_ymm_ymmm256, L256;
    VxorpsXmmdqXmmdqXmmdq / VxorpsXmmdqXmmdqMemdq, Xor, VEX_Vxorps_xmm_xmm_xmmm128, L128;
    VxorpsYmmqqYmmqqYmmqq / VxorpsYmmqqYmmqqMemqq, Xor, VEX_Vxorps_ymm_ymm_ymmm256, L256;
    VxorpdXmmdqXmmdqXmmdq / VxorpdXmmdqXmmdqMemdq, Xor, VEX_Vxorpd_xmm_xmm_xmmm128, L128;
    VxorpdYmmqqYmmqqYmmqq / VxorpdYmmqqYmmqqMemqq, Xor, VEX_Vxorpd_ymm_ymm_ymmm256, L256;
    VunpcklpsXmmdqXmmdqXmmdq / VunpcklpsXmmdqXmmdqMemdq, UnpackLow(Doubleword), VEX_Vunpcklps_xmm_xmm_xmmm128, L128;
    VunpcklpsYmmqqYmmqqYmmqq / VunpcklpsYmmqqYmmqqMemqq, UnpackLow(Doubleword), VEX_Vunpcklps_ymm_ymm_ymmm256, L256;
    VunpckhpsXmmdqXmmdqXmmdq / VunpckhpsXmmdqXmmdqMemdq, UnpackHigh(Doubleword), VEX_Vunpckhps_xmm_xmm_xmmm128, L128;
    VunpckhpsYmmqqYmmqqYmmqq / VunpckhpsYmmqqYmmqqMemqq, UnpackHigh(Doubleword), VEX_Vunpckhps_ymm_ymm_ymmm256, L256;
    VunpcklpdXmmdqXmmdqXmmdq / VunpcklpdXmmdqXmmdqMemdq, UnpackLow(Quadword), VEX_Vunpcklpd_xmm_xmm_xmmm128, L128;
    VunpcklpdYmmqqYmmqqYmmqq / VunpcklpdYmmqqYmmqqMemqq, UnpackLow(Quadword), VEX_Vunpcklpd_ymm_ymm_ymmm256, L256;
    VunpckhpdXmmdqXmmdqXmmdq / VunpckhpdXmmdqXmmdqMemdq, UnpackHigh(Quadword), VEX_Vunpckhpd_xmm_xmm_xmmm128, L128;
    VunpckhpdYmmqqYmmqqYmmqq / VunpckhpdYmmqqYmmqqMemqq, UnpackHigh(Quadword), VEX_Vunpckhpd_ymm_ymm_ymmm256, L256;
    VpmaxsbXmmdqXmmdqXmmdq / VpmaxsbXmmdqXmmdqMemdq, Maximum(Byte, Signed), VEX_Vpmaxsb_xmm_xmm_xmmm128, L128;
    VpmaxsbYmmqqYmmqqYmmqq / VpmaxsbYmmqqYmmqqMemqq, Maximum(Byte, Signed), VEX_Vpmaxsb_ymm_ymm_ymmm256, L256;
    VpmaxsdXmmdqXmmdqXmmdq / VpmaxsdXmmdqXmmdqMemdq, Maximum(Doubleword, Signed), VEX_Vpmaxsd_xmm_xmm_xmmm128, L128;
    VpmaxsdYmmqqYmmqqYmmqq / VpmaxsdYmmqqYmmqqMemqq, Maximum(Doubleword, Signed), VEX_Vpmaxsd_ymm_ymm_ymmm256, L256;
    VpmaxuwXmmdqXmmdqXmmdq / VpmaxuwXmmdqXmmdqMemdq, Maximum(Word, Unsigned), VEX_Vpmaxuw_xmm_xmm_xmmm128, L128;
    VpmaxuwYmmqqYmmqqYmmqq / VpmaxuwYmmqqYmmqqMemqq, Maximum(Word, Unsigned), VEX_Vpmaxuw_ymm_ymm_ymmm256, L256;
    VpmaxudXmmdqXmmdqXmmdq / VpmaxudXmmdqXmmdqMemdq, Maximum(Doubleword, Unsigned), VEX_Vpmaxud_xmm_xmm_xmmm128, L128;
    VpmaxudYmmqqYmmqqYmmqq / VpmaxudYmmqqYmmqqMemqq, Maximum(Doubleword, Unsigned), VEX_Vpmaxud_ymm_ymm_ymmm256, L256;
    VpminsbXmmdqXmmdqXmmdq / VpminsbXmmdqXmmdqMemdq, Minimum(Byte, Signed), VEX_Vpminsb_xmm_xmm_xmmm128, L128;
    VpminsbYmmqqYmmqqYmmqq / VpminsbYmmqqYmmqqMemqq, Minimum(Byte, Signed), VEX_Vpminsb_ymm_ymm_ymmm256, L256;
    VpminsdXmmdqXmmdqXmmdq / VpminsdXmmdqXmmdqMemdq, Minimum(Doubleword, Signed), VEX_Vpminsd_xmm_xmm_xmmm128, L128;
    VpminsdYmmqqYmmqqYmmqq / VpminsdYmmqqYmmqqMemqq, Minimum(Doubleword, Signed), VEX_Vpminsd_ymm_ymm_ymmm256, L256;
    VpminuwXmmdqXmmdqXmmdq / VpminuwXmmdqXmmdqMemdq, Minimum(Word, Unsigned), VEX_Vpminuw_xmm_xmm_xmmm128, L128;
    VpminuwYmmqqYmmqqYmmqq / VpminuwYmmqqYmmqqMemqq, Minimum(Word, Unsigned), VEX_Vpminuw_ymm_ymm_ymmm256, L256;
    VpminudXmmdqXmmdqXmmdq / VpminudXmmdqXmmdqMemdq, Minimum(Doubleword, Unsigned), VEX_Vpminud_xmm_xmm_xmmm128, L128;
    VpminudYmmqqYmmqqYmmqq / VpminudYmmqqYmmqqMemqq, Minimum(Doubleword, Unsigned), VEX_Vpminud_ymm_ymm_ymmm256, L256;
    VpcmpeqqXmmdqXmmdqXmmdq / VpcmpeqqXmmdqXmmdqMemdq, CompareEqual(Quadword), VEX_Vpcmpeqq_xmm_xmm_xmmm128, L128;
    VpcmpeqqYmmqqYmmqqYmmqq / VpcmpeqqYmmqqYmmqqMemqq, CompareEqual(Quadword), VEX_Vpcmpeqq_ymm_ymm_ymmm256, L256;
    VpcmpgtqXmmdqXmmdqXmmdq / VpcmpgtqXmmdqXmmdqMemdq, CompareGreater(Quadword), VEX_Vpcmpgtq_xmm_xmm_xmmm128, L128;
    VpcmpgtqYmmqqYmmqqYmmqq / VpcmpgtqYmmqqYmmqqMemqq, CompareGreater(Quadword), VEX_Vpcmpgtq_ymm_ymm_ymmm256, L256;
    VpackusdwXmmdqXmmdqXmmdq / VpackusdwXmmdqXmmdqMemdq, Pack(Doubleword, Unsigned), VEX_Vpackusdw_xmm_xmm_xmmm128, L128;
    VpackusdwYmmqqYmmqqYmmqq / VpackusdwYmmqqYmmqqMemqq, Pack(Doubleword, Unsigned), VEX_Vpackusdw_ymm_ymm_ymmm256, L256;
    VpmulldXmmdqXmmdqXmmdq / VpmulldXmmdqXmmdqMemdq, MultiplyLow(Doubleword), VEX_Vpmulld_xmm_xmm_xmmm128, L128;
    VpmulldYmmqqYmmqqYmmqq / VpmulldYmmqqYmmqqMemqq, MultiplyLow(Doubleword), VEX_Vpmulld_ymm_ymm_ymmm256, L256;
    VpmuldqXmmdqXmmdqXmmdq / VpmuldqXmmdqXmmdqMemdq, MultiplyDoublewords(Signed), VEX_Vpmuldq_xmm_xmm_xmmm128, L128;
    VpmuldqYmmqqYmmqqYmmqq / VpmuldqYmmqqYmmqqMemqq, MultiplyDoublewords(Signed), VEX_Vpmuldq_ymm_ymm_ymmm256, L256;
    VpsignbXmmdqXmmdqXmmdq / VpsignbXmmdqXmmdqMemdq, Sign(Byte), VEX_Vpsignb_xmm_xmm_xmmm128, L128;
    VpsignbYmmqqYmmqqYmmqq / VpsignbYmmqqYmmqqMemqq, Sign(Byte), VEX_Vpsignb_ymm_ymm_ymmm256, L256;
    VpsignwXmmdqXmmdqXmmdq / VpsignwXmmdqXmmdqMemdq, Sign(Word), VEX_Vpsignw_xmm_xmm_xmmm128, L128;
    VpsignwYmmqqYmmqqYmmqq / VpsignwYmmqqYmmqqMemqq, Sign(Word), VEX_Vpsignw_ymm_ymm_ymmm256, L256;
    VpsigndXmmdqXmmdqXmmdq / VpsigndXmmdqXmmdqMemdq, Sign(Doubleword), VEX_Vpsignd_xmm_xmm_xmmm128, L128;
    VpsigndYmmqqYmmqqYmmqq / VpsigndYmmqqYmmqqMemqq, Sign(Doubleword), VEX_Vpsignd_ymm_ymm_ymmm256, L256;
    VpshufbXmmdqXmmdqXmmdq / VpshufbXmmdqXmmdqMemdq, ShuffleBytes, VEX_Vpshufb_xmm_xmm_xmmm128, L128;
    VpshufbYmmqqYmmqqYmmqq / VpshufbYmmqqYmmqqMemqq, ShuffleBytes, VEX_Vpshufb_ymm_ymm_ymmm256, L256;
    VphaddwXmmdqXmmdqXmmdq / VphaddwXmmdqXmmdqMemdq, HorizontalAdd(Word), VEX_Vphaddw_xmm_xmm_xmmm128, L128;
    VphaddwYmmqqYmmqqYmmqq / VphaddwYmmqqYmmqqMemqq, HorizontalAdd(Word), VEX_Vphaddw_ymm_ymm_ymmm256, L256;
    VphadddXmmdqXmmdqXmmdq / VphadddXmmdqXmmdqMemdq, HorizontalAdd(Doubleword), VEX_Vphaddd_xmm_xmm_xmmm128, L128;
    VphadddYmmqqYmmqqYmmqq / VphadddYmmqqYmmqqMemqq, HorizontalAdd(Doubleword), VEX_Vphaddd_ymm_ymm_ymmm256, L256;
    VphaddswXmmdqXmmdqXmmdq / VphaddswXmmdqXmmdqMemdq, HorizontalAddSaturating, VEX_Vphaddsw_xmm_xmm_xmmm128, L128;
    VphaddswYmmqqYmmqqYmmqq / VphaddswYmmqqYmmqqMemqq, HorizontalAddSaturating, VEX_Vphaddsw_ymm_ymm_ymmm256, L256;
    VphsubwXmmdqXmmdqXmmdq / VphsubwXmmdqXmmdqMemdq, HorizontalSubtract(Word), VEX_Vphsubw_xmm_xmm_xmmm128, L128;
    VphsubwYmmqqYmmqqYmmqq / VphsubwYmmqqYmmqqMemqq, HorizontalSubtract(Word), VEX_Vphsubw_ymm_ymm_ymmm256, L256;
    VphsubdXmmdqXmmdqXmmdq / VphsubdXmmdqXmmdqMemdq, HorizontalSubtract(Doubleword), VEX_Vphsubd_xmm_xmm_xmmm128, L128;
    VphsubdYmmqqYmmqqYmmqq / VphsubdYmmqqYmmqqMemqq, HorizontalSubtract(Doubleword), VEX_Vphsubd_ymm_ymm_ymmm256, L256;
    VphsubswXmmdqXmmdqXmmdq / VphsubswXmmdqXmmdqMemdq, HorizontalSubtractSaturating, VEX_Vphsubsw_xmm_xmm_xmmm128, L128;
    VphsubswYmmqqYmmqqYmmqq / VphsubswYmmqqYmmqqMemqq, HorizontalSubtractSaturating, VEX_Vphsubsw_ymm_ymm_ymmm256, L256;
    VpmaddubswXmmdqXmmdqXmmdq / VpmaddubswXmmdqXmmdqMemdq, MultiplyAddBytes, VEX_Vpmaddubsw_xmm_xmm_xmmm128, L128;
    VpmaddubswYmmqqYmmqqYmmqq / VpmaddubswYmmqqYmmqqMemqq, MultiplyAddBytes, VEX_Vpmaddubsw_ymm_ymm_ymmm256, L256;
    VpmulhrswXmmdqXmmdqXmmdq / VpmulhrswXmmdqXmmdqMemdq, MultiplyHighRoundScale, VEX_Vpmulhrsw_xmm_xmm_xmmm128, L128;
    VpmulhrswYmmqqYmmqqYmmqq / VpmulhrswYmmqqYmmqqMemqq, MultiplyHighRoundScale, VEX_Vpmulhrsw_ymm_ymm_ymmm256, L256;
};
