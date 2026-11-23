//! The start-up probe: what this host offers both forms, learned once before any guest runs.

use core::ffi::c_void;

use rustix::io::Errno;
use rustix::mm::{MapFlags, ProtFlags};
use wcontainers_abi::PAGE_SIZE;

/// Why a form cannot run on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// Guests get 4 KiB pages on larger host pages only once split host pages are built.
    HostPageLargerThan4KiB,
    /// Invalidating code by the writer's address may miss the runner's mapping of the same line.
    AliasingInstructionCache,
    /// Something of the embedder already holds a range our layout needs.
    ReservationRefused,
    /// The zygote ended before it answered, for this reason.
    Zygote(crate::zygote::ZygoteEnd),
    /// A host call the form needs was refused.
    Host(Errno),
}

/// What the probe found.
#[derive(Debug, Clone, Copy)]
pub struct HostProbe {
    page_size: usize,
    user_space_top: usize,
    #[cfg(target_arch = "aarch64")]
    cache_type: u64,
}

impl HostProbe {
    /// Probes this host, refusing it where a form could not run.
    pub fn new() -> Result<Self, Unavailable> {
        let page_size = rustix::param::page_size();
        if page_size as u64 != PAGE_SIZE {
            return Err(Unavailable::HostPageLargerThan4KiB);
        }
        let probe = Self {
            page_size,
            user_space_top: user_space_top(page_size),
            #[cfg(target_arch = "aarch64")]
            cache_type: cache_type(),
        };
        #[cfg(target_arch = "aarch64")]
        if !probe.code_maintenance_covers_every_mapping() {
            return Err(Unavailable::AliasingInstructionCache);
        }
        Ok(probe)
    }

    /// The host's page size.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// The first address no user mapping may reach.
    pub fn user_space_top(&self) -> usize {
        self.user_space_top
    }

    /// The host's `CTR_EL0`.
    #[cfg(target_arch = "aarch64")]
    pub fn cache_type(&self) -> u64 {
        self.cache_type
    }

    /// Whether `ic ivau` by the writer's address reaches the line in every other mapping of it.
    #[cfg(target_arch = "aarch64")]
    pub fn code_maintenance_covers_every_mapping(&self) -> bool {
        let coherent = self.cache_type & CTR_EL0_DIC != 0;
        let policy = (self.cache_type >> CTR_EL0_L1IP_SHIFT) & CTR_EL0_L1IP_MASK;
        coherent || policy == CTR_EL0_L1IP_PIPT
    }
}

// CTR_EL0's fields, from Arm's register description
// (https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register).
#[cfg(target_arch = "aarch64")]
const CTR_EL0_DIC: u64 = 1 << 29;
#[cfg(target_arch = "aarch64")]
const CTR_EL0_L1IP_SHIFT: u64 = 14;
#[cfg(target_arch = "aarch64")]
const CTR_EL0_L1IP_MASK: u64 = 0b11;
#[cfg(target_arch = "aarch64")]
const CTR_EL0_L1IP_PIPT: u64 = 0b11;

#[cfg(target_arch = "aarch64")]
fn cache_type() -> u64 {
    let value: u64;
    #[expect(
        unsafe_code,
        reason = "reads CTR_EL0, which Linux lets user space read; no crate wraps it"
    )]
    // SAFETY: the read has no effect beyond its output register.
    unsafe {
        core::arch::asm!("mrs {}, ctr_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// The lowest page the search starts from, always inside user space.
const SEARCH_FLOOR: usize = 1 << 32;

/// Above every user space Linux gives on these architectures (56 bits with five-level paging).
const SEARCH_CEILING: usize = 1 << 57;

/// Finds Linux's TASK_SIZE: the first page a fixed mapping cannot take.
fn user_space_top(page_size: usize) -> usize {
    let (mut low, mut high) = (SEARCH_FLOOR / page_size, SEARCH_CEILING / page_size);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if page_can_be_mapped(middle * page_size, page_size) {
            low = middle;
        } else {
            high = middle;
        }
    }
    high * page_size
}

/// Whether a page at this address is inside user space (taken already, or mappable now).
fn page_can_be_mapped(address: usize, page_size: usize) -> bool {
    let flags = MapFlags::PRIVATE | MapFlags::FIXED_NOREPLACE | MapFlags::NORESERVE;
    #[expect(
        unsafe_code,
        reason = "maps a fixed address, which rustix leaves to the caller to keep sound"
    )]
    // SAFETY: FIXED_NOREPLACE never replaces a mapping, and the page made here is unmapped at once, unused.
    let mapped = unsafe {
        rustix::mm::mmap_anonymous(address as *mut c_void, page_size, ProtFlags::empty(), flags)
    };
    match mapped {
        Ok(page) => {
            #[expect(unsafe_code, reason = "unmaps the page just made")]
            // SAFETY: the page is ours alone and nothing refers to it.
            let unmapped = unsafe { rustix::mm::munmap(page, page_size) };
            unmapped.is_ok()
        }
        Err(Errno::EXIST) => true,
        Err(_) => false,
    }
}
