//! The inputs of the EVEX forms of the architecture's `evex` module, derived from each form's operation, vector
//! length and element: one skeleton for every form.
//!
//! A register form's operands are its three registers and how its bytes lie, then its data: the two sources'
//! values, the opmask, MXCSR and the embedded rounding or exception suppression it allows, which decide no fault and
//! so are one data operand. A memory form's are the memory operand, the first source's register, the opmask (which
//! decides which elements may fault), whether the source is broadcast where the form allows it, then the values and
//! MXCSR. The destination holds the value every register is filled with, so that a merged element differs from a
//! computed one.
//!
//! A vector's critical values are rotations of the element's critical values across all its elements: the first
//! source's a vector's element count apart, the second's every one, so that each pair meets in some element; then
//! the operation's exception pairs in every element. A register form takes each with MXCSR's critical values; then
//! with each embedded control, and with every other element selected (merging, then zeroing), the second source's
//! rotations against the first's first and the exception pairs. A memory form, whose arithmetic is its register
//! form's, takes the second source's rotations against the first's first with MXCSR at its default and with every
//! exception unmasked; its opmasks also select none, the first element alone and the last alone, so that a fault on
//! an element left out is seen to be suppressed.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Instruction as Encoded, MemoryOperand, Register, RoundingControl};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::evex::{
    Conversion, EvexForm, EvexOperation, FusedOrder, IntegerOperation, Number, read_element, write_element,
};
use wcontainers_processor_architecture_x86_64::float::Conversion as LegacyConversion;
use wcontainers_processor_architecture_x86_64::float::{
    Arrangement, DENORMALS_ARE_ZEROS, FLUSH_TO_ZERO, FloatOperation, INEXACT, Precision, ROUNDING_SHIFT,
};
use wcontainers_processor_architecture_x86_64::integer::OperandPlace;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{
    DataAccess, InstructionFetch, MXCSR_DEFAULT, PagePermission, Processor, X86_64, ZERO_PROCESSOR,
};

use crate::float::{MxcsrSet, generate_exception_pairs, generate_mxcsr_values};
use crate::integer::{CodeLayout, LAYOUTS_BEYOND_START, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::vector::rotate_floats;
use crate::{CODE_ADDRESS, FlagSet, OperandType, OperandValue, Splits};

/// Register triples (destination, EVEX.vvvv, ModRM.rm) at the page's start: low registers, then a high register in
/// each place, which EVEX.R', V' and X with B encode.
const REGISTER_TRIPLES: [[usize; 3]; 2] = [[1, 2, 3], [17, 30, 25]];
/// The first sources a memory form names besides its memory operand: a low and a high register.
const FIRST_SOURCES: [usize; 2] = [2, 29];
const NO_MASK: EvexMask = EvexMask { register: 0, value: 0, zeroing: false };
/// Every other element, from the first.
const EVERY_OTHER: u64 = 0x5555_5555_5555_5555;

/// The registers of an EVEX register form and how its bytes lie.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexEncoding {
    /// Destination, first source, second source.
    pub registers: [usize; 3],
    pub layout: CodeLayout,
}

/// The opmask an EVEX form names, k0 for none, and the value the register holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexMask {
    pub register: usize,
    pub value: u64,
    pub zeroing: bool,
}

/// The embedded control a state's encoding names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EmbeddedControl {
    None,
    Suppression,
    Rounding(RoundingControl),
}

/// What the data of a floating-point EVEX form is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexFloatData {
    pub operation: FloatOperation,
    pub precision: Precision,
    pub arrangement: Arrangement,
    pub vector_bytes: usize,
    /// A register form, whose data holds the opmask and the embedded controls.
    pub register: bool,
    pub rounding: bool,
    pub suppression: bool,
}

/// The operand types of an EVEX form, in the order the build reads them.
pub fn list_operand_types(form: &EvexForm) -> Vec<OperandType> {
    let op_code = form.code.op_code();
    let register = form.place == OperandPlace::Register;
    let data = match form.operation {
        EvexOperation::Float { operation, precision, arrangement } => OperandType::EvexFloatData(EvexFloatData {
            operation,
            precision,
            arrangement,
            vector_bytes: form.vector_bytes(),
            register,
            rounding: op_code.can_use_rounding_control(),
            suppression: op_code.can_suppress_all_exceptions(),
        }),
        EvexOperation::Fused { order, precision, arrangement, .. } => OperandType::EvexFusedData(EvexFusedData {
            order,
            precision,
            vector_bytes: form.vector_bytes(),
            scalar: arrangement == Arrangement::Scalar,
            register,
            rounding: op_code.can_use_rounding_control(),
        }),
        EvexOperation::Compare { precision, arrangement } => OperandType::EvexCompareData(EvexCompareData {
            precision,
            elements: form.elements(),
            scalar: arrangement == Arrangement::Scalar,
            predicates: true,
            register,
            suppression: op_code.can_suppress_all_exceptions(),
        }),
        EvexOperation::OrderedCompare { precision, .. } => OperandType::EvexCompareData(EvexCompareData {
            precision,
            elements: 1,
            scalar: true,
            predicates: false,
            register,
            suppression: op_code.can_suppress_all_exceptions(),
        }),
        EvexOperation::Convert(_) | EvexOperation::ToBfloat16 { .. } => OperandType::EvexConvertData(EvexConvertData {
            operation: form.operation,
            elements: form.elements(),
            register,
            rounding: op_code.can_use_rounding_control(),
            suppression: op_code.can_suppress_all_exceptions(),
            maskable: op_code.can_use_op_mask_register(),
        }),
        EvexOperation::Compress { element_bytes } | EvexOperation::Expand { element_bytes } => {
            OperandType::EvexMoveData(EvexMoveData { element_bytes, vector_bytes: form.vector_bytes(), register })
        }
        EvexOperation::Integer { operation, element_bytes } => OperandType::EvexIntegerData(EvexIntegerData {
            operation,
            element_bytes,
            vector_bytes: form.vector_bytes(),
            register,
            maskable: op_code.can_use_op_mask_register(),
        }),
    };
    let unread = OperandType::Flags(FlagSet::Unread);
    if register {
        return vec![OperandType::EvexEncoding, data, unread];
    }
    let memory = OperandType::Memory {
        splits: Splits::Allowed,
        offset: false,
    };
    [memory]
        .into_iter()
        .chain((form.reads_first_source() || form.stores()).then_some(OperandType::EvexRegister))
        .chain(op_code.can_use_op_mask_register().then_some(OperandType::EvexMask(form.elements())))
        .chain(op_code.can_broadcast().then_some(OperandType::EvexBroadcast))
        .chain([data, unread])
        .collect()
}

pub fn generate_encodings() -> Vec<OperandValue> {
    let at_start = REGISTER_TRIPLES.map(|registers| EvexEncoding { registers, layout: CodeLayout::PageStart });
    let laid_out = LAYOUTS_BEYOND_START.map(|layout| EvexEncoding { registers: REGISTER_TRIPLES[0], layout });
    at_start.into_iter().chain(laid_out).map(OperandValue::EvexEncoding).collect()
}

