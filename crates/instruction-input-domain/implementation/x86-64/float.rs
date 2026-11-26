//! The inputs of the legacy SSE floating-point forms, derived from each form's operation, precision and
//! arrangement: the two XMM operands of [`crate::vector`], whose lanes take the floating-point critical values, and
//! MXCSR.
//!
//! The destination and source of a form that reads both are one operand, so that besides every rotation of the
//! critical values against every other they hold the pairs the operation's own arithmetic takes to each exception
//! after the computation: an overflow and an underflow that are exact and one of each that is not, each also of the
//! other sign, in every lane. MXCSR's values mask and unmask each of those exceptions and inexact.

use alloc::vec;
use alloc::vec::Vec;

use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_x86_64::float::{
    Arrangement, Conversion, DENORMALS_ARE_ZEROS, FLUSH_TO_ZERO, FloatForm, FloatOperation, INEXACT, MASK_SHIFT,
    Precision, ROUNDING_SHIFT,
};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::vector::join;
use wcontainers_processor_architecture_x86_64::{MXCSR_DEFAULT, X86_64};

use crate::vector::{VectorValues, build_xmm_state, fill_lanes, rotate_floats, to_float_lane};
use crate::integer::ImmediateClass;
use crate::{FlagSet, OperandType, OperandValue, Role, Splits};

/// Every exception flag of MXCSR.
const EXCEPTION_FLAGS: u32 = (1 << MASK_SHIFT) - 1 - DENORMALS_ARE_ZEROS;

/// The destination and source of a floating-point form that reads both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FloatOperands {
    pub operation: FloatOperation,
    pub precision: Precision,
    pub arrangement: Arrangement,
}

/// The operand types of a floating-point form, in the order the build reads them; SQRT reads only its source.
pub fn list_operand_types(form: &FloatForm) -> Vec<OperandType> {
    let place = match form.place {
        OperandPlace::Register => OperandType::Encoding(2),
        _ => OperandType::Memory {
            splits: Splits::Allowed,
            offset: false,
        },
    };
    let operands = if let FloatOperation::Convert(conversion) = form.operation {
        OperandType::VectorValue(Role::Source, VectorValues::Conversion { conversion, precision: form.precision })
    } else if form.operation == FloatOperation::SquareRoot {
        OperandType::VectorValue(Role::Source, VectorValues::Floats { precision: form.precision, every_rotation: true })
    } else {
        OperandType::FloatOperands(FloatOperands {
            operation: form.operation,
            precision: form.precision,
            arrangement: form.arrangement,
        })
    };
    // A compare reads MXCSR's invalid and denormal masks and DAZ alone.
    let (predicate, mxcsr) = match form.operation {
        FloatOperation::Compare => (Some(OperandType::Immediate(ImmediateClass::Predicate)), MxcsrSet::Comparison),
        FloatOperation::OrderedCompare { .. } => (None, MxcsrSet::Comparison),
        _ => (None, MxcsrSet::Arithmetic),
    };
    [place, operands]
        .into_iter()
        .chain(predicate)
        .chain([OperandType::Mxcsr(mxcsr), OperandType::Flags(FlagSet::Unread)])
        .collect()
}

/// The destination and source values: every destination rotation (those a lane count apart for a packed form)
/// against every source rotation, then the exception pairs in every lane.
pub fn generate_operands(operands: FloatOperands) -> Vec<OperandValue> {
    let FloatOperands { operation, precision, arrangement } = operands;
    if matches!(operation, FloatOperation::HorizontalAdd | FloatOperation::HorizontalSubtract) {
        return generate_horizontal_operands(operation, precision);
    }
    // ADDSUB computes its even and odd lanes differently, so each pair meets in every lane.
    let every_destination = arrangement == Arrangement::Scalar || operation == FloatOperation::AddSubtract;
    let destinations = rotate_floats(precision, every_destination);
    let sources = rotate_floats(precision, true);
    let lane = to_float_lane(precision);
    let rotations = destinations
        .iter()
        .flat_map(|&destination| sources.iter().map(move |&source| (destination, source)))
        .collect::<Vec<_>>();
    let pairs = generate_exception_pairs(operation, precision)
        .into_iter()
        .map(|(first, second)| (fill_lanes(lane, first), fill_lanes(lane, second)));
    rotations
        .into_iter()
        .chain(pairs)
        .map(|(destination, source)| OperandValue::Vectors { destination, source })
        .collect()
}

