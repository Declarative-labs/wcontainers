//! Where registers are kept when they are not in a piece's locals: the thread block in the state memory, laid
//! out as [`wcontainers_abi::RegisterBlock`]. Every access pushes the block's place first, then reads or
//! writes at the register's fixed offset from it.

use crate::code::{Fixed, MEMORY_STATE};
use crate::thread::memarg;
use core::mem::offset_of;
use wasm_encoder::{InstructionSink, ValType};
use wcontainers_abi::RegisterBlock as B;
use wcontainers_arm64::{Emitter, Flag};

/// One register's place in the thread block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    X(u8),
    Sp,
    Pc,
    /// The low half of `pstate`, which holds the four flag bits; the high half is always zero.
    Flags,
    Vector(u8),
    ThreadPointer,
}

impl Slot {
    /// The slot of a register bit as `wcontainers_arm64::Registers` numbers them (not the flags).
    pub(crate) fn of_bit(bit: u32) -> Slot {
        match bit {
            0..31 => Slot::X(bit as u8),
            31 => Slot::Sp,
            _ => Slot::Vector((bit - 64) as u8),
        }
    }

    fn offset(self) -> u64 {
        (match self {
            Slot::X(n) => offset_of!(B, x) + 8 * usize::from(n),
            Slot::Sp => offset_of!(B, sp),
            Slot::Pc => offset_of!(B, pc),
            Slot::Flags => offset_of!(B, pstate),
            Slot::Vector(n) => offset_of!(B, v) + 16 * usize::from(n),
            Slot::ThreadPointer => offset_of!(B, tpidr_el0),
        }) as u64
    }

    pub(crate) fn ty(self) -> ValType {
        match self {
            Slot::Flags => ValType::I32,
            Slot::Vector(_) => ValType::V128,
            _ => ValType::I64,
        }
    }

    /// Read the register, with the block's place already pushed.
    pub(crate) fn load(self, s: &mut InstructionSink<'_>) {
        self.load_from(s, MEMORY_STATE);
    }

    /// Write the register, with the block's place and then the value pushed.
    pub(crate) fn store(self, s: &mut InstructionSink<'_>) {
        self.store_to(s, MEMORY_STATE);
    }

    /// [`Slot::load`] from the state memory at another index.
    pub(crate) fn load_from(self, s: &mut InstructionSink<'_>, memory: u32) {
        let at = self.offset();
        match self.ty() {
            ValType::I32 => s.i32_load(memarg(memory, at, 2)),
            ValType::V128 => s.v128_load(memarg(memory, at, 4)),
            _ => s.i64_load(memarg(memory, at, 3)),
        };
    }

    /// [`Slot::store`] to the state memory at another index.
    pub(crate) fn store_to(self, s: &mut InstructionSink<'_>, memory: u32) {
        let at = self.offset();
        match self.ty() {
            ValType::I32 => s.i32_store(memarg(memory, at, 2)),
            ValType::V128 => s.v128_store(memarg(memory, at, 4)),
            _ => s.i64_store(memarg(memory, at, 3)),
        };
    }

    /// Read the register into a local.
    pub(crate) fn load_into(self, s: &mut InstructionSink<'_>, block: Fixed, local: u32) {
        block.push(s);
        self.load(s);
        s.local_set(local);
    }

    /// Write a local to the register.
    pub(crate) fn store_from(self, s: &mut InstructionSink<'_>, block: Fixed, local: u32) {
        block.push(s);
        s.local_get(local);
        self.store(s);
    }
}

/// Push one flag, 0 or 1, from the block.
pub(crate) fn load_flag(s: &mut InstructionSink<'_>, block: Fixed, flag: Flag) {
    block.push(s);
    Slot::Flags.load(s);
    s.i32_const(flag.bit() as i32).i32_shr_u();
    if flag != Flag::N {
        s.i32_const(1).i32_and();
    }
}

/// Write the four flags to the block, each pushed as 0 or 1 by `push`.
pub(crate) fn store_flags<E: Emitter>(e: &mut E, block: Fixed, push: impl Fn(&mut E, Flag)) {
    block.push(&mut e.sink());
    for (i, flag) in Flag::ALL.into_iter().enumerate() {
        push(e, flag);
        let mut s = e.sink();
        s.i32_const(flag.bit() as i32).i32_shl();
        if i > 0 {
            s.i32_or();
        }
    }
    Slot::Flags.store(&mut e.sink());
}
