//! What a guest CPU crate supplies. Each CPU crate (`wcontainers-arm64`, later `wcontainers-x86` and a
//! RISC-V one) defines one zero-sized type and implements these traits for it; the translator, the tests and
//! the kernel are written once against them and are monomorphised for one guest, so nothing is chosen at run
//! time.
//!
//! The core, [`Personality`] and the plain types beside it, is `no_std` for the kernel. The translation half
//! ([`Guest`], [`Emitter`] and what they use) needs the standard library and wasm-encoder and is behind the
//! `translate` feature, which is on by default.

#![cfg_attr(not(feature = "translate"), no_std)]

extern crate alloc;

#[cfg(feature = "translate")]
mod effects;
#[cfg(feature = "translate")]
mod native;
#[cfg(feature = "translate")]
mod translate;

#[cfg(feature = "translate")]
pub use effects::{Effects, effects};
#[cfg(feature = "translate")]
pub use native::{
    HostCode, Native, NativeClass, PATCH_BYTES, Patch, Route, Translation, count_placement, counted,
};
pub mod position;
pub mod reason;
pub use position::{
    AddressPlace, BranchTable, DirectExit, Fixup, Held, Holds, InsideCall, NativeReason, Relocated,
    Resume, Side, Span, Stopping,
};
#[cfg(feature = "translate")]
pub use translate::*;

/// What a thread's block size is a multiple of: the bytes of a 128-bit vector.
pub const BLOCK_ALIGNMENT: usize = size_of::<u128>();

/// What is true of one guest CPU that the kernel and the runners need without translating anything: the
/// register block they see and where the thread's state words are.
pub trait Personality: Sized + Send + Sync + 'static {
    /// The saved register block (`wcontainers_abi::RegisterBlock` on ARM64).
    type Block: zerocopy::FromBytes + zerocopy::IntoBytes + zerocopy::Immutable + Copy;
    /// Where pc is in the block.
    const PC_OFFSET: usize;
    /// Where the i32 stop flag is, from the block's start (after the block).
    const STOP_FLAG_OFFSET: usize;
    /// Bytes of the block and its stop flag, a multiple of [`BLOCK_ALIGNMENT`].
    const BLOCK_SIZE: usize;
    /// Bytes after those that only the CPU side uses, zero when a thread is made. A thread's state is
    /// `BLOCK_SIZE + PRIVATE_BYTES` bytes.
    const PRIVATE_BYTES: usize;
    /// The registers a thread starts with at `pc` with stack pointer `sp` (x86: MXCSR 0x1F80, FCW 0x037F,
    /// rflags 0x202).
    fn initial_block(pc: u64, sp: u64) -> Self::Block;
}

/// A set of register bits, numbered as the guest's [`Guest::BITS`] says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Registers(pub u128);

impl Registers {
    pub const NONE: Self = Self(0);

    /// The bits of `bits` of this class, or all of them.
    pub const fn of_class(bits: &[Bit], class: Option<Class>) -> Self {
        let mut set = 0;
        let mut i = 0;
        while i < bits.len() {
            let keep = match class {
                None => true,
                Some(c) => bits[i].class as u8 == c as u8,
            };
            if keep {
                set |= 1 << bits[i].bit;
            }
            i += 1;
        }
        Self(set)
    }

    /// The set of these bits.
    pub const fn of_bits(bits: &[u32]) -> Self {
        let mut set = 0;
        let mut i = 0;
        while i < bits.len() {
            set |= 1 << bits[i];
            i += 1;
        }
        Self(set)
    }

    pub const fn has_bit(self, bit: u32) -> bool {
        self.0 >> bit & 1 == 1
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn minus(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub const fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The bits in the set, lowest first.
    pub fn bits(self) -> impl Iterator<Item = u32> {
        (0..128).filter(move |b| self.0 >> b & 1 == 1)
    }
}

/// One register bit: its number in [`Registers`] (0 to 127), what it is, how it is kept in a local, and its
/// offset in the thread block (for a flag, the flags word's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bit {
    pub bit: u32,
    pub class: Class,
    pub ty: Ty,
    pub offset: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    General,
    Flag,
    Vector,
    /// A segment base an access may add (x86 FS and GS bases), kept like a general register.
    Segment,
}

/// The WebAssembly type a register is kept in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    I32,
    I64,
    V128,
}

/// Why the instruction at `pc` (or after it, see `TrapPc`) stopped the thread. Each kind is the signal Linux
/// sends for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TrapKind {
    /// Not an instruction of the CPU we present. Detail: the instruction's bytes (`Guest::trap_detail`).
    Undefined = 0,
    /// An instruction of a feature we do not offer: illegal instruction, counted and warned. Detail: the
    /// instruction's bytes.
    Unsupported = 1,
    /// A breakpoint instruction. Detail: what the CPU reports with it (ARM64 `brk`'s comment field).
    Breakpoint = 2,
    /// An access through a stack pointer that is not a multiple of 16, or a jump to an address that is not a
    /// multiple of the instruction alignment. Detail: that stack pointer or address.
    Alignment = 3,
    /// x86 `div` or `idiv` by zero, or a quotient too large: `SIGFPE`, `FPE_INTDIV`. Detail 0.
    DivideError = 4,
    /// x86 general protection raised by the instruction itself (a privileged instruction, a non-canonical
    /// branch target, a misaligned vector access, `int n`): `SIGSEGV`, `SI_KERNEL`, address 0. Detail 0.
    GeneralProtection = 5,
    /// An unmasked floating point exception: `SIGFPE`. Detail: what the kernel works the `si_code` out from
    /// as Linux does (on x86-64 the exception flags of MXCSR that are raised and unmasked).
    FloatException = 6,
    /// x86 `int 4` (`into` has no 64-bit form): `SIGSEGV`, `SI_KERNEL`, a trap with pc after the
    /// instruction, which the kernel tells from general protection by its trap number. Detail 0.
    Overflow = 7,
}

impl TrapKind {
    pub const ALL: &'static [Self] = &[
        Self::Undefined,
        Self::Unsupported,
        Self::Breakpoint,
        Self::Alignment,
        Self::DivideError,
        Self::GeneralProtection,
        Self::FloatException,
        Self::Overflow,
    ];

    pub fn from_i32(value: i32) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| *kind as i32 == value)
    }

    /// Each kind's name where a runner reports it as text, in `ALL`'s order.
    pub const NAMES: &'static [&'static str] = &[
        "undefined",
        "unsupported",
        "breakpoint",
        "alignment",
        "divide error",
        "general protection",
        "float exception",
        "overflow",
    ];

    pub const fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.name() == name)
    }
}

/// The fault a memory fault is: a page fault at the address the memory fault hook reported, general
/// protection (address 0), or a stack fault (x86 #SS, address 0: a non-canonical access through the stack
/// segment).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryFault {
    Page,
    GeneralProtection,
    Stack,
}

impl MemoryFault {
    pub const ALL: &'static [Self] = &[Self::Page, Self::GeneralProtection, Self::Stack];
}
