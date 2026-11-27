#![no_std]

#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
use vstd::prelude::*;

verus! {

/// The stateful parts and instructions of a processor that one privilege level has beyond the unprivileged ones.
///
/// An unprivileged instruction is expected to behave the same at every privilege level.
#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait PrivilegeLevel {
    /// The stateful parts of the processor that only this level has.
    type PrivilegedProcessorSubset: Clone + Eq;
    /// The instructions that this level adds.
    type PrivilegedInstruction: Copy + Eq;
    /// Those instructions, each with a pinned behavior.
    type PrecisePrivilegedInstruction: Copy + Eq;
    // VERUS: no `impl Trait` in a trait, so the returned iterator is a named type.
    type SpecificInstructions: Iterator<Item = Self::PrivilegedInstruction>;

    /// `==` on the instructions this level adds holds exactly for the same value.
    proof fn equality_is_identity()
        ensures
            obeys_concrete_eq::<Self::PrivilegedInstruction>(),
    ;

    /// A clone of the stateful parts only this level has is the same value.
    proof fn clone_is_identity()
        ensures
            vstd::pervasive::obeys_clone_eq::<Self::PrivilegedProcessorSubset>(),
    ;

    /// Returns the set of instructions whose behavior is specific to this level.
    fn list_specific_instructions() -> Self::SpecificInstructions;
}

} // verus!
