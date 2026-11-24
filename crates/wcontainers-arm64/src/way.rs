//! The ways of running the native output's code: where its jumps find their slots and how it calls the floor
//! (docs/native.md section 3.2, docs/native-without-virtual-machine.md sections 3.1 and 3.2).

use crate::mirror::{self, SLOT_MOVED_BITS, SLOT_SHIFT};
use crate::thread_state::{COUNT, PAGE, Register, RegisterPage, SCRATCH, WITHHELD};

/// A way of running, part of a translation's cache key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Way {
    /// In a virtual machine: the entry mirror, and `brk` into the inside.
    #[default]
    Machine,
    /// Each guest process a host process: the slot table at [`PROCESS_SLOTS`], and calls into the floor's entries
    /// at [`PROCESS_ENTRIES`].
    Process,
    /// The kernel in process: the guest process in slot `slot`'s window, its host address [`window_base`] plus
    /// its guest address, every access checked and cut to the window, calls into the floor's entries at
    /// [`IN_PROCESS_ENTRIES`].
    ///
    /// [`window_base`]: Way::window_base
    InProcess { slot: u16 },
}

/// Guest addresses below 2^`PROCESS_REACH_BITS` have a slot in the process way: its ceiling.
pub const PROCESS_REACH_BITS: u32 = 44;
/// The process way's slot table: the slot of guest address `g` at `PROCESS_SLOTS | g << SLOT_SHIFT`.
pub const PROCESS_SLOTS: u64 = 1 << 46;
/// The floor's first entry in a host process; the entry of a call with code `c` is `c` entries further.
pub const PROCESS_ENTRIES: u64 = (1 << 45) + 16;
/// The bytes of one floor entry.
pub const PROCESS_ENTRY_BYTES: u64 = 16;

/// The bits of a guest address in the in-process way's window: its ceiling.
pub const WINDOW_BITS: u32 = u32::BITS;
/// Slot `k` of the in-process way starts at `k << SLOT_BITS`: its window, then as many bytes never mapped.
pub const SLOT_BITS: u32 = WINDOW_BITS + 1;
/// The most slots of the in-process way: every slot's window is below 2^[`PROCESS_REACH_BITS`].
pub const IN_PROCESS_SLOTS: u16 = (1 << (PROCESS_REACH_BITS - SLOT_BITS)) - 1;
/// The floor's first entry in process, past the process way's zygote; the entry of code `c` is `c` entries further.
pub const IN_PROCESS_ENTRIES: u64 = (1 << 45) + (1 << 32) + PROCESS_ENTRY_BYTES;
/// The in-process way's mark, at this many bytes past a window's base: translated code reads a byte there when an
/// access's address or a new stack pointer is outside the window, so that the floor finds the instruction's own
/// fault or takes the stack pointer outside its window.
pub const OUTSIDE: u64 = 1 << WINDOW_BITS;

/// The in-process way's second scratch register u, which its window check writes.
pub const CHECK: Register = 15;
/// The in-process way's register holding the window's base.
pub const BASE: Register = 24;
/// The in-process way's withheld registers: the other ways' three, then [`CHECK`] and [`BASE`], whose guest values
/// are the two words after the register page ([`IN_PROCESS_WITHHELD_AT`]). The two are the general registers named
/// least after the three by real programs' code (docs/native.md section 3.3: x15 29.2 and x24 30.3 per 1000).
const IN_PROCESS_WITHHELD: [Register; 5] = [COUNT, SCRATCH, PAGE, CHECK, BASE];
/// Where the in-process way keeps the guest values of its two more withheld registers: just after the page.
pub const IN_PROCESS_WITHHELD_AT: usize = size_of::<RegisterPage>();

const _: () = assert!(PROCESS_SLOTS.trailing_zeros() >= PROCESS_REACH_BITS + SLOT_SHIFT);
// A slot's moves build a piece's address under the slot's own top bits (mirror::SLOT_MOVED_BITS).
const _: () = assert!(PROCESS_SLOTS >> SLOT_MOVED_BITS == PROCESS_ENTRIES >> SLOT_MOVED_BITS);
const _: () = assert!(PROCESS_SLOTS >> SLOT_MOVED_BITS == IN_PROCESS_ENTRIES >> SLOT_MOVED_BITS);
// The withheld registers the ways share come first, so the register page holds them alike.
const _: () = assert!(
    IN_PROCESS_WITHHELD[0] == WITHHELD[0]
        && IN_PROCESS_WITHHELD[1] == WITHHELD[1]
        && IN_PROCESS_WITHHELD[2] == WITHHELD[2]
);

impl Way {
    /// Guest addresses below 2^this have a slot.
    pub const fn reach_bits(self) -> u32 {
        match self {
            Self::Machine => mirror::REACH_BITS,
            Self::Process => PROCESS_REACH_BITS,
            Self::InProcess { .. } => WINDOW_BITS,
        }
    }

    /// Where the slots start: every slot's address is this with the guest address shifted in. In process, the
    /// slots of the window's host addresses.
    pub const fn slot_table(self) -> u64 {
        match self {
            Self::Machine => mirror::MIRROR,
            Self::Process => PROCESS_SLOTS,
            Self::InProcess { .. } => PROCESS_SLOTS | self.window_base() << SLOT_SHIFT,
        }
    }

    /// The address of the entry slot of guest address `guest`.
    pub const fn slot(self, guest: u64) -> u64 {
        self.slot_table() | guest << SLOT_SHIFT
    }

    /// The guest address whose slot is at `slot`.
    pub const fn slot_target(self, slot: u64) -> u64 {
        (slot & !self.slot_table()) >> SLOT_SHIFT
    }

    /// The host address of guest address 0: the window's base in process, 0 in the other ways.
    pub const fn window_base(self) -> u64 {
        match self {
            Self::Machine | Self::Process => 0,
            Self::InProcess { slot } => (slot as u64) << SLOT_BITS,
        }
    }

    /// The registers translated code keeps from the guest, in the order their guest values are kept.
    pub const fn withheld(self) -> &'static [Register] {
        match self {
            Self::Machine | Self::Process => &WITHHELD,
            Self::InProcess { .. } => &IN_PROCESS_WITHHELD,
        }
    }
}
