//! The start-up probe: what this host offers both forms, learned once before any guest runs.

use rustix::io::Errno;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_linux_process::LinuxProcess;
use wcontainers_linux_process_direct::DirectLinuxProcess;

/// Why a form cannot run on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// Guests get 4 KiB pages on larger host pages only once split host pages are built.
    HostPageLargerThan4KiB,
    /// Invalidating code by the writer's address may miss the runner's mapping of the same line.
    AliasingInstructionCache,
    /// The zygote, or a stub before its first turn, ended for this reason.
    Zygote(crate::zygote::ZygoteEnd),
    /// A host call the form needs was refused.
    Host(Errno),
    /// The embedder's executable holds no section of the zygote's code: section headers stripped, or the crate linked
    /// into a shared library that another program loaded.
    ZygoteCodeNotFound,
}

/// What the probe found.
#[derive(Debug, Clone, Copy)]
pub struct HostProbe {
    page_size: usize,
    user_space_top: usize,
    mapping_limit: usize,
    #[cfg(target_arch = "aarch64")]
    cache_type: u64,
    #[cfg(target_arch = "aarch64")]
    sve: SveAtSystemCall,
}

/// What a system call does to SVE state on this host.
#[cfg(target_arch = "aarch64")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SveAtSystemCall {
    /// The host has no SVE.
    Absent,
    /// The predicates and FFR read zero afterwards, as the kernel's arm64 ABI promises.
    Dropped,
    /// Some of it survived, which the ABI does not allow.
    Kept,
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
            mapping_limit: mapping_limit(),
            #[cfg(target_arch = "aarch64")]
            cache_type: cache_type(),
            #[cfg(target_arch = "aarch64")]
            sve: sve_at_system_call(),
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

    /// The most mappings a host process may hold.
    pub fn mapping_limit(&self) -> usize {
        self.mapping_limit
    }

    /// The host's `CTR_EL0`.
    #[cfg(target_arch = "aarch64")]
    pub fn cache_type(&self) -> u64 {
        self.cache_type
    }

    /// What a system call does to SVE state, learned on a thread of its own that then ends.
    #[cfg(target_arch = "aarch64")]
    pub fn sve_at_system_call(&self) -> SveAtSystemCall {
        self.sve
    }

    /// Whether `ic ivau` by the writer's address reaches the line in every other mapping of it.
    #[cfg(target_arch = "aarch64")]
    pub fn code_maintenance_covers_every_mapping(&self) -> bool {
        let coherent = self.cache_type & CTR_EL0_DIC != 0;
        let policy = (self.cache_type >> CTR_EL0_L1IP_SHIFT) & CTR_EL0_L1IP_MASK;
        coherent || policy == CTR_EL0_L1IP_PIPT
    }

    /// The bytes of the smallest data cache line that written code must be cleaned by, or `None` where the host
    /// needs no clean (IDC); then the same for the instruction cache's invalidation (DIC).
    #[cfg(target_arch = "aarch64")]
    pub fn code_lines(&self) -> (Option<usize>, Option<usize>) {
        let line =
            |shift: u64| size_of::<u32>() << ((self.cache_type >> shift) & CTR_EL0_LINE_MASK);
        let data = (self.cache_type & CTR_EL0_IDC == 0).then(|| line(CTR_EL0_DMINLINE_SHIFT));
        let instruction =
            (self.cache_type & CTR_EL0_DIC == 0).then(|| line(CTR_EL0_IMINLINE_SHIFT));
        (data, instruction)
    }
}

/// Linux's limit on a process's mappings where the host does not say (`DEFAULT_MAX_MAP_COUNT`, include/linux/mm.h).
const DEFAULT_MAPPING_LIMIT: usize = u16::MAX as usize - 5;

/// The host's `vm.max_map_count`, or Linux's default, warned, where it cannot be read.
fn mapping_limit() -> usize {
    match std::fs::read_to_string("/proc/sys/vm/max_map_count")
        .ok()
        .and_then(|text| text.trim().parse().ok())
    {
        Some(limit) => limit,
        None => {
            eprintln!(
                "wcontainers warning: the host's limit on mappings is unreadable, Linux's default of {DEFAULT_MAPPING_LIMIT} assumed"
            );
            DEFAULT_MAPPING_LIMIT
        }
    }
}

// CTR_EL0's fields, from Arm's register description
// (https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register).
#[cfg(target_arch = "aarch64")]
const CTR_EL0_DIC: u64 = 1 << 29;
#[cfg(target_arch = "aarch64")]
const CTR_EL0_IDC: u64 = 1 << 28;
/// DminLine and IminLine: the log2 of the words in the smallest line.
#[cfg(target_arch = "aarch64")]
const CTR_EL0_DMINLINE_SHIFT: u64 = 16;
#[cfg(target_arch = "aarch64")]
pub(crate) const CTR_EL0_IMINLINE_SHIFT: u64 = 0;
#[cfg(target_arch = "aarch64")]
pub(crate) const CTR_EL0_LINE_MASK: u64 = 0b1111;
#[cfg(target_arch = "aarch64")]
const CTR_EL0_L1IP_SHIFT: u64 = 14;
#[cfg(target_arch = "aarch64")]
const CTR_EL0_L1IP_MASK: u64 = 0b11;
#[cfg(target_arch = "aarch64")]
const CTR_EL0_L1IP_PIPT: u64 = 0b11;

#[cfg(target_arch = "aarch64")]
fn cache_type() -> u64 {
    DirectLinuxProcess.read_ctr_el0()
}

/// Sets every predicate bit of p0 and FFR, makes a system call, and reads both back, on a thread that then ends.
#[cfg(target_arch = "aarch64")]
fn sve_at_system_call() -> SveAtSystemCall {
    match DirectLinuxProcess.predicates_after_system_call() {
        None => SveAtSystemCall::Absent,
        Some(left) if left.p0.iter().chain(&left.ffr).all(|byte| *byte == 0) => {
            SveAtSystemCall::Dropped
        }
        Some(_) => SveAtSystemCall::Kept,
    }
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

/// Whether a page at this address is inside user space (taken already, or mappable now); a page made here is unmapped
/// at once, unused.
fn page_can_be_mapped(address: usize, page_size: usize) -> bool {
    match DirectLinuxProcess.mmap_fixed_noreplace(address, page_size) {
        Ok(_) | Err(Errno::EXIST) => true,
        Err(_) => false,
    }
}
