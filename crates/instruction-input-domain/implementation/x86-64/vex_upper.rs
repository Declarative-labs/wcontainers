//! The vector state no VEX form names: bits 256 to 511 of ZMM0 to ZMM15, which a VEX write zeroes, and ZMM16 to
//! ZMM31, which VZEROUPPER and VZEROALL leave. A VEX form's state holds a value of its own in each, so that a model
//! that keeps them where the chip zeroes them, or zeroes them where the chip keeps them, is seen to; no VEX form
//! reads them, so one value is enough.

use iced_x86::{Decoder, DecoderOptions, EncodingKind};
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_x86_64::memory::read_bytes;
use wcontainers_processor_architecture_x86_64::{VECTOR_REGISTERS, X86_64};

/// The longest instruction.
const MAXIMUM_INSTRUCTION_BYTES: usize = 15;
const BITNESS: u32 = 64;

/// Fills the state's ZMM bits above 255 and ZMM16 to ZMM31 where its instruction is VEX-encoded and they hold zeros.
pub(crate) fn fill_upper_bits(mut state: ExecutionState<'static, X86_64>) -> ExecutionState<'static, X86_64> {
    let address = state.processor.instruction_pointer;
    let bytes: [u8; MAXIMUM_INSTRUCTION_BYTES] = read_bytes(&state.memory_mutations, address);
    let decoded = Decoder::with_ip(BITNESS, &bytes, address, DecoderOptions::NONE).decode();
    let processor = &mut state.processor;
    let untouched = processor.zmm_upper_halves.iter().flatten().chain(processor.high_vector_registers.iter().flatten()).all(|part| *part == 0);
    if decoded.encoding() == EncodingKind::VEX && untouched {
        processor.zmm_upper_halves = core::array::from_fn(|number| [to_part(number, 2), to_part(number, 3)]);
        processor.high_vector_registers = core::array::from_fn(|number| core::array::from_fn(|part| to_part(VECTOR_REGISTERS + number, part)));
    }
    state
}

/// A value distinct for each register and 128-bit part, with no zero byte.
const fn to_part(number: usize, part: usize) -> u128 {
    0xC3C3_C3C3_C3C3_C3C3_C3C3_C3C3_C3C3_C3C3 ^ ((number as u128) << 8 | part as u128 | 0x10_1010_1010)
}
