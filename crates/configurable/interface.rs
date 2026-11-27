#![no_std]

use vstd::prelude::*;
use wcontainers_abi::HostError;

verus! {

pub trait Configurable<Handle, Setting> {
    /// Sets one setting of what the handle names to the given state, idempotently.
    fn configure(&self, handle: &Handle, setting: Setting) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