pub fn generate_first_sources() -> Vec<OperandValue> {
    FIRST_SOURCES.map(OperandValue::Register).to_vec()
}

/// A memory form's opmasks: none, every other element merged and zeroed, no element, the first alone and the last
/// alone.
pub fn generate_masks(elements: usize) -> Vec<OperandValue> {
    let all = to_all(elements);
    [
        NO_MASK,
        EvexMask { register: 1, value: EVERY_OTHER & all, zeroing: false },
        EvexMask { register: 7, value: EVERY_OTHER & all, zeroing: true },
        EvexMask { register: 2, value: 0, zeroing: false },
        EvexMask { register: 3, value: 1, zeroing: true },
        EvexMask { register: 4, value: 1 << (elements - 1), zeroing: false },
    ]
    .map(OperandValue::EvexMask)
    .to_vec()
}

const fn to_all(elements: usize) -> u64 {
    if elements >= 64 { u64::MAX } else { (1 << elements) - 1 }
}

/// The data of a floating-point form, as the module's documentation lists it.
pub fn generate_float_data(data: EvexFloatData) -> Vec<OperandValue> {
    let EvexFloatData { operation, precision, arrangement, vector_bytes, register, rounding, suppression } = data;
    let values: Vec<u64> = rotate_floats(precision, true).into_iter().map(|vector| vector as u64).collect();
    let bytes = (precision.to_bits() / u8::BITS) as usize;
    let elements = vector_bytes / bytes;
    let make = |value: &dyn Fn(usize) -> u64| -> [u128; 4] {
        let mut vector = [0; 4];
        for index in 0..elements {
            write_element(&mut vector, index, bytes, value(index));
        }
        vector
    };
    let rotate = |rotation: usize| make(&|index| values[(index + rotation) % values.len()]);
    let fill = |value: u64| make(&|_| value);
    let step = if arrangement == Arrangement::Scalar { 1 } else { elements };
    let firsts: Vec<[u128; 4]> = match (operation, arrangement) {
        (FloatOperation::SquareRoot, Arrangement::Packed) => vec![rotate(0)],
        _ => (0..values.len()).step_by(step).map(rotate).collect(),
    };
    let against_first = |first: [u128; 4]| (0..values.len()).map(move |rotation| (first, rotation));
    let pairs: Vec<([u128; 4], [u128; 4])> =
        generate_exception_pairs(operation, precision).into_iter().map(|(first, second)| (fill(first), fill(second))).collect();
    let some: Vec<([u128; 4], [u128; 4])> =
        against_first(rotate(0)).map(|(first, rotation)| (first, rotate(rotation))).chain(pairs.iter().copied()).collect();
    let datum = |(first, second): ([u128; 4], [u128; 4]), mask: EvexMask, control: EmbeddedControl, mxcsr: u32| {
        OperandValue::EvexFloatData { first, second, mask, control, mxcsr }
    };
    // Every exception unmasked, to nearest.
    let unmasked = 0;
    if !register {
        let vectors = against_first(rotate(0)).map(|(first, rotation)| (first, rotate(rotation)));
        return vectors
            .flat_map(|pair| [MXCSR_DEFAULT, unmasked].map(|mxcsr| datum(pair, NO_MASK, EmbeddedControl::None, mxcsr)))
            .collect();
    }
    let plain: Vec<u32> = generate_mxcsr_values(MxcsrSet::Arithmetic)
        .into_iter()
        .filter_map(|value| match value {
            OperandValue::Mxcsr(mxcsr) => Some(mxcsr),
            _ => None,
        })
        .collect();
    let every: Vec<([u128; 4], [u128; 4])> = firsts
        .iter()
        .flat_map(|&first| against_first(first))
        .map(|(first, rotation)| (first, rotate(rotation)))
        .chain(pairs.iter().copied())
        .collect();
    let mut all: Vec<OperandValue> =
        every.iter().flat_map(|&pair| plain.iter().map(move |&mxcsr| datum(pair, NO_MASK, EmbeddedControl::None, mxcsr))).collect();
    let toward_zero = (MXCSR_DEFAULT & !(3 << ROUNDING_SHIFT)) | (3 << ROUNDING_SHIFT);
    let mut controls = Vec::new();
    if rounding {
        controls.extend([
            (EmbeddedControl::Rounding(RoundingControl::RoundToNearest), toward_zero),
            (EmbeddedControl::Rounding(RoundingControl::RoundDown), MXCSR_DEFAULT),
            (EmbeddedControl::Rounding(RoundingControl::RoundUp), MXCSR_DEFAULT),
            (EmbeddedControl::Rounding(RoundingControl::RoundTowardZero), MXCSR_DEFAULT),
        ]);
    } else if suppression {
        controls.push((EmbeddedControl::Suppression, MXCSR_DEFAULT));
    }
    // Each control against MXCSR's other rounding mode, every exception unmasked, and DAZ with FTZ.
    for (control, other_mode) in controls {
        for mxcsr in [other_mode, 0, MXCSR_DEFAULT | DENORMALS_ARE_ZEROS | FLUSH_TO_ZERO] {
            all.extend(some.iter().map(|&pair| datum(pair, NO_MASK, control, mxcsr)));
        }
    }
    let all_elements = to_all(elements);
    for mask in [
        EvexMask { register: 1, value: EVERY_OTHER & all_elements, zeroing: false },
        EvexMask { register: 7, value: EVERY_OTHER & all_elements, zeroing: true },
    ] {
        for mxcsr in [MXCSR_DEFAULT, unmasked] {
            all.extend(some.iter().map(|&pair| datum(pair, mask, EmbeddedControl::None, mxcsr)));
        }
    }
    all
}

/// What the data of a fused EVEX form is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexFusedData {
    pub order: FusedOrder,
    pub precision: Precision,
    pub vector_bytes: usize,
    pub scalar: bool,
    /// A register form, whose data holds the opmask and the embedded rounding.
    pub register: bool,
    pub rounding: bool,
}

