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
    /// The zygote, or a stub before its first turn, ended for this reason.
    Zygote(crate::zygote::ZygoteEnd),
    /// A host call the form needs was refused.
    Host(Errno),
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

// The probe's system call lets Linux zero the bits of z0 to z31 above their low 128, which its assembly does not name
// as changed. That is sound only while the compiler keeps nothing there: while this crate is built without SVE.
#[cfg(all(target_arch = "aarch64", target_feature = "sve"))]
compile_error!(
    "the SVE probe's system call may clear Z register bits that code built with SVE keeps values in"
);

/// The largest predicate register: an SVE vector of 2048 bits has predicates of 256 bits.
#[cfg(target_arch = "aarch64")]
const LARGEST_PREDICATE: usize = 2048 / 8 / 8;

/// Sets every predicate bit of p0 and FFR, makes a system call, and reads both back, on a thread that then ends.
#[cfg(target_arch = "aarch64")]
fn sve_at_system_call() -> SveAtSystemCall {
    if !std::arch::is_aarch64_feature_detected!("sve") {
        return SveAtSystemCall::Absent;
    }
    let probe = std::thread::spawn(|| {
        let mut predicate = [0_u8; LARGEST_PREDICATE];
        let mut first_fault = [0_u8; LARGEST_PREDICATE];
        #[expect(
            unsafe_code,
            reason = "SVE state across a system call can only be set and read by its own instructions"
        )]
        // SAFETY: writes at most one predicate register's bytes to each buffer, and touches only clobbered registers
        // and the Z bits above 128, which code built without SVE never keeps (the `compile_error!` above).
        unsafe {
            core::arch::asm!(
                ".arch_extension sve",
                "ptrue p0.b",
                "setffr",
                "svc #0",
                "str p0, [{predicate}]",
                "rdffr p1.b",
                "str p1, [{first_fault}]",
                predicate = in(reg) predicate.as_mut_ptr(),
                first_fault = in(reg) first_fault.as_mut_ptr(),
                inout("x8") u64::from(linux_raw_sys::general::__NR_getppid) => _,
                out("x0") _,
                out("p0") _,
                out("p1") _,
                out("ffr") _,
                options(nostack),
            );
        }
        predicate.iter().chain(&first_fault).all(|byte| *byte == 0)
    });
    match probe.join() {
        Ok(true) => SveAtSystemCall::Dropped,
        Ok(false) | Err(_) => SveAtSystemCall::Kept,
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
