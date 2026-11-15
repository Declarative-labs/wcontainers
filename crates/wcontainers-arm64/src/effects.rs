//! What an instruction reads and writes, found by running its meaning against an emitter that only records.

use crate::instruction::{Instruction, Reg};
use crate::meaning::{Access, Emitter, Flag, Flags, Target, Trap, emit};
use wasm_encoder::{InstructionSink, ValType};

/// A set of registers: x0 to x30 are bits 0 to 30, sp is 31, the flags N, Z, C and V are 32 to 35, and v0 to
/// v31 are 64 to 95.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Registers(pub u128);

impl Registers {
    pub const NONE: Self = Self(0);
    pub const FLAGS: Self = Self(0xf << 32);
    pub const ALL: Self = Self(((1 << 36) - 1) | (0xffff_ffff << 64));

    pub fn bit(r: Reg) -> Option<u32> {
        match r {
            Reg::X(n) => Some(n.into()),
            Reg::Sp => Some(31),
            Reg::Zero => None,
        }
    }

    pub fn flag_bit(flag: Flag) -> u32 {
        32 + match flag {
            Flag::N => 0,
            Flag::Z => 1,
            Flag::C => 2,
            Flag::V => 3,
        }
    }

    pub fn vector_bit(n: u8) -> u32 {
        64 + u32::from(n)
    }

    pub fn of(r: Reg) -> Self {
        Self(Self::bit(r).map_or(0, |b| 1 << b))
    }

    pub fn has(self, r: Reg) -> bool {
        Self::bit(r).is_some_and(|b| self.0 >> b & 1 == 1)
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn minus(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The bits in the set, lowest first.
    pub fn bits(self) -> impl Iterator<Item = u32> {
        (0..128).filter(move |b| self.0 >> b & 1 == 1)
    }
}

/// What one instruction does to the registers and to control flow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Read before this instruction writes them.
    pub uses: Registers,
    pub defs: Registers,
    /// How many times the meaning sets the flags; more than once means each is conditional.
    pub flag_settings: u32,
    /// How the flags are set, when the meaning sets them exactly once. Its locals mean nothing here.
    pub flags: Option<Flags>,
    /// It calls another piece of code, which may read and write every register.
    pub calls: bool,
    pub syscall: bool,
}

impl Instruction {
    pub fn effects(&self) -> Effects {
        let mut recorder = Recorder {
            effects: Effects::default(),
            scratch: Vec::new(),
            locals: 0,
        };
        emit(&mut recorder, self);
        recorder.effects
    }
}

struct Recorder {
    effects: Effects,
    scratch: Vec<u8>,
    locals: u32,
}

impl Recorder {
    fn read(&mut self, bit: u32) {
        if self.effects.defs.0 >> bit & 1 == 0 {
            self.effects.uses.0 |= 1 << bit;
        }
    }

    fn write(&mut self, bit: u32) {
        self.effects.defs.0 |= 1 << bit;
    }
}

impl Emitter for Recorder {
    fn sink(&mut self) -> InstructionSink<'_> {
        self.scratch.clear();
        InstructionSink::new(&mut self.scratch)
    }

    fn local(&mut self, _ty: ValType) -> u32 {
        self.locals += 1;
        self.locals
    }

    fn get(&mut self, r: Reg) {
        if let Some(b) = Registers::bit(r) {
            self.read(b);
        }
    }

    fn set(&mut self, r: Reg) {
        if let Some(b) = Registers::bit(r) {
            self.write(b);
        }
    }

    fn this_address(&mut self) {}

    fn set_flags(&mut self, flags: Flags) {
        self.effects.flag_settings += 1;
        self.effects.flags = (self.effects.flag_settings == 1).then_some(flags);
        self.effects.defs = self.effects.defs.union(Registers::FLAGS);
    }

    fn flag(&mut self, flag: Flag) {
        self.read(Registers::flag_bit(flag));
    }

    fn get_vector(&mut self, n: u8) {
        self.read(Registers::vector_bit(n));
    }

    fn set_vector(&mut self, n: u8) {
        self.write(Registers::vector_bit(n));
    }

    fn get_thread_pointer(&mut self) {}

    fn set_thread_pointer(&mut self) {}

    fn check(&mut self, _address: u32, _bytes: u32, _access: Access) {}

    fn jump(&mut self, _target: Target) {}

    fn jump_if(&mut self, _target: Target) {}

    fn call(&mut self, _target: Target) {
        self.effects.calls = true;
    }

    fn ret(&mut self, _target: u32) {}

    fn syscall(&mut self) {
        self.read(8);
        for n in 0..6 {
            self.read(n);
        }
        self.write(0);
        self.effects.syscall = true;
    }

    fn trap(&mut self, _trap: Trap) {}

    fn next(&mut self) {}
}
