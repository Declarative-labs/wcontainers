//! JCXZ, which only a 16-bit address size encodes: a program reaches it from Linux's 32-bit code segment, where the
//! prefix 0x67 makes the address size 16. The jump is taken where CX is zero; the target wraps at the operand size, 32
//! bits, or 16 under the prefix 0x66, and the instruction pointer past an untaken jump at 32 bits.
//!
//! A step that starts in compatibility mode leaves RSP's upper half clear, where the manual leaves it undefined: the
//! frame of the trap or fault that ends it holds ESP (measured on this chip); the other registers keep theirs.

use iced_x86::{Code, Decoder, DecoderOptions};

use super::{Memory, expect_code};
use crate::memory::read_bytes;
use crate::planted::{Bug, is_active};
use crate::{Exception, Processor, X86_64};

/// RCX, whose low 16 bits JCXZ tests.
const COUNT: usize = 1;
const STACK_POINTER: usize = 4;
/// Linux's 32-bit code segment.
const COMPATIBILITY_CODE_SEGMENT: u16 = 0x23;
const BITNESS: u32 = 32;

pub(super) fn execute_jump_if_count_zero(processor: &mut Processor, memory: Memory<'_, '_>) -> Result<(), Exception> {
    let bytes: [u8; 15] = read_bytes::<X86_64, 15>(memory, processor.instruction_pointer);
    let decoded = Decoder::with_ip(BITNESS, &bytes, processor.instruction_pointer, DecoderOptions::NONE).decode();
    expect_code(&decoded, &[Code::Jcxz_rel8_32, Code::Jcxz_rel8_16]);
    let count_mask = if is_active(Bug::CountJumpTestsDoubleword) { u64::from(u32::MAX) } else { u64::from(u16::MAX) };
    let count = processor.general_purpose_registers[COUNT] & count_mask;
    let next = decoded.next_ip() & u64::from(u32::MAX);
    let displacement = i64::from(bytes[decoded.len() - 1] as i8);
    let sixteen = decoded.code() == Code::Jcxz_rel8_16 && !is_active(Bug::SixteenBitJumpTargetWhole);
    let target_mask = if sixteen { u64::from(u16::MAX) } else { u64::from(u32::MAX) };
    processor.instruction_pointer = if count == 0 { next.wrapping_add(displacement as u64) & target_mask } else { next };
    Ok(())
}

/// Clears RSP's upper half after a step that started in Linux's 32-bit code segment.
pub(super) fn clear_stack_upper_half(start: &Processor, processor: &mut Processor) {
    if start.code_segment == COMPATIBILITY_CODE_SEGMENT && !is_active(Bug::CompatibilityStepKeepsStackUpperHalf) {
        processor.general_purpose_registers[STACK_POINTER] &= u64::from(u32::MAX);
    }
}