/// The data of a fused form: the multiplicands and the addend (placed in the destination and the sources as the
/// form's order has them) rotated so that every triple of critical values meets in some element, the first's
/// rotations a vector's element count apart (a scalar form's every pair); then the fused edges in every element: a product that cancels the
/// addend to below the precision, products on a rounding tie and just past it, an overflow the addend undoes,
/// exact and inexact underflow, an infinity times zero (with a quiet NaN addend too), infinities that cancel, and
/// three quiet NaNs in each order. MXCSR's other values, each embedded rounding and the opmasks take the first
/// rotations against every rotation of the addend, and the edges.
pub fn generate_fused_data(data: EvexFusedData) -> Vec<OperandValue> {
    let EvexFusedData { order, precision, vector_bytes, scalar, register, rounding } = data;
    let values: Vec<u64> = rotate_floats(precision, true).into_iter().map(|vector| vector as u64).collect();
    let bytes = (precision.to_bits() / u8::BITS) as usize;
    let elements = vector_bytes / bytes;
    let make = |value: &dyn Fn(usize) -> u64| -> [u128; 4] {
        let mut vector = [0; 4];
        for index in 0..elements {
            write_element(&mut vector, index, bytes, value(index));
        }
        vector
    };
    let rotate = |rotation: usize| make(&|index| values[(index + rotation) % values.len()]);
    let fill = |value: u64| make(&|_| value);
    let digits = precision.to_significand_bits() as i32;
    let fraction_bits = digits - 1;
    let bias = (1i32 << (precision.to_bits() as i32 - digits - 1)) - 1;
    let sign = 1u64 << (precision.to_bits() - 1);
    let normal = |exponent: i32, fraction: u64| (((exponent + bias) as u64) << fraction_bits) | fraction;
    let one = normal(0, 0);
    // An addend far below the product's last unit: 2^(-digits - 5), or the smallest subnormal where that is below it.
    let tiny = if bias - digits - 5 > 0 { normal(-digits - 5, 0) } else { 1 };
    let (low, high) = (digits / 2, digits - digits / 2);
    let largest = normal(bias, (1 << fraction_bits) - 1);
    let infinity = precision.to_default_nan() & !sign & !precision.to_quiet_bit();
    let quiet = |payload: u64| (precision.to_default_nan() & !sign) | payload;
    let edges: Vec<[u64; 3]> = vec![
        [normal(0, 1), normal(-1, (1 << fraction_bits) - 1), one | sign],
        [normal(0, 1 << (fraction_bits - low)), normal(0, 1 << (fraction_bits - high)), 0],
        [normal(0, 1 << (fraction_bits - low)), normal(0, 1 << (fraction_bits - high)), tiny],
        [largest, normal(1, 0), 0],
        [largest, normal(1, 0), largest | sign],
        [largest, one, largest],
        [normal(1 - bias, 0), normal(-1, 0), 0],
        [normal(1 - bias, 1), normal(-1, 0), 0],
        [infinity, 0, quiet(1)],
        [infinity, 0, one],
        [infinity, one, infinity | sign],
        [one, one, one | sign],
        [0, one, sign],
        [quiet(1), quiet(2), quiet(3)],
        [quiet(2), quiet(3), quiet(1)],
        [quiet(3), quiet(1), quiet(2)],
    ];
    // Back from multiplicands and addend to the destination and the two sources.
    let place = |[a, b, c]: [[u128; 4]; 3]| -> [[u128; 4]; 3] {
        match order {
            FusedOrder::Order132 => [a, c, b],
            FusedOrder::Order213 => [b, a, c],
            FusedOrder::Order231 => [c, a, b],
        }
    };
    let step = elements;
    let count = values.len();
    let firsts: Vec<usize> = (0..count).step_by(step).collect();
    // A scalar form's one element meets every pair of critical values: the third rotation the sum of the others.
    let triples = |seconds: &[usize]| -> Vec<[[u128; 4]; 3]> {
        let all: Vec<(usize, usize, usize)> = match scalar {
            true => (0..count).flat_map(|first| (0..count).map(move |second| (first, second, (first + second) % count))).collect(),
            false => firsts
                .iter()
                .flat_map(|&first| seconds.iter().flat_map(move |&second| (0..count).map(move |third| (first, second, third))))
                .collect(),
        };
        all.into_iter().map(|(first, second, third)| place([rotate(first), rotate(second), rotate(third)])).collect()
    };
    let edge_triples: Vec<[[u128; 4]; 3]> = edges.iter().map(|edge| place(edge.map(fill))).collect();
    let some: Vec<[[u128; 4]; 3]> = (0..values.len())
        .map(|third| place([rotate(0), rotate(0), rotate(third)]))
        .chain(edge_triples.iter().copied())
        .collect();
    let datum = |[destination, first, second]: [[u128; 4]; 3], mask: EvexMask, control: EmbeddedControl, mxcsr: u32| {
        OperandValue::EvexFusedData { destination, first, second, mask, control, mxcsr }
    };
    let unmasked = 0;
    if !register {
        return (0..values.len())
            .map(|third| place([rotate(0), rotate(0), rotate(third)]))
            .flat_map(|triple| [MXCSR_DEFAULT, unmasked].map(|mxcsr| datum(triple, NO_MASK, EmbeddedControl::None, mxcsr)))
            .collect();
    }
    let every_second: Vec<usize> = (0..values.len()).collect();
    let mut all: Vec<OperandValue> = triples(&every_second)
        .into_iter()
        .chain(edge_triples.iter().copied())
        .map(|triple| datum(triple, NO_MASK, EmbeddedControl::None, MXCSR_DEFAULT))
        .collect();
    let plain: Vec<u32> = generate_mxcsr_values(MxcsrSet::Arithmetic)
        .into_iter()
        .filter_map(|value| match value {
            OperandValue::Mxcsr(mxcsr) if mxcsr != MXCSR_DEFAULT => Some(mxcsr),
            _ => None,
        })
        .collect();
    for mxcsr in plain {
        all.extend(some.iter().map(|&triple| datum(triple, NO_MASK, EmbeddedControl::None, mxcsr)));
    }
    let toward_zero = (MXCSR_DEFAULT & !(3 << ROUNDING_SHIFT)) | (3 << ROUNDING_SHIFT);
    if rounding {
        for (control, other_mode) in [
            (RoundingControl::RoundToNearest, toward_zero),
            (RoundingControl::RoundDown, MXCSR_DEFAULT),
            (RoundingControl::RoundUp, MXCSR_DEFAULT),
            (RoundingControl::RoundTowardZero, MXCSR_DEFAULT),
        ] {
            for mxcsr in [other_mode, unmasked, MXCSR_DEFAULT | DENORMALS_ARE_ZEROS | FLUSH_TO_ZERO] {
                all.extend(some.iter().map(|&triple| datum(triple, NO_MASK, EmbeddedControl::Rounding(control), mxcsr)));
            }
        }
    }
    let all_elements = to_all(elements);
    for mask in [
        EvexMask { register: 1, value: EVERY_OTHER & all_elements, zeroing: false },
        EvexMask { register: 7, value: EVERY_OTHER & all_elements, zeroing: true },
    ] {
        for mxcsr in [MXCSR_DEFAULT, unmasked] {
            all.extend(some.iter().map(|&triple| datum(triple, mask, EmbeddedControl::None, mxcsr)));
        }
    }
    all
}

/// What the data of a compare is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexCompareData {
    pub precision: Precision,
    pub elements: usize,
    pub scalar: bool,
    /// VCMPPS and its kin, whose immediate is a predicate; else VCOMISS and its kin, which have none.
    pub predicates: bool,
    pub register: bool,
    pub suppression: bool,
}

