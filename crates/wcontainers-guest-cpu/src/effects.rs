//! What an instruction reads and writes, found by running its meaning against an emitter that only records.

use crate::{Access, Emitter, Guest, Registers, ReturnAddress, Target};
use wasm_encoder::{InstructionSink, ValType};

/// What one instruction does to the registers and to control flow.
pub struct Effects<G: Guest> {
    /// Read before this instruction writes them.
    pub uses: Registers,
    pub defs: Registers,
    /// How many times the meaning sets the flags; more than once means each is conditional.
    pub flag_settings: u32,
    /// How the flags are set, when the meaning sets them exactly once. Its locals mean nothing here.
    pub flags: Option<G::Flags>,
    /// It calls another piece of code, which may read and write every register.
    pub calls: bool,
    pub syscall: bool,
}

impl<G: Guest> Clone for Effects<G> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<G: Guest> Copy for Effects<G> {}

impl<G: Guest> core::fmt::Debug for Effects<G> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Effects")
            .field("uses", &self.uses)
            .field("defs", &self.defs)
            .field("flag_settings", &self.flag_settings)
            .field("flags", &self.flags)
            .field("calls", &self.calls)
            .field("syscall", &self.syscall)
            .finish()
    }
}

/// The effects of an instruction, as its meaning has them for this host and offer: what a meaning does may
/// depend on both (an instruction the offer leaves out traps and writes nothing).
pub fn effects<G: Guest>(
    instruction: &G::Instruction,
    host: G::Host,
    offer: G::Offer,
) -> Effects<G> {
    let mut recorder = Recorder::<G> {
        effects: Effects {
            uses: Registers::NONE,
            defs: Registers::NONE,
            flag_settings: 0,
            flags: None,
            calls: false,
            syscall: false,
        },
        host,
        offer,
        scratch: Vec::new(),
        locals: 0,
    };
    G::emit(&mut recorder, instruction);
    recorder.effects
}

struct Recorder<G: Guest> {
    effects: Effects<G>,
    host: G::Host,
    offer: G::Offer,
    scratch: Vec<u8>,
    locals: u32,
}

impl<G: Guest> Recorder<G> {
    fn read(&mut self, bit: u32) {
        if !self.effects.defs.has_bit(bit) {
            self.effects.uses.0 |= 1 << bit;
        }
    }

    fn write(&mut self, bit: u32) {
        self.effects.defs.0 |= 1 << bit;
    }
}

impl<G: Guest> Emitter<G> for Recorder<G> {
    fn sink(&mut self) -> InstructionSink<'_> {
        self.scratch.clear();
        InstructionSink::new(&mut self.scratch)
    }

    fn local(&mut self, _ty: ValType) -> u32 {
        self.locals += 1;
        self.locals
    }

    fn get(&mut self, r: G::Reg) {
        if let Some(b) = G::reg_bit(r) {
            self.read(b);
        }
    }

    fn set(&mut self, r: G::Reg) {
        if let Some(b) = G::reg_bit(r) {
            self.write(b);
        }
    }

    fn this_address(&mut self, _: i64) {}

    fn set_flags(&mut self, flags: G::Flags) {
        self.effects.flag_settings += 1;
        self.effects.flags = (self.effects.flag_settings == 1).then_some(flags);
        self.effects.defs = self.effects.defs.union(G::sets(flags));
    }

    fn flag(&mut self, flag: G::Flag) {
        let &(_, _, bit) = G::FLAGS
            .iter()
            .find(|f| f.0 == flag)
            .expect("a tracked flag");
        self.read(bit);
    }

    fn get_vector(&mut self, v: G::Vector) {
        self.read(G::vector_bit(v));
    }

    fn set_vector(&mut self, v: G::Vector) {
        self.write(G::vector_bit(v));
    }

    fn get_control(&mut self, _control: G::Control) {}

    fn set_control(&mut self, _control: G::Control) {}

    fn hook(&mut self, hook: G::Hook) {
        let registers = G::HOOKS[G::hook_index(hook)].registers;
        for bit in registers.bits() {
            self.read(bit);
        }
        self.effects.defs = self.effects.defs.union(registers);
    }

    fn host(&self) -> G::Host {
        self.host
    }

    fn offer(&self) -> G::Offer {
        self.offer
    }

    fn check(&mut self, _address: u32, _bytes: u32, _access: Access, _which: u8) {}

    fn check_range(&mut self, _address: u32, _length: u32, _access: Access) -> u32 {
        self.local(ValType::I32)
    }

    fn warn(&mut self, _reason: G::Reason) {}

    fn raise_stop(&mut self) {}

    fn jump(&mut self, _target: Target) {}

    fn jump_if(&mut self, _target: Target) {}

    fn call(&mut self, _target: Target) {
        self.effects.calls = true;
    }

    fn ret(&mut self, _target: u32) {
        if let ReturnAddress::Stack { .. } = G::RETURN_ADDRESS {
            self.read(G::SP_BIT);
            self.write(G::SP_BIT);
        }
    }

    fn syscall(&mut self) {
        let registers = G::SYSCALL;
        for r in std::iter::once(registers.number).chain(registers.args) {
            self.get(r);
        }
        self.set(registers.result);
        self.effects.syscall = true;
    }

    fn trap(&mut self, _trap: G::Trap) {}

    fn next(&mut self) {}
}
