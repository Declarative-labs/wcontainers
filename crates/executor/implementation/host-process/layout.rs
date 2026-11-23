//! Where both forms put their own pages in a host process.

use wcontainers_abi::{KIB, LOWEST_ADDRESS, PAGE_SIZE};

/// The first address above the guest area; our own pages lie from here to the slot table.
pub const OUR_AREA: usize = 1 << 44;

/// The guest's lowest address, the first a stub may map guest memory at.
pub const GUEST_START: usize = LOWEST_ADDRESS as usize;

/// The zygote's pages: its code, then its stack.
pub const ZYGOTE: usize = 1 << 45;

/// Room for the zygote's code.
pub const ZYGOTE_CODE_SIZE: usize = 64 * KIB as usize;

/// The zygote's stack, which every stub inherits until it has its own.
pub const ZYGOTE_STACK_SIZE: usize = 64 * KIB as usize;

/// Everything the zygote keeps mapped.
pub const ZYGOTE_SIZE: usize = ZYGOTE_CODE_SIZE + ZYGOTE_STACK_SIZE;

/// How far a direct branch reaches either way on ARM64: `b` and `bl` take a signed 26-bit count of instructions.
pub const BRANCH_REACH: usize = 1 << 27;

/// Where a stub's code area starts: right after the zygote's pages, so that every piece in it reaches the floor's
/// entries at the start of the zygote's code with one `bl`.
pub const CODE_AREA: usize = ZYGOTE + ZYGOTE_SIZE;

/// The code area's bytes.
pub const CODE_AREA_SIZE: usize = ZYGOTE + BRANCH_REACH - CODE_AREA;

/// The alignment of the floor's entries; the zygote's code starts with one branch to its start, padded up to it.
#[cfg(target_arch = "aarch64")]
pub const FLOOR_ENTRIES_ALIGN: usize = 16;

/// The first of the floor's entries in a stub, which translated code calls with `blr`.
#[cfg(target_arch = "aarch64")]
pub const FLOOR_ENTRIES: usize = ZYGOTE + FLOOR_ENTRIES_ALIGN;

/// One entry's bytes: it keeps x23, names itself there and branches to the save, padded to a power of two.
#[cfg(target_arch = "aarch64")]
pub const FLOOR_ENTRY_SIZE: usize = 4 * size_of::<u32>();

/// How many entries the floor has.
#[cfg(target_arch = "aarch64")]
pub const FLOOR_ENTRY_COUNT: usize = 64;

/// The most guest threads a stub holds; one control page, one register page and one signal stack each.
pub const THREADS_PER_PROCESS: usize = 1 << 15;

/// The bytes of one thread's control page, and of its register page.
pub const THREAD_PAGE_SIZE: usize = PAGE_SIZE as usize;

/// The control pages' bytes, at the start of the thread-pages memfd.
pub const CONTROL_PAGES_SIZE: usize = THREADS_PER_PROCESS * THREAD_PAGE_SIZE;

/// Where the register pages start in the thread-pages memfd, after the control pages.
pub const REGISTER_PAGES_OFFSET: usize = CONTROL_PAGES_SIZE;

/// The thread-pages memfd's size: every control page, then every register page.
pub const THREAD_PAGES_SIZE: usize = 2 * CONTROL_PAGES_SIZE;

/// Where a stub maps the thread-pages memfd: its control pages read-only, then its register pages read-write.
pub const THREAD_PAGES: usize = ZYGOTE + (1 << 32);

/// The first register page in a stub.
pub const REGISTER_PAGES: usize = THREAD_PAGES + REGISTER_PAGES_OFFSET;

/// One thread's signal stack.
pub const SIGNAL_STACK_SIZE: usize = 16 * KIB as usize;

/// Where a stub's signal stacks start, one per thread index, after its thread pages.
pub const SIGNAL_STACKS: usize = THREAD_PAGES + THREAD_PAGES_SIZE;

/// Every signal stack of a stub.
pub const SIGNAL_STACKS_SIZE: usize = THREADS_PER_PROCESS * SIGNAL_STACK_SIZE;

/// The host mappings a stub keeps outside its guest area: the zygote's code and stack, its control pages, its register
/// pages and its signal stacks.
pub const OWN_MAPPINGS: usize = 5;

const _: () = assert!(ZYGOTE >= OUR_AREA);
// Translated code calls the entries and jumps through the slot table where the native output's process way puts them.
#[cfg(target_arch = "aarch64")]
const _: () = assert!(
    FLOOR_ENTRIES as u64 == wcontainers_arm64::way::PROCESS_ENTRIES
        && FLOOR_ENTRY_SIZE as u64 == wcontainers_arm64::way::PROCESS_ENTRY_BYTES
        && SIGNAL_STACKS + SIGNAL_STACKS_SIZE <= wcontainers_arm64::way::PROCESS_SLOTS as usize
        && OUR_AREA == 1 << wcontainers_arm64::way::PROCESS_REACH_BITS
);
#[cfg(target_arch = "aarch64")]
const _: () =
    assert!(FLOOR_ENTRIES + FLOOR_ENTRY_COUNT * FLOOR_ENTRY_SIZE <= ZYGOTE + ZYGOTE_CODE_SIZE);
const _: () = assert!(THREAD_PAGES >= CODE_AREA + CODE_AREA_SIZE);
const _: () = assert!(SIGNAL_STACKS + SIGNAL_STACKS_SIZE <= 1 << 46);
// The stubs find a thread's pages and stack by shifts.
const _: () = assert!(
    THREADS_PER_PROCESS.is_power_of_two()
        && THREAD_PAGE_SIZE.is_power_of_two()
        && SIGNAL_STACK_SIZE.is_power_of_two()
);