/// The data of a compare: every pair of critical values meeting in some element, under every predicate, with MXCSR
/// at its default; then the second source's rotations against the first's first under a signaling and a quiet
/// predicate with MXCSR's compare values (every exception masked and unmasked, DAZ, every flag set), with exception
/// suppression, and with every other element selected. A memory form takes the rotations with three predicates and
/// MXCSR at its default and unmasked.
pub fn generate_compare_data(data: EvexCompareData) -> Vec<OperandValue> {
    let EvexCompareData { precision, elements, scalar, predicates, register, suppression } = data;
    let values: Vec<u64> = rotate_floats(precision, true).into_iter().map(|vector| vector as u64).collect();
    let bytes = (precision.to_bits() / u8::BITS) as usize;
    let lanes = if scalar { 16 / bytes } else { elements };
    let rotate = |rotation: usize| {
        let mut vector = [0; 4];
        for index in 0..lanes {
            write_element(&mut vector, index, bytes, values[(index + rotation) % values.len()]);
        }
        vector
    };
    let step = if scalar { 1 } else { lanes };
    let every: Vec<([u128; 4], [u128; 4])> = (0..values.len())
        .step_by(step)
        .flat_map(|first| (0..values.len()).map(move |second| (first, second)))
        .map(|(first, second)| (rotate(first), rotate(second)))
        .collect();
    let some: Vec<([u128; 4], [u128; 4])> = (0..values.len()).map(|second| (rotate(0), rotate(second))).collect();
    let datum = |(first, second): ([u128; 4], [u128; 4]), immediate: u8, mask, control, mxcsr| {
        OperandValue::EvexCompareData { first, second, immediate, mask, control, mxcsr }
    };
    let all_predicates: Vec<u8> = if predicates { (0..32).collect() } else { vec![0] };
    let (signaling, quiet) = if predicates { (1, 0x11) } else { (0, 0) };
    let compare_values = [MXCSR_DEFAULT, 0, MXCSR_DEFAULT | DENORMALS_ARE_ZEROS, MXCSR_DEFAULT | ((INEXACT << 1) - 1)];
    if !register {
        return some
            .iter()
            .flat_map(|&pair| {
                [0, signaling, quiet].into_iter().flat_map(move |immediate| {
                    [MXCSR_DEFAULT, 0].map(|mxcsr| datum(pair, immediate, NO_MASK, EmbeddedControl::None, mxcsr))
                })
            })
            .collect();
    }
    let mut all: Vec<OperandValue> = every
        .iter()
        .flat_map(|&pair| all_predicates.iter().map(move |&immediate| datum(pair, immediate, NO_MASK, EmbeddedControl::None, MXCSR_DEFAULT)))
        .collect();
    for immediate in [signaling, quiet] {
        for mxcsr in &compare_values[1..] {
            all.extend(some.iter().map(|&pair| datum(pair, immediate, NO_MASK, EmbeddedControl::None, *mxcsr)));
        }
        if suppression {
            for mxcsr in [0, MXCSR_DEFAULT | DENORMALS_ARE_ZEROS] {
                all.extend(some.iter().map(|&pair| datum(pair, immediate, NO_MASK, EmbeddedControl::Suppression, mxcsr)));
            }
        }
    }
    if predicates {
        let all_elements = to_all(elements);
        for mask in [
            EvexMask { register: 1, value: EVERY_OTHER & all_elements, zeroing: false },
            EvexMask { register: 7, value: !EVERY_OTHER & all_elements, zeroing: false },
        ] {
            for mxcsr in [MXCSR_DEFAULT, 0] {
                all.extend(some.iter().map(|&pair| datum(pair, signaling, mask, EmbeddedControl::None, mxcsr)));
            }
        }
    }
    all
}

/// What the data of a conversion is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexConvertData {
    pub operation: EvexOperation,
    pub elements: usize,
    pub register: bool,
    pub rounding: bool,
    pub suppression: bool,
    pub maskable: bool,
}

/// A source element's critical values: a floating-point source's sixteen and, for a conversion to integers, each
/// side of both ends of the integer's range and the halves that round either way; for a narrowing, the values that
/// round to the narrower precision's overflow or underflow; an integer source's edges and the integers on and past
/// a rounding tie of the floating point; a source of bfloat16, the singles on, past and either side of a tie.
fn list_source_values(operation: EvexOperation) -> Vec<u64> {
    let floats = |precision: Precision| -> Vec<u64> {
        rotate_floats(precision, true).into_iter().map(|vector| vector as u64).collect()
    };
    let float_bits = |precision: Precision, exponent: i32, fraction: u64, negative: bool| {
        let digits = precision.to_significand_bits() as i32;
        let bias = (1i32 << (precision.to_bits() as i32 - digits - 1)) - 1;
        let sign = if negative { 1u64 << (precision.to_bits() - 1) } else { 0 };
        sign | (((exponent + bias) as u64) << (digits - 1)) | fraction
    };
    match operation {
        EvexOperation::ToBfloat16 { .. } => {
            let mut values = floats(Precision::Single);
            values.extend([0x3F80_8000, 0x3F81_8000, 0x3F80_8001, 0x3F80_7FFF, 0x7F7F_FFFF, 0x0000_0001, 0x807F_FFFF, 0xFF80_0001]);
            values
        }
        EvexOperation::Convert(Conversion { from: Number::Float(precision), to, .. }) => {
            let mut values = floats(precision);
            let all_fraction = (1u64 << (precision.to_significand_bits() - 1)) - 1;
            match to {
                Number::Signed(bytes) | Number::Unsigned(bytes) => {
                    let bits = 8 * bytes as i32;
                    let largest = (1i32 << (precision.to_bits() as i32 - precision.to_significand_bits() as i32 - 1)) - 1;
                    for exponent in [bits - 1, bits].into_iter().filter(|exponent| *exponent <= largest) {
                        values.extend([
                            float_bits(precision, exponent, 0, false),
                            float_bits(precision, exponent - 1, all_fraction, false),
                            float_bits(precision, exponent, 0, true),
                            float_bits(precision, exponent, 1, true),
                        ]);
                    }
                    values.extend([
                        float_bits(precision, -1, 0, false),
                        float_bits(precision, -1, 0, true),
                        float_bits(precision, 0, all_fraction / 2 + 1, true),
                        float_bits(precision, 0, all_fraction / 2 + 1, false),
                        float_bits(precision, 1, all_fraction / 4 + 1, false),
                        float_bits(precision, -2, 0, true),
                    ]);
                }
                Number::Float(Precision::Half) => values.extend(list_edges_into_half(precision)),
                Number::Float(narrower) if narrower.to_bits() < precision.to_bits() => {
                    values.extend(crate::float::generate_conversion_edges(LegacyConversion::DoubleToSingle, precision));
                }
                Number::Float(_) => {}
            }
            values
        }
        EvexOperation::Convert(Conversion { from: Number::Signed(bytes) | Number::Unsigned(bytes), to, .. }) => {
            let mask = if bytes == 8 { u64::MAX } else { (1 << (8 * bytes)) - 1 };
            let signed_maximum = mask >> 1;
            let mut values = [0, 1, 2, mask, signed_maximum, signed_maximum + 1, (1 << 24) + 1, (1 << 24) + 3, (1 << 53) + 1, (1 << 53) + 3, ((1 << 24) + 1u64).wrapping_neg(), 0x5AA5_C33C_0FF0_9669]
                .map(|value| value & mask)
                .to_vec();
            if to == Number::Float(Precision::Half) {
                // A half's ties at its significand's edge, the largest half, and the integers that round past it.
                values.extend([(1 << 11) + 1, (1 << 11) + 3, 65504, 65519, 65520, (1u64 << 11).wrapping_neg() - 1].map(|value| value & mask));
            }
            values
        }
        _ => unreachable!("only conversions"),
    }
}

