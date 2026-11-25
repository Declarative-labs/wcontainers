//! The EVEX skeleton of [`crate::evex`]: the opmask register selects the elements computed; the others keep the
//! destination's value under merging and are zeroed under zeroing; bits beyond the vector are zeroed.
//!
//! A memory source is read element by element, and only the selected elements' bytes are checked, so that a fault
//! on an element the mask leaves out is suppressed; a broadcast source is one element, read if any element is
//! selected. Alignment checking looks at the whole reference: the vector, or the one element of a broadcast or a
//! scalar form. Without an opmask the reference is checked as a legacy form's access (its first byte canonical, then
//! its alignment, then the rest); with one, every selected byte is canonical before the alignment is checked.

use iced_x86::{Instruction as Decoded, OpKind, RoundingControl};

use super::{Memory, MemoryOperand, check_access, check_alignment, check_canonical, expect_code, locate};
use crate::evex::{EvexForm, EvexOperation, find_evex_form, read_element, write_element};
use crate::float::{Arrangement, FLAGS, MASK_SHIFT, ROUNDING_SHIFT};
use crate::memory::read_bytes;
use crate::planted::{Bug, is_active};
use crate::{Access, Exception, Flags, Instruction, Processor};

/// Every exception mask of MXCSR, which exception suppression sets.
const ALL_MASKS: u32 = FLAGS << MASK_SHIFT;

/// Applies the form to the processor; changes it only when it returns `Ok`, but for the flags #XM leaves in MXCSR.
pub(super) fn execute_evex(
    instruction: Instruction,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    let form = find_evex_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no EVEX form"));
    expect_code(decoded, &[form.code]);
    let elements = form.elements();
    let bytes = form.element_bytes();
    let selected = read_selection(processor, decoded, elements);
    let destination = decoded.op0_register().number();
    let source_operand = decoded.op_count() - 1;
    let second = match decoded.op_kind(source_operand) {
        OpKind::Register => processor.read_zmm(decoded.op_register(source_operand).number()),
        _ => read_memory_source(&form, processor, memory, decoded, selected)?,
    };
    let first = match form.reads_first_source() {
        true => processor.read_zmm(decoded.op1_register().number()),
        false => second,
    };
    let EvexOperation::Float { operation, precision, arrangement } = form.operation;
    let (mxcsr, suppressed) = to_control(processor.mxcsr, decoded);
    let mut result = processor.read_zmm(destination);
    let (mut before, mut after) = (0, 0);
    for index in 0..elements {
        if selected & (1 << index) == 0 {
            if decoded.zeroing_masking() && !is_active(Bug::EvexZeroingMerges) {
                write_element(&mut result, index, bytes, 0);
            }
            continue;
        }
        let outcome = operation.compute(
            read_element(&first, index, bytes),
            read_element(&second, index, bytes),
            precision,
            mxcsr,
            0,
        );
        before |= outcome.before;
        after |= outcome.after;
        write_element(&mut result, index, bytes, outcome.bits);
    }
    if !suppressed || is_active(Bug::EvexSuppressedExceptionsFlagged) {
        let unmasked = |flags: u32| flags & !(processor.mxcsr >> MASK_SHIFT) & FLAGS != 0;
        if unmasked(before) {
            processor.mxcsr |= before;
            return Err(Exception::SimdFloatingPoint);
        }
        if unmasked(before | after) {
            processor.mxcsr |= before | after;
            return Err(Exception::SimdFloatingPoint);
        }
        processor.mxcsr |= before | after;
    }
    let written = elements * bytes;
    if arrangement == Arrangement::Scalar {
        // The elements above the first come from the first source; the bits beyond 128 are zeroed.
        let low = first[0] & !((1u128 << (written * 8)) - 1);
        result = [(result[0] & ((1u128 << (written * 8)) - 1)) | low, 0, 0, 0];
        if is_active(Bug::EvexScalarKeepsUpperBits) {
            let old = processor.read_zmm(destination);
            result[1..].copy_from_slice(&old[1..]);
        }
    } else {
        result.iter_mut().skip(written * 8 / 128).for_each(|part| *part = 0);
    }
    let target = if is_active(Bug::EvexHighRegistersAliasLow) { destination % 16 } else { destination };
    processor.write_zmm(target, result);
    Ok(())
}

/// The elements the opmask register selects, bit by bit from the lowest; every element without one.
fn read_selection(processor: &Processor, decoded: &Decoded, elements: usize) -> u64 {
    let all = if elements >= 64 { u64::MAX } else { (1 << elements) - 1 };
    let mask = decoded.op_mask();
    if mask == iced_x86::Register::None || is_active(Bug::EvexMaskIgnored) {
        return all;
    }
    processor.read_opmask(mask.number()) & all
}

/// MXCSR as the computation reads it, and whether exceptions are suppressed: an embedded rounding control replaces
/// the rounding mode and, as exception suppression does, masks every exception; their flags are not recorded.
fn to_control(mxcsr: u32, decoded: &Decoded) -> (u32, bool) {
    let mode = match decoded.rounding_control() {
        RoundingControl::None => None,
        RoundingControl::RoundToNearest => Some(0),
        RoundingControl::RoundDown => Some(1),
        RoundingControl::RoundUp => Some(2),
        RoundingControl::RoundTowardZero => Some(3),
    };
    match mode {
        Some(_) if is_active(Bug::EvexEmbeddedRoundingIgnored) => (mxcsr | ALL_MASKS, true),
        Some(mode) => ((mxcsr & !(3 << ROUNDING_SHIFT)) | (mode << ROUNDING_SHIFT) | ALL_MASKS, true),
        None if decoded.suppress_all_exceptions() => (mxcsr | ALL_MASKS, true),
        None => (mxcsr, false),
    }
}

/// The memory source as a vector: the selected elements read, or the broadcast element in every element.
fn read_memory_source(
    form: &EvexForm,
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    selected: u64,
) -> Result<[u128; 4], Exception> {
    let operand = locate(processor, decoded);
    let bytes = form.element_bytes();
    let broadcast = decoded.is_broadcast() && !is_active(Bug::EvexBroadcastReadsVector);
    let reference = if broadcast { bytes } else { form.memory_bytes() };
    let unchecked = processor.flags.difference(Flags::ALIGNMENT_CHECK);
    let mut vector = [0; 4];
    let read = |index: usize| selected & (1 << index) != 0 || is_active(Bug::EvexMaskedFaultNotSuppressed);
    let to_element = |index: usize| {
        let at = if broadcast { 0 } else { index * bytes };
        MemoryOperand { address: operand.address.wrapping_add(at as u64), stack: operand.stack }
    };
    if decoded.op_mask() == iced_x86::Register::None {
        // Unmasked, the reference is checked as a legacy form's access is.
        check_access(processor.flags, memory, operand, reference as u64, Access::Read)?;
    } else {
        // Masked, the selected elements' bytes must be canonical before the reference's alignment is checked.
        if (0..form.elements()).any(read) {
            for index in (0..form.elements()).filter(|index| read(*index)) {
                check_canonical(to_element(index), bytes as u64)?;
            }
            check_alignment(processor.flags, operand, reference as u64)?;
        }
    }
    for index in (0..form.elements()).filter(|index| read(*index)) {
        let element = to_element(index);
        check_access(unchecked, memory, element, bytes as u64, Access::Read)?;
        let value: [u8; 8] = read_bytes(memory, element.address);
        write_element(&mut vector, index, bytes, u64::from_le_bytes(value));
    }
    Ok(vector)
}
