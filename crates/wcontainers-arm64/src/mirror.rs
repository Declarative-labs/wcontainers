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
/// Where a readable slot (the study's) keeps its piece's address, from the slot's start: its last doubleword,
/// after the load of it and the jump.
pub const SLOT_ADDRESS: u64 = SLOT_BYTES - size_of::<u64>() as u64;
/// The moves of a slot that build its piece's address in the scratch register, before its jump.
pub const SLOT_MOVES: usize = (SLOT_BYTES / size_of::<u32>() as u64) as usize - 1;
/// The bits one of a slot's moves gives: a wide move's immediate.
pub const SLOT_MOVE_BITS: u32 = u16::BITS;
/// The bits of an address a slot's moves give: the rest are the scratch register's own, which holds the slot's
/// address when the slot is entered, and must be the same in every piece's address.
pub const SLOT_MOVED_BITS: u32 = SLOT_MOVES as u32 * SLOT_MOVE_BITS;

/// The address of the entry slot of guest address `guest`.
pub const fn slot(guest: u64) -> u64 {
    MIRROR | guest << SLOT_SHIFT
}

/// The words of a return's or a call's last target through a register (docs/native.md section 3.2), by their place
/// from the first: the moves of the target into the scratch register, which the inside fills at the first miss,
/// its compare with the target, the test, the direct branch the inside links to the target's piece, and the call
/// that has the inside fill them; the jump through the target's slot follows.
pub const LAST_TARGET_TEST: usize = SLOT_MOVES + 1;
pub const LAST_TARGET_BRANCH: usize = LAST_TARGET_TEST + 1;
pub const LAST_TARGET_CALL: usize = LAST_TARGET_BRANCH + 1;
pub const LAST_TARGET_MISSED: usize = LAST_TARGET_CALL + 1;
