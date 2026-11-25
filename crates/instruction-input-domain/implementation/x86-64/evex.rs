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
use wcontainers_processor_architecture_x86_64::evex::{EvexForm, EvexOperation, read_element, write_element};
use wcontainers_processor_architecture_x86_64::float::{
    Arrangement, DENORMALS_ARE_ZEROS, FLUSH_TO_ZERO, FloatOperation, Precision, ROUNDING_SHIFT,
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
    let EvexOperation::Float { operation, precision, arrangement } = form.operation;
    let op_code = form.code.op_code();
    let register = form.place == OperandPlace::Register;
    let data = OperandType::EvexFloatData(EvexFloatData {
        operation,
        precision,
        arrangement,
        vector_bytes: form.vector_bytes(),
        register,
        rounding: op_code.can_use_rounding_control(),
        suppression: op_code.can_suppress_all_exceptions(),
    });
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
        .chain(form.reads_first_source().then_some(OperandType::EvexRegister))
        .chain([OperandType::EvexMask(form.elements())])
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

/// The register of a vector of the form's length.
fn to_vector_register(form: &EvexForm, number: usize) -> Register {
    let first = match form.vector_bytes() {
        16 => Register::XMM0,
        32 => Register::YMM0,
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
            (OperandType::EvexFloatData(_), OperandValue::EvexFloatData { first: one, second: other, mask: data_mask, control: chosen, mxcsr }) => {
                (first, second, control) = (one, other, chosen);
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
    if form.reads_first_source() {
        processor.write_zmm(registers[1], first);
    }
    let mut memory_mutations = Vec::new();
    let memory_bytes = if broadcast { form.element_bytes() } else { form.memory_bytes() };
    let memory_operand: Option<MemoryOperand> = access.map(|access| {
        let (mut operand, address) = place_memory_operand(access, memory_bytes as u64, &mut processor, &mut memory_mutations);
        operand.is_broadcast = broadcast;
        let bytes: Vec<u8> = (0..memory_bytes as u64)
            .map(|offset| address.wrapping_add(offset))
            .take_while(|byte| X86_64::ADDRESSABLE.contains(byte))
            .enumerate()
            .map(|(offset, _)| read_element(&second, offset, 1) as u8)
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
    if memory_operand.is_none() {
        processor.write_zmm(registers[2], second);
    }
    let code = form.code;
    let destination = to_vector_register(form, registers[0]);
    let first_register = to_vector_register(form, registers[1]);
    let mut encoded = match (memory_operand, form.reads_first_source()) {
        (Some(operand), true) => Encoded::with3(code, destination, first_register, operand),
        (Some(operand), false) => Encoded::with2(code, destination, operand),
        (None, true) => Encoded::with3(code, destination, first_register, to_vector_register(form, registers[2])),
        (None, false) => Encoded::with2(code, destination, to_vector_register(form, registers[2])),
    }
    .unwrap_or_else(|error| panic!("{code:?} takes its operands: {error}"));
    if mask.register != 0 {
        encoded.set_op_mask(Register::try_from(Register::K0 as usize + mask.register).expect("k1 to k7"));
        encoded.set_zeroing_masking(mask.zeroing);
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
