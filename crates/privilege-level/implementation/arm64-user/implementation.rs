#![no_std]

use core::convert::Infallible;
use core::iter::{self, Empty};

use wcontainers_privilege_level::PrivilegeLevel;

/// [EL0](https://developer.arm.com/documentation/102412/0103/Privilege-and-Exception-levels/Exception-levels#:~:text=the%20lowest%20level%20of%20privilege%20is%20referred%20to%20as%20EL0), where ordinary programs run.
///
/// Every higher level can do what it can, so it adds nothing to what every level has.
pub struct User;

impl PrivilegeLevel for User {
    type PrivilegedProcessorSubset = ();
    type PrivilegedInstruction = Infallible;
    type PrecisePrivilegedInstruction = Infallible;
    type SpecificInstructions = Empty<Infallible>;

    // VERUS: Verus knows no `Clone` of `()` nor `PartialEq` of `Infallible`, so this impl stays outside `verus!` and its
    // proof items are placeholders that exist only when Verus builds the crate, which reads none of them.
    #[cfg(verus_keep_ghost)]
    fn equality_is_identity() {}

    #[cfg(verus_keep_ghost)]
    fn clone_is_identity() {}

    fn list_specific_instructions() -> Empty<Infallible> {
        iter::empty()
    }
}
