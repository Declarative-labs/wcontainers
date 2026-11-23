//! The thread area translated code reaches on its own segment (docs/x86-native-output.md section 2): the words
//! guest code reads and writes, at fixed offsets from the segment's base.

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// The register area, at the start of the thread area.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, FromBytes, IntoBytes, Immutable, KnownLayout,
)]
#[repr(C)]
pub struct RegisterArea {
    /// The guest's GS base, which the hardware's never holds in any way of running.
    pub gs_base: u64,
}

/// The guest's GS base, as an offset from the thread area's segment base.
pub const GUEST_GS_BASE: usize = core::mem::offset_of!(RegisterArea, gs_base);
