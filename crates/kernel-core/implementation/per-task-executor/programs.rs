//! The byte-code programs the suite runs, and the accessors of the byte-code processor it is given.

use wcontainers_kernel_core_conformance::Program;
use wcontainers_kernel_core_per_task_executor::byte_code::{ByteCodeProcessor, Instruction};

/// How many times the counting program counts and calls, which fills most of one page.
const COUNTING_ROUNDS: usize = 0x7F;
/// How many instructions that do nothing the lingering program runs before its system call.
const LINGERING_STEPS: u64 = 0x400_0000;
const SYSTEM_CALL: &[u8] = &[Instruction::SystemCall as u8];
/// A program that makes a system call only after a long run of instructions that do nothing.
pub const LINGERING: Program = Program {
    length: LINGERING_STEPS + 1,
    offset: LINGERING_STEPS,
    bytes: SYSTEM_CALL,
};

/// Returns a program that adds one to the counter and makes a system call, over and over.
pub fn count() -> Vec<u8> {
    [Instruction::Count as u8, Instruction::SystemCall as u8].repeat(COUNTING_ROUNDS)
}

pub fn set_program_counter(processor: &mut ByteCodeProcessor, address: u64) {
    processor.program_counter = address;
}

pub fn read_counter(processor: &ByteCodeProcessor) -> u64 {
    processor.counter
}

pub fn set_counter(processor: &mut ByteCodeProcessor, counter: u64) {
    processor.counter = counter;
}
