//! The names of the native CPU side's slow paths, apart from the translation's own.

pub use wcontainers_guest_cpu::reason::{
    CODE_CHANGED_AFTER_IT_RAN, EXACT_FLOATING_POINT_REQUESTED, MEMORY_FAULT,
};

/// Memory copied whole when a memory is duplicated.
pub const MEMORY_COPIED_AT_FORK: &str = "memory copied at once when duplicated";

/// A piece asked of the host because none is loaded for that address.
pub const NOT_YET_TRANSLATED: &str = "code not yet translated here";

/// A turn that ends at a count, run one host instruction at a time so that it ends exactly there.
pub const TURN_LIMIT_STEPPED: &str = "turn with a count limit run in steps";

/// Every exit through the link; the detail is the last one's request word.
pub const ASKED_THE_HOST: &str = "asked the host";

/// Every loaded piece dropped to reuse the code area, because guest-physical memory ran out.
pub const CODE_AREA_EMPTIED: &str = "code area emptied";

/// Every live memory given a new identifier and the whole TLB invalidated, because identifiers ran out.
pub const IDENTIFIERS_RAN_OUT: &str = "address-space identifiers ran out";

/// Every request for a frame refused because guest-physical memory is used up.
pub const GUEST_PHYSICAL_SPACE_FULL: &str = "guest-physical space full";

/// Every block of memory broken into pages because one page of it changed.
pub const LARGE_ENTRY_SPLIT: &str = "large entry split";
