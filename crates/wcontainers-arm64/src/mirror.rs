//! The entry mirror the native output jumps through (docs/native.md section 3.2): a slot for every reachable
//! guest address `g` at `MIRROR | (g << SLOT_SHIFT)`, which the inside maps in its half.

/// Guest addresses below 2^`REACH_BITS` have a slot: the native ceiling.
pub const REACH_BITS: u32 = 45;
/// The bytes of one slot: three moves and a branch.
pub const SLOT_BYTES: u64 = 16;
/// A guest address shifted up this far is its slot's offset (an instruction is one 32-bit word).
pub const SLOT_SHIFT: u32 = (SLOT_BYTES / size_of::<u32>() as u64).trailing_zeros();
/// The mirror's base: every bit above the slots of the reachable addresses.
pub const MIRROR: u64 = u64::MAX << (REACH_BITS + SLOT_SHIFT);