/// A horizontal operation's operands, which pair a vector's adjacent lanes: every ordered pair of critical values,
/// each in an even lane and the odd lane above it, two pairs a vector, the destination's first; then each exception
/// pair of the lanes' operation so placed in every pair of lanes.
fn generate_horizontal_operands(operation: FloatOperation, precision: Precision) -> Vec<OperandValue> {
    let lane = to_float_lane(precision);
    let values = rotate_floats(precision, true).into_iter().map(|vector| vector as u64 & lane_mask(precision)).collect::<Vec<_>>();
    let pairs_per_vector = (u128::BITS / lane.to_bits() / 2) as usize;
    let lane_operation = if operation == FloatOperation::HorizontalAdd { FloatOperation::Add } else { FloatOperation::Subtract };
    let pairs: Vec<(u64, u64)> = values.iter().flat_map(|&first| values.iter().map(move |&second| (first, second))).collect();
    let fill = |pairs: &[(u64, u64)]| {
        join(lane, pairs.iter().cycle().take(pairs_per_vector).flat_map(|&(first, second)| [first, second]))
    };
    let exception_pairs = generate_exception_pairs(lane_operation, precision);
    pairs
        .chunks(2 * pairs_per_vector)
        .map(|chunk| (fill(&chunk[..pairs_per_vector]), fill(&chunk[pairs_per_vector..])))
        .chain(exception_pairs.iter().map(|&pair| (fill(&[pair]), fill(&[pair]))))
        .map(|(destination, source)| OperandValue::Vectors { destination, source })
        .collect()
}

/// The bits of one lane of the precision.
const fn lane_mask(precision: Precision) -> u64 {
    if precision.to_bits() == u64::BITS { u64::MAX } else { (1 << precision.to_bits()) - 1 }
}

/// The lane operands the operation's own arithmetic takes to overflow and to underflow, exactly and not (for MUL
/// and DIV powers of two whose exponents sum or differ to one past either end, for ADD and SUB the largest doubled
/// or plus ¾ of its unit and the smallest normal's last unit alone), each again with its result's sign flipped.
pub fn generate_exception_pairs(operation: FloatOperation, precision: Precision) -> Vec<(u64, u64)> {
    let fraction_bits = precision.to_significand_bits() - 1;
    let exponent_bits = precision.to_bits() - precision.to_significand_bits();
    let bias = (1i32 << (exponent_bits - 1)) - 1;
    let (largest_exponent, smallest_exponent) = (bias, 1 - bias);
    let sign = 1u64 << (precision.to_bits() - 1);
    // 2^exponent times one plus `fraction` units in the last place.
    let normal = |exponent: i32, fraction: u64| (((exponent + bias) as u64) << fraction_bits) | fraction;
    let one_and_a_half = 1u64 << (fraction_bits - 1);
    let all_fraction = (1u64 << fraction_bits) - 1;
    let pairs: Vec<(u64, u64)> = match operation {
        FloatOperation::Multiply => vec![
            (normal(largest_exponent, 0), normal(1, 0)),
            (normal(largest_exponent, 1), normal(1, 1)),
            (normal(smallest_exponent, 0), normal(-1, 0)),
            (normal(smallest_exponent, 1), normal(-1, 0)),
            (normal(smallest_exponent, 1), normal(-1, 1)),
        ],
        FloatOperation::Divide => vec![
            (normal(largest_exponent, 0), normal(-1, 0)),
            (normal(largest_exponent, 0), normal(-2, one_and_a_half)),
            (normal(smallest_exponent, 0), normal(1, 0)),
            (normal(smallest_exponent, 1), normal(1, 0)),
            (normal(smallest_exponent, 0), normal(1, one_and_a_half)),
        ],
        FloatOperation::AddSubtract => {
            let mut both = generate_exception_pairs(FloatOperation::Add, precision);
            both.extend(generate_exception_pairs(FloatOperation::Subtract, precision));
            return both;
        }
        FloatOperation::HorizontalAdd => return generate_exception_pairs(FloatOperation::Add, precision),
        FloatOperation::HorizontalSubtract => return generate_exception_pairs(FloatOperation::Subtract, precision),
        FloatOperation::Add | FloatOperation::Subtract => {
            let largest = normal(largest_exponent, all_fraction);
            let three_quarters_unit = normal(largest_exponent - precision.to_significand_bits() as i32, one_and_a_half);
            let subtracted = if operation == FloatOperation::Add { sign } else { 0 };
            let added = sign - subtracted;
            vec![
                (largest, largest | added),
                (largest, three_quarters_unit | added),
                (normal(smallest_exponent, 1), normal(smallest_exponent, 0) | subtracted),
            ]
        }
        FloatOperation::SquareRoot
        | FloatOperation::Minimum
        | FloatOperation::Maximum
        | FloatOperation::Compare
        | FloatOperation::OrderedCompare { .. }
        | FloatOperation::Convert(_) => Vec::new(),
    };
    // The other sign: of the first operand for MUL and DIV, of both for ADD and SUB.
    let negated: Vec<(u64, u64)> = pairs
        .iter()
        .map(|&(first, second)| match operation {
            FloatOperation::Add | FloatOperation::Subtract => (first ^ sign, second ^ sign),
            _ => (first ^ sign, second),
        })
        .collect();
    pairs.into_iter().chain(negated).collect()
}

