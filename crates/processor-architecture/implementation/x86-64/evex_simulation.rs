//! The EVEX skeleton of [`crate::evex`]: the opmask register selects the elements computed; the others keep the
//! destination's value under merging and are zeroed under zeroing; bits beyond the vector are zeroed.
//!
//! A memory source is read element by element, and only the selected elements' bytes are checked, so that a fault
//! on an element the mask leaves out is suppressed; a broadcast source is one element, read if any element is
//! selected. Alignment checking looks at the whole reference: the vector, or the one element of a broadcast or a
//! scalar form. Without an opmask the reference is checked as a legacy form's access (its first byte canonical, then
//! its alignment, then the rest); with one, every selected byte is canonical before the alignment is checked.

use iced_x86::{Instruction as Decoded, OpKind, RoundingControl};

use super::{Memory, MemoryOperand, check_access, check_alignment, check_canonical, expect_code, locate, read_register, write_register};
use crate::evex::{EvexForm, EvexOperation, find_evex_form, read_element, write_element, writes_register};
use crate::evex::fused::multiply_add;
use crate::evex::compare::compare;
use crate::evex::special::{SpecialOperation, compute_special};
use crate::evex::convert::to_bfloat16;
use crate::float::{DENORMAL, FLAGS, FloatOperation, LaneOutcome, MASK_SHIFT, ORDER_FLAGS, ROUNDING_SHIFT};

