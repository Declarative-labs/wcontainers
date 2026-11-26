//! The skeleton of RCPPS to RSQRTSS: the source read from a register or memory (16 bytes aligned to 16 for a packed
//! form), each lane looked up in the chip's table, and the destination's lanes written, a scalar form's upper lanes
//! kept.

use iced_x86::{Instruction as Decoded, OpKind};

use super::{Memory, SSE_ALIGNMENT, check_access, expect_code, locate};
use crate::memory::read_bytes;
use crate::reciprocal::{ReciprocalTable, compute, find_reciprocal_form};
use crate::{Access, Exception, Instruction, Processor};

pub(super) fn execute_reciprocal(
    instruction: Instruction,
    table: &ReciprocalTable,
    processor: &mut Processor,
    memory: Memory<'_, '_>,
    decoded: &Decoded,
) -> Result<(), Exception> {
    let form = find_reciprocal_form(instruction).unwrap_or_else(|| panic!("{instruction:?} is no form of this family"));
    expect_code(decoded, &[form.code]);
    let source = match decoded.op1_kind() {
        OpKind::Register => processor.vector_registers[decoded.op1_register().number()],
        _ => {
            let operand = locate(processor, decoded);
            let length = form.memory_bytes();
            if form.packed && !operand.address.is_multiple_of(SSE_ALIGNMENT) {
                return Err(Exception::GeneralProtection { error_code: 0 });
            }
            check_access(processor.flags, memory, operand, length, Access::Read)?;
            u128::from_le_bytes(read_bytes(memory, operand.address)) & (u128::MAX >> (128 - 8 * length))
        }
    };
    let number = decoded.op0_register().number();
    let lanes = if form.packed { 4 } else { 1 };
    let mut result = processor.vector_registers[number];
    for lane in 0..lanes {
        let shift = 32 * lane;
        let value = compute(table, form.root, (source >> shift) as u32);
        result = (result & !(u128::from(u32::MAX) << shift)) | u128::from(value) << shift;
    }
    processor.vector_registers[number] = result;
    Ok(())
}
