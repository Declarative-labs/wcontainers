//! Where registers are kept when they are not in a piece's locals: the thread block in the state memory, laid
//! out as the guest says. Every access pushes the block's place first, then reads or writes at the
//! register's fixed offset from it.

use crate::code::{Fixed, MEMORY_STATE};
use crate::layout::Layout;
use crate::thread::memarg;
use wasm_encoder::{InstructionSink, ValType};
use wcontainers_guest_cpu::{Emitter, Guest, Ty};

/// One register's place in the thread block, and the type it is read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    offset: u64,
    ty: ValType,
}

impl Slot {
    /// The slot of a register bit (not a flag).
    pub(crate) fn of_bit<G: Guest>(bit: u32) -> Slot {
        let bit = G::bit(bit);
        Slot {
            offset: bit.offset.into(),
            ty: match bit.ty {
                Ty::I32 => ValType::I32,
                Ty::I64 => ValType::I64,
                Ty::V128 => ValType::V128,
            },
        }
    }

    pub(crate) fn pc<G: Guest>() -> Slot {
        Slot {
            offset: G::PC_OFFSET as u64,
            ty: ValType::I64,
        }
    }

    /// The i32 flags word.
    pub(crate) fn flags<G: Guest>() -> Slot {
        Slot {
            offset: G::FLAGS_WORD_OFFSET as u64,
            ty: ValType::I32,
        }
    }

    /// A register kept only in the thread's state.
    pub(crate) fn control<G: Guest>(control: G::Control) -> Slot {
        Slot {
            offset: G::control_offset(control) as u64,
            ty: G::control_ty(control),
        }
    }

    pub(crate) fn ty(self) -> ValType {
        self.ty
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
        let at = self.offset;
        match self.ty {
            ValType::I32 => s.i32_load(memarg(memory, at, 2)),
            ValType::V128 => s.v128_load(memarg(memory, at, 4)),
            _ => s.i64_load(memarg(memory, at, 3)),
        };
    }

    /// [`Slot::store`] to the state memory at another index.
    pub(crate) fn store_to(self, s: &mut InstructionSink<'_>, memory: u32) {
        let at = self.offset;
        match self.ty {
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

/// Push one flag, 0 or 1, from the block's flags word.
pub(crate) fn load_flag<G: Guest>(s: &mut InstructionSink<'_>, block: Fixed, flag: G::Flag) {
    let (_, bit, _) = G::FLAGS[G::flag_index(flag)];
    block.push(s);
    Slot::flags::<G>().load(s);
    s.i32_const(bit as i32).i32_shr_u();
    if bit != 31 {
        s.i32_const(1).i32_and();
    }
}

/// Push the flags word from the tracked flags, each pushed by `flag` as 0 or 1, in `Guest::FLAGS` order.
pub(crate) fn push_word<G: Guest>(
    s: &mut InstructionSink<'_>,
    mut flag: impl FnMut(&mut InstructionSink<'_>, G::Flag),
) {
    for (i, &(f, bit, _)) in G::FLAGS.iter().enumerate() {
        flag(s, f);
        s.i32_const(bit as i32).i32_shl();
        if i > 0 {
            s.i32_or();
        }
    }
}

/// Write the tracked flags to the block, each pushed as 0 or 1 by `push`.
pub(crate) fn store_flags<G: Guest, E: Emitter<G>>(
    e: &mut E,
    block: Fixed,
    push: impl Fn(&mut E, G::Flag),
) {
    block.push(&mut e.sink());
    for (i, &(flag, bit, _)) in G::FLAGS.iter().enumerate() {
        push(e, flag);
        let mut s = e.sink();
        s.i32_const(bit as i32).i32_shl();
        if i > 0 {
            s.i32_or();
        }
    }
    Slot::flags::<G>().store(&mut e.sink());
}
