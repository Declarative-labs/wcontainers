//! AVX-512's integer forms under EVEX and those of its floating-point forms that only move or combine bits: lane
//! arithmetic and logic, packs and unpacks, shifts and rotates, the compares and tests into an opmask register, the
//! blends, the broadcasts and the moves, at each vector length, as the
//! [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (volume 2) defines them.
//!
//! Every form but a compare or test writes its destination's lanes under a writemask, where it takes one: k0 writes
//! every lane, another opmask register the lanes whose bit is set, and the others keep their value (merging) or are
//! zeroed (zeroing).
//! The destination's bits from the vector length to 511 are zeroed. A compare or test writes one bit for each lane,
//! cleared where the writemask's bit is, and zeroes the opmask register above the lanes.

use alloc::vec::Vec;

use iced_x86::{Code, OpCodeOperandKind};

use crate::forms::Instruction;
use crate::vector::{Lane, LaneOperation, Signedness, join, split};
use crate::{Processor, VECTOR_REGISTERS};

/// The bits of a ZMM register, as four 128-bit quarters, lowest first.
pub type Zmm = [u128; 4];

/// A shift's direction and fill.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneShift {
    Left,
    RightLogical,
    /// The sign in every bit once the count reaches the width.
    RightArithmetic,
}

/// Which compare VPCMP, VPCMPEQ and VPCMPGT make.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComparePredicate {
    /// VPCMP and VPCMPU: the immediate's low three bits choose equal, less, less or equal, false, not equal, not
    /// less, not less or equal, true.
    Immediate(Signedness),
    Equal,
    /// Signed.
    Greater,
}

/// What an EVEX integer form computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvexIntegerOperation {
    /// A lane operation of the legacy skeleton, each 128 bits of the first source (VEX.vvvv) with the second's; for
    /// VPABS, of the one source.
    Lanes(LaneOperation),
    /// Every lane of the first source by the second's low quadword, an XMM register or 16 bytes of memory.
    ShiftByCount(LaneShift),
    /// Every lane of the source by the immediate.
    ShiftByImmediate(LaneShift),
    /// Each lane of the first source by the second's lane, all of its bits.
    ShiftByLane(LaneShift),
    /// By the immediate or by the second source's lane, modulo the lane's width.
    Rotate { left: bool, by_lane: bool },
    /// VPTERNLOG: each bit of the result the immediate's bit that the destination's, the first source's and the
    /// second source's bits index, in that order from the highest.
    TernaryLogic,
    /// VPTESTM: a lane's bit where the sources' lanes share a bit; VPTESTNM where they share none.
    TestIntoMask { not: bool },
    CompareIntoMask(ComparePredicate),
    /// VPBLENDM: the second source's lane where the writemask's bit is set, else the first source's, or zero under
    /// zeroing.
    Blend,
    /// VPBROADCAST, VBROADCASTSS and VBROADCASTSD: the low lane of an XMM register, of memory or of a general
    /// register in every lane.
    Broadcast,
    /// VBROADCASTI32X2 to VBROADCASTF64X4: the low lanes of memory or of an XMM register, as many as the number,
    /// repeated.
    BroadcastTuple(usize),
    /// VMOVDQA and VMOVDQU; the aligned ones need memory aligned to the vector's length.
    Move { aligned: bool },
    /// VPMOVM2B to VPMOVM2Q: each lane all ones where the source opmask register's bit for it is set, else zero.
    FromMask,
    /// VPMOVB2M to VPMOVQ2M: each lane's top bit into the opmask register.
    ToMask,
    /// VPMOVZX and VPMOVSX: each lane the source's lane of the narrower width at the same place, extended.
    Extend { from: Lane, signedness: Signedness },
    /// VPMOVQB to VPMOVUSWB: each lane of the source, of the wider width, narrowed: truncated, or saturated as the
    /// signedness reads it; the vector length is the source's.
    Narrow { from: Lane, saturation: Option<Signedness> },
    /// VDBPSADBW: in each 128 bits, the second source's doublewords chosen by the immediate's two-bit fields, then in
    /// each quadword four sums of absolute byte differences between the first source's doublewords and windows of
    /// the chosen bytes.
    DoubleBlockDifferences,
}

/// Where an EVEX form's last source lies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvexIntegerPlace {
    Register,
    /// Memory, which a form that broadcasts may read as one lane repeated.
    Memory,
    /// A general register, VPBROADCAST's.
    General,
    /// The destination is memory: VMOVDQA's and VMOVDQU's store.
    Store,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvexVectorLength {
    L128,
    L256,
    L512,
}

impl EvexVectorLength {
    pub const fn to_bits(self) -> u32 {
        match self {
            Self::L128 => 128,
            Self::L256 => 256,
            Self::L512 => 512,
        }
    }

    /// The 128-bit quarters of a ZMM register a vector of this length fills.
    pub const fn to_quarters(self) -> usize {
        self.to_bits() as usize / 128
    }

    /// The length of a vector of `to` lanes as many as this length holds `from` lanes, at least 128 bits.
    pub const fn scale(self, from: Lane, to: Lane) -> Self {
        match self.to_bits() / from.to_bits() * to.to_bits() {
            ..=128 => Self::L128,
            256 => Self::L256,
            _ => Self::L512,
        }
    }
}

/// One EVEX integer form: its operation, the lanes its writemask and broadcast work on, where its last source lies,
/// its vector length and its iced-x86 codes (two where XED's form is both a load's and a store's encoding).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexIntegerForm {
    pub instruction: Instruction,
    pub operation: EvexIntegerOperation,
    pub lane: Lane,
    pub place: EvexIntegerPlace,
    pub length: EvexVectorLength,
    pub codes: &'static [Code],
}

impl EvexIntegerForm {
    /// The lanes of the vector; a narrowing form's are its source's.
    pub const fn count_lanes(&self) -> usize {
        match self.operation {
            EvexIntegerOperation::Narrow { from, .. } => (self.length.to_bits() / from.to_bits()) as usize,
            _ => (self.length.to_bits() / self.lane.to_bits()) as usize,
        }
    }

    /// Whether the form writes an opmask register rather than a vector.
    pub const fn writes_mask(&self) -> bool {
        matches!(
            self.operation,
            EvexIntegerOperation::TestIntoMask { .. } | EvexIntegerOperation::CompareIntoMask(_) | EvexIntegerOperation::ToMask
        )
    }

    /// The lane of the sources, which a broadcast repeats: twice the destination's for a pack, the words or bytes
    /// whose pairs or groups VPMADDWD, VPMADDUBSW and VPSADBW sum.
    pub const fn source_lane(&self) -> Lane {
        match self.operation {
            EvexIntegerOperation::Lanes(LaneOperation::Pack(lane, _)) => lane,
            EvexIntegerOperation::Lanes(LaneOperation::MultiplyAddWords) => Lane::Word,
            EvexIntegerOperation::Lanes(LaneOperation::MultiplyAddBytes | LaneOperation::SumAbsoluteDifferences)
            | EvexIntegerOperation::DoubleBlockDifferences => Lane::Byte,
            EvexIntegerOperation::Extend { from, .. } | EvexIntegerOperation::Narrow { from, .. } => from,
            _ => self.lane,
        }
    }

    /// The lane of the memory operand's elements, which faults are suppressed by: a narrowing store's narrow lane,
    /// else the source's.
    pub const fn memory_lane(&self) -> Lane {
        match self.operation {
            EvexIntegerOperation::Narrow { .. } => self.lane,
            _ => self.source_lane(),
        }
    }

    /// Whether the form's memory operand may be one lane broadcast ({1toN}).
    pub fn can_broadcast(&self) -> bool {
        self.codes[0].op_code().can_broadcast()
    }

    /// Whether the form takes a writemask.
    pub fn can_mask(&self) -> bool {
        self.codes[0].op_code().can_use_op_mask_register()
    }

    /// Whether the form's writemask may zero.
    pub fn can_zero(&self) -> bool {
        self.codes[0].op_code().can_use_zeroing_masking()
    }

    /// Whether each lane of the memory source is the destination's lane at the same place, so that the writemask
    /// decides which lanes are read; the others read the whole operand.
    pub const fn is_element_wise(&self) -> bool {
        match self.operation {
            EvexIntegerOperation::Lanes(operation) => !matches!(
                operation,
                LaneOperation::Pack(..)
                    | LaneOperation::UnpackHigh(_)
                    | LaneOperation::UnpackLow(_)
                    | LaneOperation::MultiplyAddWords
                    | LaneOperation::MultiplyAddBytes
                    | LaneOperation::SumAbsoluteDifferences
                    | LaneOperation::ShiftBytesLeft
                    | LaneOperation::ShiftBytesRight
                    | LaneOperation::DuplicateEvenSingles
                    | LaneOperation::DuplicateOddSingles
                    | LaneOperation::DuplicateLowDouble
            ),
            EvexIntegerOperation::ShiftByCount(_) | EvexIntegerOperation::DoubleBlockDifferences => false,
            _ => true,
        }
    }

    /// The bytes a memory operand reaches: the vector, the shift count's 16 bytes, a broadcast's lane or tuple.
    pub fn memory_bytes(&self, broadcast: bool) -> u64 {
        let op_code = self.codes[0].op_code();
        let size = if broadcast { op_code.broadcast_memory_size() } else { op_code.memory_size() };
        size.size() as u64
    }

    /// The alignment the memory operand needs whatever AC holds: the vector's length for VMOVDQA.
    pub const fn to_alignment(&self) -> Option<u64> {
        match self.operation {
            EvexIntegerOperation::Move { aligned: true } => Some(self.length.to_bits() as u64 / 8),
            _ => None,
        }
    }

    /// Whether the form reads its destination besides its writemask's merging: VPTERNLOG does.
    pub const fn reads_destination(&self) -> bool {
        matches!(self.operation, EvexIntegerOperation::TernaryLogic)
    }

    /// Whether the form names a first source in VEX.vvvv besides its last source.
    pub const fn has_first_source(&self) -> bool {
        !matches!(
            self.operation,
            EvexIntegerOperation::Lanes(
                LaneOperation::Absolute(_)
                    | LaneOperation::ShiftBytesLeft
                    | LaneOperation::ShiftBytesRight
                    | LaneOperation::DuplicateEvenSingles
                    | LaneOperation::DuplicateOddSingles
                    | LaneOperation::DuplicateLowDouble
            ) | EvexIntegerOperation::ShiftByImmediate(_)
                | EvexIntegerOperation::Rotate { by_lane: false, .. }
                | EvexIntegerOperation::Broadcast
                | EvexIntegerOperation::BroadcastTuple(_)
                | EvexIntegerOperation::Move { .. }
                | EvexIntegerOperation::FromMask
                | EvexIntegerOperation::ToMask
                | EvexIntegerOperation::Extend { .. }
                | EvexIntegerOperation::Narrow { .. }
        )
    }

    /// Whether the form has an immediate.
    pub fn has_immediate(&self) -> bool {
        self.codes[0].op_code().op_kinds().contains(&OpCodeOperandKind::imm8)
    }
}

/// The lanes of a vector, lowest first, as many as `count`.
pub fn to_lanes(lane: Lane, value: &Zmm, count: usize) -> Vec<u64> {
    value.iter().flat_map(|quarter| split(lane, *quarter)).take(count).collect()
}

/// A vector of lanes, lowest first; the bits past them zero.
pub fn from_lanes(lane: Lane, lanes: &[u64]) -> Zmm {
    let per_quarter = (u128::BITS / lane.to_bits()) as usize;
    core::array::from_fn(|quarter| {
        let start = (quarter * per_quarter).min(lanes.len());
        let end = ((quarter + 1) * per_quarter).min(lanes.len());
        join(lane, lanes[start..end].iter().copied())
    })
}

/// The whole ZMM register of the number, 0 to 31.
pub fn read_zmm(processor: &Processor, number: usize) -> Zmm {
    if number < VECTOR_REGISTERS {
        let [third, fourth] = processor.zmm_upper_halves[number];
        [processor.vector_registers[number], processor.upper_vector_halves[number], third, fourth]
    } else {
        processor.high_vector_registers[number - VECTOR_REGISTERS]
    }
}

pub fn write_zmm(processor: &mut Processor, number: usize, value: Zmm) {
    if number < VECTOR_REGISTERS {
        processor.vector_registers[number] = value[0];
        processor.upper_vector_halves[number] = value[1];
        processor.zmm_upper_halves[number] = [value[2], value[3]];
    } else {
        processor.high_vector_registers[number - VECTOR_REGISTERS] = value;
    }
}

const fn to_lane_mask(bits: u32) -> u64 {
    if bits >= u64::BITS { u64::MAX } else { (1 << bits) - 1 }
}

impl LaneShift {
    /// The lane shifted by the count, the count's bits all read.
    const fn apply(self, value: u64, count: u64, bits: u32) -> u64 {
        let mask = to_lane_mask(bits);
        let unused = u64::BITS - bits;
        match self {
            Self::Left if count >= bits as u64 => 0,
            Self::Left => (value << count) & mask,
            Self::RightLogical if count >= bits as u64 => 0,
            Self::RightLogical => value >> count,
            Self::RightArithmetic => {
                let signed = ((value << unused) as i64) >> unused;
                let count = if count >= bits as u64 { bits as u64 - 1 } else { count };
                (signed >> count) as u64 & mask
            }
        }
    }

    const fn to_lane_operation(self, lane: Lane) -> LaneOperation {
        match self {
            Self::Left => LaneOperation::ShiftLeft(lane),
            Self::RightLogical => LaneOperation::ShiftRightLogical(lane),
            Self::RightArithmetic => LaneOperation::ShiftRightArithmetic(lane),
        }
    }
}

impl EvexIntegerForm {
    /// The result before the writemask: of the destination (read by VPTERNLOG alone), the first source and the
    /// second (for a form of one source, that source), with the immediate; the bits past the vector's length zero.
    /// For a compare or test, one bit for each lane in the low quarter's low 64 bits.
    pub fn compute(&self, destination: &Zmm, first: &Zmm, second: &Zmm, immediate: u8) -> Zmm {
        let result = self.compute_unplanted(destination, first, second, immediate);
        crate::planted::mutate_evex(self, destination, first, second, immediate, result)
    }

    pub(crate) fn compute_unplanted(&self, destination: &Zmm, first: &Zmm, second: &Zmm, immediate: u8) -> Zmm {
        let lane = self.lane;
        let bits = lane.to_bits();
        let count = self.count_lanes();
        let quarters = self.length.to_quarters();
        let per_quarter = |apply: &dyn Fn(usize) -> u128| -> Zmm {
            core::array::from_fn(|quarter| if quarter < quarters { apply(quarter) } else { 0 })
        };
        let each = |combine: &dyn Fn(u64, u64) -> u64| -> Zmm {
            let (first, second) = (to_lanes(lane, first, count), to_lanes(lane, second, count));
            from_lanes(lane, &first.iter().zip(&second).map(|(&a, &b)| combine(a, b)).collect::<Vec<_>>())
        };
        let to_bitmask = |holds: &dyn Fn(u64, u64) -> bool| -> Zmm {
            let (first, second) = (to_lanes(lane, first, count), to_lanes(lane, second, count));
            let bits = first.iter().zip(&second).enumerate().fold(0u64, |all, (index, (&a, &b))| all | (u64::from(holds(a, b)) << index));
            [u128::from(bits), 0, 0, 0]
        };
        let signed = |value: u64| crate::vector::to_signed(value, bits);
        match self.operation {
            EvexIntegerOperation::Lanes(operation @ (LaneOperation::ShiftBytesLeft | LaneOperation::ShiftBytesRight)) => {
                per_quarter(&|quarter| operation.compute(second[quarter], u128::from(immediate), 0))
            }
            EvexIntegerOperation::Lanes(operation) => per_quarter(&|quarter| operation.compute(first[quarter], second[quarter], 0)),
            EvexIntegerOperation::ShiftByCount(shift) => {
                per_quarter(&|quarter| shift.to_lane_operation(lane).compute(first[quarter], second[0] & u128::from(u64::MAX), 0))
            }
            EvexIntegerOperation::ShiftByImmediate(shift) => {
                per_quarter(&|quarter| shift.to_lane_operation(lane).compute(second[quarter], u128::from(immediate), 0))
            }
            EvexIntegerOperation::ShiftByLane(shift) => each(&|value, count| shift.apply(value, count, bits)),
            EvexIntegerOperation::Rotate { left, by_lane } => {
                let source = if by_lane { first } else { second };
                let counts = if by_lane { to_lanes(lane, second, count) } else { alloc::vec![u64::from(immediate); count] };
                let values = to_lanes(lane, source, count);
                let rotated: Vec<u64> = values
                    .iter()
                    .zip(&counts)
                    .map(|(&value, &by)| rotate(value, by % u64::from(bits), bits, left))
                    .collect();
                from_lanes(lane, &rotated)
            }
            EvexIntegerOperation::TernaryLogic => per_quarter(&|quarter| {
                let (a, b, c) = (destination[quarter], first[quarter], second[quarter]);
                (0..8).filter(|index| immediate >> index & 1 != 0).fold(0, |all, index| {
                    let pick = |value: u128, bit: u32| if index >> bit & 1 != 0 { value } else { !value };
                    all | (pick(a, 2) & pick(b, 1) & pick(c, 0))
                })
            }),
            EvexIntegerOperation::TestIntoMask { not } => to_bitmask(&|a, b| (a & b != 0) != not),
            EvexIntegerOperation::CompareIntoMask(predicate) => match predicate {
                ComparePredicate::Equal => to_bitmask(&|a, b| a == b),
                ComparePredicate::Greater => to_bitmask(&|a, b| signed(a) > signed(b)),
                ComparePredicate::Immediate(signedness) => {
                    let order = |a: u64, b: u64| match signedness {
                        Signedness::Signed => signed(a).cmp(&signed(b)),
                        Signedness::Unsigned => a.cmp(&b),
                    };
                    to_bitmask(&|a, b| {
                        let order = order(a, b);
                        match immediate & 7 {
                            0 => order.is_eq(),
                            1 => order.is_lt(),
                            2 => order.is_le(),
                            3 => false,
                            4 => order.is_ne(),
                            5 => order.is_ge(),
                            6 => order.is_gt(),
                            _ => true,
                        }
                    })
                }
            },
            EvexIntegerOperation::Blend => per_quarter(&|quarter| second[quarter]),
            EvexIntegerOperation::Broadcast => {
                let value = to_lanes(lane, second, 1)[0];
                from_lanes(lane, &alloc::vec![value; count])
            }
            EvexIntegerOperation::BroadcastTuple(elements) => {
                let tuple = to_lanes(lane, second, elements);
                from_lanes(lane, &(0..count).map(|index| tuple[index % elements]).collect::<Vec<_>>())
            }
            EvexIntegerOperation::Move { .. } => per_quarter(&|quarter| second[quarter]),
            EvexIntegerOperation::FromMask => {
                let ones = to_lane_mask(bits);
                from_lanes(lane, &(0..count).map(|index| if second[0] >> index & 1 != 0 { ones } else { 0 }).collect::<Vec<_>>())
            }
            EvexIntegerOperation::ToMask => {
                let lanes = to_lanes(lane, second, count);
                [u128::from(lanes.iter().enumerate().fold(0u64, |all, (index, value)| all | (value >> (bits - 1) & 1) << index)), 0, 0, 0]
            }
            EvexIntegerOperation::Extend { from, signedness } => {
                let extended = to_lanes(from, second, count).into_iter().map(|value| match signedness {
                    Signedness::Signed => crate::vector::to_signed(value, from.to_bits()) as u64 & to_lane_mask(bits),
                    Signedness::Unsigned => value,
                });
                from_lanes(lane, &extended.collect::<Vec<_>>())
            }
            EvexIntegerOperation::Narrow { from, saturation } => {
                let narrowed = to_lanes(from, second, count).into_iter().map(|value| match saturation {
                    None => value & to_lane_mask(bits),
                    Some(Signedness::Signed) => {
                        crate::vector::saturate(lane, Signedness::Signed, i128::from(crate::vector::to_signed(value, from.to_bits())))
                    }
                    Some(Signedness::Unsigned) => crate::vector::saturate(lane, Signedness::Unsigned, i128::from(value)),
                });
                from_lanes(lane, &narrowed.collect::<Vec<_>>())
            }
            EvexIntegerOperation::DoubleBlockDifferences => per_quarter(&|quarter| {
                let chosen = split(Lane::Doubleword, second[quarter]);
                let window: Vec<u64> = (0..4)
                    .map(|field| chosen[usize::from(immediate >> (2 * field) & 3)])
                    .flat_map(|doubleword| (0..4).map(move |byte| doubleword >> (8 * byte) & 0xFF))
                    .collect();
                let bytes = split(Lane::Byte, first[quarter]);
                let sum = |first_at: usize, window_at: usize| -> u64 {
                    (0..4).map(|index| bytes[first_at + index].abs_diff(window[window_at + index])).sum()
                };
                let words = (0..2).flat_map(|half| {
                    let base = 8 * half;
                    [sum(base, base), sum(base, base + 1), sum(base + 4, base + 2), sum(base + 4, base + 3)]
                });
                join(Lane::Word, words)
            }),
        }
    }

    /// The destination after the writemask: each lane the result's where the mask's bit is set (or there is no mask),
    /// else the kept lane, or zero under zeroing; zero past the vector's length. VPBLENDM keeps its first source's
    /// lane where others keep the destination's.
    pub fn apply_writemask(&self, result: &Zmm, kept: &Zmm, mask: Option<u64>, zeroing: bool) -> Zmm {
        let count = self.count_lanes();
        let (result, kept) = (to_lanes(self.lane, result, count), to_lanes(self.lane, kept, count));
        let lanes: Vec<u64> = (0..count)
            .map(|index| match mask {
                Some(mask) if mask >> index & 1 == 0 => if zeroing { 0 } else { kept[index] },
                _ => result[index],
            })
            .collect();
        from_lanes(self.lane, &lanes)
    }

    /// The opmask register a compare or test writes: the result's bits where the writemask's are set, none past
    /// the lanes.
    pub fn apply_mask_writemask(&self, result: &Zmm, mask: Option<u64>) -> u64 {
        let lanes = to_lane_mask(self.count_lanes() as u32);
        (result[0] as u64) & mask.unwrap_or(u64::MAX) & lanes
    }
}

fn rotate(value: u64, by: u64, bits: u32, left: bool) -> u64 {
    let mask = to_lane_mask(bits);
    let by = by as u32;
    if by == 0 {
        return value;
    }
    let (high, low) = if left { (value << by, value >> (bits - by)) } else { (value << (bits - by), value >> by) };
    (high | low) & mask
}

/// The row of an EVEX integer form, or `None` for any other form.
pub fn find_evex_integer_form(instruction: Instruction) -> Option<&'static EvexIntegerForm> {
    EVEX_INTEGER_FORMS.iter().find(|form| form.instruction == instruction)
}

use EvexIntegerOperation::{
    Blend, Broadcast, BroadcastTuple, CompareIntoMask, DoubleBlockDifferences, Extend, FromMask, Lanes, Move, Narrow, Rotate,
    ShiftByCount, ShiftByImmediate, ShiftByLane, TernaryLogic, TestIntoMask, ToMask,
};
use EvexIntegerPlace::{General, Memory, Register, Store};
use Lane::{Byte, Doubleword, Quadword, Word};
use Signedness::{Signed, Unsigned};
use EvexVectorLength::{L128, L256, L512};

macro_rules! evex_forms {
    ($($instruction:ident, $operation:expr, $lane:ident, $place:ident, $length:ident, [$($code:ident),+];)*) => {
        &[$(EvexIntegerForm {
            instruction: Instruction::$instruction,
            operation: $operation,
            lane: $lane,
            place: $place,
            length: $length,
            codes: &[$(Code::$code),+],
        },)*]
    };
}

