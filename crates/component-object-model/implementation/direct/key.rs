//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The key to the glue's bookkeeping: serving, counting and giving back the C side's objects. Its only constructor
//! is `unsafe`, so code under `forbid(unsafe_code)`, which every implementation is, cannot hold one; each generated
//! `direct` crate makes one, into a private static it never hands out.

/// A token that only generated glue holds.
#[derive(Clone, Copy)]
pub struct GlueKey(());

impl GlueKey {
    /// This crate's own key.
    pub(crate) const OWN: GlueKey = GlueKey(());

    /// Returns the key.
    ///
    /// # Safety
    /// Only a generated `direct` crate calls this, into a private static that no public item returns or passes on:
    /// with the key, code can give back references the C side holds.
    #[expect(
        unsafe_code,
        reason = "the key to the glue's bookkeeping, which code under forbid cannot make"
    )]
    pub const unsafe fn new() -> GlueKey {
        GlueKey(())
    }
}
