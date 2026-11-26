//! What the translator derives from a guest's description, once per guest at compile time: the parameters of
//! a piece, the function and type indices of a code module, and the register sets the analyses use.

use core::marker::PhantomData;
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

/// How many bits a register set has.
const REGISTER_BITS: usize = size_of::<Registers>() * u8::BITS as usize;

/// Each bit's index in `bits`, 255 for a bit not used.
const fn bit_index(bits: &[Bit]) -> [u8; REGISTER_BITS] {
    let mut at = [u8::MAX; REGISTER_BITS];
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

/// What the translator derives from a guest `G`, as constants and functions of this type.
pub(crate) struct Layout<G>(PhantomData<G>);

impl<G: Guest> Layout<G> {
    /// How many registers a piece takes as its first parameters.
    pub(crate) const PASSED: u32 = G::PASSED_IN.len() as u32;
    /// How many it gives back as its results.
    pub(crate) const RESULTS: u32 = {
        // The stack pointer goes out as it comes in.
        position(G::PASSED_OUT, G::SP_BIT);
        G::PASSED_OUT.len() as u32
    };
    /// The parameter local of the stack pointer.
    pub(crate) const L_SP: u32 = position(G::PASSED_IN, G::SP_BIT);
    /// i64 parameter: the return address the caller expects, with `FLAGS_WANTED` when it reads the flags.
    pub(crate) const L_RETURN: u32 = Self::PASSED;
    /// i64 parameter: the bytes of the engine's stack the piece's callers use, as estimated.
    pub(crate) const L_DEPTH: u32 = Self::PASSED + 1;
    /// How many parameters a piece has: the passed registers, the expected return address and the depth;
    /// with the progress count kept, one more follows (see [`crate::code::parameters`]).
    pub(crate) const PARAMETERS: u32 = Self::PASSED + 2;
    /// i64 parameter, with the progress count kept: the count, also the piece's last result.
    pub(crate) const L_COUNT: u32 = Self::PARAMETERS;

    // Function indices: the translator's six hooks, the guest's hooks, then the dispatcher and the module's
    // own functions, then the pieces.
    pub(crate) const F_HOOKS: u32 = crate::code::TRANSLATOR_HOOKS.len() as u32;
    pub(crate) const F_DISPATCH: u32 = Self::F_HOOKS + G::HOOKS.len() as u32;
    pub(crate) const F_UNWIND: u32 = Self::F_DISPATCH + 1;
    pub(crate) const F_EXACT: u32 = Self::F_DISPATCH + 2;
    pub(crate) const F_SAVE: u32 = Self::F_DISPATCH + 3;
    pub(crate) const F_PERMISSION: u32 = Self::F_DISPATCH + 4;
    /// The functions that work out the flags word on the way to the runner: one for flags in locals, then
    /// one for each of the guest's shapes; none for a guest without flags.
    pub(crate) const F_FLAGS_WORD: u32 = Self::F_DISPATCH + 5;
    pub(crate) const FLAGS_WORDS: u32 = if G::FLAGS.is_empty() {
        0
    } else {
        1 + G::SHAPES.len() as u32
    };
    pub(crate) const F_FIRST_PIECE: u32 = Self::F_FLAGS_WORD + Self::FLAGS_WORDS;

    // Type indices: those of the translator's own hooks and functions, the guest's hooks, then the rest.
    pub(crate) const T_HOOKS: u32 = crate::code::T_UNWIND + 1;
    pub(crate) const T_SAVE: u32 = Self::T_HOOKS + G::HOOKS.len() as u32;
    pub(crate) const T_PERMISSION: u32 = Self::T_SAVE + 1;
    pub(crate) const T_FLAGS_WORD: u32 = Self::T_SAVE + 2;

    /// Every register bit, and the flags'.
    pub(crate) const ALL: Registers = Registers::of_class(G::BITS, None);
    pub(crate) const FLAG_BITS: Registers = Registers::of_class(G::BITS, Some(Class::Flag));
    pub(crate) const SEGMENT_BITS: Registers = Registers::of_class(G::BITS, Some(Class::Segment));
    /// General registers other than the stack pointer.
    pub(crate) const GENERAL_BUT_SP: Registers = general_but_sp(G::BITS, G::SP_BIT);
    /// Registers that live in a piece's locals and are tracked between its thread block copies: everything
    /// but the registers passed as parameters, and the flags, which have their own state.
    pub(crate) const TRACKED: Registers = Self::ALL
        .minus(Self::FLAG_BITS)
        .minus(Registers::of_bits(G::PASSED_IN));
    pub(crate) const BIT_INDEX: [u8; REGISTER_BITS] = bit_index(G::BITS);

    /// What the guest says of a register bit.
    pub(crate) fn bit(bit: u32) -> Bit {
        G::BITS[usize::from(Self::BIT_INDEX[bit as usize])]
    }

    /// The parameter a register bit is passed in, if it is passed.
    pub(crate) fn passed(bit: u32) -> Option<u32> {
        G::PASSED_IN
            .iter()
            .position(|&b| b == bit)
            .map(|i| i as u32)
    }

    /// The place of a tracked flag in `FLAGS`.
    pub(crate) fn flag_index(flag: G::Flag) -> usize {
        G::FLAGS
            .iter()
            .position(|f| f.0 == flag)
            .expect("a tracked flag")
    }

    /// log2 of the instruction alignment.
    pub(crate) const ALIGN_SHIFT: u32 = G::ALIGN.trailing_zeros();
}
