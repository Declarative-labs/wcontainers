#![no_std]

use wcontainers_abi::HostError;

#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait Configurable<Handle, Setting> {
    /// Sets one setting of what the handle names to the given state, idempotently.
    fn configure(&self, handle: &Handle, setting: Setting) -> Result<(), HostError>;
}
