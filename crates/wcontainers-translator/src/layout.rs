//! What the translator derives from a guest's description, once per guest at compile time: the parameters of
//! a piece, the function and type indices of a code module, and the register sets the analyses use.

use wcontainers_guest_cpu::{Bit, Class, Guest, Registers};

/// The position of `bit` in `list`.
const fn position(list: &[u32], bit: u32) -> u32 {
    let mut i = 0;
    while i < list.len() {
        if list[i] == bit {
            return i as u32;
        }
        i += 1;
    }
    panic!("the stack pointer is passed")
}

/// Each bit's index in `bits`, 255 for a bit not used.
const fn bit_index(bits: &[Bit]) -> [u8; 128] {
    let mut at = [u8::MAX; 128];
    let mut i = 0;
    while i < bits.len() {
        at[bits[i].bit as usize] = i as u8;
        i += 1;
    }
    at
}

const fn general_but_sp(bits: &[Bit], sp: u32) -> Registers {
    Registers::of_class(bits, Some(Class::General)).minus(Registers(1 << sp))
}

pub(crate) trait Layout: Guest {
    /// How many registers a piece takes as its first parameters.
    const PASSED: u32 = Self::PASSED_IN.len() as u32;
    /// How many it gives back as its results.
    const RESULTS: u32 = {
        // The stack pointer goes out as it comes in.
        position(Self::PASSED_OUT, Self::SP_BIT);
        Self::PASSED_OUT.len() as u32
    };
    /// The parameter local of the stack pointer.
    const L_SP: u32 = position(Self::PASSED_IN, Self::SP_BIT);
    /// i64 parameter: the return address the caller expects, with `FLAGS_WANTED` when it reads the flags.
    const L_RETURN: u32 = Self::PASSED;
    /// i64 parameter: the bytes of the engine's stack the piece's callers use, as estimated.
    const L_DEPTH: u32 = Self::PASSED + 1;
    /// How many parameters a piece has: the passed registers, the expected return address and the depth.
    const PARAMETERS: u32 = Self::PASSED + 2;

    // Function indices: the translator's six hooks, the guest's hooks, then the dispatcher and the module's
    // own functions, then the pieces.
    const F_HOOKS: u32 = 6;
    const F_DISPATCH: u32 = Self::F_HOOKS + Self::HOOKS.len() as u32;
    const F_UNWIND: u32 = Self::F_DISPATCH + 1;
    const F_EXACT: u32 = Self::F_DISPATCH + 2;
    const F_SAVE: u32 = Self::F_DISPATCH + 3;
    const F_PERMISSION: u32 = Self::F_DISPATCH + 4;
    /// The functions that work out the flags word on the way to the runner: one for flags in locals, then
    /// one for each of the guest's shapes.
    const F_FLAGS_WORD: u32 = Self::F_DISPATCH + 5;
    const FLAGS_WORDS: u32 = 1 + Self::SHAPES.len() as u32;
    const F_FIRST_PIECE: u32 = Self::F_FLAGS_WORD + Self::FLAGS_WORDS;

    // Type indices: those of the translator's own hooks and functions, the guest's hooks, then the rest.
    const T_HOOKS: u32 = 8;
    const T_SAVE: u32 = Self::T_HOOKS + Self::HOOKS.len() as u32;
    const T_PERMISSION: u32 = Self::T_SAVE + 1;
    const T_FLAGS_WORD: u32 = Self::T_SAVE + 2;

    /// Every register bit, and the flags'.
    const ALL: Registers = Registers::of_class(Self::BITS, None);
    const FLAG_BITS: Registers = Registers::of_class(Self::BITS, Some(Class::Flag));
    const SEGMENT_BITS: Registers = Registers::of_class(Self::BITS, Some(Class::Segment));
    /// General registers other than the stack pointer.
    const GENERAL_BUT_SP: Registers = general_but_sp(Self::BITS, Self::SP_BIT);
    /// Registers that live in a piece's locals and are tracked between its thread block copies: everything
    /// but the registers passed as parameters, and the flags, which have their own state.
    const TRACKED: Registers = Self::ALL
        .minus(Self::FLAG_BITS)
        .minus(Registers::of_bits(Self::PASSED_IN));
    const BIT_INDEX: [u8; 128] = bit_index(Self::BITS);

    /// What the guest says of a register bit.
    fn bit(bit: u32) -> Bit {
        Self::BITS[usize::from(Self::BIT_INDEX[bit as usize])]
    }

    /// The parameter a register bit is passed in, if it is passed.
    fn passed(bit: u32) -> Option<u32> {
        Self::PASSED_IN
            .iter()
            .position(|&b| b == bit)
            .map(|i| i as u32)
    }

    /// The place of a tracked flag in `FLAGS`.
    fn flag_index(flag: Self::Flag) -> usize {
        Self::FLAGS
            .iter()
            .position(|f| f.0 == flag)
            .expect("a tracked flag")
    }

    /// log2 of the instruction alignment.
    const ALIGN_SHIFT: u32 = Self::ALIGN.trailing_zeros();
}

impl<G: Guest> Layout for G {}