/// A wider precision's values at a half's own edges: those that round to the largest half or past it, to the
/// smallest normal and subnormal half or below, at a tie and past it, a tie at one, and a signaling NaN whose payload
/// lies below the bits a half keeps.
fn list_edges_into_half(precision: Precision) -> Vec<u64> {
    let fraction_bits = precision.to_significand_bits() as i32 - 1;
    let bias = (1i32 << (precision.to_bits() as i32 - fraction_bits - 2)) - 1;
    let sign = 1u64 << (precision.to_bits() - 1);
    let normal = |exponent: i32, fraction: u64| (((exponent + bias) as u64) << fraction_bits) | fraction;
    // The fraction bit just below the last one a half keeps, and those below it.
    let below_half = |extra: i32| 1u64 << (fraction_bits - 10 - extra);
    let all = (1u64 << fraction_bits) - 1;
    vec![
        normal(15, all & !(below_half(0) - 1)),
        normal(15, all & !(below_half(1) - 1)),
        normal(15, (all & !(below_half(0) - 1)) | below_half(2)),
        normal(16, 0),
        normal(-14, 0),
        normal(-15, all),
        normal(-24, 0),
        normal(-25, 0),
        normal(-25, 1),
        normal(-26, 0),
        normal(0, below_half(1)),
        normal(0, below_half(1) | 1),
        sign | normal(0, below_half(1) | 1),
        sign | normal(-25, 1),
        precision.to_default_nan() & !sign & !precision.to_quiet_bit() | 1,
    ]
}

/// The data of a conversion: the source's critical values rotated across its elements with MXCSR's critical
/// values; then with each embedded control the form allows and with every other element selected, merging and
/// zeroing. A memory form takes the rotations with MXCSR at its default and with every exception unmasked.
pub fn generate_convert_data(data: EvexConvertData) -> Vec<OperandValue> {
    let EvexConvertData { operation, elements, register, rounding, suppression, maskable } = data;
    let values = list_source_values(operation);
    let source_bytes = match operation {
        EvexOperation::Convert(conversion) => conversion.from.to_bytes(),
        _ => 4,
    };
    let source_elements = match operation {
        EvexOperation::ToBfloat16 { pair: true } => elements / 2,
        _ => elements,
    };
    let rotate = |rotation: usize| {
        let mut vector = [0; 4];
        for index in 0..source_elements.max(1) {
            write_element(&mut vector, index, source_bytes, values[(index + rotation) % values.len()]);
        }
        vector
    };
    let pairs: Vec<([u128; 4], [u128; 4])> =
        (0..values.len()).map(|rotation| (rotate((rotation + values.len() / 2) % values.len()), rotate(rotation))).collect();
    let datum = |(first, second): ([u128; 4], [u128; 4]), mask: EvexMask, control: EmbeddedControl, mxcsr: u32| {
        OperandValue::EvexFloatData { first, second, mask, control, mxcsr }
    };
    let unmasked = 0;
    if !register {
        return pairs
            .iter()
            .flat_map(|&pair| [MXCSR_DEFAULT, unmasked].map(|mxcsr| datum(pair, NO_MASK, EmbeddedControl::None, mxcsr)))
            .collect();
    }
    let plain: Vec<u32> = generate_mxcsr_values(MxcsrSet::Arithmetic)
        .into_iter()
        .filter_map(|value| match value {
            OperandValue::Mxcsr(mxcsr) => Some(mxcsr),
            _ => None,
        })
        .collect();
    let mut all: Vec<OperandValue> =
        pairs.iter().flat_map(|&pair| plain.iter().map(move |&mxcsr| datum(pair, NO_MASK, EmbeddedControl::None, mxcsr))).collect();
    let toward_zero = (MXCSR_DEFAULT & !(3 << ROUNDING_SHIFT)) | (3 << ROUNDING_SHIFT);
    let mut controls = Vec::new();
    if rounding {
        controls.extend([
            (EmbeddedControl::Rounding(RoundingControl::RoundToNearest), toward_zero),
            (EmbeddedControl::Rounding(RoundingControl::RoundDown), MXCSR_DEFAULT),
            (EmbeddedControl::Rounding(RoundingControl::RoundUp), MXCSR_DEFAULT),
            (EmbeddedControl::Rounding(RoundingControl::RoundTowardZero), MXCSR_DEFAULT),
        ]);
    } else if suppression {
        controls.push((EmbeddedControl::Suppression, MXCSR_DEFAULT));
    }
    for (control, other_mode) in controls {
        for mxcsr in [other_mode, unmasked, MXCSR_DEFAULT | DENORMALS_ARE_ZEROS | FLUSH_TO_ZERO] {
            all.extend(pairs.iter().map(|&pair| datum(pair, NO_MASK, control, mxcsr)));
        }
    }
    if maskable {
        let all_elements = to_all(elements);
        for mask in [
            EvexMask { register: 1, value: EVERY_OTHER & all_elements, zeroing: false },
            EvexMask { register: 7, value: EVERY_OTHER & all_elements, zeroing: true },
        ] {
            for mxcsr in [MXCSR_DEFAULT, unmasked] {
                all.extend(pairs.iter().map(|&pair| datum(pair, mask, EmbeddedControl::None, mxcsr)));
            }
        }
    }
    all
}

/// What the data of an integer EVEX form is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexIntegerData {
    pub operation: IntegerOperation,
    pub element_bytes: usize,
    pub vector_bytes: usize,
    /// A register form, whose data holds the opmask.
    pub register: bool,
    /// Whether the form takes an opmask.
    pub maskable: bool,
}

/// What a vector of an integer form's data plays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    /// Elements to be moved, each a value of its own.
    Table,
    /// Element numbers.
    Index,
    /// Numbers, at elements of this many bytes: each edge of the element (zero, one, two, the width less one and
    /// the width, the signed extremes, all ones, a pattern) rotated across the elements, then each end of the signed
    /// and unsigned range in every element.
    Lanes(usize),
    /// Quadwords about bit 52, where VPMADD52's products are cut.
    FiftyTwoBits,
    /// Elements some of which are equal: all, every other, in threes, none.
    Repeats,
    /// An opmask register's value: none, the lowest bit, every other, all, a pattern.
    Mask,
}

