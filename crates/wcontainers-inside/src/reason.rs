//! The names of the native CPU side's slow paths, apart from the translation's own.

pub use wcontainers_guest_cpu::reason::CODE_CHANGED_AFTER_IT_RAN;

/// Memory copied whole when a memory is duplicated.
pub const MEMORY_COPIED_AT_FORK: &str = "memory copied at once when duplicated";

/// A read of the float status while its flags are not kept, under the WebAssembly output's name.
pub const FLOAT_STATUS_NOT_KEPT: &str = "exact floating point requested";

/// A piece asked of the host because none is loaded for that address.
pub const NOT_YET_TRANSLATED: &str = "code not yet translated here";

/// Every exit through the link; the detail is the last one's request word.
pub const ASKED_THE_HOST: &str = "asked the host";

/// Every loaded piece dropped to reuse the code area, because guest-physical memory ran out.
pub const CODE_AREA_EMPTIED: &str = "code area emptied";

/// Every live memory given a new identifier and the whole TLB invalidated, because identifiers ran out.
pub const IDENTIFIERS_RAN_OUT: &str = "address-space identifiers ran out";