/// The order a fused form takes NaNs in: the first multiplicand's, the second's, the addend's.
const NAN_ORDER: [usize; 3] = [0, 1, 2];
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
    if let EvexOperation::Compress { .. } | EvexOperation::Expand { .. } = form.operation {
        return super::evex_compress::move_selected(&form, processor, memory, decoded, selected);
    }
    let destination = decoded.op0_register().number();
    let source_operand = if form.has_immediate() { decoded.op_count() - 2 } else { decoded.op_count() - 1 };
    let second = match decoded.op_kind(source_operand) {
        OpKind::Register if decoded.op_register(source_operand).is_gpr() => {
            [u128::from(read_register(processor, decoded.op_register(source_operand))), 0, 0, 0]
        }
        OpKind::Register if form.reads_opmask() => {
            [u128::from(processor.read_opmask(decoded.op_register(source_operand).number())), 0, 0, 0]
        }
        OpKind::Register => processor.read_zmm(decoded.op_register(source_operand).number()),
        _ => read_memory_source(&form, processor, memory, decoded, selected)?,
    };
    let first = match form.reads_first_source() {
        true => processor.read_zmm(decoded.op1_register().number()),
        false => second,
    };
    let to_register = writes_register(&form);
    let old = if form.writes_opmask() || to_register { [0; 4] } else { processor.read_zmm(destination) };
    let mut result = old;
    let computed = match form.operation {
        EvexOperation::Float { operation, precision, .. } => {
            compute_float(processor, decoded, bytes, selected, elements, |index, mxcsr, bytes| {
                operation.compute(read_element(&first, index, bytes), read_element(&second, index, bytes), precision, mxcsr, 0)
            })?
        }
        EvexOperation::Compare { precision, .. } => {
            let predicate = decoded.immediate8();
            let lanes = compute_float(processor, decoded, bytes, selected, elements, |index, mxcsr, bytes| {
                let first = read_element(&first, index, bytes);
                compare(first, read_element(&second, index, bytes), precision, mxcsr, predicate)
            })?;
            let bits = (0..elements).fold(0u64, |mask, index| mask | ((read_element(&lanes, index, bytes) & 1) << index));
            [u128::from(bits), 0, 0, 0]
        }
        EvexOperation::OrderedCompare { precision, signaling } => {
            let operation = FloatOperation::OrderedCompare { signaling };
            // The first operand is ModRM.reg's register, which the form compares rather than writes.
            let first = processor.read_zmm(destination);
            let flags = compute_float(processor, decoded, bytes, 1, 1, |_, mxcsr, bytes| {
                operation.compute(read_element(&first, 0, bytes), read_element(&second, 0, bytes), precision, mxcsr, 0)
            })?;
            let order = Flags::from_bits_truncate(flags[0] as u64 & ORDER_FLAGS);
            processor.flags = processor.flags.difference(Flags::STATUS) | order;
            return Ok(());
        }
        EvexOperation::Convert(conversion) => {
            let from = form.source_element_bytes();
            let reports_denormal = form.reports_denormal() || is_active(Bug::HalfToSingleReportsDenormal);
            compute_float(processor, decoded, bytes, selected, elements, |index, mxcsr, _| {
                let outcome = conversion.convert(read_element(&second, index, from), mxcsr);
                LaneOutcome { before: if reports_denormal { outcome.before } else { outcome.before & !DENORMAL }, ..outcome }
            })?
        }
        EvexOperation::ToBfloat16 { pair } => {
            let mut converted = [0; 4];
            for index in 0..elements {
                let (source, at) = match pair && index >= elements / 2 {
                    true => (&first, index - elements / 2),
                    false => (&second, index),
                };
                write_element(&mut converted, index, 2, u64::from(to_bfloat16(read_element(source, at, 4) as u32)));
            }
            converted
        }
        EvexOperation::Fused { kind, order, precision, .. } => {
            compute_float(processor, decoded, bytes, selected, elements, |index, mxcsr, bytes| {
                let [a, b, c] = order.arrange(&old, &first, &second).map(|vector| read_element(vector, index, bytes));
                let (negate_product, negate_addend) = kind.to_signs(index);
                multiply_add(a, b, c, negate_product, negate_addend, precision, mxcsr, NAN_ORDER)
            })?
        }
        EvexOperation::Compress { .. } | EvexOperation::Expand { .. } => unreachable!("moved above"),
        EvexOperation::Special { operation, precision, .. } => {
            let immediate = if form.has_immediate() { decoded.immediate8() } else { 0 };
            let lanes = compute_float(processor, decoded, bytes, selected, elements, |index, mxcsr, bytes| {
                compute_special(operation, read_element(&first, index, bytes), read_element(&second, index, bytes), precision, mxcsr, immediate)
            })?;
            if operation == SpecialOperation::Class {
                let bits = (0..elements).fold(0u64, |mask, index| mask | ((read_element(&lanes, index, bytes) & 1) << index));
                [u128::from(bits), 0, 0, 0]
            } else {
                lanes
            }
        }
        EvexOperation::Integer { operation, .. } => {
            let immediate = if form.has_immediate() { decoded.immediate8() } else { 0 };
            operation.compute(&old, &first, &second, immediate, form.vector_bytes(), bytes)
        }
    };
    if to_register {
        write_register(processor, decoded.op0_register(), read_element(&computed, 0, bytes));
        return Ok(());
    }
    if form.writes_opmask() {
        // The opmask selects by zeroing: a bit it leaves out is cleared, as is every bit beyond the elements.
        let keep = if is_active(Bug::EvexMaskDestinationMerges) { processor.read_opmask(destination) & !selected } else { 0 };
        processor.opmask_registers[destination] = (computed[0] as u64 & selected) | keep;
        return Ok(());
    }
    for index in 0..elements {
        if selected & (1 << index) != 0 {
            write_element(&mut result, index, bytes, read_element(&computed, index, bytes));
        } else if decoded.zeroing_masking() && !is_active(Bug::EvexZeroingMerges) {
            write_element(&mut result, index, bytes, 0);
        }
    }
    let written = elements * bytes;
    if form.elements() == 1 && !matches!(form.operation, EvexOperation::Integer { .. }) {
        // The elements above the first come from the first source (a fused form's from its destination); the bits
        // beyond 128 are zeroed.
        let upper = if matches!(form.operation, EvexOperation::Fused { .. }) { old[0] } else { first[0] };
        let low = upper & !((1u128 << (written * 8)) - 1);
        result = [(result[0] & ((1u128 << (written * 8)) - 1)) | low, 0, 0, 0];
        if is_active(Bug::EvexScalarKeepsUpperBits) {
            result[1..].copy_from_slice(&old[1..]);
        }
    } else {
        // Every bit from the last element written up to 512 is zeroed.
        for (number, part) in result.iter_mut().enumerate() {
            let kept = written.saturating_sub(number * 16).min(16);
            *part &= if kept == 16 { u128::MAX } else { (1u128 << (kept * 8)) - 1 };
        }
    }
    let target = if is_active(Bug::EvexHighRegistersAliasLow) { destination % 16 } else { destination };
    processor.write_zmm(target, result);
    Ok(())
}