/// The immediates an integer form's data takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Immediates {
    None,
    /// Selectors whose bit pairs and bits differ.
    Selector,
    /// Counts on each side of every element and lane edge.
    Count,
}

/// The roles of the destination (when read), the first source and the second, and the immediates.
fn to_roles(operation: IntegerOperation, bytes: usize) -> (Option<Role>, Role, Role, Immediates) {
    use IntegerOperation as I;
    use Role::{Index, Lanes, Table};
    match operation {
        I::DotBytes { .. } => (Some(Lanes(4)), Lanes(1), Lanes(1), Immediates::None),
        I::DotWords { .. } => (Some(Lanes(4)), Lanes(2), Lanes(2), Immediates::None),
        I::MultiplyAdd52 { .. } => (Some(Lanes(8)), Role::FiftyTwoBits, Role::FiftyTwoBits, Immediates::None),
        I::MultiShiftBytes => (None, Lanes(1), Table, Immediates::None),
        I::ShiftLeftDouble | I::ShiftRightDouble => (None, Lanes(bytes), Lanes(bytes), Immediates::Count),
        I::ShiftLeftDoubleVariable | I::ShiftRightDoubleVariable => {
            (Some(Lanes(bytes)), Lanes(bytes), Lanes(bytes), Immediates::None)
        }
        I::PopulationCount | I::LeadingZeros => (None, Table, Lanes(bytes), Immediates::None),
        I::Conflict => (None, Table, Role::Repeats, Immediates::None),
        I::GaloisMultiply => (None, Lanes(1), Lanes(1), Immediates::None),
        I::GaloisAffine { .. } | I::CarryLessMultiply => (None, Lanes(bytes), Table, Immediates::Selector),
        I::Aes { .. } => (None, Table, Table, Immediates::None),
        I::ShuffleBitsToMask => (None, Table, Index, Immediates::None),
        I::BroadcastMask => (None, Table, Role::Mask, Immediates::None),
        I::Permute => (None, Index, Table, Immediates::None),
        I::PermuteTwoTablesIndexInDestination => (Some(Index), Table, Table, Immediates::None),
        I::PermuteTwoTablesIndexInFirst => (Some(Table), Index, Table, Immediates::None),
        I::PermuteInLanes | I::ShuffleBytes => (None, Table, Index, Immediates::None),
        I::PermuteQuartersByImmediate
        | I::PermuteInLanesByImmediate
        | I::ShuffleHighWords
        | I::ShuffleLowWords
        | I::ShuffleSingles
        | I::ShuffleDoubles
        | I::ShuffleBlocks => (None, Table, Table, Immediates::Selector),
        I::AlignElements | I::AlignBytes => (None, Table, Table, Immediates::Count),
        I::UnpackHigh | I::UnpackLow => (None, Table, Table, Immediates::None),
    }
}

/// An element value of its own for each element and salt, so that every moved element is told apart.
const fn to_table_element(index: usize, salt: u64, bytes: usize) -> u64 {
    let mixed = (index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt;
    let value = mixed ^ (mixed >> 29);
    if bytes >= 8 { value } else { value & ((1 << (bytes * 8)) - 1) }
}

/// The vectors of a role for elements of `bytes` in `vector_bytes`: two tables; or indexes naming each element in
/// order, in reverse, the first, the last, the next, a scattered pattern reaching both tables, and each with bits
/// set above those an index reads, a shuffle's zeroing bit among them.
fn generate_role(role: Role, bytes: usize, vector_bytes: usize, every_rotation: bool) -> Vec<[u128; 4]> {
    let bytes = match role {
        Role::Lanes(bytes) => bytes,
        Role::FiftyTwoBits => 8,
        _ => bytes,
    };
    let elements = vector_bytes / bytes;
    let make = |value: &dyn Fn(usize) -> u64| -> [u128; 4] {
        let mut vector = [0; 4];
        for index in 0..elements {
            write_element(&mut vector, index, bytes, value(index));
        }
        vector
    };
    let width_mask = if bytes >= 8 { u64::MAX } else { (1 << (bytes * 8)) - 1 };
    let above = |value: u64| (value | (0xA5A5_A5A5_A5A5_A5A5 << (elements * 2).trailing_zeros())) & width_mask;
    match role {
        Role::Table => vec![make(&|index| to_table_element(index, 0, bytes)), make(&|index| to_table_element(index, 0x5A5A, bytes))],
        Role::Index => vec![
            make(&|index| index as u64),
            make(&|index| (elements - 1 - index) as u64),
            make(&|_| 0),
            make(&|_| (elements - 1) as u64),
            make(&|index| ((index + 1) % elements) as u64),
            make(&|index| ((index * 7 + 3) % (2 * elements)) as u64),
            make(&|index| above(((index * 5 + 1) % (2 * elements)) as u64)),
            make(&|index| above((elements - 1 - index) as u64) | if index % 3 == 0 { 0x80 & width_mask } else { 0 }),
        ],
        Role::Lanes(_) | Role::FiftyTwoBits => {
            let bits = (bytes * 8) as u32;
            let signed_maximum = width_mask >> 1;
            let edges: Vec<u64> = match role {
                Role::FiftyTwoBits => {
                    let low = (1 << 52) - 1;
                    vec![0, 1, low, 1 << 51, 1 << 52, (1 << 52) | 1, low | (0xA5A << 52), u64::MAX, 0x0005_A5A5_C33C_0FF0]
                }
                _ => vec![0, 1, 2, u64::from(bits) - 1, u64::from(bits), signed_maximum, signed_maximum + 1, width_mask, 0x5AA5_C33C_0FF0_9669 & width_mask],
            };
            let rotations = if every_rotation { edges.len() } else { 1 };
            let rotated = (0..rotations).map(|rotation| make(&|index| edges[(index + rotation) % edges.len()]));
            let ends = [0, signed_maximum, signed_maximum + 1, width_mask].map(|end| make(&|_| end));
            rotated.chain(ends).collect()
        }
        Role::Repeats => vec![
            make(&|_| to_table_element(0, 0, bytes)),
            make(&|index| to_table_element(index % 2, 0, bytes)),
            make(&|index| to_table_element(index / 3, 0, bytes)),
            make(&|index| to_table_element(index, 0, bytes)),
        ],
        Role::Mask => [0, 1, EVERY_OTHER, u64::MAX, 0x8000_0000_0000_80A5].map(|mask| [u128::from(mask), 0, 0, 0]).to_vec(),
    }
}

/// The data of an integer form: each destination, first source and second source of their roles with the first
/// immediate, then each immediate with the first destination and first source against each second source; for a
/// register form also the first destination and first source against each second source with every other element
/// selected, merging and zeroing. A memory form takes the first of each against each second source.
pub fn generate_integer_data(data: EvexIntegerData) -> Vec<OperandValue> {
    let EvexIntegerData { operation, element_bytes, vector_bytes, register, maskable } = data;
    let (destination_role, first_role, second_role, immediates) = to_roles(operation, element_bytes);
    let destinations = match destination_role {
        Some(role) => generate_role(role, element_bytes, vector_bytes, false),
        None => vec![[0; 4]],
    };
    let firsts = generate_role(first_role, element_bytes, vector_bytes, false);
    let seconds = generate_role(second_role, element_bytes, vector_bytes, true);
    let immediates: &[u8] = match immediates {
        Immediates::None => &[0],
        Immediates::Selector => &[0x1B, 0x00, 0x55, 0xAA, 0xFF, 0xE4, 0x4E, 0xB1, 0x0F, 0xF0],
        Immediates::Count => &[1, 0, 2, 3, 7, 8, 9, 15, 16, 17, 31, 32, 63, 64, 0xFF],
    };
    let datum = |destination, first, second, immediate, mask| OperandValue::EvexIntegerData {
        destination,
        first,
        second,
        immediate,
        mask,
    };
    if !register {
        return seconds.iter().map(|&second| datum(destinations[0], firsts[0], second, immediates[0], NO_MASK)).collect();
    }
    let mut all = Vec::new();
    for &destination in &destinations {
        for &first in &firsts {
            for &second in &seconds {
                all.push(datum(destination, first, second, immediates[0], NO_MASK));
            }
        }
    }
    for &immediate in &immediates[1..] {
        for &second in &seconds {
            all.push(datum(destinations[0], firsts[0], second, immediate, NO_MASK));
        }
    }
    if !maskable {
        return all;
    }
    let all_elements = to_all(vector_bytes / element_bytes);
    for mask in [
        EvexMask { register: 1, value: EVERY_OTHER & all_elements, zeroing: false },
        EvexMask { register: 7, value: EVERY_OTHER & all_elements, zeroing: true },
    ] {
        for &second in &seconds {
            all.push(datum(destinations[0], firsts[firsts.len() - 1], second, immediates[0], mask));
        }
    }
    all
}

/// What the data of VCOMPRESS and VEXPAND is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvexMoveData {
    pub element_bytes: usize,
    pub vector_bytes: usize,
    /// A register form, whose data holds the opmask.
    pub register: bool,
}