/// Which of MXCSR's fields a form reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MxcsrSet {
    Arithmetic,
    /// The invalid and denormal masks and DAZ.
    Comparison,
}

/// MXCSR's critical values: for arithmetic every exception masked in each rounding mode, every one unmasked, all
/// but inexact unmasked, inexact alone unmasked, flush-to-zero alone and denormals-are-zeros alone, and every flag
/// already set; for a compare every exception masked, every one unmasked, DAZ, and every flag set.
pub fn generate_mxcsr_values(set: MxcsrSet) -> Vec<OperandValue> {
    let rounding = |mode: u32| (MXCSR_DEFAULT & !(3 << ROUNDING_SHIFT)) | (mode << ROUNDING_SHIFT);
    if set == MxcsrSet::Comparison {
        let values = [MXCSR_DEFAULT, 0, MXCSR_DEFAULT | DENORMALS_ARE_ZEROS, MXCSR_DEFAULT | EXCEPTION_FLAGS];
        return values.map(OperandValue::Mxcsr).to_vec();
    }
    [
        MXCSR_DEFAULT,
        rounding(1),
        rounding(2),
        rounding(3),
        0,
        INEXACT << MASK_SHIFT,
        MXCSR_DEFAULT & !(INEXACT << MASK_SHIFT),
        MXCSR_DEFAULT | FLUSH_TO_ZERO,
        MXCSR_DEFAULT | DENORMALS_ARE_ZEROS,
        MXCSR_DEFAULT | EXCEPTION_FLAGS,
    ]
    .map(OperandValue::Mxcsr)
    .to_vec()
}

/// Builds the state of a floating-point form from one value of each of its operand types.
pub fn build_state(form: &FloatForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    build_xmm_state(form.code, form.memory_bytes(), operands)
}

/// The lane values at a conversion's own edges, each to fill a whole source: for a narrowing, the doubles that
/// round to a single's overflow or underflow, exactly, at a tie and past it, a tie at one, and a signaling NaN whose
/// payload lies below the bits a single keeps; for a conversion to integers, the halves around zero and each end of
/// the doubleword range, with the values a half and one past it; for doublewords into singles, the ties and the
/// values past them at the significand's edge.
pub fn generate_conversion_edges(conversion: Conversion, precision: Precision) -> Vec<u64> {
    let fraction_bits = precision.to_significand_bits() - 1;
    let bias = (1i32 << (precision.to_bits() - precision.to_significand_bits() - 1)) - 1;
    let sign = 1u64 << (precision.to_bits() - 1);
    let normal = |exponent: i32, fraction: u64| (((exponent + bias) as u64) << fraction_bits) | fraction;
    let half = 1u64 << (fraction_bits - 1);
    match conversion {
        Conversion::DoubleToSingle => {
            let single_digits = f32::MANTISSA_DIGITS as i32;
            let (largest, smallest) = (f32::MAX_EXP - 1, f32::MIN_EXP - 1);
            let below_single = |extra: i32| 1u64 << (fraction_bits as i32 - single_digits - extra + 1);
            vec![
                normal(largest, (1 << fraction_bits) - below_single(0)),
                normal(largest, (1 << fraction_bits) - below_single(1)),
                normal(largest + 1, 0),
                normal(smallest, 0),
                normal(smallest - 1, 0),
                normal(smallest - single_digits, 0),
                normal(smallest - single_digits + 1, half),
                normal(0, below_single(1)),
                normal(0, below_single(1) | 1),
                sign | normal(0, below_single(1) | 1),
                precision.to_default_nan() & !sign & !precision.to_quiet_bit() | 1,
            ]
        }
        Conversion::FloatToInteger { .. } => {
            // 2^31 less a half and less one, 2^31, and -2^31 less nothing, a half and one, as the precision holds them.
            let below = |amount: u64| if fraction_bits > 31 { (1 << fraction_bits) - (amount << (fraction_bits - 31)) } else { (1 << fraction_bits) - 1 };
            let beyond = |amount: u64| if fraction_bits > 31 { amount << (fraction_bits - 32) } else { 1 };
            vec![
                normal(-1, 0),
                normal(0, half),
                normal(1, half >> 1),
                sign | normal(-1, 0),
                sign | normal(1, half >> 1),
                normal(30, below(1)),
                normal(30, below(2)),
                normal(31, 0),
                sign | normal(31, 0),
                sign | normal(31, beyond(1)),
                sign | normal(31, beyond(2)),
            ]
        }
        Conversion::IntegerToFloat => {
            let digits = precision.to_significand_bits();
            if digits > u32::BITS {
                return Vec::new();
            }
            let edge = 1u64 << digits;
            [edge + 1, edge + 3, edge + 2, (edge + 1).wrapping_neg(), (edge + 3).wrapping_neg()]
                .map(|value| value & u64::from(u32::MAX))
                .to_vec()
        }
        Conversion::SingleToDouble => vec![precision.to_default_nan() & !sign & !precision.to_quiet_bit() | 1],
    }
}
