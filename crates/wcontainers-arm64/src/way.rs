//! The ways of running the native output's code: where its jumps find their slots and how it calls the floor
//! (docs/native.md section 3.2, docs/native-without-virtual-machine.md sections 3.1 and 3.2).

use crate::mirror::{self, SLOT_MOVED_BITS, SLOT_SHIFT};

/// A way of running, part of a translation's cache key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Way {
    /// In a virtual machine: the entry mirror, and `brk` into the inside.
    #[default]
    Machine,
    /// Each guest process a host process: the slot table at [`PROCESS_SLOTS`], and calls into the floor's entries
    /// at [`PROCESS_ENTRIES`].
    Process,
}

/// Guest addresses below 2^`PROCESS_REACH_BITS` have a slot in the process way: its ceiling.
pub const PROCESS_REACH_BITS: u32 = 44;
/// The process way's slot table: the slot of guest address `g` at `PROCESS_SLOTS | g << SLOT_SHIFT`.
pub const PROCESS_SLOTS: u64 = 1 << 46;
/// The floor's first entry in a host process; the entry of a call with code `c` is `c` entries further.
pub const PROCESS_ENTRIES: u64 = (1 << 45) + 16;
/// The bytes of one floor entry.
pub const PROCESS_ENTRY_BYTES: u64 = 16;

const _: () = assert!(PROCESS_SLOTS.trailing_zeros() >= PROCESS_REACH_BITS + SLOT_SHIFT);
// A slot's moves build a piece's address under the slot's own top bits (mirror::SLOT_MOVED_BITS).
const _: () = assert!(PROCESS_SLOTS >> SLOT_MOVED_BITS == PROCESS_ENTRIES >> SLOT_MOVED_BITS);

impl Way {
    /// Guest addresses below 2^this have a slot.
    pub const fn reach_bits(self) -> u32 {
        match self {
            Self::Machine => mirror::REACH_BITS,
            Self::Process => PROCESS_REACH_BITS,
        }
    }

    /// Where the slots start: every slot's address is this with the guest address shifted in.
    pub const fn slot_table(self) -> u64 {
        match self {
            Self::Machine => mirror::MIRROR,
            Self::Process => PROCESS_SLOTS,
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
}