/// Each selected element of a floating-point form computed under MXCSR or the embedded control; the flags found are
/// added to MXCSR unless suppressed, and an unmasked one raises #XM.
fn compute_float(
    processor: &mut Processor,
    decoded: &Decoded,
    bytes: usize,
    selected: u64,
    elements: usize,
    compute: impl Fn(usize, u32, usize) -> LaneOutcome,
) -> Result<[u128; 4], Exception> {
    let (mxcsr, suppressed) = to_control(processor.mxcsr, decoded);
    let mut result = [0; 4];
    let (mut before, mut after) = (0, 0);
    for index in (0..elements).filter(|index| selected & (1 << index) != 0) {
        let outcome = compute(index, mxcsr, bytes);
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
    Ok(result)
}

/// The elements the opmask register selects, bit by bit from the lowest; every element without one.
pub(super) fn read_selection(processor: &Processor, decoded: &Decoded, elements: usize) -> u64 {
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

/// The memory source as a vector: the selected elements read, or the broadcast element in every element; a form
/// that does not suppress faults reads the whole reference whatever the opmask selects.
fn read_memory_source(
    form: &EvexForm,
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    selected: u64,
) -> Result<[u128; 4], Exception> {
    let operand = locate(processor, decoded);
    let broadcast = decoded.is_broadcast() && !is_active(Bug::EvexBroadcastReadsVector);
    let bytes = if broadcast { form.memory_element_bytes } else { form.source_element_bytes() };
    let elements = if broadcast { form.vector_bytes() / bytes } else { form.memory_bytes() / bytes };
    let reference = if broadcast { bytes } else { form.memory_bytes() };
    let unchecked = processor.flags.difference(Flags::ALIGNMENT_CHECK);
    let suppresses = form.suppresses_faults() || is_active(Bug::EvexPermuteFaultsSuppressed);
    let whole = !suppresses || is_active(Bug::EvexMaskedFaultNotSuppressed);
    let selected = if whole || (broadcast && selected != 0) { u64::MAX } else { selected };
    let mut vector = [0; 4];
    let read = |index: usize| selected & (1 << index) != 0;
    let to_element = |index: usize| {
        let at = if broadcast { 0 } else { index * bytes };
        MemoryOperand { address: operand.address.wrapping_add(at as u64), stack: operand.stack }
    };
    if decoded.op_mask() == iced_x86::Register::None || whole {
        // Unmasked, the reference is checked as a legacy form's access is.
        check_access(processor.flags, memory, operand, reference as u64, Access::Read)?;
    } else if (0..elements).any(read) {
        // Masked, the selected elements' bytes must be canonical before the reference's alignment is checked.
        for index in (0..elements).filter(|index| read(*index)) {
            check_canonical(to_element(index), bytes as u64)?;
        }
        check_alignment(processor.flags, operand, reference as u64)?;
    }
    for index in (0..elements).filter(|index| read(*index)) {
        let element = to_element(index);
        check_access(unchecked, memory, element, bytes as u64, Access::Read)?;
        let value: [u8; 8] = read_bytes(memory, element.address);
        write_element(&mut vector, index, bytes, u64::from_le_bytes(value));
    }
    Ok(vector)
}
