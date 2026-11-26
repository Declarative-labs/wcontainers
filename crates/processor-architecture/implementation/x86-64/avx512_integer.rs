//! The AVX-512 foundation set's integer forms under EVEX: lane arithmetic and logic, shifts and rotates, the
//! compares and tests into an opmask register, VPBLENDM, the broadcasts and the moves, at each vector length, as the
//! [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (volume 2) defines them.
//!
//! Every form but a compare or test writes its destination's lanes under a writemask: k0 writes every lane, another
//! opmask register the lanes whose bit is set, and the others keep their value (merging) or are zeroed (zeroing).
//! The destination's bits from the vector length to 511 are zeroed. A compare or test writes one bit for each lane,
//! cleared where the writemask's bit is, and zeroes the opmask register above the lanes.

use alloc::vec::Vec;

use iced_x86::Code;

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
    /// VPBROADCAST: the low lane of an XMM register, of memory or of a general register in every lane.
    Broadcast,
    /// VBROADCASTI32X4 and VBROADCASTI64X4: 16 or 32 bytes of memory repeated.
    BroadcastTuple,
    /// VMOVDQA and VMOVDQU; the aligned ones need memory aligned to the vector's length.
    Move { aligned: bool },
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
    /// The lanes of the vector.
    pub const fn count_lanes(&self) -> usize {
        (self.length.to_bits() / self.lane.to_bits()) as usize
    }

    /// Whether the form writes an opmask register rather than a vector.
    pub const fn writes_mask(&self) -> bool {
        matches!(self.operation, EvexIntegerOperation::TestIntoMask { .. } | EvexIntegerOperation::CompareIntoMask(_))
    }

    /// Whether the form's memory operand may be one lane broadcast ({1toN}).
    pub const fn can_broadcast(&self) -> bool {
        !matches!(
            self.operation,
            EvexIntegerOperation::ShiftByCount(_) | EvexIntegerOperation::Broadcast | EvexIntegerOperation::BroadcastTuple | EvexIntegerOperation::Move { .. }
        )
    }

    /// The bytes a memory operand reaches: the vector, the shift count's 16 bytes, a broadcast's lane or tuple.
    pub const fn memory_bytes(&self, broadcast: bool) -> u64 {
        let lane = self.lane.to_bits() as u64 / 8;
        match self.operation {
            _ if broadcast => lane,
            EvexIntegerOperation::ShiftByCount(_) => 16,
            EvexIntegerOperation::Broadcast => lane,
            // VBROADCASTI32X4 repeats 16 bytes, VBROADCASTI64X4 32.
            EvexIntegerOperation::BroadcastTuple => lane * 4,
            _ => self.length.to_bits() as u64 / 8,
        }
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
            EvexIntegerOperation::Lanes(LaneOperation::Absolute(_))
                | EvexIntegerOperation::ShiftByImmediate(_)
                | EvexIntegerOperation::Rotate { by_lane: false, .. }
                | EvexIntegerOperation::Broadcast
                | EvexIntegerOperation::BroadcastTuple
                | EvexIntegerOperation::Move { .. }
        )
    }

    /// Whether the form has an immediate.
    pub const fn has_immediate(&self) -> bool {
        matches!(
            self.operation,
            EvexIntegerOperation::ShiftByImmediate(_)
                | EvexIntegerOperation::Rotate { by_lane: false, .. }
                | EvexIntegerOperation::TernaryLogic
                | EvexIntegerOperation::CompareIntoMask(ComparePredicate::Immediate(_))
        )
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
            EvexIntegerOperation::BroadcastTuple => {
                let tuple = self.memory_bytes(false) as usize / 16;
                per_quarter(&|quarter| second[quarter % tuple])
            }
            EvexIntegerOperation::Move { .. } => per_quarter(&|quarter| second[quarter]),
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
    Blend, Broadcast, BroadcastTuple, CompareIntoMask, Lanes, Move, Rotate, ShiftByCount, ShiftByImmediate, ShiftByLane,
    TernaryLogic, TestIntoMask,
};
use EvexIntegerPlace::{General, Memory, Register, Store};
use Lane::{Doubleword, Quadword};
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

/// Every EVEX integer form of the foundation set modeled.
pub static EVEX_INTEGER_FORMS: &[EvexIntegerForm] = evex_forms! {
    Vbroadcasti32x4Ymmu32MaskmskwMemu32Avx512, BroadcastTuple, Doubleword, Memory, L256, [EVEX_Vbroadcasti32x4_ymm_k1z_m128];
    Vbroadcasti32x4Zmmu32MaskmskwMemu32Avx512, BroadcastTuple, Doubleword, Memory, L512, [EVEX_Vbroadcasti32x4_zmm_k1z_m128];
    Vbroadcasti64x4Zmmu64MaskmskwMemu64Avx512, BroadcastTuple, Quadword, Memory, L512, [EVEX_Vbroadcasti64x4_zmm_k1z_m256];
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
};