/// The data of VCOMPRESS and VEXPAND: a table of elements each its own value as the vector moved, and the other
/// table in memory before; a register form takes each table with every opmask of [`list_move_masks`], merging and
/// zeroing. A memory form's opmask is an operand of its own.
pub fn generate_move_data(data: EvexMoveData) -> Vec<OperandValue> {
    let EvexMoveData { element_bytes, vector_bytes, register } = data;
    let tables = generate_role(Role::Table, element_bytes, vector_bytes, false);
    let datum = |source: [u128; 4], held: [u128; 4], mask| OperandValue::EvexIntegerData {
        destination: [0; 4],
        first: held,
        second: source,
        immediate: 0,
        mask,
    };
    if !register {
        return vec![datum(tables[0], tables[1], NO_MASK)];
    }
    let masks = list_move_masks(vector_bytes / element_bytes);
    tables.iter().zip(tables.iter().rev()).flat_map(|(&source, &held)| masks.iter().map(move |&mask| datum(source, held, mask))).collect()
}

/// A register form's opmasks: none, every other element, the upper half, the last alone and a scattered pattern,
/// each merging and zeroing, and none selected.
fn list_move_masks(elements: usize) -> Vec<EvexMask> {
    let all = to_all(elements);
    let patterns = [EVERY_OTHER & all, all & !(all >> (elements / 2)), 1 << (elements - 1), 0xA5C3_0F96_5AA5_3CF1 & all, 0];
    let mut masks = vec![NO_MASK];
    for (number, value) in patterns.into_iter().enumerate() {
        masks.push(EvexMask { register: 1 + number % 7, value, zeroing: false });
        masks.push(EvexMask { register: 7 - number % 7, value, zeroing: true });
    }
    masks
}

/// k1 to k7 for a register number of a triple or an access.
const fn to_opmask_number(number: usize) -> usize {
    1 + number % 7
}

fn to_opmask_register(number: usize) -> Register {
    Register::try_from(Register::K0 as usize + number).expect("k0 to k7")
}

/// The vector register of the width the form's operand of the index takes.
fn to_vector_register(form: &EvexForm, operand: usize, number: usize) -> Register {
    use iced_x86::OpCodeOperandKind as Kind;
    let first = match form.code.op_code().op_kinds()[operand] {
        Kind::xmm_reg | Kind::xmm_rm | Kind::xmm_vvvv | Kind::xmm_or_mem => Register::XMM0,
        Kind::ymm_reg | Kind::ymm_rm | Kind::ymm_vvvv | Kind::ymm_or_mem => Register::YMM0,
        // A general-purpose register: one of the sixteen.
        Kind::r32_reg | Kind::r32_or_mem => return Register::try_from(Register::EAX as usize + number % 16).expect("EAX to R15D"),
        Kind::r64_reg | Kind::r64_or_mem => return Register::try_from(Register::RAX as usize + number % 16).expect("RAX to R15"),
        _ => Register::ZMM0,
    };
    Register::try_from(first as usize + number).expect("XMM, YMM and ZMM 0 to 31")
}

