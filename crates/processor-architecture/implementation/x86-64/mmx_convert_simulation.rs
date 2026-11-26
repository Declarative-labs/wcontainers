//! The skeleton of the conversions between MMX and XMM registers and of MASKMOVQ: #MF while an x87 exception is
//! pending for a form that names an MMX register, the source read (16 bytes aligned to 16), the x87 state put in
//! MMX's use, the two lanes converted under MXCSR with #XM where an exception is unmasked, and the destination
//! written.

use iced_x86::{Instruction as Decoded, OpKind, Register};
use wcontainers_processor_architecture::MemoryDelta;

use super::{Memory, MemoryOperand, SSE_ALIGNMENT, check_access, check_canonical, expect_code, find_fault, locate};
use crate::behavior::StoreAcrossFault;
use crate::float::{Arrangement, Conversion, FloatForm, FloatOperation, Precision};
use crate::memory::read_bytes;
use crate::mmx::{enter_mmx, is_pending};
use crate::mmx_convert::{MmxConvertBehavior, MmxConvertOperation, find_mmx_convert_form};
use crate::planted::{Bug, is_active};
use crate::sse::MaskedStoreFaults;
use crate::{Access, Exception, Flags, Instruction, Processor, X86_64};

const DESTINATION_INDEX: usize = 7;
const QUADWORD_BYTES: usize = 8;

pub(super) fn execute_mmx_convert(
    instruction: Instruction,
    behavior: MmxConvertBehavior,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let form = find_mmx_convert_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no form of this family"));
    expect_code(decoded, &[form.code]);
    if form.names_mmx() && is_pending(&processor.x87) && !is_active(Bug::MmxConvertIgnoresPendingException) {
        return Err(Exception::X87FloatingPoint);
    }
    let MmxConvertOperation::Convert(conversion) = form.operation else {
        // MASKMOVQ puts the x87 state in MMX's use before its store, which stays so where the store faults.
        if !is_active(Bug::MmxConvertEntersMmxLast) {
            enter_mmx(&mut processor.x87, None);
        }
        store_masked(behavior, processor, memory, decoded, on_memory_mutated)?;
        enter_mmx(&mut processor.x87, None);
        return Ok(());
    };
    let source: u128 = match decoded.op1_kind() {
        OpKind::Register if decoded.op1_register().is_mm() => u128::from(processor.x87.registers[decoded.op1_register().number()] as u64),
        OpKind::Register => processor.vector_registers[decoded.op1_register().number()],
        _ => {
            let operand = locate(processor, decoded);
            let length = form.memory_bytes();
            if length == 16 && !operand.address.is_multiple_of(SSE_ALIGNMENT) && !is_active(Bug::MmxConvertMisalignedNoFault) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            check_access(processor.flags, memory, operand, length, Access::Read)?;
            u128::from_le_bytes(read_bytes(memory, operand.address)) & (u128::MAX >> (128 - 8 * length))
        }
    };
    // Once the source is read the chip puts the x87 state in MMX's use, which stays so where the conversion raises #XM.
    if form.names_mmx() && !is_active(Bug::MmxConvertEntersMmxLast) || is_active(Bug::ConvertFromMemoryEntersMmx) {
        enter_mmx(&mut processor.x87, None);
    }
    // Two lanes of the source; the SSE conversion's lanes above them hold zeros, which convert exactly.
    let lanes = match (conversion, form.precision) {
        (Conversion::FloatToInteger { .. }, Precision::Double) => source,
        _ => source & u128::from(u64::MAX),
    };
    let sse = FloatForm {
        instruction,
        operation: FloatOperation::Convert(conversion),
        precision: form.precision,
        arrangement: Arrangement::Packed,
        place: form.place,
        code: form.code,
    };
    let (result, flags) = match sse.apply(0, lanes, processor.mxcsr, 0) {
        Ok(converted) => converted,
        Err(flags) => {
            processor.mxcsr |= flags;
            return Err(Exception::SimdFloatingPoint);
        }
    };
    processor.mxcsr |= flags;
    let destination = decoded.op0_register().number();
    match (conversion, form.precision) {
        (Conversion::IntegerToFloat, Precision::Single) => {
            let kept = if is_active(Bug::ConvertToSinglesZeroesUpperHalf) { 0 } else { processor.vector_registers[destination] & !u128::from(u64::MAX) };
            processor.vector_registers[destination] = kept | (result & u128::from(u64::MAX));
        }
        (Conversion::IntegerToFloat, Precision::Double) => processor.vector_registers[destination] = result,
        _ => {
            enter_mmx(&mut processor.x87, Some((destination, result as u64)));
            return Ok(());
        }
    }
    if form.names_mmx() {
        enter_mmx(&mut processor.x87, None);
    }
    Ok(())
}

/// MASKMOVQ's store: the selected bytes of the data register at DS:RDI, each checked byte canonical, the whole
/// aligned under AC, and the bytes below a fault written by the behavior's rule.
fn store_masked(
    behavior: MmxConvertBehavior,
    processor: &Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
) -> Result<(), Exception> {
    let rdi = processor.general_purpose_registers[DESTINATION_INDEX];
    let offset = if decoded.op0_kind() == OpKind::MemorySegEDI { rdi & u64::from(u32::MAX) } else { rdi };
    let base = match decoded.memory_segment() {
        Register::FS => processor.fs_base,
        Register::GS => processor.gs_base,
        _ => 0,
    };
    let address = offset.wrapping_add(base);
    let read_mmx = |register: Register| processor.x87.registers[register.number()] as u64;
    let data = read_mmx(decoded.op1_register()).to_le_bytes();
    let mask = read_mmx(decoded.op2_register()).to_le_bytes();
    let selected = |index: usize| mask[index] & 0x80 != 0 || is_active(Bug::MaskedQuadwordWritesEveryByte);
    let checked: alloc::vec::Vec<usize> =
        (0..QUADWORD_BYTES).filter(|&index| behavior.masked_faults == MaskedStoreFaults::EveryByte || selected(index)).collect();
    if checked.is_empty() {
        return Ok(());
    }
    let byte_address = |index: usize| address.wrapping_add(index as u64);
    for &index in &checked {
        check_canonical(MemoryOperand { address: byte_address(index), stack: false }, 1)?;
    }
    if processor.flags.contains(Flags::ALIGNMENT_CHECK) && !address.is_multiple_of(QUADWORD_BYTES as u64) {
        return Err(Exception::AlignmentCheck);
    }
    let fault = checked.iter().position(|&index| find_fault(memory, byte_address(index), 1, Access::Write).is_some());
    let written: &[usize] = match fault {
        None => &checked,
        Some(position) if behavior.across_fault == StoreAcrossFault::WritesBytesBelowFault => &checked[..position],
        Some(_) => &[],
    };
    let bytes: alloc::vec::Vec<usize> = written.iter().copied().filter(|&index| selected(index)).collect();
    let mut runs = bytes.as_slice();
    while let Some(&first) = runs.first() {
        let length = runs.iter().enumerate().take_while(|(offset, index)| **index == first + offset).count();
        on_memory_mutated(MemoryDelta::ValueAssignment {
            start: byte_address(first),
            bytes: alloc::borrow::Cow::Owned(data[first..first + length].to_vec()),
        });
        runs = &runs[length..];
    }
    match fault.and_then(|position| find_fault(memory, byte_address(checked[position]), 1, Access::Write)) {
        Some((fault_address, page_present)) => Err(Exception::PageFault { access: Access::Write, page_present, address: fault_address }),
        None => Ok(()),
    }
}
