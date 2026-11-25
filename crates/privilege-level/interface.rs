#![no_std]

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

verus! {

/// The stateful parts and instructions of a processor that one privilege level has beyond the unprivileged ones.
///
/// An unprivileged instruction is expected to behave the same at every privilege level.
pub trait PrivilegeLevel {
    /// The stateful parts of the processor that only this level has.
    type PrivilegedProcessorSubset: Clone + Eq;
    /// The instructions that this level adds.
    type PrivilegedInstruction: Copy + Eq;
    /// Those instructions, each with a pinned behavior.
    type PrecisePrivilegedInstruction: Copy + Eq;
    // VERUS: no `impl Trait` in a trait, so the returned iterator is a named type.
    type SpecificInstructions: Iterator<Item = Self::PrivilegedInstruction>;

    /// The instructions this level adds, in the order they are listed.
    spec fn specific_instructions() -> Seq<Self::PrivilegedInstruction>;

    /// Returns the set of instructions whose behavior is specific to this level.
    fn list_specific_instructions() -> (instructions: Self::SpecificInstructions)
        ensures
            instructions.obeys_prophetic_iter_laws(),
            instructions.remaining() == Self::specific_instructions(),
            instructions.decrease() is Some,
            Self::specific_instructions().no_duplicates(),
    ;
}

} // verus!