/// Builds the state of an EVEX form from one value of each of its operand types.
pub fn build_state(form: &EvexForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        ..ZERO_PROCESSOR
    };
    fill_vectors(&mut processor);
    let (mut registers, mut access, mut layout) = ([0; 3], None, CodeLayout::PageStart);
    let (mut mask, mut broadcast, mut control) = (NO_MASK, false, EmbeddedControl::None);
    let (mut first, mut second) = ([0; 4], [0; 4]);
    let (mut destination_value, mut immediate) = (None, None);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::EvexEncoding, OperandValue::EvexEncoding(encoding)) => {
                registers = encoding.registers;
                layout = encoding.layout;
            }
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                registers[0] = chosen.register;
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::EvexRegister, OperandValue::Register(number)) => registers[1] = number,
            (OperandType::EvexMask(_), OperandValue::EvexMask(chosen)) => mask = chosen,
            (OperandType::EvexBroadcast, OperandValue::Broadcast(chosen)) => broadcast = chosen,
            (OperandType::EvexFloatData(_) | OperandType::EvexConvertData(_), OperandValue::EvexFloatData { first: one, second: other, mask: data_mask, control: chosen, mxcsr }) => {
                (first, second, control) = (one, other, chosen);
                processor.mxcsr = mxcsr;
                if data_mask.register != 0 {
                    mask = data_mask;
                }
            }
            (
                OperandType::EvexIntegerData(_) | OperandType::EvexMoveData(_),
                OperandValue::EvexIntegerData { destination: old, first: one, second: other, immediate: chosen, mask: data_mask },
            ) => {
                (destination_value, first, second, immediate) = (Some(old), one, other, Some(chosen));
                if data_mask.register != 0 {
                    mask = data_mask;
                }
            }
            (
                OperandType::EvexFusedData(_),
                OperandValue::EvexFusedData { destination: old, first: one, second: other, mask: data_mask, control: chosen, mxcsr },
            ) => {
                (destination_value, first, second, control) = (Some(old), one, other, chosen);
                processor.mxcsr = mxcsr;
                if data_mask.register != 0 {
                    mask = data_mask;
                }
            }
            (
                OperandType::EvexCompareData(_),
                OperandValue::EvexCompareData { first: one, second: other, immediate: chosen, mask: data_mask, control: embedded, mxcsr },
            ) => {
                (first, second, immediate, control) = (one, other, Some(chosen), embedded);
                processor.mxcsr = mxcsr;
                if data_mask.register != 0 {
                    mask = data_mask;
                }
            }
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    processor.opmask_registers[mask.register] = mask.value;
    if form.reads_destination()
        && let Some(value) = destination_value
    {
        write_vector(&mut processor, registers[0], value, form.vector_bytes());
    }
    if form.reads_first_source() {
        write_vector(&mut processor, registers[1], first, form.vector_bytes());
    }
    // VCOMISS and its kin compare ModRM.reg's register, which they do not write.
    if matches!(form.operation, EvexOperation::OrderedCompare { .. }) {
        write_vector(&mut processor, registers[0], first, 16);
    }
    let mut memory_mutations = Vec::new();
    let memory_bytes = if broadcast { form.memory_element_bytes } else { form.memory_bytes() };
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let (mut operand, address) = place_memory_operand(access, memory_bytes as u64, &mut processor, &mut memory_mutations);
        operand.is_broadcast = broadcast;
        let bytes: Vec<u8> = (0..memory_bytes as u64)
            .map(|offset| address.wrapping_add(offset))
            .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
            .enumerate()
            .map(|(offset, _)| read_element(if form.stores() { &first } else { &second }, offset, 1) as u8)
            .collect();
        if !bytes.is_empty() {
            memory_mutations.push(MemoryDelta::ValueAssignment {
                start: address,
                bytes: Cow::Owned(bytes),
            });
        }
        operand
    });
    // A register named twice holds the second source's value, the last written.
    if memory_operand.is_none() && form.reads_opmask() {
        processor.opmask_registers[to_opmask_number(registers[2])] = second[0] as u64;
    } else if memory_operand.is_none() && to_vector_register(form, second_source_operand(form), registers[2]).is_gpr() {
        processor.general_purpose_registers[registers[2] % 16] = read_element(&second, 0, 8);
    } else if memory_operand.is_none() {
        write_vector(&mut processor, registers[2], second, form.vector_bytes());
    }
    let code = form.code;
    let destination = match form.writes_opmask() {
        true => to_opmask_register(to_opmask_number(registers[0])),
        false => to_vector_register(form, 0, registers[0]),
    };
    let first_register = to_vector_register(form, 1, registers[1]);
    let second_operand = second_source_operand(form);
    let second_register = match form.reads_opmask() {
        true => to_opmask_register(to_opmask_number(registers[2])),
        false => to_vector_register(form, second_operand, registers[2]),
    };
    let immediate = form.has_immediate().then(|| u32::from(immediate.unwrap_or(0)));
    if form.stores() {
        write_vector(&mut processor, registers[1], second, form.vector_bytes());
    }
    let mut encoded = match (memory_operand, form.reads_first_source(), immediate) {
        // VCOMPRESS into memory: the memory operand first, then the vector it packs.
        (Some(operand), _, None) if form.stores() => Encoded::with2(code, operand, to_vector_register(form, 1, registers[1])),
        (Some(operand), true, None) => Encoded::with3(code, destination, first_register, operand),
        (Some(operand), false, None) => Encoded::with2(code, destination, operand),
        (None, true, None) => Encoded::with3(code, destination, first_register, second_register),
        (None, false, None) => Encoded::with2(code, destination, second_register),
        (Some(operand), true, Some(value)) => Encoded::with4(code, destination, first_register, operand, value),
        (Some(operand), false, Some(value)) => Encoded::with3(code, destination, operand, value),
        (None, true, Some(value)) => Encoded::with4(code, destination, first_register, second_register, value),
        (None, false, Some(value)) => Encoded::with3(code, destination, second_register, value),
    }
    .unwrap_or_else(|error| panic!("{code:?} takes its operands: {error}"));
    if mask.register != 0 {
        encoded.set_op_mask(to_opmask_register(mask.register));
        encoded.set_zeroing_masking(mask.zeroing && code.op_code().can_use_zeroing_masking());
    }
    match control {
        EmbeddedControl::None => {}
        EmbeddedControl::Suppression => encoded.set_suppress_all_exceptions(true),
        EmbeddedControl::Rounding(rounding) => encoded.set_rounding_control(rounding),
    }
    let (code_page, start, bytes) = lay_out(layout, &encode_at_code_address(&encoded), to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped {
        data_access: DataAccess::ReadOnly,
        instruction_fetch: InstructionFetch::Allowed,
    };
    let mut all = vec![
        MemoryDelta::PermissionAssignment {
            addresses: code_page..code_page + PAGE_BYTES,
            permission: executable,
        },
        MemoryDelta::ValueAssignment {
            start,
            bytes: Cow::Owned(bytes),
        },
    ];
    all.append(&mut memory_mutations);
    ExecutionState {
        processor,
        memory_mutations: all,
    }
}

/// The index of the form's operand that is its second source.
fn second_source_operand(form: &EvexForm) -> usize {
    if form.reads_first_source() { 2 } else { 1 }
}

/// Writes the vector's low `vector_bytes` into the register, which keeps its fill above them.
fn write_vector(processor: &mut Processor, number: usize, value: [u128; 4], vector_bytes: usize) {
    let mut register = processor.read_zmm(number);
    register[..vector_bytes / 16].copy_from_slice(&value[..vector_bytes / 16]);
    processor.write_zmm(number, register);
}

/// Fills every ZMM register with a value no critical value is: register n's 128-bit parts each the complement of
/// n plus 32 times the part's index, rotated by the part's index in bytes; and each opmask register with its number
/// in every byte, so that a form that writes or keeps one is seen to.
pub fn fill_vectors(processor: &mut Processor) {
    for number in 0..32 {
        let value = array::from_fn(|part| (!((number + 32 * part) as u128)).rotate_left(8 * part as u32));
        processor.write_zmm(number, value);
    }
    processor.opmask_registers = array::from_fn(|number| 0x0101_0101_0101_0101 * number as u64);
}
