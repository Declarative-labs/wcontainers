//! Where both forms put their own pages in a host process.

use wcontainers_abi::KIB;

/// The first address above the guest area; our own pages lie from here to the slot table.
pub const OUR_AREA: usize = 1 << 44;

/// The zygote's pages: its code, then its stack.
pub const ZYGOTE: usize = 1 << 45;

/// Room for the zygote's code.
pub const ZYGOTE_CODE_SIZE: usize = 64 * KIB as usize;

/// The zygote's stack, which every stub inherits until it has its own.
pub const ZYGOTE_STACK_SIZE: usize = 64 * KIB as usize;

/// Everything the zygote keeps mapped.
pub const ZYGOTE_SIZE: usize = ZYGOTE_CODE_SIZE + ZYGOTE_STACK_SIZE;

const _: () = assert!(ZYGOTE >= OUR_AREA);
