//! Entry, the exception vectors, and the switch to the guest's level and back.

use core::arch::global_asm;
use core::mem::offset_of;

use wcontainers_abi::RegisterBlock;

/// A thread's registers at the guest's level, as the floor saves them when it takes an exception from there.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct UserRegisters {
    /// x0 to x30, as many as the guest's register block holds.
    pub x: [u64; RegisterBlock::ZERO.x.len()],
    pub sp: u64,
    pub pc: u64,
    pub pstate: u64,
}

/// What brought the guest's level back to the floor.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taken {
    /// A synchronous exception: ESR_EL1 says which.
    Synchronous,
    /// An interrupt.
    Interrupt,
    /// A fast interrupt or a system error, never expected.
    Other,
}

/// Bytes of the vector table, whose base VBAR_EL1 needs aligned to its size.
const VECTORS_BYTES: usize = 2048;

/// Vectors in the table: four kinds of exception from each of four sources.
const VECTORS: usize = 16;

/// Bytes saved for each pair of registers on the floor's stack.
const PAIR: usize = 2 * size_of::<u64>();

unsafe extern "C" {
    /// Runs the guest's level from `registers` until an exception brings it back, saving its state there.
    fn floor_enter_user(registers: *mut UserRegisters) -> Taken;
    /// The exception vectors.
    static floor_vectors: u8;
}

/// Runs the guest's level from `registers` until an exception brings it back.
pub fn enter_user(registers: &mut UserRegisters) -> Taken {
    // SAFETY: the switch saves and restores every register the floor's code relies on.
    unsafe { floor_enter_user(registers) }
}

/// The address VBAR_EL1 takes.
pub fn vectors() -> u64 {
    &raw const floor_vectors as u64
}

global_asm!(
    r#"
.section .text.entry, "ax"
.global _start
_start:
    bl floor_main
0:  b 0b

.section .text.vectors, "ax"
.balign {vectors_bytes}
.global floor_vectors
floor_vectors:
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; stp x0, x1, [sp, #-{pair}]!; mov x0, #{synchronous}; b floor_from_user
    .balign {entry}; stp x0, x1, [sp, #-{pair}]!; mov x0, #{interrupt}; b floor_from_user
    .balign {entry}; stp x0, x1, [sp, #-{pair}]!; mov x0, #{other}; b floor_from_user
    .balign {entry}; stp x0, x1, [sp, #-{pair}]!; mov x0, #{other}; b floor_from_user
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected
    .balign {entry}; b floor_unexpected

.text
floor_unexpected:
    mrs x0, esr_el1
    mrs x1, elr_el1
    mrs x2, far_el1
    bl floor_fatal
0:  b 0b

.global floor_enter_user
floor_enter_user:
    stp x29, x30, [sp, #-{pair}]!
    stp x27, x28, [sp, #-{pair}]!
    stp x25, x26, [sp, #-{pair}]!
    stp x23, x24, [sp, #-{pair}]!
    stp x21, x22, [sp, #-{pair}]!
    stp x19, x20, [sp, #-{pair}]!
    msr tpidr_el1, x0
    // Register n is at x + n words: each number below is the register's own.
    ldr x1, [x0, #{sp}]
    msr sp_el0, x1
    ldr x1, [x0, #{pc}]
    msr elr_el1, x1
    ldr x1, [x0, #{pstate}]
    msr spsr_el1, x1
    ldp x2, x3, [x0, #{x} + 2 * {word}]
    ldp x4, x5, [x0, #{x} + 4 * {word}]
    ldp x6, x7, [x0, #{x} + 6 * {word}]
    ldp x8, x9, [x0, #{x} + 8 * {word}]
    ldp x10, x11, [x0, #{x} + 10 * {word}]
    ldp x12, x13, [x0, #{x} + 12 * {word}]
    ldp x14, x15, [x0, #{x} + 14 * {word}]
    ldp x16, x17, [x0, #{x} + 16 * {word}]
    ldp x18, x19, [x0, #{x} + 18 * {word}]
    ldp x20, x21, [x0, #{x} + 20 * {word}]
    ldp x22, x23, [x0, #{x} + 22 * {word}]
    ldp x24, x25, [x0, #{x} + 24 * {word}]
    ldp x26, x27, [x0, #{x} + 26 * {word}]
    ldp x28, x29, [x0, #{x} + 28 * {word}]
    ldr x30, [x0, #{x} + 30 * {word}]
    ldp x0, x1, [x0, #{x}]
    eret

floor_from_user:
    mrs x1, tpidr_el1
    // Register n is at x + n words: each number below is the register's own.
    stp x2, x3, [x1, #{x} + 2 * {word}]
    stp x4, x5, [x1, #{x} + 4 * {word}]
    stp x6, x7, [x1, #{x} + 6 * {word}]
    stp x8, x9, [x1, #{x} + 8 * {word}]
    stp x10, x11, [x1, #{x} + 10 * {word}]
    stp x12, x13, [x1, #{x} + 12 * {word}]
    stp x14, x15, [x1, #{x} + 14 * {word}]
    stp x16, x17, [x1, #{x} + 16 * {word}]
    stp x18, x19, [x1, #{x} + 18 * {word}]
    stp x20, x21, [x1, #{x} + 20 * {word}]
    stp x22, x23, [x1, #{x} + 22 * {word}]
    stp x24, x25, [x1, #{x} + 24 * {word}]
    stp x26, x27, [x1, #{x} + 26 * {word}]
    stp x28, x29, [x1, #{x} + 28 * {word}]
    str x30, [x1, #{x} + 30 * {word}]
    ldp x2, x3, [sp], #{pair}
    stp x2, x3, [x1, #{x}]
    mrs x2, sp_el0
    str x2, [x1, #{sp}]
    mrs x2, elr_el1
    str x2, [x1, #{pc}]
    mrs x2, spsr_el1
    str x2, [x1, #{pstate}]
    ldp x19, x20, [sp], #{pair}
    ldp x21, x22, [sp], #{pair}
    ldp x23, x24, [sp], #{pair}
    ldp x25, x26, [sp], #{pair}
    ldp x27, x28, [sp], #{pair}
    ldp x29, x30, [sp], #{pair}
    ret
"#,
    vectors_bytes = const VECTORS_BYTES,
    entry = const VECTORS_BYTES / VECTORS,
    pair = const PAIR,
    word = const size_of::<u64>(),
    synchronous = const Taken::Synchronous as u64,
    interrupt = const Taken::Interrupt as u64,
    other = const Taken::Other as u64,
    x = const offset_of!(UserRegisters, x),
    sp = const offset_of!(UserRegisters, sp),
    pc = const offset_of!(UserRegisters, pc),
    pstate = const offset_of!(UserRegisters, pstate),
);