/// Every EVEX integer form modeled.
pub static EVEX_INTEGER_FORMS: &[EvexIntegerForm] = evex_forms! {
    Vbroadcasti32x4Ymmu32MaskmskwMemu32Avx512, BroadcastTuple(4), Doubleword, Memory, L256, [EVEX_Vbroadcasti32x4_ymm_k1z_m128];
    Vbroadcasti32x4Zmmu32MaskmskwMemu32Avx512, BroadcastTuple(4), Doubleword, Memory, L512, [EVEX_Vbroadcasti32x4_zmm_k1z_m128];
    Vbroadcasti64x4Zmmu64MaskmskwMemu64Avx512, BroadcastTuple(4), Quadword, Memory, L512, [EVEX_Vbroadcasti64x4_zmm_k1z_m256];
    Vmovdqa32Memu32MaskmskwXmmu32Avx512, Move { aligned: true }, Doubleword, Store, L128, [EVEX_Vmovdqa32_xmmm128_k1z_xmm];
    Vmovdqa32Memu32MaskmskwYmmu32Avx512, Move { aligned: true }, Doubleword, Store, L256, [EVEX_Vmovdqa32_ymmm256_k1z_ymm];
    Vmovdqa32Memu32MaskmskwZmmu32Avx512, Move { aligned: true }, Doubleword, Store, L512, [EVEX_Vmovdqa32_zmmm512_k1z_zmm];
    Vmovdqa32Xmmu32MaskmskwMemu32Avx512, Move { aligned: true }, Doubleword, Memory, L128, [EVEX_Vmovdqa32_xmm_k1z_xmmm128];
    Vmovdqa32Xmmu32MaskmskwXmmu32Avx512, Move { aligned: true }, Doubleword, Register, L128, [EVEX_Vmovdqa32_xmm_k1z_xmmm128, EVEX_Vmovdqa32_xmmm128_k1z_xmm];
    Vmovdqa32Ymmu32MaskmskwMemu32Avx512, Move { aligned: true }, Doubleword, Memory, L256, [EVEX_Vmovdqa32_ymm_k1z_ymmm256];
    Vmovdqa32Ymmu32MaskmskwYmmu32Avx512, Move { aligned: true }, Doubleword, Register, L256, [EVEX_Vmovdqa32_ymm_k1z_ymmm256, EVEX_Vmovdqa32_ymmm256_k1z_ymm];
    Vmovdqa32Zmmu32MaskmskwMemu32Avx512, Move { aligned: true }, Doubleword, Memory, L512, [EVEX_Vmovdqa32_zmm_k1z_zmmm512];
    Vmovdqa32Zmmu32MaskmskwZmmu32Avx512, Move { aligned: true }, Doubleword, Register, L512, [EVEX_Vmovdqa32_zmm_k1z_zmmm512, EVEX_Vmovdqa32_zmmm512_k1z_zmm];
    Vmovdqa64Memu64MaskmskwXmmu64Avx512, Move { aligned: true }, Quadword, Store, L128, [EVEX_Vmovdqa64_xmmm128_k1z_xmm];
    Vmovdqa64Memu64MaskmskwYmmu64Avx512, Move { aligned: true }, Quadword, Store, L256, [EVEX_Vmovdqa64_ymmm256_k1z_ymm];
    Vmovdqa64Memu64MaskmskwZmmu64Avx512, Move { aligned: true }, Quadword, Store, L512, [EVEX_Vmovdqa64_zmmm512_k1z_zmm];
    Vmovdqa64Xmmu64MaskmskwMemu64Avx512, Move { aligned: true }, Quadword, Memory, L128, [EVEX_Vmovdqa64_xmm_k1z_xmmm128];
    Vmovdqa64Xmmu64MaskmskwXmmu64Avx512, Move { aligned: true }, Quadword, Register, L128, [EVEX_Vmovdqa64_xmm_k1z_xmmm128, EVEX_Vmovdqa64_xmmm128_k1z_xmm];
    Vmovdqa64Ymmu64MaskmskwMemu64Avx512, Move { aligned: true }, Quadword, Memory, L256, [EVEX_Vmovdqa64_ymm_k1z_ymmm256];
    Vmovdqa64Ymmu64MaskmskwYmmu64Avx512, Move { aligned: true }, Quadword, Register, L256, [EVEX_Vmovdqa64_ymm_k1z_ymmm256, EVEX_Vmovdqa64_ymmm256_k1z_ymm];
    Vmovdqa64Zmmu64MaskmskwMemu64Avx512, Move { aligned: true }, Quadword, Memory, L512, [EVEX_Vmovdqa64_zmm_k1z_zmmm512];
    Vmovdqa64Zmmu64MaskmskwZmmu64Avx512, Move { aligned: true }, Quadword, Register, L512, [EVEX_Vmovdqa64_zmm_k1z_zmmm512, EVEX_Vmovdqa64_zmmm512_k1z_zmm];
    Vmovdqu32Memu32MaskmskwXmmu32Avx512, Move { aligned: false }, Doubleword, Store, L128, [EVEX_Vmovdqu32_xmmm128_k1z_xmm];
    Vmovdqu32Memu32MaskmskwYmmu32Avx512, Move { aligned: false }, Doubleword, Store, L256, [EVEX_Vmovdqu32_ymmm256_k1z_ymm];
    Vmovdqu32Memu32MaskmskwZmmu32Avx512, Move { aligned: false }, Doubleword, Store, L512, [EVEX_Vmovdqu32_zmmm512_k1z_zmm];
    Vmovdqu32Xmmu32MaskmskwMemu32Avx512, Move { aligned: false }, Doubleword, Memory, L128, [EVEX_Vmovdqu32_xmm_k1z_xmmm128];
    Vmovdqu32Xmmu32MaskmskwXmmu32Avx512, Move { aligned: false }, Doubleword, Register, L128, [EVEX_Vmovdqu32_xmm_k1z_xmmm128, EVEX_Vmovdqu32_xmmm128_k1z_xmm];
    Vmovdqu32Ymmu32MaskmskwMemu32Avx512, Move { aligned: false }, Doubleword, Memory, L256, [EVEX_Vmovdqu32_ymm_k1z_ymmm256];
    Vmovdqu32Ymmu32MaskmskwYmmu32Avx512, Move { aligned: false }, Doubleword, Register, L256, [EVEX_Vmovdqu32_ymm_k1z_ymmm256, EVEX_Vmovdqu32_ymmm256_k1z_ymm];
    Vmovdqu32Zmmu32MaskmskwMemu32Avx512, Move { aligned: false }, Doubleword, Memory, L512, [EVEX_Vmovdqu32_zmm_k1z_zmmm512];
    Vmovdqu32Zmmu32MaskmskwZmmu32Avx512, Move { aligned: false }, Doubleword, Register, L512, [EVEX_Vmovdqu32_zmm_k1z_zmmm512, EVEX_Vmovdqu32_zmmm512_k1z_zmm];
    Vmovdqu64Memu64MaskmskwXmmu64Avx512, Move { aligned: false }, Quadword, Store, L128, [EVEX_Vmovdqu64_xmmm128_k1z_xmm];
    Vmovdqu64Memu64MaskmskwYmmu64Avx512, Move { aligned: false }, Quadword, Store, L256, [EVEX_Vmovdqu64_ymmm256_k1z_ymm];
    Vmovdqu64Memu64MaskmskwZmmu64Avx512, Move { aligned: false }, Quadword, Store, L512, [EVEX_Vmovdqu64_zmmm512_k1z_zmm];
    Vmovdqu64Xmmu64MaskmskwMemu64Avx512, Move { aligned: false }, Quadword, Memory, L128, [EVEX_Vmovdqu64_xmm_k1z_xmmm128];
    Vmovdqu64Xmmu64MaskmskwXmmu64Avx512, Move { aligned: false }, Quadword, Register, L128, [EVEX_Vmovdqu64_xmm_k1z_xmmm128, EVEX_Vmovdqu64_xmmm128_k1z_xmm];
    Vmovdqu64Ymmu64MaskmskwMemu64Avx512, Move { aligned: false }, Quadword, Memory, L256, [EVEX_Vmovdqu64_ymm_k1z_ymmm256];
    Vmovdqu64Ymmu64MaskmskwYmmu64Avx512, Move { aligned: false }, Quadword, Register, L256, [EVEX_Vmovdqu64_ymm_k1z_ymmm256, EVEX_Vmovdqu64_ymmm256_k1z_ymm];
    Vmovdqu64Zmmu64MaskmskwMemu64Avx512, Move { aligned: false }, Quadword, Memory, L512, [EVEX_Vmovdqu64_zmm_k1z_zmmm512];
    Vmovdqu64Zmmu64MaskmskwZmmu64Avx512, Move { aligned: false }, Quadword, Register, L512, [EVEX_Vmovdqu64_zmm_k1z_zmmm512, EVEX_Vmovdqu64_zmmm512_k1z_zmm];
    VpabsdXmmi32MaskmskwMemi32Avx512, Lanes(LaneOperation::Absolute(Doubleword)), Doubleword, Memory, L128, [EVEX_Vpabsd_xmm_k1z_xmmm128b32];
    VpabsdXmmi32MaskmskwXmmi32Avx512, Lanes(LaneOperation::Absolute(Doubleword)), Doubleword, Register, L128, [EVEX_Vpabsd_xmm_k1z_xmmm128b32];
    VpabsdYmmi32MaskmskwMemi32Avx512, Lanes(LaneOperation::Absolute(Doubleword)), Doubleword, Memory, L256, [EVEX_Vpabsd_ymm_k1z_ymmm256b32];
    VpabsdYmmi32MaskmskwYmmi32Avx512, Lanes(LaneOperation::Absolute(Doubleword)), Doubleword, Register, L256, [EVEX_Vpabsd_ymm_k1z_ymmm256b32];
    VpabsdZmmi32MaskmskwMemi32Avx512, Lanes(LaneOperation::Absolute(Doubleword)), Doubleword, Memory, L512, [EVEX_Vpabsd_zmm_k1z_zmmm512b32];
    VpabsdZmmi32MaskmskwZmmi32Avx512, Lanes(LaneOperation::Absolute(Doubleword)), Doubleword, Register, L512, [EVEX_Vpabsd_zmm_k1z_zmmm512b32];
    VpabsqXmmi64MaskmskwMemi64Avx512, Lanes(LaneOperation::Absolute(Quadword)), Quadword, Memory, L128, [EVEX_Vpabsq_xmm_k1z_xmmm128b64];
    VpabsqXmmi64MaskmskwXmmi64Avx512, Lanes(LaneOperation::Absolute(Quadword)), Quadword, Register, L128, [EVEX_Vpabsq_xmm_k1z_xmmm128b64];
    VpabsqYmmi64MaskmskwMemi64Avx512, Lanes(LaneOperation::Absolute(Quadword)), Quadword, Memory, L256, [EVEX_Vpabsq_ymm_k1z_ymmm256b64];
    VpabsqYmmi64MaskmskwYmmi64Avx512, Lanes(LaneOperation::Absolute(Quadword)), Quadword, Register, L256, [EVEX_Vpabsq_ymm_k1z_ymmm256b64];
    VpabsqZmmi64MaskmskwMemi64Avx512, Lanes(LaneOperation::Absolute(Quadword)), Quadword, Memory, L512, [EVEX_Vpabsq_zmm_k1z_zmmm512b64];
    VpabsqZmmi64MaskmskwZmmi64Avx512, Lanes(LaneOperation::Absolute(Quadword)), Quadword, Register, L512, [EVEX_Vpabsq_zmm_k1z_zmmm512b64];
    VpadddXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Add(Doubleword)), Doubleword, Memory, L128, [EVEX_Vpaddd_xmm_k1z_xmm_xmmm128b32];
    VpadddXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Add(Doubleword)), Doubleword, Register, L128, [EVEX_Vpaddd_xmm_k1z_xmm_xmmm128b32];
    VpadddYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Add(Doubleword)), Doubleword, Memory, L256, [EVEX_Vpaddd_ymm_k1z_ymm_ymmm256b32];
    VpadddYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Add(Doubleword)), Doubleword, Register, L256, [EVEX_Vpaddd_ymm_k1z_ymm_ymmm256b32];
    VpadddZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Add(Doubleword)), Doubleword, Memory, L512, [EVEX_Vpaddd_zmm_k1z_zmm_zmmm512b32];
    VpadddZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Add(Doubleword)), Doubleword, Register, L512, [EVEX_Vpaddd_zmm_k1z_zmm_zmmm512b32];
    VpaddqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::Add(Quadword)), Quadword, Memory, L128, [EVEX_Vpaddq_xmm_k1z_xmm_xmmm128b64];
    VpaddqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::Add(Quadword)), Quadword, Register, L128, [EVEX_Vpaddq_xmm_k1z_xmm_xmmm128b64];
    VpaddqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::Add(Quadword)), Quadword, Memory, L256, [EVEX_Vpaddq_ymm_k1z_ymm_ymmm256b64];
    VpaddqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::Add(Quadword)), Quadword, Register, L256, [EVEX_Vpaddq_ymm_k1z_ymm_ymmm256b64];
    VpaddqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::Add(Quadword)), Quadword, Memory, L512, [EVEX_Vpaddq_zmm_k1z_zmm_zmmm512b64];
    VpaddqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::Add(Quadword)), Quadword, Register, L512, [EVEX_Vpaddq_zmm_k1z_zmm_zmmm512b64];
    VpanddXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::And), Doubleword, Memory, L128, [EVEX_Vpandd_xmm_k1z_xmm_xmmm128b32];
    VpanddXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::And), Doubleword, Register, L128, [EVEX_Vpandd_xmm_k1z_xmm_xmmm128b32];
    VpanddYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::And), Doubleword, Memory, L256, [EVEX_Vpandd_ymm_k1z_ymm_ymmm256b32];
    VpanddYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::And), Doubleword, Register, L256, [EVEX_Vpandd_ymm_k1z_ymm_ymmm256b32];
    VpanddZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::And), Doubleword, Memory, L512, [EVEX_Vpandd_zmm_k1z_zmm_zmmm512b32];
    VpanddZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::And), Doubleword, Register, L512, [EVEX_Vpandd_zmm_k1z_zmm_zmmm512b32];
    VpandndXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Memory, L128, [EVEX_Vpandnd_xmm_k1z_xmm_xmmm128b32];
    VpandndXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Register, L128, [EVEX_Vpandnd_xmm_k1z_xmm_xmmm128b32];
    VpandndYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Memory, L256, [EVEX_Vpandnd_ymm_k1z_ymm_ymmm256b32];
    VpandndYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Register, L256, [EVEX_Vpandnd_ymm_k1z_ymm_ymmm256b32];
    VpandndZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Memory, L512, [EVEX_Vpandnd_zmm_k1z_zmm_zmmm512b32];
    VpandndZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Register, L512, [EVEX_Vpandnd_zmm_k1z_zmm_zmmm512b32];
    VpandnqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Memory, L128, [EVEX_Vpandnq_xmm_k1z_xmm_xmmm128b64];
    VpandnqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Register, L128, [EVEX_Vpandnq_xmm_k1z_xmm_xmmm128b64];
    VpandnqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Memory, L256, [EVEX_Vpandnq_ymm_k1z_ymm_ymmm256b64];
    VpandnqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Register, L256, [EVEX_Vpandnq_ymm_k1z_ymm_ymmm256b64];
    VpandnqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Memory, L512, [EVEX_Vpandnq_zmm_k1z_zmm_zmmm512b64];
    VpandnqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Register, L512, [EVEX_Vpandnq_zmm_k1z_zmm_zmmm512b64];
    VpandqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::And), Quadword, Memory, L128, [EVEX_Vpandq_xmm_k1z_xmm_xmmm128b64];
    VpandqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::And), Quadword, Register, L128, [EVEX_Vpandq_xmm_k1z_xmm_xmmm128b64];
    VpandqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::And), Quadword, Memory, L256, [EVEX_Vpandq_ymm_k1z_ymm_ymmm256b64];
    VpandqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::And), Quadword, Register, L256, [EVEX_Vpandq_ymm_k1z_ymm_ymmm256b64];
    VpandqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::And), Quadword, Memory, L512, [EVEX_Vpandq_zmm_k1z_zmm_zmmm512b64];
    VpandqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::And), Quadword, Register, L512, [EVEX_Vpandq_zmm_k1z_zmm_zmmm512b64];
    VpblendmdXmmu32MaskmskwXmmu32Memu32Avx512, Blend, Doubleword, Memory, L128, [EVEX_Vpblendmd_xmm_k1z_xmm_xmmm128b32];
    VpblendmdXmmu32MaskmskwXmmu32Xmmu32Avx512, Blend, Doubleword, Register, L128, [EVEX_Vpblendmd_xmm_k1z_xmm_xmmm128b32];
    VpblendmdYmmu32MaskmskwYmmu32Memu32Avx512, Blend, Doubleword, Memory, L256, [EVEX_Vpblendmd_ymm_k1z_ymm_ymmm256b32];
    VpblendmdYmmu32MaskmskwYmmu32Ymmu32Avx512, Blend, Doubleword, Register, L256, [EVEX_Vpblendmd_ymm_k1z_ymm_ymmm256b32];
    VpblendmdZmmu32MaskmskwZmmu32Memu32Avx512, Blend, Doubleword, Memory, L512, [EVEX_Vpblendmd_zmm_k1z_zmm_zmmm512b32];
    VpblendmdZmmu32MaskmskwZmmu32Zmmu32Avx512, Blend, Doubleword, Register, L512, [EVEX_Vpblendmd_zmm_k1z_zmm_zmmm512b32];
    VpblendmqXmmu64MaskmskwXmmu64Memu64Avx512, Blend, Quadword, Memory, L128, [EVEX_Vpblendmq_xmm_k1z_xmm_xmmm128b64];
    VpblendmqXmmu64MaskmskwXmmu64Xmmu64Avx512, Blend, Quadword, Register, L128, [EVEX_Vpblendmq_xmm_k1z_xmm_xmmm128b64];
    VpblendmqYmmu64MaskmskwYmmu64Memu64Avx512, Blend, Quadword, Memory, L256, [EVEX_Vpblendmq_ymm_k1z_ymm_ymmm256b64];
    VpblendmqYmmu64MaskmskwYmmu64Ymmu64Avx512, Blend, Quadword, Register, L256, [EVEX_Vpblendmq_ymm_k1z_ymm_ymmm256b64];
    VpblendmqZmmu64MaskmskwZmmu64Memu64Avx512, Blend, Quadword, Memory, L512, [EVEX_Vpblendmq_zmm_k1z_zmm_zmmm512b64];
    VpblendmqZmmu64MaskmskwZmmu64Zmmu64Avx512, Blend, Quadword, Register, L512, [EVEX_Vpblendmq_zmm_k1z_zmm_zmmm512b64];
    VpbroadcastdXmmu32MaskmskwGpr32u32Avx512, Broadcast, Doubleword, General, L128, [EVEX_Vpbroadcastd_xmm_k1z_r32];
    VpbroadcastdXmmu32MaskmskwMemu32Avx512, Broadcast, Doubleword, Memory, L128, [EVEX_Vpbroadcastd_xmm_k1z_xmmm32];
    VpbroadcastdXmmu32MaskmskwXmmu32Avx512, Broadcast, Doubleword, Register, L128, [EVEX_Vpbroadcastd_xmm_k1z_xmmm32];
    VpbroadcastdYmmu32MaskmskwGpr32u32Avx512, Broadcast, Doubleword, General, L256, [EVEX_Vpbroadcastd_ymm_k1z_r32];
    VpbroadcastdYmmu32MaskmskwMemu32Avx512, Broadcast, Doubleword, Memory, L256, [EVEX_Vpbroadcastd_ymm_k1z_xmmm32];
    VpbroadcastdYmmu32MaskmskwXmmu32Avx512, Broadcast, Doubleword, Register, L256, [EVEX_Vpbroadcastd_ymm_k1z_xmmm32];
    VpbroadcastdZmmu32MaskmskwGpr32u32Avx512, Broadcast, Doubleword, General, L512, [EVEX_Vpbroadcastd_zmm_k1z_r32];
    VpbroadcastdZmmu32MaskmskwMemu32Avx512, Broadcast, Doubleword, Memory, L512, [EVEX_Vpbroadcastd_zmm_k1z_xmmm32];
    VpbroadcastdZmmu32MaskmskwXmmu32Avx512, Broadcast, Doubleword, Register, L512, [EVEX_Vpbroadcastd_zmm_k1z_xmmm32];
    VpbroadcastqXmmu64MaskmskwGpr64u64Avx512, Broadcast, Quadword, General, L128, [EVEX_Vpbroadcastq_xmm_k1z_r64];
    VpbroadcastqXmmu64MaskmskwMemu64Avx512, Broadcast, Quadword, Memory, L128, [EVEX_Vpbroadcastq_xmm_k1z_xmmm64];
    VpbroadcastqXmmu64MaskmskwXmmu64Avx512, Broadcast, Quadword, Register, L128, [EVEX_Vpbroadcastq_xmm_k1z_xmmm64];
    VpbroadcastqYmmu64MaskmskwGpr64u64Avx512, Broadcast, Quadword, General, L256, [EVEX_Vpbroadcastq_ymm_k1z_r64];
    VpbroadcastqYmmu64MaskmskwMemu64Avx512, Broadcast, Quadword, Memory, L256, [EVEX_Vpbroadcastq_ymm_k1z_xmmm64];
    VpbroadcastqYmmu64MaskmskwXmmu64Avx512, Broadcast, Quadword, Register, L256, [EVEX_Vpbroadcastq_ymm_k1z_xmmm64];
    VpbroadcastqZmmu64MaskmskwGpr64u64Avx512, Broadcast, Quadword, General, L512, [EVEX_Vpbroadcastq_zmm_k1z_r64];
    VpbroadcastqZmmu64MaskmskwMemu64Avx512, Broadcast, Quadword, Memory, L512, [EVEX_Vpbroadcastq_zmm_k1z_xmmm64];
    VpbroadcastqZmmu64MaskmskwXmmu64Avx512, Broadcast, Quadword, Register, L512, [EVEX_Vpbroadcastq_zmm_k1z_xmmm64];
    VpcmpdMaskmskwMaskmskwXmmi32Memi32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Doubleword, Memory, L128, [EVEX_Vpcmpd_kr_k1_xmm_xmmm128b32_imm8];
    VpcmpdMaskmskwMaskmskwXmmi32Xmmi32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Doubleword, Register, L128, [EVEX_Vpcmpd_kr_k1_xmm_xmmm128b32_imm8];
    VpcmpdMaskmskwMaskmskwYmmi32Memi32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Doubleword, Memory, L256, [EVEX_Vpcmpd_kr_k1_ymm_ymmm256b32_imm8];
    VpcmpdMaskmskwMaskmskwYmmi32Ymmi32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Doubleword, Register, L256, [EVEX_Vpcmpd_kr_k1_ymm_ymmm256b32_imm8];
    VpcmpdMaskmskwMaskmskwZmmi32Memi32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Doubleword, Memory, L512, [EVEX_Vpcmpd_kr_k1_zmm_zmmm512b32_imm8];
    VpcmpdMaskmskwMaskmskwZmmi32Zmmi32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Doubleword, Register, L512, [EVEX_Vpcmpd_kr_k1_zmm_zmmm512b32_imm8];
    VpcmpeqdMaskmskwMaskmskwXmmu32Memu32Avx512, CompareIntoMask(ComparePredicate::Equal), Doubleword, Memory, L128, [EVEX_Vpcmpeqd_kr_k1_xmm_xmmm128b32];
    VpcmpeqdMaskmskwMaskmskwXmmu32Xmmu32Avx512, CompareIntoMask(ComparePredicate::Equal), Doubleword, Register, L128, [EVEX_Vpcmpeqd_kr_k1_xmm_xmmm128b32];
    VpcmpeqdMaskmskwMaskmskwYmmu32Memu32Avx512, CompareIntoMask(ComparePredicate::Equal), Doubleword, Memory, L256, [EVEX_Vpcmpeqd_kr_k1_ymm_ymmm256b32];
    VpcmpeqdMaskmskwMaskmskwYmmu32Ymmu32Avx512, CompareIntoMask(ComparePredicate::Equal), Doubleword, Register, L256, [EVEX_Vpcmpeqd_kr_k1_ymm_ymmm256b32];
    VpcmpeqdMaskmskwMaskmskwZmmu32Memu32Avx512, CompareIntoMask(ComparePredicate::Equal), Doubleword, Memory, L512, [EVEX_Vpcmpeqd_kr_k1_zmm_zmmm512b32];
    VpcmpeqdMaskmskwMaskmskwZmmu32Zmmu32Avx512, CompareIntoMask(ComparePredicate::Equal), Doubleword, Register, L512, [EVEX_Vpcmpeqd_kr_k1_zmm_zmmm512b32];
    VpcmpeqqMaskmskwMaskmskwXmmu64Memu64Avx512, CompareIntoMask(ComparePredicate::Equal), Quadword, Memory, L128, [EVEX_Vpcmpeqq_kr_k1_xmm_xmmm128b64];
    VpcmpeqqMaskmskwMaskmskwXmmu64Xmmu64Avx512, CompareIntoMask(ComparePredicate::Equal), Quadword, Register, L128, [EVEX_Vpcmpeqq_kr_k1_xmm_xmmm128b64];
    VpcmpeqqMaskmskwMaskmskwYmmu64Memu64Avx512, CompareIntoMask(ComparePredicate::Equal), Quadword, Memory, L256, [EVEX_Vpcmpeqq_kr_k1_ymm_ymmm256b64];
    VpcmpeqqMaskmskwMaskmskwYmmu64Ymmu64Avx512, CompareIntoMask(ComparePredicate::Equal), Quadword, Register, L256, [EVEX_Vpcmpeqq_kr_k1_ymm_ymmm256b64];
    VpcmpeqqMaskmskwMaskmskwZmmu64Memu64Avx512, CompareIntoMask(ComparePredicate::Equal), Quadword, Memory, L512, [EVEX_Vpcmpeqq_kr_k1_zmm_zmmm512b64];
    VpcmpeqqMaskmskwMaskmskwZmmu64Zmmu64Avx512, CompareIntoMask(ComparePredicate::Equal), Quadword, Register, L512, [EVEX_Vpcmpeqq_kr_k1_zmm_zmmm512b64];
    VpcmpgtdMaskmskwMaskmskwXmmi32Memi32Avx512, CompareIntoMask(ComparePredicate::Greater), Doubleword, Memory, L128, [EVEX_Vpcmpgtd_kr_k1_xmm_xmmm128b32];
    VpcmpgtdMaskmskwMaskmskwXmmi32Xmmi32Avx512, CompareIntoMask(ComparePredicate::Greater), Doubleword, Register, L128, [EVEX_Vpcmpgtd_kr_k1_xmm_xmmm128b32];
    VpcmpgtdMaskmskwMaskmskwYmmi32Memi32Avx512, CompareIntoMask(ComparePredicate::Greater), Doubleword, Memory, L256, [EVEX_Vpcmpgtd_kr_k1_ymm_ymmm256b32];
    VpcmpgtdMaskmskwMaskmskwYmmi32Ymmi32Avx512, CompareIntoMask(ComparePredicate::Greater), Doubleword, Register, L256, [EVEX_Vpcmpgtd_kr_k1_ymm_ymmm256b32];
    VpcmpgtdMaskmskwMaskmskwZmmi32Memi32Avx512, CompareIntoMask(ComparePredicate::Greater), Doubleword, Memory, L512, [EVEX_Vpcmpgtd_kr_k1_zmm_zmmm512b32];
    VpcmpgtdMaskmskwMaskmskwZmmi32Zmmi32Avx512, CompareIntoMask(ComparePredicate::Greater), Doubleword, Register, L512, [EVEX_Vpcmpgtd_kr_k1_zmm_zmmm512b32];
    VpcmpgtqMaskmskwMaskmskwXmmi64Memi64Avx512, CompareIntoMask(ComparePredicate::Greater), Quadword, Memory, L128, [EVEX_Vpcmpgtq_kr_k1_xmm_xmmm128b64];
    VpcmpgtqMaskmskwMaskmskwXmmi64Xmmi64Avx512, CompareIntoMask(ComparePredicate::Greater), Quadword, Register, L128, [EVEX_Vpcmpgtq_kr_k1_xmm_xmmm128b64];
    VpcmpgtqMaskmskwMaskmskwYmmi64Memi64Avx512, CompareIntoMask(ComparePredicate::Greater), Quadword, Memory, L256, [EVEX_Vpcmpgtq_kr_k1_ymm_ymmm256b64];
    VpcmpgtqMaskmskwMaskmskwYmmi64Ymmi64Avx512, CompareIntoMask(ComparePredicate::Greater), Quadword, Register, L256, [EVEX_Vpcmpgtq_kr_k1_ymm_ymmm256b64];
    VpcmpgtqMaskmskwMaskmskwZmmi64Memi64Avx512, CompareIntoMask(ComparePredicate::Greater), Quadword, Memory, L512, [EVEX_Vpcmpgtq_kr_k1_zmm_zmmm512b64];
    VpcmpgtqMaskmskwMaskmskwZmmi64Zmmi64Avx512, CompareIntoMask(ComparePredicate::Greater), Quadword, Register, L512, [EVEX_Vpcmpgtq_kr_k1_zmm_zmmm512b64];
    VpcmpqMaskmskwMaskmskwXmmi64Memi64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Quadword, Memory, L128, [EVEX_Vpcmpq_kr_k1_xmm_xmmm128b64_imm8];
    VpcmpqMaskmskwMaskmskwXmmi64Xmmi64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Quadword, Register, L128, [EVEX_Vpcmpq_kr_k1_xmm_xmmm128b64_imm8];
    VpcmpqMaskmskwMaskmskwYmmi64Memi64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Quadword, Memory, L256, [EVEX_Vpcmpq_kr_k1_ymm_ymmm256b64_imm8];
    VpcmpqMaskmskwMaskmskwYmmi64Ymmi64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Quadword, Register, L256, [EVEX_Vpcmpq_kr_k1_ymm_ymmm256b64_imm8];
    VpcmpqMaskmskwMaskmskwZmmi64Memi64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Quadword, Memory, L512, [EVEX_Vpcmpq_kr_k1_zmm_zmmm512b64_imm8];
    VpcmpqMaskmskwMaskmskwZmmi64Zmmi64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Quadword, Register, L512, [EVEX_Vpcmpq_kr_k1_zmm_zmmm512b64_imm8];
    VpcmpudMaskmskwMaskmskwXmmu32Memu32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Doubleword, Memory, L128, [EVEX_Vpcmpud_kr_k1_xmm_xmmm128b32_imm8];
    VpcmpudMaskmskwMaskmskwXmmu32Xmmu32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Doubleword, Register, L128, [EVEX_Vpcmpud_kr_k1_xmm_xmmm128b32_imm8];
    VpcmpudMaskmskwMaskmskwYmmu32Memu32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Doubleword, Memory, L256, [EVEX_Vpcmpud_kr_k1_ymm_ymmm256b32_imm8];
    VpcmpudMaskmskwMaskmskwYmmu32Ymmu32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Doubleword, Register, L256, [EVEX_Vpcmpud_kr_k1_ymm_ymmm256b32_imm8];
    VpcmpudMaskmskwMaskmskwZmmu32Memu32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Doubleword, Memory, L512, [EVEX_Vpcmpud_kr_k1_zmm_zmmm512b32_imm8];
    VpcmpudMaskmskwMaskmskwZmmu32Zmmu32Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Doubleword, Register, L512, [EVEX_Vpcmpud_kr_k1_zmm_zmmm512b32_imm8];
    VpcmpuqMaskmskwMaskmskwXmmu64Memu64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Quadword, Memory, L128, [EVEX_Vpcmpuq_kr_k1_xmm_xmmm128b64_imm8];
    VpcmpuqMaskmskwMaskmskwXmmu64Xmmu64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Quadword, Register, L128, [EVEX_Vpcmpuq_kr_k1_xmm_xmmm128b64_imm8];
    VpcmpuqMaskmskwMaskmskwYmmu64Memu64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Quadword, Memory, L256, [EVEX_Vpcmpuq_kr_k1_ymm_ymmm256b64_imm8];
    VpcmpuqMaskmskwMaskmskwYmmu64Ymmu64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Quadword, Register, L256, [EVEX_Vpcmpuq_kr_k1_ymm_ymmm256b64_imm8];
    VpcmpuqMaskmskwMaskmskwZmmu64Memu64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Quadword, Memory, L512, [EVEX_Vpcmpuq_kr_k1_zmm_zmmm512b64_imm8];
    VpcmpuqMaskmskwMaskmskwZmmu64Zmmu64Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Quadword, Register, L512, [EVEX_Vpcmpuq_kr_k1_zmm_zmmm512b64_imm8];
    VpmaxsdXmmi32MaskmskwXmmi32Memi32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Signed)), Doubleword, Memory, L128, [EVEX_Vpmaxsd_xmm_k1z_xmm_xmmm128b32];
    VpmaxsdXmmi32MaskmskwXmmi32Xmmi32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Signed)), Doubleword, Register, L128, [EVEX_Vpmaxsd_xmm_k1z_xmm_xmmm128b32];
    VpmaxsdYmmi32MaskmskwYmmi32Memi32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Signed)), Doubleword, Memory, L256, [EVEX_Vpmaxsd_ymm_k1z_ymm_ymmm256b32];
    VpmaxsdYmmi32MaskmskwYmmi32Ymmi32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Signed)), Doubleword, Register, L256, [EVEX_Vpmaxsd_ymm_k1z_ymm_ymmm256b32];
    VpmaxsdZmmi32MaskmskwZmmi32Memi32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Signed)), Doubleword, Memory, L512, [EVEX_Vpmaxsd_zmm_k1z_zmm_zmmm512b32];
    VpmaxsdZmmi32MaskmskwZmmi32Zmmi32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Signed)), Doubleword, Register, L512, [EVEX_Vpmaxsd_zmm_k1z_zmm_zmmm512b32];
    VpmaxsqXmmi64MaskmskwXmmi64Memi64Avx512, Lanes(LaneOperation::Maximum(Quadword, Signed)), Quadword, Memory, L128, [EVEX_Vpmaxsq_xmm_k1z_xmm_xmmm128b64];
    VpmaxsqXmmi64MaskmskwXmmi64Xmmi64Avx512, Lanes(LaneOperation::Maximum(Quadword, Signed)), Quadword, Register, L128, [EVEX_Vpmaxsq_xmm_k1z_xmm_xmmm128b64];
    VpmaxsqYmmi64MaskmskwYmmi64Memi64Avx512, Lanes(LaneOperation::Maximum(Quadword, Signed)), Quadword, Memory, L256, [EVEX_Vpmaxsq_ymm_k1z_ymm_ymmm256b64];
    VpmaxsqYmmi64MaskmskwYmmi64Ymmi64Avx512, Lanes(LaneOperation::Maximum(Quadword, Signed)), Quadword, Register, L256, [EVEX_Vpmaxsq_ymm_k1z_ymm_ymmm256b64];
    VpmaxsqZmmi64MaskmskwZmmi64Memi64Avx512, Lanes(LaneOperation::Maximum(Quadword, Signed)), Quadword, Memory, L512, [EVEX_Vpmaxsq_zmm_k1z_zmm_zmmm512b64];
    VpmaxsqZmmi64MaskmskwZmmi64Zmmi64Avx512, Lanes(LaneOperation::Maximum(Quadword, Signed)), Quadword, Register, L512, [EVEX_Vpmaxsq_zmm_k1z_zmm_zmmm512b64];
    VpmaxudXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Unsigned)), Doubleword, Memory, L128, [EVEX_Vpmaxud_xmm_k1z_xmm_xmmm128b32];
    VpmaxudXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Unsigned)), Doubleword, Register, L128, [EVEX_Vpmaxud_xmm_k1z_xmm_xmmm128b32];
    VpmaxudYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Unsigned)), Doubleword, Memory, L256, [EVEX_Vpmaxud_ymm_k1z_ymm_ymmm256b32];
    VpmaxudYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Unsigned)), Doubleword, Register, L256, [EVEX_Vpmaxud_ymm_k1z_ymm_ymmm256b32];
    VpmaxudZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Unsigned)), Doubleword, Memory, L512, [EVEX_Vpmaxud_zmm_k1z_zmm_zmmm512b32];
    VpmaxudZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Maximum(Doubleword, Unsigned)), Doubleword, Register, L512, [EVEX_Vpmaxud_zmm_k1z_zmm_zmmm512b32];
    VpmaxuqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::Maximum(Quadword, Unsigned)), Quadword, Memory, L128, [EVEX_Vpmaxuq_xmm_k1z_xmm_xmmm128b64];
    VpmaxuqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::Maximum(Quadword, Unsigned)), Quadword, Register, L128, [EVEX_Vpmaxuq_xmm_k1z_xmm_xmmm128b64];
    VpmaxuqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::Maximum(Quadword, Unsigned)), Quadword, Memory, L256, [EVEX_Vpmaxuq_ymm_k1z_ymm_ymmm256b64];
    VpmaxuqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::Maximum(Quadword, Unsigned)), Quadword, Register, L256, [EVEX_Vpmaxuq_ymm_k1z_ymm_ymmm256b64];
    VpmaxuqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::Maximum(Quadword, Unsigned)), Quadword, Memory, L512, [EVEX_Vpmaxuq_zmm_k1z_zmm_zmmm512b64];
    VpmaxuqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::Maximum(Quadword, Unsigned)), Quadword, Register, L512, [EVEX_Vpmaxuq_zmm_k1z_zmm_zmmm512b64];
    VpminsdXmmi32MaskmskwXmmi32Memi32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Signed)), Doubleword, Memory, L128, [EVEX_Vpminsd_xmm_k1z_xmm_xmmm128b32];
    VpminsdXmmi32MaskmskwXmmi32Xmmi32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Signed)), Doubleword, Register, L128, [EVEX_Vpminsd_xmm_k1z_xmm_xmmm128b32];
    VpminsdYmmi32MaskmskwYmmi32Memi32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Signed)), Doubleword, Memory, L256, [EVEX_Vpminsd_ymm_k1z_ymm_ymmm256b32];
    VpminsdYmmi32MaskmskwYmmi32Ymmi32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Signed)), Doubleword, Register, L256, [EVEX_Vpminsd_ymm_k1z_ymm_ymmm256b32];
    VpminsdZmmi32MaskmskwZmmi32Memi32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Signed)), Doubleword, Memory, L512, [EVEX_Vpminsd_zmm_k1z_zmm_zmmm512b32];
    VpminsdZmmi32MaskmskwZmmi32Zmmi32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Signed)), Doubleword, Register, L512, [EVEX_Vpminsd_zmm_k1z_zmm_zmmm512b32];
    VpminsqXmmi64MaskmskwXmmi64Memi64Avx512, Lanes(LaneOperation::Minimum(Quadword, Signed)), Quadword, Memory, L128, [EVEX_Vpminsq_xmm_k1z_xmm_xmmm128b64];
    VpminsqXmmi64MaskmskwXmmi64Xmmi64Avx512, Lanes(LaneOperation::Minimum(Quadword, Signed)), Quadword, Register, L128, [EVEX_Vpminsq_xmm_k1z_xmm_xmmm128b64];
    VpminsqYmmi64MaskmskwYmmi64Memi64Avx512, Lanes(LaneOperation::Minimum(Quadword, Signed)), Quadword, Memory, L256, [EVEX_Vpminsq_ymm_k1z_ymm_ymmm256b64];
    VpminsqYmmi64MaskmskwYmmi64Ymmi64Avx512, Lanes(LaneOperation::Minimum(Quadword, Signed)), Quadword, Register, L256, [EVEX_Vpminsq_ymm_k1z_ymm_ymmm256b64];
    VpminsqZmmi64MaskmskwZmmi64Memi64Avx512, Lanes(LaneOperation::Minimum(Quadword, Signed)), Quadword, Memory, L512, [EVEX_Vpminsq_zmm_k1z_zmm_zmmm512b64];
    VpminsqZmmi64MaskmskwZmmi64Zmmi64Avx512, Lanes(LaneOperation::Minimum(Quadword, Signed)), Quadword, Register, L512, [EVEX_Vpminsq_zmm_k1z_zmm_zmmm512b64];
    VpminudXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Unsigned)), Doubleword, Memory, L128, [EVEX_Vpminud_xmm_k1z_xmm_xmmm128b32];
    VpminudXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Unsigned)), Doubleword, Register, L128, [EVEX_Vpminud_xmm_k1z_xmm_xmmm128b32];
    VpminudYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Unsigned)), Doubleword, Memory, L256, [EVEX_Vpminud_ymm_k1z_ymm_ymmm256b32];
    VpminudYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Unsigned)), Doubleword, Register, L256, [EVEX_Vpminud_ymm_k1z_ymm_ymmm256b32];
    VpminudZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Unsigned)), Doubleword, Memory, L512, [EVEX_Vpminud_zmm_k1z_zmm_zmmm512b32];
    VpminudZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Minimum(Doubleword, Unsigned)), Doubleword, Register, L512, [EVEX_Vpminud_zmm_k1z_zmm_zmmm512b32];
    VpminuqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::Minimum(Quadword, Unsigned)), Quadword, Memory, L128, [EVEX_Vpminuq_xmm_k1z_xmm_xmmm128b64];
    VpminuqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::Minimum(Quadword, Unsigned)), Quadword, Register, L128, [EVEX_Vpminuq_xmm_k1z_xmm_xmmm128b64];
    VpminuqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::Minimum(Quadword, Unsigned)), Quadword, Memory, L256, [EVEX_Vpminuq_ymm_k1z_ymm_ymmm256b64];
    VpminuqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::Minimum(Quadword, Unsigned)), Quadword, Register, L256, [EVEX_Vpminuq_ymm_k1z_ymm_ymmm256b64];
    VpminuqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::Minimum(Quadword, Unsigned)), Quadword, Memory, L512, [EVEX_Vpminuq_zmm_k1z_zmm_zmmm512b64];
    VpminuqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::Minimum(Quadword, Unsigned)), Quadword, Register, L512, [EVEX_Vpminuq_zmm_k1z_zmm_zmmm512b64];
    VpmuldqXmmi64MaskmskwXmmi64Memi64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Signed)), Quadword, Memory, L128, [EVEX_Vpmuldq_xmm_k1z_xmm_xmmm128b64];
    VpmuldqXmmi64MaskmskwXmmi64Xmmi64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Signed)), Quadword, Register, L128, [EVEX_Vpmuldq_xmm_k1z_xmm_xmmm128b64];
    VpmuldqYmmi64MaskmskwYmmi64Memi64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Signed)), Quadword, Memory, L256, [EVEX_Vpmuldq_ymm_k1z_ymm_ymmm256b64];
    VpmuldqYmmi64MaskmskwYmmi64Ymmi64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Signed)), Quadword, Register, L256, [EVEX_Vpmuldq_ymm_k1z_ymm_ymmm256b64];
    VpmuldqZmmi64MaskmskwZmmi64Memi64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Signed)), Quadword, Memory, L512, [EVEX_Vpmuldq_zmm_k1z_zmm_zmmm512b64];
    VpmuldqZmmi64MaskmskwZmmi64Zmmi64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Signed)), Quadword, Register, L512, [EVEX_Vpmuldq_zmm_k1z_zmm_zmmm512b64];
    VpmulldXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::MultiplyLow(Doubleword)), Doubleword, Memory, L128, [EVEX_Vpmulld_xmm_k1z_xmm_xmmm128b32];
    VpmulldXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::MultiplyLow(Doubleword)), Doubleword, Register, L128, [EVEX_Vpmulld_xmm_k1z_xmm_xmmm128b32];
    VpmulldYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::MultiplyLow(Doubleword)), Doubleword, Memory, L256, [EVEX_Vpmulld_ymm_k1z_ymm_ymmm256b32];
    VpmulldYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::MultiplyLow(Doubleword)), Doubleword, Register, L256, [EVEX_Vpmulld_ymm_k1z_ymm_ymmm256b32];
    VpmulldZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::MultiplyLow(Doubleword)), Doubleword, Memory, L512, [EVEX_Vpmulld_zmm_k1z_zmm_zmmm512b32];
    VpmulldZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::MultiplyLow(Doubleword)), Doubleword, Register, L512, [EVEX_Vpmulld_zmm_k1z_zmm_zmmm512b32];
    VpmuludqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Unsigned)), Quadword, Memory, L128, [EVEX_Vpmuludq_xmm_k1z_xmm_xmmm128b64];
    VpmuludqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Unsigned)), Quadword, Register, L128, [EVEX_Vpmuludq_xmm_k1z_xmm_xmmm128b64];
    VpmuludqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Unsigned)), Quadword, Memory, L256, [EVEX_Vpmuludq_ymm_k1z_ymm_ymmm256b64];
    VpmuludqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Unsigned)), Quadword, Register, L256, [EVEX_Vpmuludq_ymm_k1z_ymm_ymmm256b64];
    VpmuludqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Unsigned)), Quadword, Memory, L512, [EVEX_Vpmuludq_zmm_k1z_zmm_zmmm512b64];
    VpmuludqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::MultiplyDoublewords(Unsigned)), Quadword, Register, L512, [EVEX_Vpmuludq_zmm_k1z_zmm_zmmm512b64];
    VpordXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Or), Doubleword, Memory, L128, [EVEX_Vpord_xmm_k1z_xmm_xmmm128b32];
    VpordXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Or), Doubleword, Register, L128, [EVEX_Vpord_xmm_k1z_xmm_xmmm128b32];
    VpordYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Or), Doubleword, Memory, L256, [EVEX_Vpord_ymm_k1z_ymm_ymmm256b32];
    VpordYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Or), Doubleword, Register, L256, [EVEX_Vpord_ymm_k1z_ymm_ymmm256b32];
    VpordZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Or), Doubleword, Memory, L512, [EVEX_Vpord_zmm_k1z_zmm_zmmm512b32];
    VpordZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Or), Doubleword, Register, L512, [EVEX_Vpord_zmm_k1z_zmm_zmmm512b32];
    VporqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::Or), Quadword, Memory, L128, [EVEX_Vporq_xmm_k1z_xmm_xmmm128b64];
    VporqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::Or), Quadword, Register, L128, [EVEX_Vporq_xmm_k1z_xmm_xmmm128b64];
    VporqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::Or), Quadword, Memory, L256, [EVEX_Vporq_ymm_k1z_ymm_ymmm256b64];
    VporqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::Or), Quadword, Register, L256, [EVEX_Vporq_ymm_k1z_ymm_ymmm256b64];
    VporqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::Or), Quadword, Memory, L512, [EVEX_Vporq_zmm_k1z_zmm_zmmm512b64];
    VporqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::Or), Quadword, Register, L512, [EVEX_Vporq_zmm_k1z_zmm_zmmm512b64];
    VproldXmmu32MaskmskwMemu32Imm8Avx512, Rotate { left: true, by_lane: false }, Doubleword, Memory, L128, [EVEX_Vprold_xmm_k1z_xmmm128b32_imm8];
    VproldXmmu32MaskmskwXmmu32Imm8Avx512, Rotate { left: true, by_lane: false }, Doubleword, Register, L128, [EVEX_Vprold_xmm_k1z_xmmm128b32_imm8];
    VproldYmmu32MaskmskwMemu32Imm8Avx512, Rotate { left: true, by_lane: false }, Doubleword, Memory, L256, [EVEX_Vprold_ymm_k1z_ymmm256b32_imm8];
    VproldYmmu32MaskmskwYmmu32Imm8Avx512, Rotate { left: true, by_lane: false }, Doubleword, Register, L256, [EVEX_Vprold_ymm_k1z_ymmm256b32_imm8];
    VproldZmmu32MaskmskwMemu32Imm8Avx512, Rotate { left: true, by_lane: false }, Doubleword, Memory, L512, [EVEX_Vprold_zmm_k1z_zmmm512b32_imm8];
    VproldZmmu32MaskmskwZmmu32Imm8Avx512, Rotate { left: true, by_lane: false }, Doubleword, Register, L512, [EVEX_Vprold_zmm_k1z_zmmm512b32_imm8];
    VprolqXmmu64MaskmskwMemu64Imm8Avx512, Rotate { left: true, by_lane: false }, Quadword, Memory, L128, [EVEX_Vprolq_xmm_k1z_xmmm128b64_imm8];
    VprolqXmmu64MaskmskwXmmu64Imm8Avx512, Rotate { left: true, by_lane: false }, Quadword, Register, L128, [EVEX_Vprolq_xmm_k1z_xmmm128b64_imm8];
    VprolqYmmu64MaskmskwMemu64Imm8Avx512, Rotate { left: true, by_lane: false }, Quadword, Memory, L256, [EVEX_Vprolq_ymm_k1z_ymmm256b64_imm8];
    VprolqYmmu64MaskmskwYmmu64Imm8Avx512, Rotate { left: true, by_lane: false }, Quadword, Register, L256, [EVEX_Vprolq_ymm_k1z_ymmm256b64_imm8];
    VprolqZmmu64MaskmskwMemu64Imm8Avx512, Rotate { left: true, by_lane: false }, Quadword, Memory, L512, [EVEX_Vprolq_zmm_k1z_zmmm512b64_imm8];
    VprolqZmmu64MaskmskwZmmu64Imm8Avx512, Rotate { left: true, by_lane: false }, Quadword, Register, L512, [EVEX_Vprolq_zmm_k1z_zmmm512b64_imm8];
    VprolvdXmmu32MaskmskwXmmu32Memu32Avx512, Rotate { left: true, by_lane: true }, Doubleword, Memory, L128, [EVEX_Vprolvd_xmm_k1z_xmm_xmmm128b32];
    VprolvdXmmu32MaskmskwXmmu32Xmmu32Avx512, Rotate { left: true, by_lane: true }, Doubleword, Register, L128, [EVEX_Vprolvd_xmm_k1z_xmm_xmmm128b32];
    VprolvdYmmu32MaskmskwYmmu32Memu32Avx512, Rotate { left: true, by_lane: true }, Doubleword, Memory, L256, [EVEX_Vprolvd_ymm_k1z_ymm_ymmm256b32];
    VprolvdYmmu32MaskmskwYmmu32Ymmu32Avx512, Rotate { left: true, by_lane: true }, Doubleword, Register, L256, [EVEX_Vprolvd_ymm_k1z_ymm_ymmm256b32];
    VprolvdZmmu32MaskmskwZmmu32Memu32Avx512, Rotate { left: true, by_lane: true }, Doubleword, Memory, L512, [EVEX_Vprolvd_zmm_k1z_zmm_zmmm512b32];
    VprolvdZmmu32MaskmskwZmmu32Zmmu32Avx512, Rotate { left: true, by_lane: true }, Doubleword, Register, L512, [EVEX_Vprolvd_zmm_k1z_zmm_zmmm512b32];
    VprolvqXmmu64MaskmskwXmmu64Memu64Avx512, Rotate { left: true, by_lane: true }, Quadword, Memory, L128, [EVEX_Vprolvq_xmm_k1z_xmm_xmmm128b64];
    VprolvqXmmu64MaskmskwXmmu64Xmmu64Avx512, Rotate { left: true, by_lane: true }, Quadword, Register, L128, [EVEX_Vprolvq_xmm_k1z_xmm_xmmm128b64];
    VprolvqYmmu64MaskmskwYmmu64Memu64Avx512, Rotate { left: true, by_lane: true }, Quadword, Memory, L256, [EVEX_Vprolvq_ymm_k1z_ymm_ymmm256b64];
    VprolvqYmmu64MaskmskwYmmu64Ymmu64Avx512, Rotate { left: true, by_lane: true }, Quadword, Register, L256, [EVEX_Vprolvq_ymm_k1z_ymm_ymmm256b64];
    VprolvqZmmu64MaskmskwZmmu64Memu64Avx512, Rotate { left: true, by_lane: true }, Quadword, Memory, L512, [EVEX_Vprolvq_zmm_k1z_zmm_zmmm512b64];
    VprolvqZmmu64MaskmskwZmmu64Zmmu64Avx512, Rotate { left: true, by_lane: true }, Quadword, Register, L512, [EVEX_Vprolvq_zmm_k1z_zmm_zmmm512b64];
    VprordXmmu32MaskmskwMemu32Imm8Avx512, Rotate { left: false, by_lane: false }, Doubleword, Memory, L128, [EVEX_Vprord_xmm_k1z_xmmm128b32_imm8];
    VprordXmmu32MaskmskwXmmu32Imm8Avx512, Rotate { left: false, by_lane: false }, Doubleword, Register, L128, [EVEX_Vprord_xmm_k1z_xmmm128b32_imm8];
    VprordYmmu32MaskmskwMemu32Imm8Avx512, Rotate { left: false, by_lane: false }, Doubleword, Memory, L256, [EVEX_Vprord_ymm_k1z_ymmm256b32_imm8];
    VprordYmmu32MaskmskwYmmu32Imm8Avx512, Rotate { left: false, by_lane: false }, Doubleword, Register, L256, [EVEX_Vprord_ymm_k1z_ymmm256b32_imm8];
    VprordZmmu32MaskmskwMemu32Imm8Avx512, Rotate { left: false, by_lane: false }, Doubleword, Memory, L512, [EVEX_Vprord_zmm_k1z_zmmm512b32_imm8];
    VprordZmmu32MaskmskwZmmu32Imm8Avx512, Rotate { left: false, by_lane: false }, Doubleword, Register, L512, [EVEX_Vprord_zmm_k1z_zmmm512b32_imm8];
    VprorqXmmu64MaskmskwMemu64Imm8Avx512, Rotate { left: false, by_lane: false }, Quadword, Memory, L128, [EVEX_Vprorq_xmm_k1z_xmmm128b64_imm8];
    VprorqXmmu64MaskmskwXmmu64Imm8Avx512, Rotate { left: false, by_lane: false }, Quadword, Register, L128, [EVEX_Vprorq_xmm_k1z_xmmm128b64_imm8];
    VprorqYmmu64MaskmskwMemu64Imm8Avx512, Rotate { left: false, by_lane: false }, Quadword, Memory, L256, [EVEX_Vprorq_ymm_k1z_ymmm256b64_imm8];
    VprorqYmmu64MaskmskwYmmu64Imm8Avx512, Rotate { left: false, by_lane: false }, Quadword, Register, L256, [EVEX_Vprorq_ymm_k1z_ymmm256b64_imm8];
    VprorqZmmu64MaskmskwMemu64Imm8Avx512, Rotate { left: false, by_lane: false }, Quadword, Memory, L512, [EVEX_Vprorq_zmm_k1z_zmmm512b64_imm8];
    VprorqZmmu64MaskmskwZmmu64Imm8Avx512, Rotate { left: false, by_lane: false }, Quadword, Register, L512, [EVEX_Vprorq_zmm_k1z_zmmm512b64_imm8];
    VprorvdXmmu32MaskmskwXmmu32Memu32Avx512, Rotate { left: false, by_lane: true }, Doubleword, Memory, L128, [EVEX_Vprorvd_xmm_k1z_xmm_xmmm128b32];
    VprorvdXmmu32MaskmskwXmmu32Xmmu32Avx512, Rotate { left: false, by_lane: true }, Doubleword, Register, L128, [EVEX_Vprorvd_xmm_k1z_xmm_xmmm128b32];
    VprorvdYmmu32MaskmskwYmmu32Memu32Avx512, Rotate { left: false, by_lane: true }, Doubleword, Memory, L256, [EVEX_Vprorvd_ymm_k1z_ymm_ymmm256b32];
    VprorvdYmmu32MaskmskwYmmu32Ymmu32Avx512, Rotate { left: false, by_lane: true }, Doubleword, Register, L256, [EVEX_Vprorvd_ymm_k1z_ymm_ymmm256b32];
    VprorvdZmmu32MaskmskwZmmu32Memu32Avx512, Rotate { left: false, by_lane: true }, Doubleword, Memory, L512, [EVEX_Vprorvd_zmm_k1z_zmm_zmmm512b32];
    VprorvdZmmu32MaskmskwZmmu32Zmmu32Avx512, Rotate { left: false, by_lane: true }, Doubleword, Register, L512, [EVEX_Vprorvd_zmm_k1z_zmm_zmmm512b32];
    VprorvqXmmu64MaskmskwXmmu64Memu64Avx512, Rotate { left: false, by_lane: true }, Quadword, Memory, L128, [EVEX_Vprorvq_xmm_k1z_xmm_xmmm128b64];
    VprorvqXmmu64MaskmskwXmmu64Xmmu64Avx512, Rotate { left: false, by_lane: true }, Quadword, Register, L128, [EVEX_Vprorvq_xmm_k1z_xmm_xmmm128b64];
    VprorvqYmmu64MaskmskwYmmu64Memu64Avx512, Rotate { left: false, by_lane: true }, Quadword, Memory, L256, [EVEX_Vprorvq_ymm_k1z_ymm_ymmm256b64];
    VprorvqYmmu64MaskmskwYmmu64Ymmu64Avx512, Rotate { left: false, by_lane: true }, Quadword, Register, L256, [EVEX_Vprorvq_ymm_k1z_ymm_ymmm256b64];
    VprorvqZmmu64MaskmskwZmmu64Memu64Avx512, Rotate { left: false, by_lane: true }, Quadword, Memory, L512, [EVEX_Vprorvq_zmm_k1z_zmm_zmmm512b64];
    VprorvqZmmu64MaskmskwZmmu64Zmmu64Avx512, Rotate { left: false, by_lane: true }, Quadword, Register, L512, [EVEX_Vprorvq_zmm_k1z_zmm_zmmm512b64];
    VpslldXmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::Left), Doubleword, Memory, L128, [EVEX_Vpslld_xmm_k1z_xmmm128b32_imm8];
    VpslldXmmu32MaskmskwXmmu32Imm8Avx512, ShiftByImmediate(LaneShift::Left), Doubleword, Register, L128, [EVEX_Vpslld_xmm_k1z_xmmm128b32_imm8];
    VpslldXmmu32MaskmskwXmmu32Memu32Avx512, ShiftByCount(LaneShift::Left), Doubleword, Memory, L128, [EVEX_Vpslld_xmm_k1z_xmm_xmmm128];
    VpslldXmmu32MaskmskwXmmu32Xmmu32Avx512, ShiftByCount(LaneShift::Left), Doubleword, Register, L128, [EVEX_Vpslld_xmm_k1z_xmm_xmmm128];
    VpslldYmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::Left), Doubleword, Memory, L256, [EVEX_Vpslld_ymm_k1z_ymmm256b32_imm8];
    VpslldYmmu32MaskmskwYmmu32Imm8Avx512, ShiftByImmediate(LaneShift::Left), Doubleword, Register, L256, [EVEX_Vpslld_ymm_k1z_ymmm256b32_imm8];
    VpslldYmmu32MaskmskwYmmu32Memu32Avx512, ShiftByCount(LaneShift::Left), Doubleword, Memory, L256, [EVEX_Vpslld_ymm_k1z_ymm_xmmm128];
    VpslldYmmu32MaskmskwYmmu32Xmmu32Avx512, ShiftByCount(LaneShift::Left), Doubleword, Register, L256, [EVEX_Vpslld_ymm_k1z_ymm_xmmm128];
    VpslldZmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::Left), Doubleword, Memory, L512, [EVEX_Vpslld_zmm_k1z_zmmm512b32_imm8];
    VpslldZmmu32MaskmskwZmmu32Imm8Avx512, ShiftByImmediate(LaneShift::Left), Doubleword, Register, L512, [EVEX_Vpslld_zmm_k1z_zmmm512b32_imm8];
    VpslldZmmu32MaskmskwZmmu32Memu32Avx512, ShiftByCount(LaneShift::Left), Doubleword, Memory, L512, [EVEX_Vpslld_zmm_k1z_zmm_xmmm128];
    VpslldZmmu32MaskmskwZmmu32Xmmu32Avx512, ShiftByCount(LaneShift::Left), Doubleword, Register, L512, [EVEX_Vpslld_zmm_k1z_zmm_xmmm128];
    VpsllqXmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::Left), Quadword, Memory, L128, [EVEX_Vpsllq_xmm_k1z_xmmm128b64_imm8];
    VpsllqXmmu64MaskmskwXmmu64Imm8Avx512, ShiftByImmediate(LaneShift::Left), Quadword, Register, L128, [EVEX_Vpsllq_xmm_k1z_xmmm128b64_imm8];
    VpsllqXmmu64MaskmskwXmmu64Memu64Avx512, ShiftByCount(LaneShift::Left), Quadword, Memory, L128, [EVEX_Vpsllq_xmm_k1z_xmm_xmmm128];
    VpsllqXmmu64MaskmskwXmmu64Xmmu64Avx512, ShiftByCount(LaneShift::Left), Quadword, Register, L128, [EVEX_Vpsllq_xmm_k1z_xmm_xmmm128];
    VpsllqYmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::Left), Quadword, Memory, L256, [EVEX_Vpsllq_ymm_k1z_ymmm256b64_imm8];
    VpsllqYmmu64MaskmskwYmmu64Imm8Avx512, ShiftByImmediate(LaneShift::Left), Quadword, Register, L256, [EVEX_Vpsllq_ymm_k1z_ymmm256b64_imm8];
    VpsllqYmmu64MaskmskwYmmu64Memu64Avx512, ShiftByCount(LaneShift::Left), Quadword, Memory, L256, [EVEX_Vpsllq_ymm_k1z_ymm_xmmm128];
    VpsllqYmmu64MaskmskwYmmu64Xmmu64Avx512, ShiftByCount(LaneShift::Left), Quadword, Register, L256, [EVEX_Vpsllq_ymm_k1z_ymm_xmmm128];
    VpsllqZmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::Left), Quadword, Memory, L512, [EVEX_Vpsllq_zmm_k1z_zmmm512b64_imm8];
    VpsllqZmmu64MaskmskwZmmu64Imm8Avx512, ShiftByImmediate(LaneShift::Left), Quadword, Register, L512, [EVEX_Vpsllq_zmm_k1z_zmmm512b64_imm8];
    VpsllqZmmu64MaskmskwZmmu64Memu64Avx512, ShiftByCount(LaneShift::Left), Quadword, Memory, L512, [EVEX_Vpsllq_zmm_k1z_zmm_xmmm128];
    VpsllqZmmu64MaskmskwZmmu64Xmmu64Avx512, ShiftByCount(LaneShift::Left), Quadword, Register, L512, [EVEX_Vpsllq_zmm_k1z_zmm_xmmm128];
    VpsllvdXmmu32MaskmskwXmmu32Memu32Avx512, ShiftByLane(LaneShift::Left), Doubleword, Memory, L128, [EVEX_Vpsllvd_xmm_k1z_xmm_xmmm128b32];
    VpsllvdXmmu32MaskmskwXmmu32Xmmu32Avx512, ShiftByLane(LaneShift::Left), Doubleword, Register, L128, [EVEX_Vpsllvd_xmm_k1z_xmm_xmmm128b32];
    VpsllvdYmmu32MaskmskwYmmu32Memu32Avx512, ShiftByLane(LaneShift::Left), Doubleword, Memory, L256, [EVEX_Vpsllvd_ymm_k1z_ymm_ymmm256b32];
    VpsllvdYmmu32MaskmskwYmmu32Ymmu32Avx512, ShiftByLane(LaneShift::Left), Doubleword, Register, L256, [EVEX_Vpsllvd_ymm_k1z_ymm_ymmm256b32];
    VpsllvdZmmu32MaskmskwZmmu32Memu32Avx512, ShiftByLane(LaneShift::Left), Doubleword, Memory, L512, [EVEX_Vpsllvd_zmm_k1z_zmm_zmmm512b32];
    VpsllvdZmmu32MaskmskwZmmu32Zmmu32Avx512, ShiftByLane(LaneShift::Left), Doubleword, Register, L512, [EVEX_Vpsllvd_zmm_k1z_zmm_zmmm512b32];
    VpsllvqXmmu64MaskmskwXmmu64Memu64Avx512, ShiftByLane(LaneShift::Left), Quadword, Memory, L128, [EVEX_Vpsllvq_xmm_k1z_xmm_xmmm128b64];
    VpsllvqXmmu64MaskmskwXmmu64Xmmu64Avx512, ShiftByLane(LaneShift::Left), Quadword, Register, L128, [EVEX_Vpsllvq_xmm_k1z_xmm_xmmm128b64];
    VpsllvqYmmu64MaskmskwYmmu64Memu64Avx512, ShiftByLane(LaneShift::Left), Quadword, Memory, L256, [EVEX_Vpsllvq_ymm_k1z_ymm_ymmm256b64];
    VpsllvqYmmu64MaskmskwYmmu64Ymmu64Avx512, ShiftByLane(LaneShift::Left), Quadword, Register, L256, [EVEX_Vpsllvq_ymm_k1z_ymm_ymmm256b64];
    VpsllvqZmmu64MaskmskwZmmu64Memu64Avx512, ShiftByLane(LaneShift::Left), Quadword, Memory, L512, [EVEX_Vpsllvq_zmm_k1z_zmm_zmmm512b64];
    VpsllvqZmmu64MaskmskwZmmu64Zmmu64Avx512, ShiftByLane(LaneShift::Left), Quadword, Register, L512, [EVEX_Vpsllvq_zmm_k1z_zmm_zmmm512b64];
    VpsradXmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Doubleword, Memory, L128, [EVEX_Vpsrad_xmm_k1z_xmmm128b32_imm8];
    VpsradXmmu32MaskmskwXmmu32Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Doubleword, Register, L128, [EVEX_Vpsrad_xmm_k1z_xmmm128b32_imm8];
    VpsradXmmu32MaskmskwXmmu32Memu32Avx512, ShiftByCount(LaneShift::RightArithmetic), Doubleword, Memory, L128, [EVEX_Vpsrad_xmm_k1z_xmm_xmmm128];
    VpsradXmmu32MaskmskwXmmu32Xmmu32Avx512, ShiftByCount(LaneShift::RightArithmetic), Doubleword, Register, L128, [EVEX_Vpsrad_xmm_k1z_xmm_xmmm128];
    VpsradYmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Doubleword, Memory, L256, [EVEX_Vpsrad_ymm_k1z_ymmm256b32_imm8];
    VpsradYmmu32MaskmskwYmmu32Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Doubleword, Register, L256, [EVEX_Vpsrad_ymm_k1z_ymmm256b32_imm8];
    VpsradYmmu32MaskmskwYmmu32Memu32Avx512, ShiftByCount(LaneShift::RightArithmetic), Doubleword, Memory, L256, [EVEX_Vpsrad_ymm_k1z_ymm_xmmm128];
    VpsradYmmu32MaskmskwYmmu32Xmmu32Avx512, ShiftByCount(LaneShift::RightArithmetic), Doubleword, Register, L256, [EVEX_Vpsrad_ymm_k1z_ymm_xmmm128];
    VpsradZmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Doubleword, Memory, L512, [EVEX_Vpsrad_zmm_k1z_zmmm512b32_imm8];
    VpsradZmmu32MaskmskwZmmu32Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Doubleword, Register, L512, [EVEX_Vpsrad_zmm_k1z_zmmm512b32_imm8];
    VpsradZmmu32MaskmskwZmmu32Memu32Avx512, ShiftByCount(LaneShift::RightArithmetic), Doubleword, Memory, L512, [EVEX_Vpsrad_zmm_k1z_zmm_xmmm128];
    VpsradZmmu32MaskmskwZmmu32Xmmu32Avx512, ShiftByCount(LaneShift::RightArithmetic), Doubleword, Register, L512, [EVEX_Vpsrad_zmm_k1z_zmm_xmmm128];
    VpsraqXmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Quadword, Memory, L128, [EVEX_Vpsraq_xmm_k1z_xmmm128b64_imm8];
    VpsraqXmmu64MaskmskwXmmu64Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Quadword, Register, L128, [EVEX_Vpsraq_xmm_k1z_xmmm128b64_imm8];
    VpsraqXmmu64MaskmskwXmmu64Memu64Avx512, ShiftByCount(LaneShift::RightArithmetic), Quadword, Memory, L128, [EVEX_Vpsraq_xmm_k1z_xmm_xmmm128];
    VpsraqXmmu64MaskmskwXmmu64Xmmu64Avx512, ShiftByCount(LaneShift::RightArithmetic), Quadword, Register, L128, [EVEX_Vpsraq_xmm_k1z_xmm_xmmm128];
    VpsraqYmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Quadword, Memory, L256, [EVEX_Vpsraq_ymm_k1z_ymmm256b64_imm8];
    VpsraqYmmu64MaskmskwYmmu64Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Quadword, Register, L256, [EVEX_Vpsraq_ymm_k1z_ymmm256b64_imm8];
    VpsraqYmmu64MaskmskwYmmu64Memu64Avx512, ShiftByCount(LaneShift::RightArithmetic), Quadword, Memory, L256, [EVEX_Vpsraq_ymm_k1z_ymm_xmmm128];
    VpsraqYmmu64MaskmskwYmmu64Xmmu64Avx512, ShiftByCount(LaneShift::RightArithmetic), Quadword, Register, L256, [EVEX_Vpsraq_ymm_k1z_ymm_xmmm128];
    VpsraqZmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Quadword, Memory, L512, [EVEX_Vpsraq_zmm_k1z_zmmm512b64_imm8];
    VpsraqZmmu64MaskmskwZmmu64Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Quadword, Register, L512, [EVEX_Vpsraq_zmm_k1z_zmmm512b64_imm8];
    VpsraqZmmu64MaskmskwZmmu64Memu64Avx512, ShiftByCount(LaneShift::RightArithmetic), Quadword, Memory, L512, [EVEX_Vpsraq_zmm_k1z_zmm_xmmm128];
    VpsraqZmmu64MaskmskwZmmu64Xmmu64Avx512, ShiftByCount(LaneShift::RightArithmetic), Quadword, Register, L512, [EVEX_Vpsraq_zmm_k1z_zmm_xmmm128];
    VpsravdXmmu32MaskmskwXmmu32Memu32Avx512, ShiftByLane(LaneShift::RightArithmetic), Doubleword, Memory, L128, [EVEX_Vpsravd_xmm_k1z_xmm_xmmm128b32];
    VpsravdXmmu32MaskmskwXmmu32Xmmu32Avx512, ShiftByLane(LaneShift::RightArithmetic), Doubleword, Register, L128, [EVEX_Vpsravd_xmm_k1z_xmm_xmmm128b32];
    VpsravdYmmu32MaskmskwYmmu32Memu32Avx512, ShiftByLane(LaneShift::RightArithmetic), Doubleword, Memory, L256, [EVEX_Vpsravd_ymm_k1z_ymm_ymmm256b32];
    VpsravdYmmu32MaskmskwYmmu32Ymmu32Avx512, ShiftByLane(LaneShift::RightArithmetic), Doubleword, Register, L256, [EVEX_Vpsravd_ymm_k1z_ymm_ymmm256b32];
    VpsravdZmmu32MaskmskwZmmu32Memu32Avx512, ShiftByLane(LaneShift::RightArithmetic), Doubleword, Memory, L512, [EVEX_Vpsravd_zmm_k1z_zmm_zmmm512b32];
    VpsravdZmmu32MaskmskwZmmu32Zmmu32Avx512, ShiftByLane(LaneShift::RightArithmetic), Doubleword, Register, L512, [EVEX_Vpsravd_zmm_k1z_zmm_zmmm512b32];
    VpsravqXmmu64MaskmskwXmmu64Memu64Avx512, ShiftByLane(LaneShift::RightArithmetic), Quadword, Memory, L128, [EVEX_Vpsravq_xmm_k1z_xmm_xmmm128b64];
    VpsravqXmmu64MaskmskwXmmu64Xmmu64Avx512, ShiftByLane(LaneShift::RightArithmetic), Quadword, Register, L128, [EVEX_Vpsravq_xmm_k1z_xmm_xmmm128b64];
    VpsravqYmmu64MaskmskwYmmu64Memu64Avx512, ShiftByLane(LaneShift::RightArithmetic), Quadword, Memory, L256, [EVEX_Vpsravq_ymm_k1z_ymm_ymmm256b64];
    VpsravqYmmu64MaskmskwYmmu64Ymmu64Avx512, ShiftByLane(LaneShift::RightArithmetic), Quadword, Register, L256, [EVEX_Vpsravq_ymm_k1z_ymm_ymmm256b64];
    VpsravqZmmu64MaskmskwZmmu64Memu64Avx512, ShiftByLane(LaneShift::RightArithmetic), Quadword, Memory, L512, [EVEX_Vpsravq_zmm_k1z_zmm_zmmm512b64];
    VpsravqZmmu64MaskmskwZmmu64Zmmu64Avx512, ShiftByLane(LaneShift::RightArithmetic), Quadword, Register, L512, [EVEX_Vpsravq_zmm_k1z_zmm_zmmm512b64];
    VpsrldXmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Doubleword, Memory, L128, [EVEX_Vpsrld_xmm_k1z_xmmm128b32_imm8];
    VpsrldXmmu32MaskmskwXmmu32Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Doubleword, Register, L128, [EVEX_Vpsrld_xmm_k1z_xmmm128b32_imm8];
    VpsrldXmmu32MaskmskwXmmu32Memu32Avx512, ShiftByCount(LaneShift::RightLogical), Doubleword, Memory, L128, [EVEX_Vpsrld_xmm_k1z_xmm_xmmm128];
    VpsrldXmmu32MaskmskwXmmu32Xmmu32Avx512, ShiftByCount(LaneShift::RightLogical), Doubleword, Register, L128, [EVEX_Vpsrld_xmm_k1z_xmm_xmmm128];
    VpsrldYmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Doubleword, Memory, L256, [EVEX_Vpsrld_ymm_k1z_ymmm256b32_imm8];
    VpsrldYmmu32MaskmskwYmmu32Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Doubleword, Register, L256, [EVEX_Vpsrld_ymm_k1z_ymmm256b32_imm8];
    VpsrldYmmu32MaskmskwYmmu32Memu32Avx512, ShiftByCount(LaneShift::RightLogical), Doubleword, Memory, L256, [EVEX_Vpsrld_ymm_k1z_ymm_xmmm128];
    VpsrldYmmu32MaskmskwYmmu32Xmmu32Avx512, ShiftByCount(LaneShift::RightLogical), Doubleword, Register, L256, [EVEX_Vpsrld_ymm_k1z_ymm_xmmm128];
    VpsrldZmmu32MaskmskwMemu32Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Doubleword, Memory, L512, [EVEX_Vpsrld_zmm_k1z_zmmm512b32_imm8];
    VpsrldZmmu32MaskmskwZmmu32Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Doubleword, Register, L512, [EVEX_Vpsrld_zmm_k1z_zmmm512b32_imm8];
    VpsrldZmmu32MaskmskwZmmu32Memu32Avx512, ShiftByCount(LaneShift::RightLogical), Doubleword, Memory, L512, [EVEX_Vpsrld_zmm_k1z_zmm_xmmm128];
    VpsrldZmmu32MaskmskwZmmu32Xmmu32Avx512, ShiftByCount(LaneShift::RightLogical), Doubleword, Register, L512, [EVEX_Vpsrld_zmm_k1z_zmm_xmmm128];
    VpsrlqXmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Quadword, Memory, L128, [EVEX_Vpsrlq_xmm_k1z_xmmm128b64_imm8];
    VpsrlqXmmu64MaskmskwXmmu64Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Quadword, Register, L128, [EVEX_Vpsrlq_xmm_k1z_xmmm128b64_imm8];
    VpsrlqXmmu64MaskmskwXmmu64Memu64Avx512, ShiftByCount(LaneShift::RightLogical), Quadword, Memory, L128, [EVEX_Vpsrlq_xmm_k1z_xmm_xmmm128];
    VpsrlqXmmu64MaskmskwXmmu64Xmmu64Avx512, ShiftByCount(LaneShift::RightLogical), Quadword, Register, L128, [EVEX_Vpsrlq_xmm_k1z_xmm_xmmm128];
    VpsrlqYmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Quadword, Memory, L256, [EVEX_Vpsrlq_ymm_k1z_ymmm256b64_imm8];
    VpsrlqYmmu64MaskmskwYmmu64Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Quadword, Register, L256, [EVEX_Vpsrlq_ymm_k1z_ymmm256b64_imm8];
    VpsrlqYmmu64MaskmskwYmmu64Memu64Avx512, ShiftByCount(LaneShift::RightLogical), Quadword, Memory, L256, [EVEX_Vpsrlq_ymm_k1z_ymm_xmmm128];
    VpsrlqYmmu64MaskmskwYmmu64Xmmu64Avx512, ShiftByCount(LaneShift::RightLogical), Quadword, Register, L256, [EVEX_Vpsrlq_ymm_k1z_ymm_xmmm128];
    VpsrlqZmmu64MaskmskwMemu64Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Quadword, Memory, L512, [EVEX_Vpsrlq_zmm_k1z_zmmm512b64_imm8];
    VpsrlqZmmu64MaskmskwZmmu64Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Quadword, Register, L512, [EVEX_Vpsrlq_zmm_k1z_zmmm512b64_imm8];
    VpsrlqZmmu64MaskmskwZmmu64Memu64Avx512, ShiftByCount(LaneShift::RightLogical), Quadword, Memory, L512, [EVEX_Vpsrlq_zmm_k1z_zmm_xmmm128];
    VpsrlqZmmu64MaskmskwZmmu64Xmmu64Avx512, ShiftByCount(LaneShift::RightLogical), Quadword, Register, L512, [EVEX_Vpsrlq_zmm_k1z_zmm_xmmm128];
    VpsrlvdXmmu32MaskmskwXmmu32Memu32Avx512, ShiftByLane(LaneShift::RightLogical), Doubleword, Memory, L128, [EVEX_Vpsrlvd_xmm_k1z_xmm_xmmm128b32];
    VpsrlvdXmmu32MaskmskwXmmu32Xmmu32Avx512, ShiftByLane(LaneShift::RightLogical), Doubleword, Register, L128, [EVEX_Vpsrlvd_xmm_k1z_xmm_xmmm128b32];
    VpsrlvdYmmu32MaskmskwYmmu32Memu32Avx512, ShiftByLane(LaneShift::RightLogical), Doubleword, Memory, L256, [EVEX_Vpsrlvd_ymm_k1z_ymm_ymmm256b32];
    VpsrlvdYmmu32MaskmskwYmmu32Ymmu32Avx512, ShiftByLane(LaneShift::RightLogical), Doubleword, Register, L256, [EVEX_Vpsrlvd_ymm_k1z_ymm_ymmm256b32];
    VpsrlvdZmmu32MaskmskwZmmu32Memu32Avx512, ShiftByLane(LaneShift::RightLogical), Doubleword, Memory, L512, [EVEX_Vpsrlvd_zmm_k1z_zmm_zmmm512b32];
    VpsrlvdZmmu32MaskmskwZmmu32Zmmu32Avx512, ShiftByLane(LaneShift::RightLogical), Doubleword, Register, L512, [EVEX_Vpsrlvd_zmm_k1z_zmm_zmmm512b32];
    VpsrlvqXmmu64MaskmskwXmmu64Memu64Avx512, ShiftByLane(LaneShift::RightLogical), Quadword, Memory, L128, [EVEX_Vpsrlvq_xmm_k1z_xmm_xmmm128b64];
    VpsrlvqXmmu64MaskmskwXmmu64Xmmu64Avx512, ShiftByLane(LaneShift::RightLogical), Quadword, Register, L128, [EVEX_Vpsrlvq_xmm_k1z_xmm_xmmm128b64];
    VpsrlvqYmmu64MaskmskwYmmu64Memu64Avx512, ShiftByLane(LaneShift::RightLogical), Quadword, Memory, L256, [EVEX_Vpsrlvq_ymm_k1z_ymm_ymmm256b64];
    VpsrlvqYmmu64MaskmskwYmmu64Ymmu64Avx512, ShiftByLane(LaneShift::RightLogical), Quadword, Register, L256, [EVEX_Vpsrlvq_ymm_k1z_ymm_ymmm256b64];
    VpsrlvqZmmu64MaskmskwZmmu64Memu64Avx512, ShiftByLane(LaneShift::RightLogical), Quadword, Memory, L512, [EVEX_Vpsrlvq_zmm_k1z_zmm_zmmm512b64];
    VpsrlvqZmmu64MaskmskwZmmu64Zmmu64Avx512, ShiftByLane(LaneShift::RightLogical), Quadword, Register, L512, [EVEX_Vpsrlvq_zmm_k1z_zmm_zmmm512b64];
    VpsubdXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Subtract(Doubleword)), Doubleword, Memory, L128, [EVEX_Vpsubd_xmm_k1z_xmm_xmmm128b32];
    VpsubdXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Subtract(Doubleword)), Doubleword, Register, L128, [EVEX_Vpsubd_xmm_k1z_xmm_xmmm128b32];
    VpsubdYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Subtract(Doubleword)), Doubleword, Memory, L256, [EVEX_Vpsubd_ymm_k1z_ymm_ymmm256b32];
    VpsubdYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Subtract(Doubleword)), Doubleword, Register, L256, [EVEX_Vpsubd_ymm_k1z_ymm_ymmm256b32];
    VpsubdZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Subtract(Doubleword)), Doubleword, Memory, L512, [EVEX_Vpsubd_zmm_k1z_zmm_zmmm512b32];
    VpsubdZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Subtract(Doubleword)), Doubleword, Register, L512, [EVEX_Vpsubd_zmm_k1z_zmm_zmmm512b32];
    VpsubqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::Subtract(Quadword)), Quadword, Memory, L128, [EVEX_Vpsubq_xmm_k1z_xmm_xmmm128b64];
    VpsubqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::Subtract(Quadword)), Quadword, Register, L128, [EVEX_Vpsubq_xmm_k1z_xmm_xmmm128b64];
    VpsubqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::Subtract(Quadword)), Quadword, Memory, L256, [EVEX_Vpsubq_ymm_k1z_ymm_ymmm256b64];
    VpsubqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::Subtract(Quadword)), Quadword, Register, L256, [EVEX_Vpsubq_ymm_k1z_ymm_ymmm256b64];
    VpsubqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::Subtract(Quadword)), Quadword, Memory, L512, [EVEX_Vpsubq_zmm_k1z_zmm_zmmm512b64];
    VpsubqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::Subtract(Quadword)), Quadword, Register, L512, [EVEX_Vpsubq_zmm_k1z_zmm_zmmm512b64];
    VpternlogdXmmu32MaskmskwXmmu32Memu32Imm8Avx512, TernaryLogic, Doubleword, Memory, L128, [EVEX_Vpternlogd_xmm_k1z_xmm_xmmm128b32_imm8];
    VpternlogdXmmu32MaskmskwXmmu32Xmmu32Imm8Avx512, TernaryLogic, Doubleword, Register, L128, [EVEX_Vpternlogd_xmm_k1z_xmm_xmmm128b32_imm8];
    VpternlogdYmmu32MaskmskwYmmu32Memu32Imm8Avx512, TernaryLogic, Doubleword, Memory, L256, [EVEX_Vpternlogd_ymm_k1z_ymm_ymmm256b32_imm8];
    VpternlogdYmmu32MaskmskwYmmu32Ymmu32Imm8Avx512, TernaryLogic, Doubleword, Register, L256, [EVEX_Vpternlogd_ymm_k1z_ymm_ymmm256b32_imm8];
    VpternlogdZmmu32MaskmskwZmmu32Memu32Imm8Avx512, TernaryLogic, Doubleword, Memory, L512, [EVEX_Vpternlogd_zmm_k1z_zmm_zmmm512b32_imm8];
    VpternlogdZmmu32MaskmskwZmmu32Zmmu32Imm8Avx512, TernaryLogic, Doubleword, Register, L512, [EVEX_Vpternlogd_zmm_k1z_zmm_zmmm512b32_imm8];
    VpternlogqXmmu64MaskmskwXmmu64Memu64Imm8Avx512, TernaryLogic, Quadword, Memory, L128, [EVEX_Vpternlogq_xmm_k1z_xmm_xmmm128b64_imm8];
    VpternlogqXmmu64MaskmskwXmmu64Xmmu64Imm8Avx512, TernaryLogic, Quadword, Register, L128, [EVEX_Vpternlogq_xmm_k1z_xmm_xmmm128b64_imm8];
    VpternlogqYmmu64MaskmskwYmmu64Memu64Imm8Avx512, TernaryLogic, Quadword, Memory, L256, [EVEX_Vpternlogq_ymm_k1z_ymm_ymmm256b64_imm8];
    VpternlogqYmmu64MaskmskwYmmu64Ymmu64Imm8Avx512, TernaryLogic, Quadword, Register, L256, [EVEX_Vpternlogq_ymm_k1z_ymm_ymmm256b64_imm8];
    VpternlogqZmmu64MaskmskwZmmu64Memu64Imm8Avx512, TernaryLogic, Quadword, Memory, L512, [EVEX_Vpternlogq_zmm_k1z_zmm_zmmm512b64_imm8];
    VpternlogqZmmu64MaskmskwZmmu64Zmmu64Imm8Avx512, TernaryLogic, Quadword, Register, L512, [EVEX_Vpternlogq_zmm_k1z_zmm_zmmm512b64_imm8];
    VptestmdMaskmskwMaskmskwXmmu32Memu32Avx512, TestIntoMask { not: false }, Doubleword, Memory, L128, [EVEX_Vptestmd_kr_k1_xmm_xmmm128b32];
    VptestmdMaskmskwMaskmskwXmmu32Xmmu32Avx512, TestIntoMask { not: false }, Doubleword, Register, L128, [EVEX_Vptestmd_kr_k1_xmm_xmmm128b32];
    VptestmdMaskmskwMaskmskwYmmu32Memu32Avx512, TestIntoMask { not: false }, Doubleword, Memory, L256, [EVEX_Vptestmd_kr_k1_ymm_ymmm256b32];
    VptestmdMaskmskwMaskmskwYmmu32Ymmu32Avx512, TestIntoMask { not: false }, Doubleword, Register, L256, [EVEX_Vptestmd_kr_k1_ymm_ymmm256b32];
    VptestmdMaskmskwMaskmskwZmmu32Memu32Avx512, TestIntoMask { not: false }, Doubleword, Memory, L512, [EVEX_Vptestmd_kr_k1_zmm_zmmm512b32];
    VptestmdMaskmskwMaskmskwZmmu32Zmmu32Avx512, TestIntoMask { not: false }, Doubleword, Register, L512, [EVEX_Vptestmd_kr_k1_zmm_zmmm512b32];
    VptestmqMaskmskwMaskmskwXmmu64Memu64Avx512, TestIntoMask { not: false }, Quadword, Memory, L128, [EVEX_Vptestmq_kr_k1_xmm_xmmm128b64];
    VptestmqMaskmskwMaskmskwXmmu64Xmmu64Avx512, TestIntoMask { not: false }, Quadword, Register, L128, [EVEX_Vptestmq_kr_k1_xmm_xmmm128b64];
    VptestmqMaskmskwMaskmskwYmmu64Memu64Avx512, TestIntoMask { not: false }, Quadword, Memory, L256, [EVEX_Vptestmq_kr_k1_ymm_ymmm256b64];
    VptestmqMaskmskwMaskmskwYmmu64Ymmu64Avx512, TestIntoMask { not: false }, Quadword, Register, L256, [EVEX_Vptestmq_kr_k1_ymm_ymmm256b64];
    VptestmqMaskmskwMaskmskwZmmu64Memu64Avx512, TestIntoMask { not: false }, Quadword, Memory, L512, [EVEX_Vptestmq_kr_k1_zmm_zmmm512b64];
    VptestmqMaskmskwMaskmskwZmmu64Zmmu64Avx512, TestIntoMask { not: false }, Quadword, Register, L512, [EVEX_Vptestmq_kr_k1_zmm_zmmm512b64];
    VptestnmdMaskmskwMaskmskwXmmu32Memu32Avx512, TestIntoMask { not: true }, Doubleword, Memory, L128, [EVEX_Vptestnmd_kr_k1_xmm_xmmm128b32];
    VptestnmdMaskmskwMaskmskwXmmu32Xmmu32Avx512, TestIntoMask { not: true }, Doubleword, Register, L128, [EVEX_Vptestnmd_kr_k1_xmm_xmmm128b32];
    VptestnmdMaskmskwMaskmskwYmmu32Memu32Avx512, TestIntoMask { not: true }, Doubleword, Memory, L256, [EVEX_Vptestnmd_kr_k1_ymm_ymmm256b32];
    VptestnmdMaskmskwMaskmskwYmmu32Ymmu32Avx512, TestIntoMask { not: true }, Doubleword, Register, L256, [EVEX_Vptestnmd_kr_k1_ymm_ymmm256b32];
    VptestnmdMaskmskwMaskmskwZmmu32Memu32Avx512, TestIntoMask { not: true }, Doubleword, Memory, L512, [EVEX_Vptestnmd_kr_k1_zmm_zmmm512b32];
    VptestnmdMaskmskwMaskmskwZmmu32Zmmu32Avx512, TestIntoMask { not: true }, Doubleword, Register, L512, [EVEX_Vptestnmd_kr_k1_zmm_zmmm512b32];
    VptestnmqMaskmskwMaskmskwXmmu64Memu64Avx512, TestIntoMask { not: true }, Quadword, Memory, L128, [EVEX_Vptestnmq_kr_k1_xmm_xmmm128b64];
    VptestnmqMaskmskwMaskmskwXmmu64Xmmu64Avx512, TestIntoMask { not: true }, Quadword, Register, L128, [EVEX_Vptestnmq_kr_k1_xmm_xmmm128b64];
    VptestnmqMaskmskwMaskmskwYmmu64Memu64Avx512, TestIntoMask { not: true }, Quadword, Memory, L256, [EVEX_Vptestnmq_kr_k1_ymm_ymmm256b64];
    VptestnmqMaskmskwMaskmskwYmmu64Ymmu64Avx512, TestIntoMask { not: true }, Quadword, Register, L256, [EVEX_Vptestnmq_kr_k1_ymm_ymmm256b64];
    VptestnmqMaskmskwMaskmskwZmmu64Memu64Avx512, TestIntoMask { not: true }, Quadword, Memory, L512, [EVEX_Vptestnmq_kr_k1_zmm_zmmm512b64];
    VptestnmqMaskmskwMaskmskwZmmu64Zmmu64Avx512, TestIntoMask { not: true }, Quadword, Register, L512, [EVEX_Vptestnmq_kr_k1_zmm_zmmm512b64];
    VpxordXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Memory, L128, [EVEX_Vpxord_xmm_k1z_xmm_xmmm128b32];
    VpxordXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Register, L128, [EVEX_Vpxord_xmm_k1z_xmm_xmmm128b32];
    VpxordYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Memory, L256, [EVEX_Vpxord_ymm_k1z_ymm_ymmm256b32];
    VpxordYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Register, L256, [EVEX_Vpxord_ymm_k1z_ymm_ymmm256b32];
    VpxordZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Memory, L512, [EVEX_Vpxord_zmm_k1z_zmm_zmmm512b32];
    VpxordZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Register, L512, [EVEX_Vpxord_zmm_k1z_zmm_zmmm512b32];
    VpxorqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::Xor), Quadword, Memory, L128, [EVEX_Vpxorq_xmm_k1z_xmm_xmmm128b64];
    VpxorqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::Xor), Quadword, Register, L128, [EVEX_Vpxorq_xmm_k1z_xmm_xmmm128b64];
    VpxorqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::Xor), Quadword, Memory, L256, [EVEX_Vpxorq_ymm_k1z_ymm_ymmm256b64];
    VpxorqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::Xor), Quadword, Register, L256, [EVEX_Vpxorq_ymm_k1z_ymm_ymmm256b64];
    VpxorqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::Xor), Quadword, Memory, L512, [EVEX_Vpxorq_zmm_k1z_zmm_zmmm512b64];
    VpxorqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::Xor), Quadword, Register, L512, [EVEX_Vpxorq_zmm_k1z_zmm_zmmm512b64];
    // Byte and word arithmetic, VPMULLQ, word shifts, byte and word compares and tests, blends and the floating-point logic.
    VandnpdXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Memory, L128, [EVEX_Vandnpd_xmm_k1z_xmm_xmmm128b64];
    VandnpdXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Register, L128, [EVEX_Vandnpd_xmm_k1z_xmm_xmmm128b64];
    VandnpdYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Memory, L256, [EVEX_Vandnpd_ymm_k1z_ymm_ymmm256b64];
    VandnpdYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Register, L256, [EVEX_Vandnpd_ymm_k1z_ymm_ymmm256b64];
    VandnpdZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Memory, L512, [EVEX_Vandnpd_zmm_k1z_zmm_zmmm512b64];
    VandnpdZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::AndNot), Quadword, Register, L512, [EVEX_Vandnpd_zmm_k1z_zmm_zmmm512b64];
    VandnpsXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Memory, L128, [EVEX_Vandnps_xmm_k1z_xmm_xmmm128b32];
    VandnpsXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Register, L128, [EVEX_Vandnps_xmm_k1z_xmm_xmmm128b32];
    VandnpsYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Memory, L256, [EVEX_Vandnps_ymm_k1z_ymm_ymmm256b32];
    VandnpsYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Register, L256, [EVEX_Vandnps_ymm_k1z_ymm_ymmm256b32];
    VandnpsZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Memory, L512, [EVEX_Vandnps_zmm_k1z_zmm_zmmm512b32];
    VandnpsZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::AndNot), Doubleword, Register, L512, [EVEX_Vandnps_zmm_k1z_zmm_zmmm512b32];
    VandpdXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::And), Quadword, Memory, L128, [EVEX_Vandpd_xmm_k1z_xmm_xmmm128b64];
    VandpdXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::And), Quadword, Register, L128, [EVEX_Vandpd_xmm_k1z_xmm_xmmm128b64];
    VandpdYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::And), Quadword, Memory, L256, [EVEX_Vandpd_ymm_k1z_ymm_ymmm256b64];
    VandpdYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::And), Quadword, Register, L256, [EVEX_Vandpd_ymm_k1z_ymm_ymmm256b64];
    VandpdZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::And), Quadword, Memory, L512, [EVEX_Vandpd_zmm_k1z_zmm_zmmm512b64];
    VandpdZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::And), Quadword, Register, L512, [EVEX_Vandpd_zmm_k1z_zmm_zmmm512b64];
    VandpsXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::And), Doubleword, Memory, L128, [EVEX_Vandps_xmm_k1z_xmm_xmmm128b32];
    VandpsXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::And), Doubleword, Register, L128, [EVEX_Vandps_xmm_k1z_xmm_xmmm128b32];
    VandpsYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::And), Doubleword, Memory, L256, [EVEX_Vandps_ymm_k1z_ymm_ymmm256b32];
    VandpsYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::And), Doubleword, Register, L256, [EVEX_Vandps_ymm_k1z_ymm_ymmm256b32];
    VandpsZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::And), Doubleword, Memory, L512, [EVEX_Vandps_zmm_k1z_zmm_zmmm512b32];
    VandpsZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::And), Doubleword, Register, L512, [EVEX_Vandps_zmm_k1z_zmm_zmmm512b32];
    VblendmpdXmmf64MaskmskwXmmf64Memf64Avx512, Blend, Quadword, Memory, L128, [EVEX_Vblendmpd_xmm_k1z_xmm_xmmm128b64];
    VblendmpdXmmf64MaskmskwXmmf64Xmmf64Avx512, Blend, Quadword, Register, L128, [EVEX_Vblendmpd_xmm_k1z_xmm_xmmm128b64];
    VblendmpdYmmf64MaskmskwYmmf64Memf64Avx512, Blend, Quadword, Memory, L256, [EVEX_Vblendmpd_ymm_k1z_ymm_ymmm256b64];
    VblendmpdYmmf64MaskmskwYmmf64Ymmf64Avx512, Blend, Quadword, Register, L256, [EVEX_Vblendmpd_ymm_k1z_ymm_ymmm256b64];
    VblendmpdZmmf64MaskmskwZmmf64Memf64Avx512, Blend, Quadword, Memory, L512, [EVEX_Vblendmpd_zmm_k1z_zmm_zmmm512b64];
    VblendmpdZmmf64MaskmskwZmmf64Zmmf64Avx512, Blend, Quadword, Register, L512, [EVEX_Vblendmpd_zmm_k1z_zmm_zmmm512b64];
    VblendmpsXmmf32MaskmskwXmmf32Memf32Avx512, Blend, Doubleword, Memory, L128, [EVEX_Vblendmps_xmm_k1z_xmm_xmmm128b32];
    VblendmpsXmmf32MaskmskwXmmf32Xmmf32Avx512, Blend, Doubleword, Register, L128, [EVEX_Vblendmps_xmm_k1z_xmm_xmmm128b32];
    VblendmpsYmmf32MaskmskwYmmf32Memf32Avx512, Blend, Doubleword, Memory, L256, [EVEX_Vblendmps_ymm_k1z_ymm_ymmm256b32];
    VblendmpsYmmf32MaskmskwYmmf32Ymmf32Avx512, Blend, Doubleword, Register, L256, [EVEX_Vblendmps_ymm_k1z_ymm_ymmm256b32];
    VblendmpsZmmf32MaskmskwZmmf32Memf32Avx512, Blend, Doubleword, Memory, L512, [EVEX_Vblendmps_zmm_k1z_zmm_zmmm512b32];
    VblendmpsZmmf32MaskmskwZmmf32Zmmf32Avx512, Blend, Doubleword, Register, L512, [EVEX_Vblendmps_zmm_k1z_zmm_zmmm512b32];
    VorpdXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::Or), Quadword, Memory, L128, [EVEX_Vorpd_xmm_k1z_xmm_xmmm128b64];
    VorpdXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::Or), Quadword, Register, L128, [EVEX_Vorpd_xmm_k1z_xmm_xmmm128b64];
    VorpdYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::Or), Quadword, Memory, L256, [EVEX_Vorpd_ymm_k1z_ymm_ymmm256b64];
    VorpdYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::Or), Quadword, Register, L256, [EVEX_Vorpd_ymm_k1z_ymm_ymmm256b64];
    VorpdZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::Or), Quadword, Memory, L512, [EVEX_Vorpd_zmm_k1z_zmm_zmmm512b64];
    VorpdZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::Or), Quadword, Register, L512, [EVEX_Vorpd_zmm_k1z_zmm_zmmm512b64];
    VorpsXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Or), Doubleword, Memory, L128, [EVEX_Vorps_xmm_k1z_xmm_xmmm128b32];
    VorpsXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Or), Doubleword, Register, L128, [EVEX_Vorps_xmm_k1z_xmm_xmmm128b32];
    VorpsYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Or), Doubleword, Memory, L256, [EVEX_Vorps_ymm_k1z_ymm_ymmm256b32];
    VorpsYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Or), Doubleword, Register, L256, [EVEX_Vorps_ymm_k1z_ymm_ymmm256b32];
    VorpsZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Or), Doubleword, Memory, L512, [EVEX_Vorps_zmm_k1z_zmm_zmmm512b32];
    VorpsZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Or), Doubleword, Register, L512, [EVEX_Vorps_zmm_k1z_zmm_zmmm512b32];
    VpabsbXmmi8MaskmskwMemi8Avx512, Lanes(LaneOperation::Absolute(Byte)), Byte, Memory, L128, [EVEX_Vpabsb_xmm_k1z_xmmm128];
    VpabsbXmmi8MaskmskwXmmi8Avx512, Lanes(LaneOperation::Absolute(Byte)), Byte, Register, L128, [EVEX_Vpabsb_xmm_k1z_xmmm128];
    VpabsbYmmi8MaskmskwMemi8Avx512, Lanes(LaneOperation::Absolute(Byte)), Byte, Memory, L256, [EVEX_Vpabsb_ymm_k1z_ymmm256];
    VpabsbYmmi8MaskmskwYmmi8Avx512, Lanes(LaneOperation::Absolute(Byte)), Byte, Register, L256, [EVEX_Vpabsb_ymm_k1z_ymmm256];
    VpabsbZmmi8MaskmskwMemi8Avx512, Lanes(LaneOperation::Absolute(Byte)), Byte, Memory, L512, [EVEX_Vpabsb_zmm_k1z_zmmm512];
    VpabsbZmmi8MaskmskwZmmi8Avx512, Lanes(LaneOperation::Absolute(Byte)), Byte, Register, L512, [EVEX_Vpabsb_zmm_k1z_zmmm512];
    VpabswXmmi16MaskmskwMemi16Avx512, Lanes(LaneOperation::Absolute(Word)), Word, Memory, L128, [EVEX_Vpabsw_xmm_k1z_xmmm128];
    VpabswXmmi16MaskmskwXmmi16Avx512, Lanes(LaneOperation::Absolute(Word)), Word, Register, L128, [EVEX_Vpabsw_xmm_k1z_xmmm128];
    VpabswYmmi16MaskmskwMemi16Avx512, Lanes(LaneOperation::Absolute(Word)), Word, Memory, L256, [EVEX_Vpabsw_ymm_k1z_ymmm256];
    VpabswYmmi16MaskmskwYmmi16Avx512, Lanes(LaneOperation::Absolute(Word)), Word, Register, L256, [EVEX_Vpabsw_ymm_k1z_ymmm256];
    VpabswZmmi16MaskmskwMemi16Avx512, Lanes(LaneOperation::Absolute(Word)), Word, Memory, L512, [EVEX_Vpabsw_zmm_k1z_zmmm512];
    VpabswZmmi16MaskmskwZmmi16Avx512, Lanes(LaneOperation::Absolute(Word)), Word, Register, L512, [EVEX_Vpabsw_zmm_k1z_zmmm512];
    VpaddbXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::Add(Byte)), Byte, Memory, L128, [EVEX_Vpaddb_xmm_k1z_xmm_xmmm128];
    VpaddbXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::Add(Byte)), Byte, Register, L128, [EVEX_Vpaddb_xmm_k1z_xmm_xmmm128];
    VpaddbYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::Add(Byte)), Byte, Memory, L256, [EVEX_Vpaddb_ymm_k1z_ymm_ymmm256];
    VpaddbYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::Add(Byte)), Byte, Register, L256, [EVEX_Vpaddb_ymm_k1z_ymm_ymmm256];
    VpaddbZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::Add(Byte)), Byte, Memory, L512, [EVEX_Vpaddb_zmm_k1z_zmm_zmmm512];
    VpaddbZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::Add(Byte)), Byte, Register, L512, [EVEX_Vpaddb_zmm_k1z_zmm_zmmm512];
    VpaddsbXmmi8MaskmskwXmmi8Memi8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Signed)), Byte, Memory, L128, [EVEX_Vpaddsb_xmm_k1z_xmm_xmmm128];
    VpaddsbXmmi8MaskmskwXmmi8Xmmi8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Signed)), Byte, Register, L128, [EVEX_Vpaddsb_xmm_k1z_xmm_xmmm128];
    VpaddsbYmmi8MaskmskwYmmi8Memi8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Signed)), Byte, Memory, L256, [EVEX_Vpaddsb_ymm_k1z_ymm_ymmm256];
    VpaddsbYmmi8MaskmskwYmmi8Ymmi8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Signed)), Byte, Register, L256, [EVEX_Vpaddsb_ymm_k1z_ymm_ymmm256];
    VpaddsbZmmi8MaskmskwZmmi8Memi8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Signed)), Byte, Memory, L512, [EVEX_Vpaddsb_zmm_k1z_zmm_zmmm512];
    VpaddsbZmmi8MaskmskwZmmi8Zmmi8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Signed)), Byte, Register, L512, [EVEX_Vpaddsb_zmm_k1z_zmm_zmmm512];
    VpaddswXmmi16MaskmskwXmmi16Memi16Avx512, Lanes(LaneOperation::AddSaturating(Word, Signed)), Word, Memory, L128, [EVEX_Vpaddsw_xmm_k1z_xmm_xmmm128];
    VpaddswXmmi16MaskmskwXmmi16Xmmi16Avx512, Lanes(LaneOperation::AddSaturating(Word, Signed)), Word, Register, L128, [EVEX_Vpaddsw_xmm_k1z_xmm_xmmm128];
    VpaddswYmmi16MaskmskwYmmi16Memi16Avx512, Lanes(LaneOperation::AddSaturating(Word, Signed)), Word, Memory, L256, [EVEX_Vpaddsw_ymm_k1z_ymm_ymmm256];
    VpaddswYmmi16MaskmskwYmmi16Ymmi16Avx512, Lanes(LaneOperation::AddSaturating(Word, Signed)), Word, Register, L256, [EVEX_Vpaddsw_ymm_k1z_ymm_ymmm256];
    VpaddswZmmi16MaskmskwZmmi16Memi16Avx512, Lanes(LaneOperation::AddSaturating(Word, Signed)), Word, Memory, L512, [EVEX_Vpaddsw_zmm_k1z_zmm_zmmm512];
    VpaddswZmmi16MaskmskwZmmi16Zmmi16Avx512, Lanes(LaneOperation::AddSaturating(Word, Signed)), Word, Register, L512, [EVEX_Vpaddsw_zmm_k1z_zmm_zmmm512];
    VpaddusbXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Unsigned)), Byte, Memory, L128, [EVEX_Vpaddusb_xmm_k1z_xmm_xmmm128];
    VpaddusbXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Unsigned)), Byte, Register, L128, [EVEX_Vpaddusb_xmm_k1z_xmm_xmmm128];
    VpaddusbYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Unsigned)), Byte, Memory, L256, [EVEX_Vpaddusb_ymm_k1z_ymm_ymmm256];
    VpaddusbYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Unsigned)), Byte, Register, L256, [EVEX_Vpaddusb_ymm_k1z_ymm_ymmm256];
    VpaddusbZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Unsigned)), Byte, Memory, L512, [EVEX_Vpaddusb_zmm_k1z_zmm_zmmm512];
    VpaddusbZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::AddSaturating(Byte, Unsigned)), Byte, Register, L512, [EVEX_Vpaddusb_zmm_k1z_zmm_zmmm512];
    VpadduswXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::AddSaturating(Word, Unsigned)), Word, Memory, L128, [EVEX_Vpaddusw_xmm_k1z_xmm_xmmm128];
    VpadduswXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::AddSaturating(Word, Unsigned)), Word, Register, L128, [EVEX_Vpaddusw_xmm_k1z_xmm_xmmm128];
    VpadduswYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::AddSaturating(Word, Unsigned)), Word, Memory, L256, [EVEX_Vpaddusw_ymm_k1z_ymm_ymmm256];
    VpadduswYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::AddSaturating(Word, Unsigned)), Word, Register, L256, [EVEX_Vpaddusw_ymm_k1z_ymm_ymmm256];
    VpadduswZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::AddSaturating(Word, Unsigned)), Word, Memory, L512, [EVEX_Vpaddusw_zmm_k1z_zmm_zmmm512];
    VpadduswZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::AddSaturating(Word, Unsigned)), Word, Register, L512, [EVEX_Vpaddusw_zmm_k1z_zmm_zmmm512];
    VpaddwXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::Add(Word)), Word, Memory, L128, [EVEX_Vpaddw_xmm_k1z_xmm_xmmm128];
    VpaddwXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::Add(Word)), Word, Register, L128, [EVEX_Vpaddw_xmm_k1z_xmm_xmmm128];
    VpaddwYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::Add(Word)), Word, Memory, L256, [EVEX_Vpaddw_ymm_k1z_ymm_ymmm256];
    VpaddwYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::Add(Word)), Word, Register, L256, [EVEX_Vpaddw_ymm_k1z_ymm_ymmm256];
    VpaddwZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::Add(Word)), Word, Memory, L512, [EVEX_Vpaddw_zmm_k1z_zmm_zmmm512];
    VpaddwZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::Add(Word)), Word, Register, L512, [EVEX_Vpaddw_zmm_k1z_zmm_zmmm512];
    VpavgbXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::Average(Byte)), Byte, Memory, L128, [EVEX_Vpavgb_xmm_k1z_xmm_xmmm128];
    VpavgbXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::Average(Byte)), Byte, Register, L128, [EVEX_Vpavgb_xmm_k1z_xmm_xmmm128];
    VpavgbYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::Average(Byte)), Byte, Memory, L256, [EVEX_Vpavgb_ymm_k1z_ymm_ymmm256];
    VpavgbYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::Average(Byte)), Byte, Register, L256, [EVEX_Vpavgb_ymm_k1z_ymm_ymmm256];
    VpavgbZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::Average(Byte)), Byte, Memory, L512, [EVEX_Vpavgb_zmm_k1z_zmm_zmmm512];
    VpavgbZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::Average(Byte)), Byte, Register, L512, [EVEX_Vpavgb_zmm_k1z_zmm_zmmm512];
    VpavgwXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::Average(Word)), Word, Memory, L128, [EVEX_Vpavgw_xmm_k1z_xmm_xmmm128];
    VpavgwXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::Average(Word)), Word, Register, L128, [EVEX_Vpavgw_xmm_k1z_xmm_xmmm128];
    VpavgwYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::Average(Word)), Word, Memory, L256, [EVEX_Vpavgw_ymm_k1z_ymm_ymmm256];
    VpavgwYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::Average(Word)), Word, Register, L256, [EVEX_Vpavgw_ymm_k1z_ymm_ymmm256];
    VpavgwZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::Average(Word)), Word, Memory, L512, [EVEX_Vpavgw_zmm_k1z_zmm_zmmm512];
    VpavgwZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::Average(Word)), Word, Register, L512, [EVEX_Vpavgw_zmm_k1z_zmm_zmmm512];
    VpblendmbXmmu8MaskmskwXmmu8Memu8Avx512, Blend, Byte, Memory, L128, [EVEX_Vpblendmb_xmm_k1z_xmm_xmmm128];
    VpblendmbXmmu8MaskmskwXmmu8Xmmu8Avx512, Blend, Byte, Register, L128, [EVEX_Vpblendmb_xmm_k1z_xmm_xmmm128];
    VpblendmbYmmu8MaskmskwYmmu8Memu8Avx512, Blend, Byte, Memory, L256, [EVEX_Vpblendmb_ymm_k1z_ymm_ymmm256];
    VpblendmbYmmu8MaskmskwYmmu8Ymmu8Avx512, Blend, Byte, Register, L256, [EVEX_Vpblendmb_ymm_k1z_ymm_ymmm256];
    VpblendmbZmmu8MaskmskwZmmu8Memu8Avx512, Blend, Byte, Memory, L512, [EVEX_Vpblendmb_zmm_k1z_zmm_zmmm512];
    VpblendmbZmmu8MaskmskwZmmu8Zmmu8Avx512, Blend, Byte, Register, L512, [EVEX_Vpblendmb_zmm_k1z_zmm_zmmm512];
    VpblendmwXmmu16MaskmskwXmmu16Memu16Avx512, Blend, Word, Memory, L128, [EVEX_Vpblendmw_xmm_k1z_xmm_xmmm128];
    VpblendmwXmmu16MaskmskwXmmu16Xmmu16Avx512, Blend, Word, Register, L128, [EVEX_Vpblendmw_xmm_k1z_xmm_xmmm128];
    VpblendmwYmmu16MaskmskwYmmu16Memu16Avx512, Blend, Word, Memory, L256, [EVEX_Vpblendmw_ymm_k1z_ymm_ymmm256];
    VpblendmwYmmu16MaskmskwYmmu16Ymmu16Avx512, Blend, Word, Register, L256, [EVEX_Vpblendmw_ymm_k1z_ymm_ymmm256];
    VpblendmwZmmu16MaskmskwZmmu16Memu16Avx512, Blend, Word, Memory, L512, [EVEX_Vpblendmw_zmm_k1z_zmm_zmmm512];
    VpblendmwZmmu16MaskmskwZmmu16Zmmu16Avx512, Blend, Word, Register, L512, [EVEX_Vpblendmw_zmm_k1z_zmm_zmmm512];
    VpcmpbMaskmskwMaskmskwXmmi8Memi8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Byte, Memory, L128, [EVEX_Vpcmpb_kr_k1_xmm_xmmm128_imm8];
    VpcmpbMaskmskwMaskmskwXmmi8Xmmi8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Byte, Register, L128, [EVEX_Vpcmpb_kr_k1_xmm_xmmm128_imm8];
    VpcmpbMaskmskwMaskmskwYmmi8Memi8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Byte, Memory, L256, [EVEX_Vpcmpb_kr_k1_ymm_ymmm256_imm8];
    VpcmpbMaskmskwMaskmskwYmmi8Ymmi8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Byte, Register, L256, [EVEX_Vpcmpb_kr_k1_ymm_ymmm256_imm8];
    VpcmpbMaskmskwMaskmskwZmmi8Memi8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Byte, Memory, L512, [EVEX_Vpcmpb_kr_k1_zmm_zmmm512_imm8];
    VpcmpbMaskmskwMaskmskwZmmi8Zmmi8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Byte, Register, L512, [EVEX_Vpcmpb_kr_k1_zmm_zmmm512_imm8];
    VpcmpeqbMaskmskwMaskmskwXmmu8Memu8Avx512, CompareIntoMask(ComparePredicate::Equal), Byte, Memory, L128, [EVEX_Vpcmpeqb_kr_k1_xmm_xmmm128];
    VpcmpeqbMaskmskwMaskmskwXmmu8Xmmu8Avx512, CompareIntoMask(ComparePredicate::Equal), Byte, Register, L128, [EVEX_Vpcmpeqb_kr_k1_xmm_xmmm128];
    VpcmpeqbMaskmskwMaskmskwYmmu8Memu8Avx512, CompareIntoMask(ComparePredicate::Equal), Byte, Memory, L256, [EVEX_Vpcmpeqb_kr_k1_ymm_ymmm256];
    VpcmpeqbMaskmskwMaskmskwYmmu8Ymmu8Avx512, CompareIntoMask(ComparePredicate::Equal), Byte, Register, L256, [EVEX_Vpcmpeqb_kr_k1_ymm_ymmm256];
    VpcmpeqbMaskmskwMaskmskwZmmu8Memu8Avx512, CompareIntoMask(ComparePredicate::Equal), Byte, Memory, L512, [EVEX_Vpcmpeqb_kr_k1_zmm_zmmm512];
    VpcmpeqbMaskmskwMaskmskwZmmu8Zmmu8Avx512, CompareIntoMask(ComparePredicate::Equal), Byte, Register, L512, [EVEX_Vpcmpeqb_kr_k1_zmm_zmmm512];
    VpcmpeqwMaskmskwMaskmskwXmmu16Memu16Avx512, CompareIntoMask(ComparePredicate::Equal), Word, Memory, L128, [EVEX_Vpcmpeqw_kr_k1_xmm_xmmm128];
    VpcmpeqwMaskmskwMaskmskwXmmu16Xmmu16Avx512, CompareIntoMask(ComparePredicate::Equal), Word, Register, L128, [EVEX_Vpcmpeqw_kr_k1_xmm_xmmm128];
    VpcmpeqwMaskmskwMaskmskwYmmu16Memu16Avx512, CompareIntoMask(ComparePredicate::Equal), Word, Memory, L256, [EVEX_Vpcmpeqw_kr_k1_ymm_ymmm256];
    VpcmpeqwMaskmskwMaskmskwYmmu16Ymmu16Avx512, CompareIntoMask(ComparePredicate::Equal), Word, Register, L256, [EVEX_Vpcmpeqw_kr_k1_ymm_ymmm256];
    VpcmpeqwMaskmskwMaskmskwZmmu16Memu16Avx512, CompareIntoMask(ComparePredicate::Equal), Word, Memory, L512, [EVEX_Vpcmpeqw_kr_k1_zmm_zmmm512];
    VpcmpeqwMaskmskwMaskmskwZmmu16Zmmu16Avx512, CompareIntoMask(ComparePredicate::Equal), Word, Register, L512, [EVEX_Vpcmpeqw_kr_k1_zmm_zmmm512];
    VpcmpgtbMaskmskwMaskmskwXmmu8Memu8Avx512, CompareIntoMask(ComparePredicate::Greater), Byte, Memory, L128, [EVEX_Vpcmpgtb_kr_k1_xmm_xmmm128];
    VpcmpgtbMaskmskwMaskmskwXmmu8Xmmu8Avx512, CompareIntoMask(ComparePredicate::Greater), Byte, Register, L128, [EVEX_Vpcmpgtb_kr_k1_xmm_xmmm128];
    VpcmpgtbMaskmskwMaskmskwYmmu8Memu8Avx512, CompareIntoMask(ComparePredicate::Greater), Byte, Memory, L256, [EVEX_Vpcmpgtb_kr_k1_ymm_ymmm256];
    VpcmpgtbMaskmskwMaskmskwYmmu8Ymmu8Avx512, CompareIntoMask(ComparePredicate::Greater), Byte, Register, L256, [EVEX_Vpcmpgtb_kr_k1_ymm_ymmm256];
    VpcmpgtbMaskmskwMaskmskwZmmu8Memu8Avx512, CompareIntoMask(ComparePredicate::Greater), Byte, Memory, L512, [EVEX_Vpcmpgtb_kr_k1_zmm_zmmm512];
    VpcmpgtbMaskmskwMaskmskwZmmu8Zmmu8Avx512, CompareIntoMask(ComparePredicate::Greater), Byte, Register, L512, [EVEX_Vpcmpgtb_kr_k1_zmm_zmmm512];
    VpcmpgtwMaskmskwMaskmskwXmmu16Memu16Avx512, CompareIntoMask(ComparePredicate::Greater), Word, Memory, L128, [EVEX_Vpcmpgtw_kr_k1_xmm_xmmm128];
    VpcmpgtwMaskmskwMaskmskwXmmu16Xmmu16Avx512, CompareIntoMask(ComparePredicate::Greater), Word, Register, L128, [EVEX_Vpcmpgtw_kr_k1_xmm_xmmm128];
    VpcmpgtwMaskmskwMaskmskwYmmu16Memu16Avx512, CompareIntoMask(ComparePredicate::Greater), Word, Memory, L256, [EVEX_Vpcmpgtw_kr_k1_ymm_ymmm256];
    VpcmpgtwMaskmskwMaskmskwYmmu16Ymmu16Avx512, CompareIntoMask(ComparePredicate::Greater), Word, Register, L256, [EVEX_Vpcmpgtw_kr_k1_ymm_ymmm256];
    VpcmpgtwMaskmskwMaskmskwZmmu16Memu16Avx512, CompareIntoMask(ComparePredicate::Greater), Word, Memory, L512, [EVEX_Vpcmpgtw_kr_k1_zmm_zmmm512];
    VpcmpgtwMaskmskwMaskmskwZmmu16Zmmu16Avx512, CompareIntoMask(ComparePredicate::Greater), Word, Register, L512, [EVEX_Vpcmpgtw_kr_k1_zmm_zmmm512];
    VpcmpubMaskmskwMaskmskwXmmu8Memu8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Byte, Memory, L128, [EVEX_Vpcmpub_kr_k1_xmm_xmmm128_imm8];
    VpcmpubMaskmskwMaskmskwXmmu8Xmmu8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Byte, Register, L128, [EVEX_Vpcmpub_kr_k1_xmm_xmmm128_imm8];
    VpcmpubMaskmskwMaskmskwYmmu8Memu8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Byte, Memory, L256, [EVEX_Vpcmpub_kr_k1_ymm_ymmm256_imm8];
    VpcmpubMaskmskwMaskmskwYmmu8Ymmu8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Byte, Register, L256, [EVEX_Vpcmpub_kr_k1_ymm_ymmm256_imm8];
    VpcmpubMaskmskwMaskmskwZmmu8Memu8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Byte, Memory, L512, [EVEX_Vpcmpub_kr_k1_zmm_zmmm512_imm8];
    VpcmpubMaskmskwMaskmskwZmmu8Zmmu8Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Byte, Register, L512, [EVEX_Vpcmpub_kr_k1_zmm_zmmm512_imm8];
    VpcmpuwMaskmskwMaskmskwXmmu16Memu16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Word, Memory, L128, [EVEX_Vpcmpuw_kr_k1_xmm_xmmm128_imm8];
    VpcmpuwMaskmskwMaskmskwXmmu16Xmmu16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Word, Register, L128, [EVEX_Vpcmpuw_kr_k1_xmm_xmmm128_imm8];
    VpcmpuwMaskmskwMaskmskwYmmu16Memu16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Word, Memory, L256, [EVEX_Vpcmpuw_kr_k1_ymm_ymmm256_imm8];
    VpcmpuwMaskmskwMaskmskwYmmu16Ymmu16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Word, Register, L256, [EVEX_Vpcmpuw_kr_k1_ymm_ymmm256_imm8];
    VpcmpuwMaskmskwMaskmskwZmmu16Memu16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Word, Memory, L512, [EVEX_Vpcmpuw_kr_k1_zmm_zmmm512_imm8];
    VpcmpuwMaskmskwMaskmskwZmmu16Zmmu16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Unsigned)), Word, Register, L512, [EVEX_Vpcmpuw_kr_k1_zmm_zmmm512_imm8];
    VpcmpwMaskmskwMaskmskwXmmi16Memi16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Word, Memory, L128, [EVEX_Vpcmpw_kr_k1_xmm_xmmm128_imm8];
    VpcmpwMaskmskwMaskmskwXmmi16Xmmi16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Word, Register, L128, [EVEX_Vpcmpw_kr_k1_xmm_xmmm128_imm8];
    VpcmpwMaskmskwMaskmskwYmmi16Memi16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Word, Memory, L256, [EVEX_Vpcmpw_kr_k1_ymm_ymmm256_imm8];
    VpcmpwMaskmskwMaskmskwYmmi16Ymmi16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Word, Register, L256, [EVEX_Vpcmpw_kr_k1_ymm_ymmm256_imm8];
    VpcmpwMaskmskwMaskmskwZmmi16Memi16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Word, Memory, L512, [EVEX_Vpcmpw_kr_k1_zmm_zmmm512_imm8];
    VpcmpwMaskmskwMaskmskwZmmi16Zmmi16Imm8Avx512, CompareIntoMask(ComparePredicate::Immediate(Signed)), Word, Register, L512, [EVEX_Vpcmpw_kr_k1_zmm_zmmm512_imm8];
    VpmaddubswXmmi16MaskmskwXmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyAddBytes), Word, Memory, L128, [EVEX_Vpmaddubsw_xmm_k1z_xmm_xmmm128];
    VpmaddubswXmmi16MaskmskwXmmi16Xmmi16Avx512, Lanes(LaneOperation::MultiplyAddBytes), Word, Register, L128, [EVEX_Vpmaddubsw_xmm_k1z_xmm_xmmm128];
    VpmaddubswYmmi16MaskmskwYmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyAddBytes), Word, Memory, L256, [EVEX_Vpmaddubsw_ymm_k1z_ymm_ymmm256];
    VpmaddubswYmmi16MaskmskwYmmi16Ymmi16Avx512, Lanes(LaneOperation::MultiplyAddBytes), Word, Register, L256, [EVEX_Vpmaddubsw_ymm_k1z_ymm_ymmm256];
    VpmaddubswZmmi16MaskmskwZmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyAddBytes), Word, Memory, L512, [EVEX_Vpmaddubsw_zmm_k1z_zmm_zmmm512];
    VpmaddubswZmmi16MaskmskwZmmi16Zmmi16Avx512, Lanes(LaneOperation::MultiplyAddBytes), Word, Register, L512, [EVEX_Vpmaddubsw_zmm_k1z_zmm_zmmm512];
    VpmaddwdXmmi32MaskmskwXmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyAddWords), Doubleword, Memory, L128, [EVEX_Vpmaddwd_xmm_k1z_xmm_xmmm128];
    VpmaddwdXmmi32MaskmskwXmmi16Xmmi16Avx512, Lanes(LaneOperation::MultiplyAddWords), Doubleword, Register, L128, [EVEX_Vpmaddwd_xmm_k1z_xmm_xmmm128];
    VpmaddwdYmmi32MaskmskwYmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyAddWords), Doubleword, Memory, L256, [EVEX_Vpmaddwd_ymm_k1z_ymm_ymmm256];
    VpmaddwdYmmi32MaskmskwYmmi16Ymmi16Avx512, Lanes(LaneOperation::MultiplyAddWords), Doubleword, Register, L256, [EVEX_Vpmaddwd_ymm_k1z_ymm_ymmm256];
    VpmaddwdZmmi32MaskmskwZmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyAddWords), Doubleword, Memory, L512, [EVEX_Vpmaddwd_zmm_k1z_zmm_zmmm512];
    VpmaddwdZmmi32MaskmskwZmmi16Zmmi16Avx512, Lanes(LaneOperation::MultiplyAddWords), Doubleword, Register, L512, [EVEX_Vpmaddwd_zmm_k1z_zmm_zmmm512];
    VpmaxsbXmmi8MaskmskwXmmi8Memi8Avx512, Lanes(LaneOperation::Maximum(Byte, Signed)), Byte, Memory, L128, [EVEX_Vpmaxsb_xmm_k1z_xmm_xmmm128];
    VpmaxsbXmmi8MaskmskwXmmi8Xmmi8Avx512, Lanes(LaneOperation::Maximum(Byte, Signed)), Byte, Register, L128, [EVEX_Vpmaxsb_xmm_k1z_xmm_xmmm128];
    VpmaxsbYmmi8MaskmskwYmmi8Memi8Avx512, Lanes(LaneOperation::Maximum(Byte, Signed)), Byte, Memory, L256, [EVEX_Vpmaxsb_ymm_k1z_ymm_ymmm256];
    VpmaxsbYmmi8MaskmskwYmmi8Ymmi8Avx512, Lanes(LaneOperation::Maximum(Byte, Signed)), Byte, Register, L256, [EVEX_Vpmaxsb_ymm_k1z_ymm_ymmm256];
    VpmaxsbZmmi8MaskmskwZmmi8Memi8Avx512, Lanes(LaneOperation::Maximum(Byte, Signed)), Byte, Memory, L512, [EVEX_Vpmaxsb_zmm_k1z_zmm_zmmm512];
    VpmaxsbZmmi8MaskmskwZmmi8Zmmi8Avx512, Lanes(LaneOperation::Maximum(Byte, Signed)), Byte, Register, L512, [EVEX_Vpmaxsb_zmm_k1z_zmm_zmmm512];
    VpmaxswXmmi16MaskmskwXmmi16Memi16Avx512, Lanes(LaneOperation::Maximum(Word, Signed)), Word, Memory, L128, [EVEX_Vpmaxsw_xmm_k1z_xmm_xmmm128];
    VpmaxswXmmi16MaskmskwXmmi16Xmmi16Avx512, Lanes(LaneOperation::Maximum(Word, Signed)), Word, Register, L128, [EVEX_Vpmaxsw_xmm_k1z_xmm_xmmm128];
    VpmaxswYmmi16MaskmskwYmmi16Memi16Avx512, Lanes(LaneOperation::Maximum(Word, Signed)), Word, Memory, L256, [EVEX_Vpmaxsw_ymm_k1z_ymm_ymmm256];
    VpmaxswYmmi16MaskmskwYmmi16Ymmi16Avx512, Lanes(LaneOperation::Maximum(Word, Signed)), Word, Register, L256, [EVEX_Vpmaxsw_ymm_k1z_ymm_ymmm256];
    VpmaxswZmmi16MaskmskwZmmi16Memi16Avx512, Lanes(LaneOperation::Maximum(Word, Signed)), Word, Memory, L512, [EVEX_Vpmaxsw_zmm_k1z_zmm_zmmm512];
    VpmaxswZmmi16MaskmskwZmmi16Zmmi16Avx512, Lanes(LaneOperation::Maximum(Word, Signed)), Word, Register, L512, [EVEX_Vpmaxsw_zmm_k1z_zmm_zmmm512];
    VpmaxubXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::Maximum(Byte, Unsigned)), Byte, Memory, L128, [EVEX_Vpmaxub_xmm_k1z_xmm_xmmm128];
    VpmaxubXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::Maximum(Byte, Unsigned)), Byte, Register, L128, [EVEX_Vpmaxub_xmm_k1z_xmm_xmmm128];
    VpmaxubYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::Maximum(Byte, Unsigned)), Byte, Memory, L256, [EVEX_Vpmaxub_ymm_k1z_ymm_ymmm256];
    VpmaxubYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::Maximum(Byte, Unsigned)), Byte, Register, L256, [EVEX_Vpmaxub_ymm_k1z_ymm_ymmm256];
    VpmaxubZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::Maximum(Byte, Unsigned)), Byte, Memory, L512, [EVEX_Vpmaxub_zmm_k1z_zmm_zmmm512];
    VpmaxubZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::Maximum(Byte, Unsigned)), Byte, Register, L512, [EVEX_Vpmaxub_zmm_k1z_zmm_zmmm512];
    VpmaxuwXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::Maximum(Word, Unsigned)), Word, Memory, L128, [EVEX_Vpmaxuw_xmm_k1z_xmm_xmmm128];
    VpmaxuwXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::Maximum(Word, Unsigned)), Word, Register, L128, [EVEX_Vpmaxuw_xmm_k1z_xmm_xmmm128];
    VpmaxuwYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::Maximum(Word, Unsigned)), Word, Memory, L256, [EVEX_Vpmaxuw_ymm_k1z_ymm_ymmm256];
    VpmaxuwYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::Maximum(Word, Unsigned)), Word, Register, L256, [EVEX_Vpmaxuw_ymm_k1z_ymm_ymmm256];
    VpmaxuwZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::Maximum(Word, Unsigned)), Word, Memory, L512, [EVEX_Vpmaxuw_zmm_k1z_zmm_zmmm512];
    VpmaxuwZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::Maximum(Word, Unsigned)), Word, Register, L512, [EVEX_Vpmaxuw_zmm_k1z_zmm_zmmm512];
    VpminsbXmmi8MaskmskwXmmi8Memi8Avx512, Lanes(LaneOperation::Minimum(Byte, Signed)), Byte, Memory, L128, [EVEX_Vpminsb_xmm_k1z_xmm_xmmm128];
    VpminsbXmmi8MaskmskwXmmi8Xmmi8Avx512, Lanes(LaneOperation::Minimum(Byte, Signed)), Byte, Register, L128, [EVEX_Vpminsb_xmm_k1z_xmm_xmmm128];
    VpminsbYmmi8MaskmskwYmmi8Memi8Avx512, Lanes(LaneOperation::Minimum(Byte, Signed)), Byte, Memory, L256, [EVEX_Vpminsb_ymm_k1z_ymm_ymmm256];
    VpminsbYmmi8MaskmskwYmmi8Ymmi8Avx512, Lanes(LaneOperation::Minimum(Byte, Signed)), Byte, Register, L256, [EVEX_Vpminsb_ymm_k1z_ymm_ymmm256];
    VpminsbZmmi8MaskmskwZmmi8Memi8Avx512, Lanes(LaneOperation::Minimum(Byte, Signed)), Byte, Memory, L512, [EVEX_Vpminsb_zmm_k1z_zmm_zmmm512];
    VpminsbZmmi8MaskmskwZmmi8Zmmi8Avx512, Lanes(LaneOperation::Minimum(Byte, Signed)), Byte, Register, L512, [EVEX_Vpminsb_zmm_k1z_zmm_zmmm512];
    VpminswXmmi16MaskmskwXmmi16Memi16Avx512, Lanes(LaneOperation::Minimum(Word, Signed)), Word, Memory, L128, [EVEX_Vpminsw_xmm_k1z_xmm_xmmm128];
    VpminswXmmi16MaskmskwXmmi16Xmmi16Avx512, Lanes(LaneOperation::Minimum(Word, Signed)), Word, Register, L128, [EVEX_Vpminsw_xmm_k1z_xmm_xmmm128];
    VpminswYmmi16MaskmskwYmmi16Memi16Avx512, Lanes(LaneOperation::Minimum(Word, Signed)), Word, Memory, L256, [EVEX_Vpminsw_ymm_k1z_ymm_ymmm256];
    VpminswYmmi16MaskmskwYmmi16Ymmi16Avx512, Lanes(LaneOperation::Minimum(Word, Signed)), Word, Register, L256, [EVEX_Vpminsw_ymm_k1z_ymm_ymmm256];
    VpminswZmmi16MaskmskwZmmi16Memi16Avx512, Lanes(LaneOperation::Minimum(Word, Signed)), Word, Memory, L512, [EVEX_Vpminsw_zmm_k1z_zmm_zmmm512];
    VpminswZmmi16MaskmskwZmmi16Zmmi16Avx512, Lanes(LaneOperation::Minimum(Word, Signed)), Word, Register, L512, [EVEX_Vpminsw_zmm_k1z_zmm_zmmm512];
    VpminubXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::Minimum(Byte, Unsigned)), Byte, Memory, L128, [EVEX_Vpminub_xmm_k1z_xmm_xmmm128];
    VpminubXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::Minimum(Byte, Unsigned)), Byte, Register, L128, [EVEX_Vpminub_xmm_k1z_xmm_xmmm128];
    VpminubYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::Minimum(Byte, Unsigned)), Byte, Memory, L256, [EVEX_Vpminub_ymm_k1z_ymm_ymmm256];
    VpminubYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::Minimum(Byte, Unsigned)), Byte, Register, L256, [EVEX_Vpminub_ymm_k1z_ymm_ymmm256];
    VpminubZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::Minimum(Byte, Unsigned)), Byte, Memory, L512, [EVEX_Vpminub_zmm_k1z_zmm_zmmm512];
    VpminubZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::Minimum(Byte, Unsigned)), Byte, Register, L512, [EVEX_Vpminub_zmm_k1z_zmm_zmmm512];
    VpminuwXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::Minimum(Word, Unsigned)), Word, Memory, L128, [EVEX_Vpminuw_xmm_k1z_xmm_xmmm128];
    VpminuwXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::Minimum(Word, Unsigned)), Word, Register, L128, [EVEX_Vpminuw_xmm_k1z_xmm_xmmm128];
    VpminuwYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::Minimum(Word, Unsigned)), Word, Memory, L256, [EVEX_Vpminuw_ymm_k1z_ymm_ymmm256];
    VpminuwYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::Minimum(Word, Unsigned)), Word, Register, L256, [EVEX_Vpminuw_ymm_k1z_ymm_ymmm256];
    VpminuwZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::Minimum(Word, Unsigned)), Word, Memory, L512, [EVEX_Vpminuw_zmm_k1z_zmm_zmmm512];
    VpminuwZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::Minimum(Word, Unsigned)), Word, Register, L512, [EVEX_Vpminuw_zmm_k1z_zmm_zmmm512];
    VpmulhrswXmmi16MaskmskwXmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyHighRoundScale), Word, Memory, L128, [EVEX_Vpmulhrsw_xmm_k1z_xmm_xmmm128];
    VpmulhrswXmmi16MaskmskwXmmi16Xmmi16Avx512, Lanes(LaneOperation::MultiplyHighRoundScale), Word, Register, L128, [EVEX_Vpmulhrsw_xmm_k1z_xmm_xmmm128];
    VpmulhrswYmmi16MaskmskwYmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyHighRoundScale), Word, Memory, L256, [EVEX_Vpmulhrsw_ymm_k1z_ymm_ymmm256];
    VpmulhrswYmmi16MaskmskwYmmi16Ymmi16Avx512, Lanes(LaneOperation::MultiplyHighRoundScale), Word, Register, L256, [EVEX_Vpmulhrsw_ymm_k1z_ymm_ymmm256];
    VpmulhrswZmmi16MaskmskwZmmi16Memi16Avx512, Lanes(LaneOperation::MultiplyHighRoundScale), Word, Memory, L512, [EVEX_Vpmulhrsw_zmm_k1z_zmm_zmmm512];
    VpmulhrswZmmi16MaskmskwZmmi16Zmmi16Avx512, Lanes(LaneOperation::MultiplyHighRoundScale), Word, Register, L512, [EVEX_Vpmulhrsw_zmm_k1z_zmm_zmmm512];
    VpmulhuwXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Unsigned)), Word, Memory, L128, [EVEX_Vpmulhuw_xmm_k1z_xmm_xmmm128];
    VpmulhuwXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Unsigned)), Word, Register, L128, [EVEX_Vpmulhuw_xmm_k1z_xmm_xmmm128];
    VpmulhuwYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Unsigned)), Word, Memory, L256, [EVEX_Vpmulhuw_ymm_k1z_ymm_ymmm256];
    VpmulhuwYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Unsigned)), Word, Register, L256, [EVEX_Vpmulhuw_ymm_k1z_ymm_ymmm256];
    VpmulhuwZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Unsigned)), Word, Memory, L512, [EVEX_Vpmulhuw_zmm_k1z_zmm_zmmm512];
    VpmulhuwZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Unsigned)), Word, Register, L512, [EVEX_Vpmulhuw_zmm_k1z_zmm_zmmm512];
    VpmulhwXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Signed)), Word, Memory, L128, [EVEX_Vpmulhw_xmm_k1z_xmm_xmmm128];
    VpmulhwXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Signed)), Word, Register, L128, [EVEX_Vpmulhw_xmm_k1z_xmm_xmmm128];
    VpmulhwYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Signed)), Word, Memory, L256, [EVEX_Vpmulhw_ymm_k1z_ymm_ymmm256];
    VpmulhwYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Signed)), Word, Register, L256, [EVEX_Vpmulhw_ymm_k1z_ymm_ymmm256];
    VpmulhwZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Signed)), Word, Memory, L512, [EVEX_Vpmulhw_zmm_k1z_zmm_zmmm512];
    VpmulhwZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::MultiplyHighWords(Signed)), Word, Register, L512, [EVEX_Vpmulhw_zmm_k1z_zmm_zmmm512];
    VpmullqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::MultiplyLow(Quadword)), Quadword, Memory, L128, [EVEX_Vpmullq_xmm_k1z_xmm_xmmm128b64];
    VpmullqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::MultiplyLow(Quadword)), Quadword, Register, L128, [EVEX_Vpmullq_xmm_k1z_xmm_xmmm128b64];
    VpmullqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::MultiplyLow(Quadword)), Quadword, Memory, L256, [EVEX_Vpmullq_ymm_k1z_ymm_ymmm256b64];
    VpmullqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::MultiplyLow(Quadword)), Quadword, Register, L256, [EVEX_Vpmullq_ymm_k1z_ymm_ymmm256b64];
    VpmullqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::MultiplyLow(Quadword)), Quadword, Memory, L512, [EVEX_Vpmullq_zmm_k1z_zmm_zmmm512b64];
    VpmullqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::MultiplyLow(Quadword)), Quadword, Register, L512, [EVEX_Vpmullq_zmm_k1z_zmm_zmmm512b64];
    VpmullwXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyLow(Word)), Word, Memory, L128, [EVEX_Vpmullw_xmm_k1z_xmm_xmmm128];
    VpmullwXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::MultiplyLow(Word)), Word, Register, L128, [EVEX_Vpmullw_xmm_k1z_xmm_xmmm128];
    VpmullwYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyLow(Word)), Word, Memory, L256, [EVEX_Vpmullw_ymm_k1z_ymm_ymmm256];
    VpmullwYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::MultiplyLow(Word)), Word, Register, L256, [EVEX_Vpmullw_ymm_k1z_ymm_ymmm256];
    VpmullwZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::MultiplyLow(Word)), Word, Memory, L512, [EVEX_Vpmullw_zmm_k1z_zmm_zmmm512];
    VpmullwZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::MultiplyLow(Word)), Word, Register, L512, [EVEX_Vpmullw_zmm_k1z_zmm_zmmm512];
    VpsadbwXmmu16Xmmu8Memu8Avx512, Lanes(LaneOperation::SumAbsoluteDifferences), Quadword, Memory, L128, [EVEX_Vpsadbw_xmm_xmm_xmmm128];
    VpsadbwXmmu16Xmmu8Xmmu8Avx512, Lanes(LaneOperation::SumAbsoluteDifferences), Quadword, Register, L128, [EVEX_Vpsadbw_xmm_xmm_xmmm128];
    VpsadbwYmmu16Ymmu8Memu8Avx512, Lanes(LaneOperation::SumAbsoluteDifferences), Quadword, Memory, L256, [EVEX_Vpsadbw_ymm_ymm_ymmm256];
    VpsadbwYmmu16Ymmu8Ymmu8Avx512, Lanes(LaneOperation::SumAbsoluteDifferences), Quadword, Register, L256, [EVEX_Vpsadbw_ymm_ymm_ymmm256];
    VpsadbwZmmu16Zmmu8Memu8Avx512, Lanes(LaneOperation::SumAbsoluteDifferences), Quadword, Memory, L512, [EVEX_Vpsadbw_zmm_zmm_zmmm512];
    VpsadbwZmmu16Zmmu8Zmmu8Avx512, Lanes(LaneOperation::SumAbsoluteDifferences), Quadword, Register, L512, [EVEX_Vpsadbw_zmm_zmm_zmmm512];
    VpslldqXmmu8Memu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesLeft), Quadword, Memory, L128, [EVEX_Vpslldq_xmm_xmmm128_imm8];
    VpslldqXmmu8Xmmu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesLeft), Quadword, Register, L128, [EVEX_Vpslldq_xmm_xmmm128_imm8];
    VpslldqYmmu8Memu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesLeft), Quadword, Memory, L256, [EVEX_Vpslldq_ymm_ymmm256_imm8];
    VpslldqYmmu8Ymmu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesLeft), Quadword, Register, L256, [EVEX_Vpslldq_ymm_ymmm256_imm8];
    VpslldqZmmu8Memu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesLeft), Quadword, Memory, L512, [EVEX_Vpslldq_zmm_zmmm512_imm8];
    VpslldqZmmu8Zmmu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesLeft), Quadword, Register, L512, [EVEX_Vpslldq_zmm_zmmm512_imm8];
    VpsllvwXmmu16MaskmskwXmmu16Memu16Avx512, ShiftByLane(LaneShift::Left), Word, Memory, L128, [EVEX_Vpsllvw_xmm_k1z_xmm_xmmm128];
    VpsllvwXmmu16MaskmskwXmmu16Xmmu16Avx512, ShiftByLane(LaneShift::Left), Word, Register, L128, [EVEX_Vpsllvw_xmm_k1z_xmm_xmmm128];
    VpsllvwYmmu16MaskmskwYmmu16Memu16Avx512, ShiftByLane(LaneShift::Left), Word, Memory, L256, [EVEX_Vpsllvw_ymm_k1z_ymm_ymmm256];
    VpsllvwYmmu16MaskmskwYmmu16Ymmu16Avx512, ShiftByLane(LaneShift::Left), Word, Register, L256, [EVEX_Vpsllvw_ymm_k1z_ymm_ymmm256];
    VpsllvwZmmu16MaskmskwZmmu16Memu16Avx512, ShiftByLane(LaneShift::Left), Word, Memory, L512, [EVEX_Vpsllvw_zmm_k1z_zmm_zmmm512];
    VpsllvwZmmu16MaskmskwZmmu16Zmmu16Avx512, ShiftByLane(LaneShift::Left), Word, Register, L512, [EVEX_Vpsllvw_zmm_k1z_zmm_zmmm512];
    VpsllwXmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::Left), Word, Memory, L128, [EVEX_Vpsllw_xmm_k1z_xmmm128_imm8];
    VpsllwXmmu16MaskmskwXmmu16Imm8Avx512, ShiftByImmediate(LaneShift::Left), Word, Register, L128, [EVEX_Vpsllw_xmm_k1z_xmmm128_imm8];
    VpsllwXmmu16MaskmskwXmmu16Memu16Avx512, ShiftByCount(LaneShift::Left), Word, Memory, L128, [EVEX_Vpsllw_xmm_k1z_xmm_xmmm128];
    VpsllwXmmu16MaskmskwXmmu16Xmmu16Avx512, ShiftByCount(LaneShift::Left), Word, Register, L128, [EVEX_Vpsllw_xmm_k1z_xmm_xmmm128];
    VpsllwYmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::Left), Word, Memory, L256, [EVEX_Vpsllw_ymm_k1z_ymmm256_imm8];
    VpsllwYmmu16MaskmskwYmmu16Imm8Avx512, ShiftByImmediate(LaneShift::Left), Word, Register, L256, [EVEX_Vpsllw_ymm_k1z_ymmm256_imm8];
    VpsllwYmmu16MaskmskwYmmu16Memu16Avx512, ShiftByCount(LaneShift::Left), Word, Memory, L256, [EVEX_Vpsllw_ymm_k1z_ymm_xmmm128];
    VpsllwYmmu16MaskmskwYmmu16Xmmu16Avx512, ShiftByCount(LaneShift::Left), Word, Register, L256, [EVEX_Vpsllw_ymm_k1z_ymm_xmmm128];
    VpsllwZmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::Left), Word, Memory, L512, [EVEX_Vpsllw_zmm_k1z_zmmm512_imm8];
    VpsllwZmmu16MaskmskwZmmu16Imm8Avx512, ShiftByImmediate(LaneShift::Left), Word, Register, L512, [EVEX_Vpsllw_zmm_k1z_zmmm512_imm8];
    VpsllwZmmu16MaskmskwZmmu16Memu16Avx512, ShiftByCount(LaneShift::Left), Word, Memory, L512, [EVEX_Vpsllw_zmm_k1z_zmm_xmmm128];
    VpsllwZmmu16MaskmskwZmmu16Xmmu16Avx512, ShiftByCount(LaneShift::Left), Word, Register, L512, [EVEX_Vpsllw_zmm_k1z_zmm_xmmm128];
    VpsravwXmmu16MaskmskwXmmu16Memu16Avx512, ShiftByLane(LaneShift::RightArithmetic), Word, Memory, L128, [EVEX_Vpsravw_xmm_k1z_xmm_xmmm128];
    VpsravwXmmu16MaskmskwXmmu16Xmmu16Avx512, ShiftByLane(LaneShift::RightArithmetic), Word, Register, L128, [EVEX_Vpsravw_xmm_k1z_xmm_xmmm128];
    VpsravwYmmu16MaskmskwYmmu16Memu16Avx512, ShiftByLane(LaneShift::RightArithmetic), Word, Memory, L256, [EVEX_Vpsravw_ymm_k1z_ymm_ymmm256];
    VpsravwYmmu16MaskmskwYmmu16Ymmu16Avx512, ShiftByLane(LaneShift::RightArithmetic), Word, Register, L256, [EVEX_Vpsravw_ymm_k1z_ymm_ymmm256];
    VpsravwZmmu16MaskmskwZmmu16Memu16Avx512, ShiftByLane(LaneShift::RightArithmetic), Word, Memory, L512, [EVEX_Vpsravw_zmm_k1z_zmm_zmmm512];
    VpsravwZmmu16MaskmskwZmmu16Zmmu16Avx512, ShiftByLane(LaneShift::RightArithmetic), Word, Register, L512, [EVEX_Vpsravw_zmm_k1z_zmm_zmmm512];
    VpsrawXmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Word, Memory, L128, [EVEX_Vpsraw_xmm_k1z_xmmm128_imm8];
    VpsrawXmmu16MaskmskwXmmu16Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Word, Register, L128, [EVEX_Vpsraw_xmm_k1z_xmmm128_imm8];
    VpsrawXmmu16MaskmskwXmmu16Memu16Avx512, ShiftByCount(LaneShift::RightArithmetic), Word, Memory, L128, [EVEX_Vpsraw_xmm_k1z_xmm_xmmm128];
    VpsrawXmmu16MaskmskwXmmu16Xmmu16Avx512, ShiftByCount(LaneShift::RightArithmetic), Word, Register, L128, [EVEX_Vpsraw_xmm_k1z_xmm_xmmm128];
    VpsrawYmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Word, Memory, L256, [EVEX_Vpsraw_ymm_k1z_ymmm256_imm8];
    VpsrawYmmu16MaskmskwYmmu16Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Word, Register, L256, [EVEX_Vpsraw_ymm_k1z_ymmm256_imm8];
    VpsrawYmmu16MaskmskwYmmu16Memu16Avx512, ShiftByCount(LaneShift::RightArithmetic), Word, Memory, L256, [EVEX_Vpsraw_ymm_k1z_ymm_xmmm128];
    VpsrawYmmu16MaskmskwYmmu16Xmmu16Avx512, ShiftByCount(LaneShift::RightArithmetic), Word, Register, L256, [EVEX_Vpsraw_ymm_k1z_ymm_xmmm128];
    VpsrawZmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Word, Memory, L512, [EVEX_Vpsraw_zmm_k1z_zmmm512_imm8];
    VpsrawZmmu16MaskmskwZmmu16Imm8Avx512, ShiftByImmediate(LaneShift::RightArithmetic), Word, Register, L512, [EVEX_Vpsraw_zmm_k1z_zmmm512_imm8];
    VpsrawZmmu16MaskmskwZmmu16Memu16Avx512, ShiftByCount(LaneShift::RightArithmetic), Word, Memory, L512, [EVEX_Vpsraw_zmm_k1z_zmm_xmmm128];
    VpsrawZmmu16MaskmskwZmmu16Xmmu16Avx512, ShiftByCount(LaneShift::RightArithmetic), Word, Register, L512, [EVEX_Vpsraw_zmm_k1z_zmm_xmmm128];
    VpsrldqXmmu8Memu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesRight), Quadword, Memory, L128, [EVEX_Vpsrldq_xmm_xmmm128_imm8];
    VpsrldqXmmu8Xmmu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesRight), Quadword, Register, L128, [EVEX_Vpsrldq_xmm_xmmm128_imm8];
    VpsrldqYmmu8Memu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesRight), Quadword, Memory, L256, [EVEX_Vpsrldq_ymm_ymmm256_imm8];
    VpsrldqYmmu8Ymmu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesRight), Quadword, Register, L256, [EVEX_Vpsrldq_ymm_ymmm256_imm8];
    VpsrldqZmmu8Memu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesRight), Quadword, Memory, L512, [EVEX_Vpsrldq_zmm_zmmm512_imm8];
    VpsrldqZmmu8Zmmu8Imm8Avx512, Lanes(LaneOperation::ShiftBytesRight), Quadword, Register, L512, [EVEX_Vpsrldq_zmm_zmmm512_imm8];
    VpsrlvwXmmu16MaskmskwXmmu16Memu16Avx512, ShiftByLane(LaneShift::RightLogical), Word, Memory, L128, [EVEX_Vpsrlvw_xmm_k1z_xmm_xmmm128];
    VpsrlvwXmmu16MaskmskwXmmu16Xmmu16Avx512, ShiftByLane(LaneShift::RightLogical), Word, Register, L128, [EVEX_Vpsrlvw_xmm_k1z_xmm_xmmm128];
    VpsrlvwYmmu16MaskmskwYmmu16Memu16Avx512, ShiftByLane(LaneShift::RightLogical), Word, Memory, L256, [EVEX_Vpsrlvw_ymm_k1z_ymm_ymmm256];
    VpsrlvwYmmu16MaskmskwYmmu16Ymmu16Avx512, ShiftByLane(LaneShift::RightLogical), Word, Register, L256, [EVEX_Vpsrlvw_ymm_k1z_ymm_ymmm256];
    VpsrlvwZmmu16MaskmskwZmmu16Memu16Avx512, ShiftByLane(LaneShift::RightLogical), Word, Memory, L512, [EVEX_Vpsrlvw_zmm_k1z_zmm_zmmm512];
    VpsrlvwZmmu16MaskmskwZmmu16Zmmu16Avx512, ShiftByLane(LaneShift::RightLogical), Word, Register, L512, [EVEX_Vpsrlvw_zmm_k1z_zmm_zmmm512];
    VpsrlwXmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Word, Memory, L128, [EVEX_Vpsrlw_xmm_k1z_xmmm128_imm8];
    VpsrlwXmmu16MaskmskwXmmu16Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Word, Register, L128, [EVEX_Vpsrlw_xmm_k1z_xmmm128_imm8];
    VpsrlwXmmu16MaskmskwXmmu16Memu16Avx512, ShiftByCount(LaneShift::RightLogical), Word, Memory, L128, [EVEX_Vpsrlw_xmm_k1z_xmm_xmmm128];
    VpsrlwXmmu16MaskmskwXmmu16Xmmu16Avx512, ShiftByCount(LaneShift::RightLogical), Word, Register, L128, [EVEX_Vpsrlw_xmm_k1z_xmm_xmmm128];
    VpsrlwYmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Word, Memory, L256, [EVEX_Vpsrlw_ymm_k1z_ymmm256_imm8];
    VpsrlwYmmu16MaskmskwYmmu16Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Word, Register, L256, [EVEX_Vpsrlw_ymm_k1z_ymmm256_imm8];
    VpsrlwYmmu16MaskmskwYmmu16Memu16Avx512, ShiftByCount(LaneShift::RightLogical), Word, Memory, L256, [EVEX_Vpsrlw_ymm_k1z_ymm_xmmm128];
    VpsrlwYmmu16MaskmskwYmmu16Xmmu16Avx512, ShiftByCount(LaneShift::RightLogical), Word, Register, L256, [EVEX_Vpsrlw_ymm_k1z_ymm_xmmm128];
    VpsrlwZmmu16MaskmskwMemu16Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Word, Memory, L512, [EVEX_Vpsrlw_zmm_k1z_zmmm512_imm8];
    VpsrlwZmmu16MaskmskwZmmu16Imm8Avx512, ShiftByImmediate(LaneShift::RightLogical), Word, Register, L512, [EVEX_Vpsrlw_zmm_k1z_zmmm512_imm8];
    VpsrlwZmmu16MaskmskwZmmu16Memu16Avx512, ShiftByCount(LaneShift::RightLogical), Word, Memory, L512, [EVEX_Vpsrlw_zmm_k1z_zmm_xmmm128];
    VpsrlwZmmu16MaskmskwZmmu16Xmmu16Avx512, ShiftByCount(LaneShift::RightLogical), Word, Register, L512, [EVEX_Vpsrlw_zmm_k1z_zmm_xmmm128];
    VpsubbXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::Subtract(Byte)), Byte, Memory, L128, [EVEX_Vpsubb_xmm_k1z_xmm_xmmm128];
    VpsubbXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::Subtract(Byte)), Byte, Register, L128, [EVEX_Vpsubb_xmm_k1z_xmm_xmmm128];
    VpsubbYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::Subtract(Byte)), Byte, Memory, L256, [EVEX_Vpsubb_ymm_k1z_ymm_ymmm256];
    VpsubbYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::Subtract(Byte)), Byte, Register, L256, [EVEX_Vpsubb_ymm_k1z_ymm_ymmm256];
    VpsubbZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::Subtract(Byte)), Byte, Memory, L512, [EVEX_Vpsubb_zmm_k1z_zmm_zmmm512];
    VpsubbZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::Subtract(Byte)), Byte, Register, L512, [EVEX_Vpsubb_zmm_k1z_zmm_zmmm512];
    VpsubsbXmmi8MaskmskwXmmi8Memi8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Signed)), Byte, Memory, L128, [EVEX_Vpsubsb_xmm_k1z_xmm_xmmm128];
    VpsubsbXmmi8MaskmskwXmmi8Xmmi8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Signed)), Byte, Register, L128, [EVEX_Vpsubsb_xmm_k1z_xmm_xmmm128];
    VpsubsbYmmi8MaskmskwYmmi8Memi8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Signed)), Byte, Memory, L256, [EVEX_Vpsubsb_ymm_k1z_ymm_ymmm256];
    VpsubsbYmmi8MaskmskwYmmi8Ymmi8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Signed)), Byte, Register, L256, [EVEX_Vpsubsb_ymm_k1z_ymm_ymmm256];
    VpsubsbZmmi8MaskmskwZmmi8Memi8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Signed)), Byte, Memory, L512, [EVEX_Vpsubsb_zmm_k1z_zmm_zmmm512];
    VpsubsbZmmi8MaskmskwZmmi8Zmmi8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Signed)), Byte, Register, L512, [EVEX_Vpsubsb_zmm_k1z_zmm_zmmm512];
    VpsubswXmmi16MaskmskwXmmi16Memi16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Signed)), Word, Memory, L128, [EVEX_Vpsubsw_xmm_k1z_xmm_xmmm128];
    VpsubswXmmi16MaskmskwXmmi16Xmmi16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Signed)), Word, Register, L128, [EVEX_Vpsubsw_xmm_k1z_xmm_xmmm128];
    VpsubswYmmi16MaskmskwYmmi16Memi16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Signed)), Word, Memory, L256, [EVEX_Vpsubsw_ymm_k1z_ymm_ymmm256];
    VpsubswYmmi16MaskmskwYmmi16Ymmi16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Signed)), Word, Register, L256, [EVEX_Vpsubsw_ymm_k1z_ymm_ymmm256];
    VpsubswZmmi16MaskmskwZmmi16Memi16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Signed)), Word, Memory, L512, [EVEX_Vpsubsw_zmm_k1z_zmm_zmmm512];
    VpsubswZmmi16MaskmskwZmmi16Zmmi16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Signed)), Word, Register, L512, [EVEX_Vpsubsw_zmm_k1z_zmm_zmmm512];
    VpsubusbXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Unsigned)), Byte, Memory, L128, [EVEX_Vpsubusb_xmm_k1z_xmm_xmmm128];
    VpsubusbXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Unsigned)), Byte, Register, L128, [EVEX_Vpsubusb_xmm_k1z_xmm_xmmm128];
    VpsubusbYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Unsigned)), Byte, Memory, L256, [EVEX_Vpsubusb_ymm_k1z_ymm_ymmm256];
    VpsubusbYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Unsigned)), Byte, Register, L256, [EVEX_Vpsubusb_ymm_k1z_ymm_ymmm256];
    VpsubusbZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Unsigned)), Byte, Memory, L512, [EVEX_Vpsubusb_zmm_k1z_zmm_zmmm512];
    VpsubusbZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::SubtractSaturating(Byte, Unsigned)), Byte, Register, L512, [EVEX_Vpsubusb_zmm_k1z_zmm_zmmm512];
    VpsubuswXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Unsigned)), Word, Memory, L128, [EVEX_Vpsubusw_xmm_k1z_xmm_xmmm128];
    VpsubuswXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Unsigned)), Word, Register, L128, [EVEX_Vpsubusw_xmm_k1z_xmm_xmmm128];
    VpsubuswYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Unsigned)), Word, Memory, L256, [EVEX_Vpsubusw_ymm_k1z_ymm_ymmm256];
    VpsubuswYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Unsigned)), Word, Register, L256, [EVEX_Vpsubusw_ymm_k1z_ymm_ymmm256];
    VpsubuswZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Unsigned)), Word, Memory, L512, [EVEX_Vpsubusw_zmm_k1z_zmm_zmmm512];
    VpsubuswZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::SubtractSaturating(Word, Unsigned)), Word, Register, L512, [EVEX_Vpsubusw_zmm_k1z_zmm_zmmm512];
    VpsubwXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::Subtract(Word)), Word, Memory, L128, [EVEX_Vpsubw_xmm_k1z_xmm_xmmm128];
    VpsubwXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::Subtract(Word)), Word, Register, L128, [EVEX_Vpsubw_xmm_k1z_xmm_xmmm128];
    VpsubwYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::Subtract(Word)), Word, Memory, L256, [EVEX_Vpsubw_ymm_k1z_ymm_ymmm256];
    VpsubwYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::Subtract(Word)), Word, Register, L256, [EVEX_Vpsubw_ymm_k1z_ymm_ymmm256];
    VpsubwZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::Subtract(Word)), Word, Memory, L512, [EVEX_Vpsubw_zmm_k1z_zmm_zmmm512];
    VpsubwZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::Subtract(Word)), Word, Register, L512, [EVEX_Vpsubw_zmm_k1z_zmm_zmmm512];
    VptestmbMaskmskwMaskmskwXmmu8Memu8Avx512, TestIntoMask { not: false }, Byte, Memory, L128, [EVEX_Vptestmb_kr_k1_xmm_xmmm128];
    VptestmbMaskmskwMaskmskwXmmu8Xmmu8Avx512, TestIntoMask { not: false }, Byte, Register, L128, [EVEX_Vptestmb_kr_k1_xmm_xmmm128];
    VptestmbMaskmskwMaskmskwYmmu8Memu8Avx512, TestIntoMask { not: false }, Byte, Memory, L256, [EVEX_Vptestmb_kr_k1_ymm_ymmm256];
    VptestmbMaskmskwMaskmskwYmmu8Ymmu8Avx512, TestIntoMask { not: false }, Byte, Register, L256, [EVEX_Vptestmb_kr_k1_ymm_ymmm256];
    VptestmbMaskmskwMaskmskwZmmu8Memu8Avx512, TestIntoMask { not: false }, Byte, Memory, L512, [EVEX_Vptestmb_kr_k1_zmm_zmmm512];
    VptestmbMaskmskwMaskmskwZmmu8Zmmu8Avx512, TestIntoMask { not: false }, Byte, Register, L512, [EVEX_Vptestmb_kr_k1_zmm_zmmm512];
    VptestmwMaskmskwMaskmskwXmmu16Memu16Avx512, TestIntoMask { not: false }, Word, Memory, L128, [EVEX_Vptestmw_kr_k1_xmm_xmmm128];
    VptestmwMaskmskwMaskmskwXmmu16Xmmu16Avx512, TestIntoMask { not: false }, Word, Register, L128, [EVEX_Vptestmw_kr_k1_xmm_xmmm128];
    VptestmwMaskmskwMaskmskwYmmu16Memu16Avx512, TestIntoMask { not: false }, Word, Memory, L256, [EVEX_Vptestmw_kr_k1_ymm_ymmm256];
    VptestmwMaskmskwMaskmskwYmmu16Ymmu16Avx512, TestIntoMask { not: false }, Word, Register, L256, [EVEX_Vptestmw_kr_k1_ymm_ymmm256];
    VptestmwMaskmskwMaskmskwZmmu16Memu16Avx512, TestIntoMask { not: false }, Word, Memory, L512, [EVEX_Vptestmw_kr_k1_zmm_zmmm512];
    VptestmwMaskmskwMaskmskwZmmu16Zmmu16Avx512, TestIntoMask { not: false }, Word, Register, L512, [EVEX_Vptestmw_kr_k1_zmm_zmmm512];
    VptestnmbMaskmskwMaskmskwXmmu8Memu8Avx512, TestIntoMask { not: true }, Byte, Memory, L128, [EVEX_Vptestnmb_kr_k1_xmm_xmmm128];
    VptestnmbMaskmskwMaskmskwXmmu8Xmmu8Avx512, TestIntoMask { not: true }, Byte, Register, L128, [EVEX_Vptestnmb_kr_k1_xmm_xmmm128];
    VptestnmbMaskmskwMaskmskwYmmu8Memu8Avx512, TestIntoMask { not: true }, Byte, Memory, L256, [EVEX_Vptestnmb_kr_k1_ymm_ymmm256];
    VptestnmbMaskmskwMaskmskwYmmu8Ymmu8Avx512, TestIntoMask { not: true }, Byte, Register, L256, [EVEX_Vptestnmb_kr_k1_ymm_ymmm256];
    VptestnmbMaskmskwMaskmskwZmmu8Memu8Avx512, TestIntoMask { not: true }, Byte, Memory, L512, [EVEX_Vptestnmb_kr_k1_zmm_zmmm512];
    VptestnmbMaskmskwMaskmskwZmmu8Zmmu8Avx512, TestIntoMask { not: true }, Byte, Register, L512, [EVEX_Vptestnmb_kr_k1_zmm_zmmm512];
    VptestnmwMaskmskwMaskmskwXmmu16Memu16Avx512, TestIntoMask { not: true }, Word, Memory, L128, [EVEX_Vptestnmw_kr_k1_xmm_xmmm128];
    VptestnmwMaskmskwMaskmskwXmmu16Xmmu16Avx512, TestIntoMask { not: true }, Word, Register, L128, [EVEX_Vptestnmw_kr_k1_xmm_xmmm128];
    VptestnmwMaskmskwMaskmskwYmmu16Memu16Avx512, TestIntoMask { not: true }, Word, Memory, L256, [EVEX_Vptestnmw_kr_k1_ymm_ymmm256];
    VptestnmwMaskmskwMaskmskwYmmu16Ymmu16Avx512, TestIntoMask { not: true }, Word, Register, L256, [EVEX_Vptestnmw_kr_k1_ymm_ymmm256];
    VptestnmwMaskmskwMaskmskwZmmu16Memu16Avx512, TestIntoMask { not: true }, Word, Memory, L512, [EVEX_Vptestnmw_kr_k1_zmm_zmmm512];
    VptestnmwMaskmskwMaskmskwZmmu16Zmmu16Avx512, TestIntoMask { not: true }, Word, Register, L512, [EVEX_Vptestnmw_kr_k1_zmm_zmmm512];
    VxorpdXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::Xor), Quadword, Memory, L128, [EVEX_Vxorpd_xmm_k1z_xmm_xmmm128b64];
    VxorpdXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::Xor), Quadword, Register, L128, [EVEX_Vxorpd_xmm_k1z_xmm_xmmm128b64];
    VxorpdYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::Xor), Quadword, Memory, L256, [EVEX_Vxorpd_ymm_k1z_ymm_ymmm256b64];
    VxorpdYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::Xor), Quadword, Register, L256, [EVEX_Vxorpd_ymm_k1z_ymm_ymmm256b64];
    VxorpdZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::Xor), Quadword, Memory, L512, [EVEX_Vxorpd_zmm_k1z_zmm_zmmm512b64];
    VxorpdZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::Xor), Quadword, Register, L512, [EVEX_Vxorpd_zmm_k1z_zmm_zmmm512b64];
    VxorpsXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Memory, L128, [EVEX_Vxorps_xmm_k1z_xmm_xmmm128b32];
    VxorpsXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Register, L128, [EVEX_Vxorps_xmm_k1z_xmm_xmmm128b32];
    VxorpsYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Memory, L256, [EVEX_Vxorps_ymm_k1z_ymm_ymmm256b32];
    VxorpsYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Register, L256, [EVEX_Vxorps_ymm_k1z_ymm_ymmm256b32];
    VxorpsZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Memory, L512, [EVEX_Vxorps_zmm_k1z_zmm_zmmm512b32];
    VxorpsZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Xor), Doubleword, Register, L512, [EVEX_Vxorps_zmm_k1z_zmm_zmmm512b32];
    // Packs and unpacks.
    VpackssdwXmmi16MaskmskwXmmi32Memi32Avx512, Lanes(LaneOperation::Pack(Doubleword, Signed)), Word, Memory, L128, [EVEX_Vpackssdw_xmm_k1z_xmm_xmmm128b32];
    VpackssdwXmmi16MaskmskwXmmi32Xmmi32Avx512, Lanes(LaneOperation::Pack(Doubleword, Signed)), Word, Register, L128, [EVEX_Vpackssdw_xmm_k1z_xmm_xmmm128b32];
    VpackssdwYmmi16MaskmskwYmmi32Memi32Avx512, Lanes(LaneOperation::Pack(Doubleword, Signed)), Word, Memory, L256, [EVEX_Vpackssdw_ymm_k1z_ymm_ymmm256b32];
    VpackssdwYmmi16MaskmskwYmmi32Ymmi32Avx512, Lanes(LaneOperation::Pack(Doubleword, Signed)), Word, Register, L256, [EVEX_Vpackssdw_ymm_k1z_ymm_ymmm256b32];
    VpackssdwZmmi16MaskmskwZmmi32Memi32Avx512, Lanes(LaneOperation::Pack(Doubleword, Signed)), Word, Memory, L512, [EVEX_Vpackssdw_zmm_k1z_zmm_zmmm512b32];
    VpackssdwZmmi16MaskmskwZmmi32Zmmi32Avx512, Lanes(LaneOperation::Pack(Doubleword, Signed)), Word, Register, L512, [EVEX_Vpackssdw_zmm_k1z_zmm_zmmm512b32];
    VpacksswbXmmi8MaskmskwXmmi16Memi16Avx512, Lanes(LaneOperation::Pack(Word, Signed)), Byte, Memory, L128, [EVEX_Vpacksswb_xmm_k1z_xmm_xmmm128];
    VpacksswbXmmi8MaskmskwXmmi16Xmmi16Avx512, Lanes(LaneOperation::Pack(Word, Signed)), Byte, Register, L128, [EVEX_Vpacksswb_xmm_k1z_xmm_xmmm128];
    VpacksswbYmmi8MaskmskwYmmi16Memi16Avx512, Lanes(LaneOperation::Pack(Word, Signed)), Byte, Memory, L256, [EVEX_Vpacksswb_ymm_k1z_ymm_ymmm256];
    VpacksswbYmmi8MaskmskwYmmi16Ymmi16Avx512, Lanes(LaneOperation::Pack(Word, Signed)), Byte, Register, L256, [EVEX_Vpacksswb_ymm_k1z_ymm_ymmm256];
    VpacksswbZmmi8MaskmskwZmmi16Memi16Avx512, Lanes(LaneOperation::Pack(Word, Signed)), Byte, Memory, L512, [EVEX_Vpacksswb_zmm_k1z_zmm_zmmm512];
    VpacksswbZmmi8MaskmskwZmmi16Zmmi16Avx512, Lanes(LaneOperation::Pack(Word, Signed)), Byte, Register, L512, [EVEX_Vpacksswb_zmm_k1z_zmm_zmmm512];
    VpackusdwXmmu16MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::Pack(Doubleword, Unsigned)), Word, Memory, L128, [EVEX_Vpackusdw_xmm_k1z_xmm_xmmm128b32];
    VpackusdwXmmu16MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::Pack(Doubleword, Unsigned)), Word, Register, L128, [EVEX_Vpackusdw_xmm_k1z_xmm_xmmm128b32];
    VpackusdwYmmu16MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::Pack(Doubleword, Unsigned)), Word, Memory, L256, [EVEX_Vpackusdw_ymm_k1z_ymm_ymmm256b32];
    VpackusdwYmmu16MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::Pack(Doubleword, Unsigned)), Word, Register, L256, [EVEX_Vpackusdw_ymm_k1z_ymm_ymmm256b32];
    VpackusdwZmmu16MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::Pack(Doubleword, Unsigned)), Word, Memory, L512, [EVEX_Vpackusdw_zmm_k1z_zmm_zmmm512b32];
    VpackusdwZmmu16MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::Pack(Doubleword, Unsigned)), Word, Register, L512, [EVEX_Vpackusdw_zmm_k1z_zmm_zmmm512b32];
    VpackuswbXmmu8MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::Pack(Word, Unsigned)), Byte, Memory, L128, [EVEX_Vpackuswb_xmm_k1z_xmm_xmmm128];
    VpackuswbXmmu8MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::Pack(Word, Unsigned)), Byte, Register, L128, [EVEX_Vpackuswb_xmm_k1z_xmm_xmmm128];
    VpackuswbYmmu8MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::Pack(Word, Unsigned)), Byte, Memory, L256, [EVEX_Vpackuswb_ymm_k1z_ymm_ymmm256];
    VpackuswbYmmu8MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::Pack(Word, Unsigned)), Byte, Register, L256, [EVEX_Vpackuswb_ymm_k1z_ymm_ymmm256];
    VpackuswbZmmu8MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::Pack(Word, Unsigned)), Byte, Memory, L512, [EVEX_Vpackuswb_zmm_k1z_zmm_zmmm512];
    VpackuswbZmmu8MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::Pack(Word, Unsigned)), Byte, Register, L512, [EVEX_Vpackuswb_zmm_k1z_zmm_zmmm512];
    VpunpckhbwXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::UnpackHigh(Byte)), Byte, Memory, L128, [EVEX_Vpunpckhbw_xmm_k1z_xmm_xmmm128];
    VpunpckhbwXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::UnpackHigh(Byte)), Byte, Register, L128, [EVEX_Vpunpckhbw_xmm_k1z_xmm_xmmm128];
    VpunpckhbwYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::UnpackHigh(Byte)), Byte, Memory, L256, [EVEX_Vpunpckhbw_ymm_k1z_ymm_ymmm256];
    VpunpckhbwYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::UnpackHigh(Byte)), Byte, Register, L256, [EVEX_Vpunpckhbw_ymm_k1z_ymm_ymmm256];
    VpunpckhbwZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::UnpackHigh(Byte)), Byte, Memory, L512, [EVEX_Vpunpckhbw_zmm_k1z_zmm_zmmm512];
    VpunpckhbwZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::UnpackHigh(Byte)), Byte, Register, L512, [EVEX_Vpunpckhbw_zmm_k1z_zmm_zmmm512];
    VpunpckhdqXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::UnpackHigh(Doubleword)), Doubleword, Memory, L128, [EVEX_Vpunpckhdq_xmm_k1z_xmm_xmmm128b32];
    VpunpckhdqXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::UnpackHigh(Doubleword)), Doubleword, Register, L128, [EVEX_Vpunpckhdq_xmm_k1z_xmm_xmmm128b32];
    VpunpckhdqYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::UnpackHigh(Doubleword)), Doubleword, Memory, L256, [EVEX_Vpunpckhdq_ymm_k1z_ymm_ymmm256b32];
    VpunpckhdqYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::UnpackHigh(Doubleword)), Doubleword, Register, L256, [EVEX_Vpunpckhdq_ymm_k1z_ymm_ymmm256b32];
    VpunpckhdqZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::UnpackHigh(Doubleword)), Doubleword, Memory, L512, [EVEX_Vpunpckhdq_zmm_k1z_zmm_zmmm512b32];
    VpunpckhdqZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::UnpackHigh(Doubleword)), Doubleword, Register, L512, [EVEX_Vpunpckhdq_zmm_k1z_zmm_zmmm512b32];
    VpunpckhqdqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::UnpackHigh(Quadword)), Quadword, Memory, L128, [EVEX_Vpunpckhqdq_xmm_k1z_xmm_xmmm128b64];
    VpunpckhqdqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::UnpackHigh(Quadword)), Quadword, Register, L128, [EVEX_Vpunpckhqdq_xmm_k1z_xmm_xmmm128b64];
    VpunpckhqdqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::UnpackHigh(Quadword)), Quadword, Memory, L256, [EVEX_Vpunpckhqdq_ymm_k1z_ymm_ymmm256b64];
    VpunpckhqdqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::UnpackHigh(Quadword)), Quadword, Register, L256, [EVEX_Vpunpckhqdq_ymm_k1z_ymm_ymmm256b64];
    VpunpckhqdqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::UnpackHigh(Quadword)), Quadword, Memory, L512, [EVEX_Vpunpckhqdq_zmm_k1z_zmm_zmmm512b64];
    VpunpckhqdqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::UnpackHigh(Quadword)), Quadword, Register, L512, [EVEX_Vpunpckhqdq_zmm_k1z_zmm_zmmm512b64];
    VpunpckhwdXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::UnpackHigh(Word)), Word, Memory, L128, [EVEX_Vpunpckhwd_xmm_k1z_xmm_xmmm128];
    VpunpckhwdXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::UnpackHigh(Word)), Word, Register, L128, [EVEX_Vpunpckhwd_xmm_k1z_xmm_xmmm128];
    VpunpckhwdYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::UnpackHigh(Word)), Word, Memory, L256, [EVEX_Vpunpckhwd_ymm_k1z_ymm_ymmm256];
    VpunpckhwdYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::UnpackHigh(Word)), Word, Register, L256, [EVEX_Vpunpckhwd_ymm_k1z_ymm_ymmm256];
    VpunpckhwdZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::UnpackHigh(Word)), Word, Memory, L512, [EVEX_Vpunpckhwd_zmm_k1z_zmm_zmmm512];
    VpunpckhwdZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::UnpackHigh(Word)), Word, Register, L512, [EVEX_Vpunpckhwd_zmm_k1z_zmm_zmmm512];
    VpunpcklbwXmmu8MaskmskwXmmu8Memu8Avx512, Lanes(LaneOperation::UnpackLow(Byte)), Byte, Memory, L128, [EVEX_Vpunpcklbw_xmm_k1z_xmm_xmmm128];
    VpunpcklbwXmmu8MaskmskwXmmu8Xmmu8Avx512, Lanes(LaneOperation::UnpackLow(Byte)), Byte, Register, L128, [EVEX_Vpunpcklbw_xmm_k1z_xmm_xmmm128];
    VpunpcklbwYmmu8MaskmskwYmmu8Memu8Avx512, Lanes(LaneOperation::UnpackLow(Byte)), Byte, Memory, L256, [EVEX_Vpunpcklbw_ymm_k1z_ymm_ymmm256];
    VpunpcklbwYmmu8MaskmskwYmmu8Ymmu8Avx512, Lanes(LaneOperation::UnpackLow(Byte)), Byte, Register, L256, [EVEX_Vpunpcklbw_ymm_k1z_ymm_ymmm256];
    VpunpcklbwZmmu8MaskmskwZmmu8Memu8Avx512, Lanes(LaneOperation::UnpackLow(Byte)), Byte, Memory, L512, [EVEX_Vpunpcklbw_zmm_k1z_zmm_zmmm512];
    VpunpcklbwZmmu8MaskmskwZmmu8Zmmu8Avx512, Lanes(LaneOperation::UnpackLow(Byte)), Byte, Register, L512, [EVEX_Vpunpcklbw_zmm_k1z_zmm_zmmm512];
    VpunpckldqXmmu32MaskmskwXmmu32Memu32Avx512, Lanes(LaneOperation::UnpackLow(Doubleword)), Doubleword, Memory, L128, [EVEX_Vpunpckldq_xmm_k1z_xmm_xmmm128b32];
    VpunpckldqXmmu32MaskmskwXmmu32Xmmu32Avx512, Lanes(LaneOperation::UnpackLow(Doubleword)), Doubleword, Register, L128, [EVEX_Vpunpckldq_xmm_k1z_xmm_xmmm128b32];
    VpunpckldqYmmu32MaskmskwYmmu32Memu32Avx512, Lanes(LaneOperation::UnpackLow(Doubleword)), Doubleword, Memory, L256, [EVEX_Vpunpckldq_ymm_k1z_ymm_ymmm256b32];
    VpunpckldqYmmu32MaskmskwYmmu32Ymmu32Avx512, Lanes(LaneOperation::UnpackLow(Doubleword)), Doubleword, Register, L256, [EVEX_Vpunpckldq_ymm_k1z_ymm_ymmm256b32];
    VpunpckldqZmmu32MaskmskwZmmu32Memu32Avx512, Lanes(LaneOperation::UnpackLow(Doubleword)), Doubleword, Memory, L512, [EVEX_Vpunpckldq_zmm_k1z_zmm_zmmm512b32];
    VpunpckldqZmmu32MaskmskwZmmu32Zmmu32Avx512, Lanes(LaneOperation::UnpackLow(Doubleword)), Doubleword, Register, L512, [EVEX_Vpunpckldq_zmm_k1z_zmm_zmmm512b32];
    VpunpcklqdqXmmu64MaskmskwXmmu64Memu64Avx512, Lanes(LaneOperation::UnpackLow(Quadword)), Quadword, Memory, L128, [EVEX_Vpunpcklqdq_xmm_k1z_xmm_xmmm128b64];
    VpunpcklqdqXmmu64MaskmskwXmmu64Xmmu64Avx512, Lanes(LaneOperation::UnpackLow(Quadword)), Quadword, Register, L128, [EVEX_Vpunpcklqdq_xmm_k1z_xmm_xmmm128b64];
    VpunpcklqdqYmmu64MaskmskwYmmu64Memu64Avx512, Lanes(LaneOperation::UnpackLow(Quadword)), Quadword, Memory, L256, [EVEX_Vpunpcklqdq_ymm_k1z_ymm_ymmm256b64];
    VpunpcklqdqYmmu64MaskmskwYmmu64Ymmu64Avx512, Lanes(LaneOperation::UnpackLow(Quadword)), Quadword, Register, L256, [EVEX_Vpunpcklqdq_ymm_k1z_ymm_ymmm256b64];
    VpunpcklqdqZmmu64MaskmskwZmmu64Memu64Avx512, Lanes(LaneOperation::UnpackLow(Quadword)), Quadword, Memory, L512, [EVEX_Vpunpcklqdq_zmm_k1z_zmm_zmmm512b64];
    VpunpcklqdqZmmu64MaskmskwZmmu64Zmmu64Avx512, Lanes(LaneOperation::UnpackLow(Quadword)), Quadword, Register, L512, [EVEX_Vpunpcklqdq_zmm_k1z_zmm_zmmm512b64];
    VpunpcklwdXmmu16MaskmskwXmmu16Memu16Avx512, Lanes(LaneOperation::UnpackLow(Word)), Word, Memory, L128, [EVEX_Vpunpcklwd_xmm_k1z_xmm_xmmm128];
    VpunpcklwdXmmu16MaskmskwXmmu16Xmmu16Avx512, Lanes(LaneOperation::UnpackLow(Word)), Word, Register, L128, [EVEX_Vpunpcklwd_xmm_k1z_xmm_xmmm128];
    VpunpcklwdYmmu16MaskmskwYmmu16Memu16Avx512, Lanes(LaneOperation::UnpackLow(Word)), Word, Memory, L256, [EVEX_Vpunpcklwd_ymm_k1z_ymm_ymmm256];
    VpunpcklwdYmmu16MaskmskwYmmu16Ymmu16Avx512, Lanes(LaneOperation::UnpackLow(Word)), Word, Register, L256, [EVEX_Vpunpcklwd_ymm_k1z_ymm_ymmm256];
    VpunpcklwdZmmu16MaskmskwZmmu16Memu16Avx512, Lanes(LaneOperation::UnpackLow(Word)), Word, Memory, L512, [EVEX_Vpunpcklwd_zmm_k1z_zmm_zmmm512];
    VpunpcklwdZmmu16MaskmskwZmmu16Zmmu16Avx512, Lanes(LaneOperation::UnpackLow(Word)), Word, Register, L512, [EVEX_Vpunpcklwd_zmm_k1z_zmm_zmmm512];
    // Broadcasts.
    Vbroadcastf32x2Ymmf32MaskmskwMemf32Avx512, BroadcastTuple(2), Doubleword, Memory, L256, [EVEX_Vbroadcastf32x2_ymm_k1z_xmmm64];
    Vbroadcastf32x2Ymmf32MaskmskwXmmf32Avx512, BroadcastTuple(2), Doubleword, Register, L256, [EVEX_Vbroadcastf32x2_ymm_k1z_xmmm64];
    Vbroadcastf32x2Zmmf32MaskmskwMemf32Avx512, BroadcastTuple(2), Doubleword, Memory, L512, [EVEX_Vbroadcastf32x2_zmm_k1z_xmmm64];
    Vbroadcastf32x2Zmmf32MaskmskwXmmf32Avx512, BroadcastTuple(2), Doubleword, Register, L512, [EVEX_Vbroadcastf32x2_zmm_k1z_xmmm64];
    Vbroadcastf32x4Ymmf32MaskmskwMemf32Avx512, BroadcastTuple(4), Doubleword, Memory, L256, [EVEX_Vbroadcastf32x4_ymm_k1z_m128];
    Vbroadcastf32x4Zmmf32MaskmskwMemf32Avx512, BroadcastTuple(4), Doubleword, Memory, L512, [EVEX_Vbroadcastf32x4_zmm_k1z_m128];
    Vbroadcastf32x8Zmmf32MaskmskwMemf32Avx512, BroadcastTuple(8), Doubleword, Memory, L512, [EVEX_Vbroadcastf32x8_zmm_k1z_m256];
    Vbroadcastf64x2Ymmf64MaskmskwMemf64Avx512, BroadcastTuple(2), Quadword, Memory, L256, [EVEX_Vbroadcastf64x2_ymm_k1z_m128];
    Vbroadcastf64x2Zmmf64MaskmskwMemf64Avx512, BroadcastTuple(2), Quadword, Memory, L512, [EVEX_Vbroadcastf64x2_zmm_k1z_m128];
    Vbroadcastf64x4Zmmf64MaskmskwMemf64Avx512, BroadcastTuple(4), Quadword, Memory, L512, [EVEX_Vbroadcastf64x4_zmm_k1z_m256];
    Vbroadcasti32x2Xmmu32MaskmskwMemu32Avx512, BroadcastTuple(2), Doubleword, Memory, L128, [EVEX_Vbroadcasti32x2_xmm_k1z_xmmm64];
    Vbroadcasti32x2Xmmu32MaskmskwXmmu32Avx512, BroadcastTuple(2), Doubleword, Register, L128, [EVEX_Vbroadcasti32x2_xmm_k1z_xmmm64];
    Vbroadcasti32x2Ymmu32MaskmskwMemu32Avx512, BroadcastTuple(2), Doubleword, Memory, L256, [EVEX_Vbroadcasti32x2_ymm_k1z_xmmm64];
    Vbroadcasti32x2Ymmu32MaskmskwXmmu32Avx512, BroadcastTuple(2), Doubleword, Register, L256, [EVEX_Vbroadcasti32x2_ymm_k1z_xmmm64];
    Vbroadcasti32x2Zmmu32MaskmskwMemu32Avx512, BroadcastTuple(2), Doubleword, Memory, L512, [EVEX_Vbroadcasti32x2_zmm_k1z_xmmm64];
    Vbroadcasti32x2Zmmu32MaskmskwXmmu32Avx512, BroadcastTuple(2), Doubleword, Register, L512, [EVEX_Vbroadcasti32x2_zmm_k1z_xmmm64];
    Vbroadcasti32x8Zmmu32MaskmskwMemu32Avx512, BroadcastTuple(8), Doubleword, Memory, L512, [EVEX_Vbroadcasti32x8_zmm_k1z_m256];
    Vbroadcasti64x2Ymmu64MaskmskwMemu64Avx512, BroadcastTuple(2), Quadword, Memory, L256, [EVEX_Vbroadcasti64x2_ymm_k1z_m128];
    Vbroadcasti64x2Zmmu64MaskmskwMemu64Avx512, BroadcastTuple(2), Quadword, Memory, L512, [EVEX_Vbroadcasti64x2_zmm_k1z_m128];
    VbroadcastsdYmmf64MaskmskwMemf64Avx512, Broadcast, Quadword, Memory, L256, [EVEX_Vbroadcastsd_ymm_k1z_xmmm64];
    VbroadcastsdYmmf64MaskmskwXmmf64Avx512, Broadcast, Quadword, Register, L256, [EVEX_Vbroadcastsd_ymm_k1z_xmmm64];
    VbroadcastsdZmmf64MaskmskwMemf64Avx512, Broadcast, Quadword, Memory, L512, [EVEX_Vbroadcastsd_zmm_k1z_xmmm64];
    VbroadcastsdZmmf64MaskmskwXmmf64Avx512, Broadcast, Quadword, Register, L512, [EVEX_Vbroadcastsd_zmm_k1z_xmmm64];
    VbroadcastssXmmf32MaskmskwMemf32Avx512, Broadcast, Doubleword, Memory, L128, [EVEX_Vbroadcastss_xmm_k1z_xmmm32];
    VbroadcastssXmmf32MaskmskwXmmf32Avx512, Broadcast, Doubleword, Register, L128, [EVEX_Vbroadcastss_xmm_k1z_xmmm32];
    VbroadcastssYmmf32MaskmskwMemf32Avx512, Broadcast, Doubleword, Memory, L256, [EVEX_Vbroadcastss_ymm_k1z_xmmm32];
    VbroadcastssYmmf32MaskmskwXmmf32Avx512, Broadcast, Doubleword, Register, L256, [EVEX_Vbroadcastss_ymm_k1z_xmmm32];
    VbroadcastssZmmf32MaskmskwMemf32Avx512, Broadcast, Doubleword, Memory, L512, [EVEX_Vbroadcastss_zmm_k1z_xmmm32];
    VbroadcastssZmmf32MaskmskwXmmf32Avx512, Broadcast, Doubleword, Register, L512, [EVEX_Vbroadcastss_zmm_k1z_xmmm32];
    VpbroadcastbXmmu8MaskmskwGpr32u8Avx512, Broadcast, Byte, General, L128, [EVEX_Vpbroadcastb_xmm_k1z_r32];
    VpbroadcastbXmmu8MaskmskwMemu8Avx512, Broadcast, Byte, Memory, L128, [EVEX_Vpbroadcastb_xmm_k1z_xmmm8];
    VpbroadcastbXmmu8MaskmskwXmmu8Avx512, Broadcast, Byte, Register, L128, [EVEX_Vpbroadcastb_xmm_k1z_xmmm8];
    VpbroadcastbYmmu8MaskmskwGpr32u8Avx512, Broadcast, Byte, General, L256, [EVEX_Vpbroadcastb_ymm_k1z_r32];
    VpbroadcastbYmmu8MaskmskwMemu8Avx512, Broadcast, Byte, Memory, L256, [EVEX_Vpbroadcastb_ymm_k1z_xmmm8];
    VpbroadcastbYmmu8MaskmskwXmmu8Avx512, Broadcast, Byte, Register, L256, [EVEX_Vpbroadcastb_ymm_k1z_xmmm8];
    VpbroadcastbZmmu8MaskmskwGpr32u8Avx512, Broadcast, Byte, General, L512, [EVEX_Vpbroadcastb_zmm_k1z_r32];
    VpbroadcastbZmmu8MaskmskwMemu8Avx512, Broadcast, Byte, Memory, L512, [EVEX_Vpbroadcastb_zmm_k1z_xmmm8];
    VpbroadcastbZmmu8MaskmskwXmmu8Avx512, Broadcast, Byte, Register, L512, [EVEX_Vpbroadcastb_zmm_k1z_xmmm8];
    VpbroadcastwXmmu16MaskmskwGpr32u16Avx512, Broadcast, Word, General, L128, [EVEX_Vpbroadcastw_xmm_k1z_r32];
    VpbroadcastwXmmu16MaskmskwMemu16Avx512, Broadcast, Word, Memory, L128, [EVEX_Vpbroadcastw_xmm_k1z_xmmm16];
    VpbroadcastwXmmu16MaskmskwXmmu16Avx512, Broadcast, Word, Register, L128, [EVEX_Vpbroadcastw_xmm_k1z_xmmm16];
    VpbroadcastwYmmu16MaskmskwGpr32u16Avx512, Broadcast, Word, General, L256, [EVEX_Vpbroadcastw_ymm_k1z_r32];
    VpbroadcastwYmmu16MaskmskwMemu16Avx512, Broadcast, Word, Memory, L256, [EVEX_Vpbroadcastw_ymm_k1z_xmmm16];
    VpbroadcastwYmmu16MaskmskwXmmu16Avx512, Broadcast, Word, Register, L256, [EVEX_Vpbroadcastw_ymm_k1z_xmmm16];
    VpbroadcastwZmmu16MaskmskwGpr32u16Avx512, Broadcast, Word, General, L512, [EVEX_Vpbroadcastw_zmm_k1z_r32];
    VpbroadcastwZmmu16MaskmskwMemu16Avx512, Broadcast, Word, Memory, L512, [EVEX_Vpbroadcastw_zmm_k1z_xmmm16];
    VpbroadcastwZmmu16MaskmskwXmmu16Avx512, Broadcast, Word, Register, L512, [EVEX_Vpbroadcastw_zmm_k1z_xmmm16];
    // Moves.
    VmovapdMemf64MaskmskwXmmf64Avx512, Move { aligned: true }, Quadword, Store, L128, [EVEX_Vmovapd_xmmm128_k1z_xmm];
    VmovapdMemf64MaskmskwYmmf64Avx512, Move { aligned: true }, Quadword, Store, L256, [EVEX_Vmovapd_ymmm256_k1z_ymm];
    VmovapdMemf64MaskmskwZmmf64Avx512, Move { aligned: true }, Quadword, Store, L512, [EVEX_Vmovapd_zmmm512_k1z_zmm];
    VmovapdXmmf64MaskmskwMemf64Avx512, Move { aligned: true }, Quadword, Memory, L128, [EVEX_Vmovapd_xmm_k1z_xmmm128];
    VmovapdXmmf64MaskmskwXmmf64Avx512, Move { aligned: true }, Quadword, Register, L128, [EVEX_Vmovapd_xmm_k1z_xmmm128, EVEX_Vmovapd_xmmm128_k1z_xmm];
    VmovapdYmmf64MaskmskwMemf64Avx512, Move { aligned: true }, Quadword, Memory, L256, [EVEX_Vmovapd_ymm_k1z_ymmm256];
    VmovapdYmmf64MaskmskwYmmf64Avx512, Move { aligned: true }, Quadword, Register, L256, [EVEX_Vmovapd_ymm_k1z_ymmm256, EVEX_Vmovapd_ymmm256_k1z_ymm];
    VmovapdZmmf64MaskmskwMemf64Avx512, Move { aligned: true }, Quadword, Memory, L512, [EVEX_Vmovapd_zmm_k1z_zmmm512];
    VmovapdZmmf64MaskmskwZmmf64Avx512, Move { aligned: true }, Quadword, Register, L512, [EVEX_Vmovapd_zmm_k1z_zmmm512, EVEX_Vmovapd_zmmm512_k1z_zmm];
    VmovapsMemf32MaskmskwXmmf32Avx512, Move { aligned: true }, Doubleword, Store, L128, [EVEX_Vmovaps_xmmm128_k1z_xmm];
    VmovapsMemf32MaskmskwYmmf32Avx512, Move { aligned: true }, Doubleword, Store, L256, [EVEX_Vmovaps_ymmm256_k1z_ymm];
    VmovapsMemf32MaskmskwZmmf32Avx512, Move { aligned: true }, Doubleword, Store, L512, [EVEX_Vmovaps_zmmm512_k1z_zmm];
    VmovapsXmmf32MaskmskwMemf32Avx512, Move { aligned: true }, Doubleword, Memory, L128, [EVEX_Vmovaps_xmm_k1z_xmmm128];
    VmovapsXmmf32MaskmskwXmmf32Avx512, Move { aligned: true }, Doubleword, Register, L128, [EVEX_Vmovaps_xmm_k1z_xmmm128, EVEX_Vmovaps_xmmm128_k1z_xmm];
    VmovapsYmmf32MaskmskwMemf32Avx512, Move { aligned: true }, Doubleword, Memory, L256, [EVEX_Vmovaps_ymm_k1z_ymmm256];
    VmovapsYmmf32MaskmskwYmmf32Avx512, Move { aligned: true }, Doubleword, Register, L256, [EVEX_Vmovaps_ymm_k1z_ymmm256, EVEX_Vmovaps_ymmm256_k1z_ymm];
    VmovapsZmmf32MaskmskwMemf32Avx512, Move { aligned: true }, Doubleword, Memory, L512, [EVEX_Vmovaps_zmm_k1z_zmmm512];
    VmovapsZmmf32MaskmskwZmmf32Avx512, Move { aligned: true }, Doubleword, Register, L512, [EVEX_Vmovaps_zmm_k1z_zmmm512, EVEX_Vmovaps_zmmm512_k1z_zmm];
    VmovddupXmmf64MaskmskwMemf64Avx512, Lanes(LaneOperation::DuplicateLowDouble), Quadword, Memory, L128, [EVEX_Vmovddup_xmm_k1z_xmmm64];
    VmovddupXmmf64MaskmskwXmmf64Avx512, Lanes(LaneOperation::DuplicateLowDouble), Quadword, Register, L128, [EVEX_Vmovddup_xmm_k1z_xmmm64];
    VmovddupYmmf64MaskmskwMemf64Avx512, Lanes(LaneOperation::DuplicateLowDouble), Quadword, Memory, L256, [EVEX_Vmovddup_ymm_k1z_ymmm256];
    VmovddupYmmf64MaskmskwYmmf64Avx512, Lanes(LaneOperation::DuplicateLowDouble), Quadword, Register, L256, [EVEX_Vmovddup_ymm_k1z_ymmm256];
    VmovddupZmmf64MaskmskwMemf64Avx512, Lanes(LaneOperation::DuplicateLowDouble), Quadword, Memory, L512, [EVEX_Vmovddup_zmm_k1z_zmmm512];
    VmovddupZmmf64MaskmskwZmmf64Avx512, Lanes(LaneOperation::DuplicateLowDouble), Quadword, Register, L512, [EVEX_Vmovddup_zmm_k1z_zmmm512];
    Vmovdqu16Memu16MaskmskwXmmu16Avx512, Move { aligned: false }, Word, Store, L128, [EVEX_Vmovdqu16_xmmm128_k1z_xmm];
    Vmovdqu16Memu16MaskmskwYmmu16Avx512, Move { aligned: false }, Word, Store, L256, [EVEX_Vmovdqu16_ymmm256_k1z_ymm];
    Vmovdqu16Memu16MaskmskwZmmu16Avx512, Move { aligned: false }, Word, Store, L512, [EVEX_Vmovdqu16_zmmm512_k1z_zmm];
    Vmovdqu16Xmmu16MaskmskwMemu16Avx512, Move { aligned: false }, Word, Memory, L128, [EVEX_Vmovdqu16_xmm_k1z_xmmm128];
    Vmovdqu16Xmmu16MaskmskwXmmu16Avx512, Move { aligned: false }, Word, Register, L128, [EVEX_Vmovdqu16_xmm_k1z_xmmm128, EVEX_Vmovdqu16_xmmm128_k1z_xmm];
    Vmovdqu16Ymmu16MaskmskwMemu16Avx512, Move { aligned: false }, Word, Memory, L256, [EVEX_Vmovdqu16_ymm_k1z_ymmm256];
    Vmovdqu16Ymmu16MaskmskwYmmu16Avx512, Move { aligned: false }, Word, Register, L256, [EVEX_Vmovdqu16_ymm_k1z_ymmm256, EVEX_Vmovdqu16_ymmm256_k1z_ymm];
    Vmovdqu16Zmmu16MaskmskwMemu16Avx512, Move { aligned: false }, Word, Memory, L512, [EVEX_Vmovdqu16_zmm_k1z_zmmm512];
    Vmovdqu16Zmmu16MaskmskwZmmu16Avx512, Move { aligned: false }, Word, Register, L512, [EVEX_Vmovdqu16_zmm_k1z_zmmm512, EVEX_Vmovdqu16_zmmm512_k1z_zmm];
    Vmovdqu8Memu8MaskmskwXmmu8Avx512, Move { aligned: false }, Byte, Store, L128, [EVEX_Vmovdqu8_xmmm128_k1z_xmm];
    Vmovdqu8Memu8MaskmskwYmmu8Avx512, Move { aligned: false }, Byte, Store, L256, [EVEX_Vmovdqu8_ymmm256_k1z_ymm];
    Vmovdqu8Memu8MaskmskwZmmu8Avx512, Move { aligned: false }, Byte, Store, L512, [EVEX_Vmovdqu8_zmmm512_k1z_zmm];
    Vmovdqu8Xmmu8MaskmskwMemu8Avx512, Move { aligned: false }, Byte, Memory, L128, [EVEX_Vmovdqu8_xmm_k1z_xmmm128];
    Vmovdqu8Xmmu8MaskmskwXmmu8Avx512, Move { aligned: false }, Byte, Register, L128, [EVEX_Vmovdqu8_xmm_k1z_xmmm128, EVEX_Vmovdqu8_xmmm128_k1z_xmm];
    Vmovdqu8Ymmu8MaskmskwMemu8Avx512, Move { aligned: false }, Byte, Memory, L256, [EVEX_Vmovdqu8_ymm_k1z_ymmm256];
    Vmovdqu8Ymmu8MaskmskwYmmu8Avx512, Move { aligned: false }, Byte, Register, L256, [EVEX_Vmovdqu8_ymm_k1z_ymmm256, EVEX_Vmovdqu8_ymmm256_k1z_ymm];
    Vmovdqu8Zmmu8MaskmskwMemu8Avx512, Move { aligned: false }, Byte, Memory, L512, [EVEX_Vmovdqu8_zmm_k1z_zmmm512];
    Vmovdqu8Zmmu8MaskmskwZmmu8Avx512, Move { aligned: false }, Byte, Register, L512, [EVEX_Vmovdqu8_zmm_k1z_zmmm512, EVEX_Vmovdqu8_zmmm512_k1z_zmm];
    VmovntdqMemu32Xmmu32Avx512, Move { aligned: true }, Quadword, Store, L128, [EVEX_Vmovntdq_m128_xmm];
    VmovntdqMemu32Ymmu32Avx512, Move { aligned: true }, Quadword, Store, L256, [EVEX_Vmovntdq_m256_ymm];
    VmovntdqMemu32Zmmu32Avx512, Move { aligned: true }, Quadword, Store, L512, [EVEX_Vmovntdq_m512_zmm];
    VmovntdqaXmmu32Memu32Avx512, Move { aligned: true }, Quadword, Memory, L128, [EVEX_Vmovntdqa_xmm_m128];
    VmovntdqaYmmu32Memu32Avx512, Move { aligned: true }, Quadword, Memory, L256, [EVEX_Vmovntdqa_ymm_m256];
    VmovntdqaZmmu32Memu32Avx512, Move { aligned: true }, Quadword, Memory, L512, [EVEX_Vmovntdqa_zmm_m512];
    VmovntpdMemf64Xmmf64Avx512, Move { aligned: true }, Quadword, Store, L128, [EVEX_Vmovntpd_m128_xmm];
    VmovntpdMemf64Ymmf64Avx512, Move { aligned: true }, Quadword, Store, L256, [EVEX_Vmovntpd_m256_ymm];
    VmovntpdMemf64Zmmf64Avx512, Move { aligned: true }, Quadword, Store, L512, [EVEX_Vmovntpd_m512_zmm];
    VmovntpsMemf32Xmmf32Avx512, Move { aligned: true }, Doubleword, Store, L128, [EVEX_Vmovntps_m128_xmm];
    VmovntpsMemf32Ymmf32Avx512, Move { aligned: true }, Doubleword, Store, L256, [EVEX_Vmovntps_m256_ymm];
    VmovntpsMemf32Zmmf32Avx512, Move { aligned: true }, Doubleword, Store, L512, [EVEX_Vmovntps_m512_zmm];
    VmovshdupXmmf32MaskmskwMemf32Avx512, Lanes(LaneOperation::DuplicateOddSingles), Doubleword, Memory, L128, [EVEX_Vmovshdup_xmm_k1z_xmmm128];
    VmovshdupXmmf32MaskmskwXmmf32Avx512, Lanes(LaneOperation::DuplicateOddSingles), Doubleword, Register, L128, [EVEX_Vmovshdup_xmm_k1z_xmmm128];
    VmovshdupYmmf32MaskmskwMemf32Avx512, Lanes(LaneOperation::DuplicateOddSingles), Doubleword, Memory, L256, [EVEX_Vmovshdup_ymm_k1z_ymmm256];
    VmovshdupYmmf32MaskmskwYmmf32Avx512, Lanes(LaneOperation::DuplicateOddSingles), Doubleword, Register, L256, [EVEX_Vmovshdup_ymm_k1z_ymmm256];
    VmovshdupZmmf32MaskmskwMemf32Avx512, Lanes(LaneOperation::DuplicateOddSingles), Doubleword, Memory, L512, [EVEX_Vmovshdup_zmm_k1z_zmmm512];
    VmovshdupZmmf32MaskmskwZmmf32Avx512, Lanes(LaneOperation::DuplicateOddSingles), Doubleword, Register, L512, [EVEX_Vmovshdup_zmm_k1z_zmmm512];
    VmovsldupXmmf32MaskmskwMemf32Avx512, Lanes(LaneOperation::DuplicateEvenSingles), Doubleword, Memory, L128, [EVEX_Vmovsldup_xmm_k1z_xmmm128];
    VmovsldupXmmf32MaskmskwXmmf32Avx512, Lanes(LaneOperation::DuplicateEvenSingles), Doubleword, Register, L128, [EVEX_Vmovsldup_xmm_k1z_xmmm128];
    VmovsldupYmmf32MaskmskwMemf32Avx512, Lanes(LaneOperation::DuplicateEvenSingles), Doubleword, Memory, L256, [EVEX_Vmovsldup_ymm_k1z_ymmm256];
    VmovsldupYmmf32MaskmskwYmmf32Avx512, Lanes(LaneOperation::DuplicateEvenSingles), Doubleword, Register, L256, [EVEX_Vmovsldup_ymm_k1z_ymmm256];
    VmovsldupZmmf32MaskmskwMemf32Avx512, Lanes(LaneOperation::DuplicateEvenSingles), Doubleword, Memory, L512, [EVEX_Vmovsldup_zmm_k1z_zmmm512];
    VmovsldupZmmf32MaskmskwZmmf32Avx512, Lanes(LaneOperation::DuplicateEvenSingles), Doubleword, Register, L512, [EVEX_Vmovsldup_zmm_k1z_zmmm512];
    VmovupdMemf64MaskmskwXmmf64Avx512, Move { aligned: false }, Quadword, Store, L128, [EVEX_Vmovupd_xmmm128_k1z_xmm];
    VmovupdMemf64MaskmskwYmmf64Avx512, Move { aligned: false }, Quadword, Store, L256, [EVEX_Vmovupd_ymmm256_k1z_ymm];
    VmovupdMemf64MaskmskwZmmf64Avx512, Move { aligned: false }, Quadword, Store, L512, [EVEX_Vmovupd_zmmm512_k1z_zmm];
    VmovupdXmmf64MaskmskwMemf64Avx512, Move { aligned: false }, Quadword, Memory, L128, [EVEX_Vmovupd_xmm_k1z_xmmm128];
    VmovupdXmmf64MaskmskwXmmf64Avx512, Move { aligned: false }, Quadword, Register, L128, [EVEX_Vmovupd_xmm_k1z_xmmm128, EVEX_Vmovupd_xmmm128_k1z_xmm];
    VmovupdYmmf64MaskmskwMemf64Avx512, Move { aligned: false }, Quadword, Memory, L256, [EVEX_Vmovupd_ymm_k1z_ymmm256];
    VmovupdYmmf64MaskmskwYmmf64Avx512, Move { aligned: false }, Quadword, Register, L256, [EVEX_Vmovupd_ymm_k1z_ymmm256, EVEX_Vmovupd_ymmm256_k1z_ymm];
    VmovupdZmmf64MaskmskwMemf64Avx512, Move { aligned: false }, Quadword, Memory, L512, [EVEX_Vmovupd_zmm_k1z_zmmm512];
    VmovupdZmmf64MaskmskwZmmf64Avx512, Move { aligned: false }, Quadword, Register, L512, [EVEX_Vmovupd_zmm_k1z_zmmm512, EVEX_Vmovupd_zmmm512_k1z_zmm];
    VmovupsMemf32MaskmskwXmmf32Avx512, Move { aligned: false }, Doubleword, Store, L128, [EVEX_Vmovups_xmmm128_k1z_xmm];
    VmovupsMemf32MaskmskwYmmf32Avx512, Move { aligned: false }, Doubleword, Store, L256, [EVEX_Vmovups_ymmm256_k1z_ymm];
    VmovupsMemf32MaskmskwZmmf32Avx512, Move { aligned: false }, Doubleword, Store, L512, [EVEX_Vmovups_zmmm512_k1z_zmm];
    VmovupsXmmf32MaskmskwMemf32Avx512, Move { aligned: false }, Doubleword, Memory, L128, [EVEX_Vmovups_xmm_k1z_xmmm128];
    VmovupsXmmf32MaskmskwXmmf32Avx512, Move { aligned: false }, Doubleword, Register, L128, [EVEX_Vmovups_xmm_k1z_xmmm128, EVEX_Vmovups_xmmm128_k1z_xmm];
    VmovupsYmmf32MaskmskwMemf32Avx512, Move { aligned: false }, Doubleword, Memory, L256, [EVEX_Vmovups_ymm_k1z_ymmm256];
    VmovupsYmmf32MaskmskwYmmf32Avx512, Move { aligned: false }, Doubleword, Register, L256, [EVEX_Vmovups_ymm_k1z_ymmm256, EVEX_Vmovups_ymmm256_k1z_ymm];
    VmovupsZmmf32MaskmskwMemf32Avx512, Move { aligned: false }, Doubleword, Memory, L512, [EVEX_Vmovups_zmm_k1z_zmmm512];
    VmovupsZmmf32MaskmskwZmmf32Avx512, Move { aligned: false }, Doubleword, Register, L512, [EVEX_Vmovups_zmm_k1z_zmmm512, EVEX_Vmovups_zmmm512_k1z_zmm];
    // Conversions between opmask registers and vectors.
    Vpmovb2mMaskmskwXmmu8Avx512, ToMask, Byte, Register, L128, [EVEX_Vpmovb2m_kr_xmm];
    Vpmovb2mMaskmskwYmmu8Avx512, ToMask, Byte, Register, L256, [EVEX_Vpmovb2m_kr_ymm];
    Vpmovb2mMaskmskwZmmu8Avx512, ToMask, Byte, Register, L512, [EVEX_Vpmovb2m_kr_zmm];
    Vpmovd2mMaskmskwXmmu32Avx512, ToMask, Doubleword, Register, L128, [EVEX_Vpmovd2m_kr_xmm];
    Vpmovd2mMaskmskwYmmu32Avx512, ToMask, Doubleword, Register, L256, [EVEX_Vpmovd2m_kr_ymm];
    Vpmovd2mMaskmskwZmmu32Avx512, ToMask, Doubleword, Register, L512, [EVEX_Vpmovd2m_kr_zmm];
    Vpmovm2bXmmu8MaskmskwAvx512, FromMask, Byte, Register, L128, [EVEX_Vpmovm2b_xmm_kr];
    Vpmovm2bYmmu8MaskmskwAvx512, FromMask, Byte, Register, L256, [EVEX_Vpmovm2b_ymm_kr];
    Vpmovm2bZmmu8MaskmskwAvx512, FromMask, Byte, Register, L512, [EVEX_Vpmovm2b_zmm_kr];
    Vpmovm2dXmmu32MaskmskwAvx512, FromMask, Doubleword, Register, L128, [EVEX_Vpmovm2d_xmm_kr];
    Vpmovm2dYmmu32MaskmskwAvx512, FromMask, Doubleword, Register, L256, [EVEX_Vpmovm2d_ymm_kr];
    Vpmovm2dZmmu32MaskmskwAvx512, FromMask, Doubleword, Register, L512, [EVEX_Vpmovm2d_zmm_kr];
    Vpmovm2qXmmu64MaskmskwAvx512, FromMask, Quadword, Register, L128, [EVEX_Vpmovm2q_xmm_kr];
    Vpmovm2qYmmu64MaskmskwAvx512, FromMask, Quadword, Register, L256, [EVEX_Vpmovm2q_ymm_kr];
    Vpmovm2qZmmu64MaskmskwAvx512, FromMask, Quadword, Register, L512, [EVEX_Vpmovm2q_zmm_kr];
    Vpmovm2wXmmu16MaskmskwAvx512, FromMask, Word, Register, L128, [EVEX_Vpmovm2w_xmm_kr];
    Vpmovm2wYmmu16MaskmskwAvx512, FromMask, Word, Register, L256, [EVEX_Vpmovm2w_ymm_kr];
    Vpmovm2wZmmu16MaskmskwAvx512, FromMask, Word, Register, L512, [EVEX_Vpmovm2w_zmm_kr];
    Vpmovq2mMaskmskwXmmu64Avx512, ToMask, Quadword, Register, L128, [EVEX_Vpmovq2m_kr_xmm];
    Vpmovq2mMaskmskwYmmu64Avx512, ToMask, Quadword, Register, L256, [EVEX_Vpmovq2m_kr_ymm];
    Vpmovq2mMaskmskwZmmu64Avx512, ToMask, Quadword, Register, L512, [EVEX_Vpmovq2m_kr_zmm];
    Vpmovw2mMaskmskwXmmu16Avx512, ToMask, Word, Register, L128, [EVEX_Vpmovw2m_kr_xmm];
    Vpmovw2mMaskmskwYmmu16Avx512, ToMask, Word, Register, L256, [EVEX_Vpmovw2m_kr_ymm];
    Vpmovw2mMaskmskwZmmu16Avx512, ToMask, Word, Register, L512, [EVEX_Vpmovw2m_kr_zmm];
};
