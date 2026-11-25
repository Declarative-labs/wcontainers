//! The native back end's monitor: the host program's part that makes the virtual machine, loads the inside,
//! runs its virtual CPUs and answers what the inside asks through the link.
//!
//! One interface of ours, [`Hypervisor`], is over KVM and Hypervisor.framework; nothing above it names a
//! platform.

use std::io;
use std::ptr::{self, NonNull};

use object::{Object, ObjectSegment};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_arm64::Arm64;
use wcontainers_executor_hardware_virtualization_inside::boot::{
    BOOT_MAP_BYTES, BootRegisters, boot_registers, write_boot_map,
};
use wcontainers_executor_hardware_virtualization_inside::layout::{IMAGE, MEMORY_BASE, linear};
use wcontainers_executor_hardware_virtualization_inside::link::{Link, Start};
use wcontainers_guest_cpu::Guest;
use zerocopy::IntoBytes;

#[cfg(target_os = "macos")]
pub mod hvf;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub mod kvm;
pub mod routines;
pub mod translate;

/// The host's hypervisor.
#[cfg(target_os = "macos")]
pub type Host = hvf::HypervisorFramework;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub type Host = kvm::KernelVirtualMachine;

/// What went wrong.
#[derive(Debug)]
pub enum Error {
    /// The hypervisor refused, with its own code.
    Hypervisor(String),
    /// The host refused memory.
    Host(io::Error),
    /// The inside's image is not one the monitor can load.
    Image(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Why a virtual CPU came back to the monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// The inside rang the doorbell: its link page holds a request.
    Doorbell,
    /// Another thread stopped it.
    Stopped,
    /// An exception the inside could not take, with the syndrome and addresses the hypervisor gave.
    Exception {
        syndrome: u64,
        address: u64,
        physical: u64,
    },
}

/// A register the monitor reads or writes on a virtual CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    Pc,
    Pstate,
    /// A general register, x0 to x30.
    X(u8),
    StackEl1,
    Mair,
    Tcr,
    Sctlr,
    Cpacr,
    Ttbr1,
    /// MPIDR_EL1: the CPU's affinity, which the interrupt controller routes by.
    Affinity,
    /// ID_AA64MMFR0_EL1.
    MemoryModelFeatures,
    /// ID_AA64PFR0_EL1.
    ProcessorFeatures,
    /// ID_AA64PFR1_EL1.
    ProcessorFeatures1,
    /// SP_EL0: the guest's level's stack pointer.
    StackEl0,
    /// ELR_EL1: where an exception taken to the inside came from, and where `eret` goes.
    ExceptionLink,
    /// SPSR_EL1: the state an exception taken to the inside saved, and that `eret` restores.
    SavedState,
    /// ESR_EL1: why an exception was taken to the inside.
    Syndrome,
    /// FAR_EL1: the address a fault taken to the inside was at.
    FaultAddress,
    /// VBAR_EL1: the inside's exception vectors.
    Vectors,
    /// MDSCR_EL1: the debug controls, software step among them.
    DebugControl,
    /// TPIDR_EL0: the guest's level's thread pointer.
    ThreadPointer,
    /// FPSR.
    FloatStatus,
    /// FPCR.
    FloatControl,
    /// TPIDR_EL1: the inside's thread register.
    InsideThread,
}

/// The interrupt numbers of a machine.
#[derive(Clone, Copy, Debug)]
pub struct Interrupts {
    pub timer: u32,
    pub first_raised: u32,
}

/// A way of making hardware virtual machines.
pub trait Hypervisor {
    type VirtualMachine: VirtualMachine;
    /// Makes the process's one virtual machine, which has `cpus` virtual CPUs at most.
    fn machine(cpus: u64) -> Result<Self::VirtualMachine>;
}

/// One hardware virtual machine on the host.
pub trait VirtualMachine: Send + Sync {
    type Cpu: VirtualCpu;
    /// Maps `length` bytes of host memory at `host` at guest-physical `at`.
    fn map(&self, host: NonNull<u8>, at: u64, length: u64) -> Result<()>;
    /// Gives back the host pages behind a guest-physical range of `memory`, which the machine maps; the range
    /// reads as zeros after.
    fn give_back(&self, memory: &Memory, at: u64, length: u64) -> Result<()>;
    /// Raises or lowers a host interrupt.
    fn raise(&self, interrupt: u32, level: bool) -> Result<()>;
    /// Makes a virtual CPU on the calling thread, which alone may run it.
    fn cpu(&self) -> Result<Self::Cpu>;
    /// The machine's interrupt numbers.
    fn interrupts(&self) -> Result<Interrupts>;
}

/// One virtual CPU.
pub trait VirtualCpu {
    type Stopper: Stopper;
    /// Runs until an exit.
    fn run(&mut self) -> Result<Exit>;
    fn get(&self, register: Register) -> Result<u64>;
    fn set(&mut self, register: Register, value: u64) -> Result<()>;
    /// A vector register, v0 to v31, whole.
    fn vector(&self, n: u8) -> Result<u128>;
    fn set_vector(&mut self, n: u8, value: u128) -> Result<()>;
    /// What stops it from another thread.
    fn stopper(&self) -> Self::Stopper;
    /// The guest's virtual counter reads the host's counter less this.
    fn counter_offset(&self) -> Result<u64>;
}

/// Stops a running virtual CPU from another thread.
pub trait Stopper: Send + Sync + Clone + 'static {
    fn stop(&self) -> Result<()>;
}

/// Guest-physical memory: one reservation the host backs as it is touched.
pub struct Memory {
    host: NonNull<u8>,
    length: u64,
}

// SAFETY: the reservation is plain memory shared with the virtual machine; who writes where is the inside's
// and the monitor's protocol, and only field drop order keeps it mapped while virtual CPUs reach it.
#[expect(
    unsafe_code,
    reason = "promises Memory may move to another thread, through a raw pointer that is not Send by itself; the reservation is not its alone, since the virtual machine maps it and virtual CPUs on other threads reach it wherever Memory goes, and only field drop order keeps it mapped while they do (the UNSOUND drop below); Lend's mapping replaces it"
)]
unsafe impl Send for Memory {}
// SAFETY: as above.
#[expect(
    unsafe_code,
    reason = "UNEXPLAINED: promises Memory may be shared between threads on a who-writes-where protocol of the inside and the monitor that no type or check enforces; Lend and Words replace it"
)]
unsafe impl Sync for Memory {}

impl Memory {
    /// Reserves `length` bytes.
    pub fn reserve(length: u64) -> Result<Memory> {
        Ok(Memory {
            host: fresh(None, length)?,
            length,
        })
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    /// The host address of guest-physical `at`.
    pub fn host(&self, at: u64) -> *mut u8 {
        assert!(
            at >= MEMORY_BASE && at < MEMORY_BASE + self.length,
            "outside guest memory"
        );
        self.host.as_ptr().wrapping_add((at - MEMORY_BASE) as usize)
    }

    /// Whether the `length` bytes from guest-physical `at` all lie in guest memory.
    pub fn contains(&self, at: u64, length: u64) -> bool {
        at >= MEMORY_BASE
            && at
                .checked_add(length)
                .is_some_and(|end| end <= MEMORY_BASE + self.length)
    }

    /// The host address of the `length` bytes from guest-physical `at`, which must all lie in guest memory.
    pub fn host_bytes(&self, at: u64, length: u64) -> *mut u8 {
        assert!(self.contains(at, length), "outside guest memory");
        self.host.as_ptr().wrapping_add((at - MEMORY_BASE) as usize)
    }

    /// Returns the bytes from guest-physical `at`, for one copy while no virtual CPU writes them.
    #[cfg(all(host_hypervisor, target_arch = "aarch64"))]
    #[expect(
        unsafe_code,
        reason = "UNSOUND: a shared slice of guest memory that a virtual CPU on another thread may write while it lives; Words replaces it"
    )]
    fn bytes(&self, at: u64, length: u64) -> &[u8] {
        let host = self.host_bytes(at, length);
        // SAFETY: not shown: inside the reservation, but a virtual CPU on another thread may write these bytes
        // while the slice lives (see the reason).
        unsafe { std::slice::from_raw_parts(host, length as usize) }
    }

    /// Returns the bytes from guest-physical `at` to write, for one copy while no virtual CPU touches them.
    #[cfg(all(host_hypervisor, target_arch = "aarch64"))]
    #[expect(
        unsafe_code,
        reason = "UNSOUND: a mutable slice of guest memory that a virtual CPU on another thread may reach while it lives; Words replaces it"
    )]
    fn bytes_mut(&mut self, at: u64, length: u64) -> &mut [u8] {
        let host = self.host_bytes(at, length);
        // SAFETY: not shown, as above: a virtual CPU on another thread may reach these bytes while the slice
        // lives; the borrow of `self` only keeps this program from lending them twice.
        unsafe { std::slice::from_raw_parts_mut(host, length as usize) }
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        // SAFETY: not shown: a virtual machine that still maps the reservation is kept from reaching it after
        // this only by field drop order (see the reason).
        #[expect(
            unsafe_code,
            reason = "UNSOUND: unmaps the reservation, which a virtual machine that still maps it is kept from reaching only by field drop order; Lend replaces it"
        )]
        unsafe { libc::munmap(self.host.as_ptr().cast(), self.length as usize) };
    }
}

/// Fresh anonymous memory, at `at` in place of what was there, or anywhere.
fn fresh(at: Option<NonNull<u8>>, length: u64) -> Result<NonNull<u8>> {
    let (address, fixed) = match at {
        Some(at) => (at.as_ptr().cast(), libc::MAP_FIXED),
        None => (ptr::null_mut(), 0),
    };
    let flags = libc::MAP_PRIVATE | libc::MAP_ANON | libc::MAP_NORESERVE | fixed;
    // SAFETY: not shown: when fixed, this maps over whatever address it is given; its one fixed caller checks
    // first that the range is guest memory.
    #[expect(
        unsafe_code,
        reason = "UNSOUND: a safe fn that maps over any fixed address it is given with no check of its own; its one fixed caller, HVF give_back, checks the range first; Lend's mapping and Words' zero fill replace it"
    )]
    let host = unsafe {
        libc::mmap(
            address,
            length as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            flags,
            -1,
            0,
        )
    };
    if host == libc::MAP_FAILED {
        return Err(Error::Host(io::Error::last_os_error()));
    }
    Ok(NonNull::new(host.cast()).expect("mmap gave null"))
}

/// Where the monitor put things in guest-physical memory when it loaded the inside.
#[derive(Clone, Copy, Debug)]
pub struct Loaded {
    /// The inside's entry point.
    pub entry: u64,
    /// The boot map's top-level table.
    pub boot_root: u64,
    /// The first virtual CPU's link page; the next ones follow.
    pub links: u64,
    /// The first virtual CPU's start record and stack page; the next ones follow.
    pub stacks: u64,
    /// The first byte the inside may take frames from.
    pub free: u64,
}

/// Pages each virtual CPU's stack takes; its start record is in the page below.
pub const STACK_PAGES: u64 = 16;

/// Loads the inside's ELF `image` into `memory` and writes the boot map, with room for `cpus` virtual CPUs.
pub fn load(memory: &Memory, image: &[u8], cpus: u64) -> Result<Loaded> {
    // The ELF reader needs its headers aligned, which embedded bytes are not.
    let mut aligned = vec![0u64; image.len().div_ceil(size_of::<u64>())];
    aligned.as_mut_bytes()[..image.len()].copy_from_slice(image);
    let image = &aligned.as_bytes()[..image.len()];
    let file = object::File::parse(image).map_err(|e| Error::Image(e.to_string()))?;
    let mut end = MEMORY_BASE;
    for segment in file.segments() {
        let data = segment.data().map_err(|e| Error::Image(e.to_string()))?;
        if segment.address() < IMAGE {
            return Err(Error::Image(format!(
                "segment at {:#x} is below the image's address",
                segment.address()
            )));
        }
        let at = segment.address() - linear(0);
        if !memory.contains(at, segment.size().max(data.len() as u64)) {
            return Err(Error::Image(format!(
                "segment at {:#x} does not fit guest memory",
                segment.address()
            )));
        }
        let host = memory.host_bytes(at, data.len() as u64);
        // SAFETY: the segment lies in guest memory (checked); that no virtual CPU or other thread reaches it
        // during the copy rests on the one caller loading before it makes any, not on a check.
        #[expect(
            unsafe_code,
            reason = "UNEXPLAINED: copies an ELF segment into guest memory from a safe pub fn that takes a shared &Memory, so nothing keeps virtual CPUs or other threads off those bytes during the copy; its one caller loads before it makes any virtual CPU; host_bytes checked the whole range; Words replaces it"
        )]
        unsafe { ptr::copy_nonoverlapping(data.as_ptr(), host, data.len()) };
        end = end.max(at + segment.size());
    }
    let boot_root = end.next_multiple_of(PAGE_SIZE);
    write_boot_map(boot_root, memory.length(), |at, entry| {
        let host = memory.host_bytes(at, size_of::<u64>() as u64).cast::<u64>();
        assert!(host.is_aligned(), "a boot map entry is a whole word");
        // SAFETY: the boot map lies in guest memory, after the image, and the entry is aligned; that no virtual
        // CPU reaches it meanwhile rests on the one caller loading before it makes any, not on a check.
        #[expect(
            unsafe_code,
            reason = "UNEXPLAINED: writes a boot page-table entry into guest memory from the same safe pub fn as the copy above, which nothing keeps from running while virtual CPUs do; its one caller loads before it makes any; host_bytes checked the range and the assert its alignment; Words replaces it"
        )]
        unsafe { ptr::write(host, entry) };
    });
    let links = boot_root + BOOT_MAP_BYTES;
    let stacks = links + cpus * PAGE_SIZE;
    let free = stacks + cpus * (STACK_PAGES + 1) * PAGE_SIZE;
    Ok(Loaded {
        entry: file.entry(),
        boot_root,
        links,
        stacks,
        free,
    })
}

/// Sets up virtual CPU `index` to enter the inside: writes its start record and its boot registers.
pub fn boot<C: VirtualCpu>(
    cpu: &mut C,
    memory: &Memory,
    loaded: &Loaded,
    index: u64,
    start: Start,
) -> Result<()> {
    let page = loaded.stacks + index * (STACK_PAGES + 1) * PAGE_SIZE;
    let start = Start {
        cpu: index,
        link: loaded.links + index * PAGE_SIZE,
        boot_root: loaded.boot_root,
        free: loaded.free,
        ..start
    };
    let host = memory.host_bytes(page, size_of::<Start>() as u64).cast::<Start>();
    assert!(host.is_aligned(), "a start record starts its page");
    // SAFETY: the start record lies in guest memory, aligned; that other virtual CPUs, which may already run,
    // keep off its page rests only on the inside's layout, not on a check.
    #[expect(
        unsafe_code,
        reason = "UNEXPLAINED: writes virtual CPU index's start record into the page set aside for it, from a safe pub fn that takes any index; other virtual CPUs may already run (the floor test boots each on its own thread), and only the inside's layout keeps them off that page; host_bytes checked the range and the assert its alignment; Words replaces it"
    )]
    unsafe { ptr::write(host, start) };
    let features = cpu.get(Register::MemoryModelFeatures)?;
    let BootRegisters {
        mair,
        tcr,
        sctlr,
        cpacr,
        ttbr1,
        pstate,
    } = boot_registers(loaded.boot_root, features, Arm64::SP_ALIGNMENT_CHECKED);
    for (register, value) in [
        (Register::Mair, mair),
        (Register::Tcr, tcr),
        (Register::Cpacr, cpacr),
        (Register::Ttbr1, ttbr1),
        (Register::Sctlr, sctlr),
        (Register::Affinity, index),
        (
            Register::StackEl1,
            linear(page + (STACK_PAGES + 1) * PAGE_SIZE),
        ),
        (Register::X(0), linear(page)),
        (Register::Pc, loaded.entry),
        (Register::Pstate, pstate),
    ] {
        cpu.set(register, value)?;
    }
    Ok(())
}

/// The link page of virtual CPU `index`.
#[expect(
    unsafe_code,
    reason = "UNSOUND: a safe fn returning &'static mut Link into guest memory, which outlives the reservation, aliases on a second call and races the inside; Words replaces it"
)]
pub fn link(memory: &Memory, loaded: &Loaded, index: u64) -> &'static mut Link {
    // SAFETY: not shown: the reference outlives the reservation, a second call aliases it, and the inside
    // writes the page while it lives (see the reason).
    unsafe { &mut *memory.host(loaded.links + index * PAGE_SIZE).cast::<Link>() }
}
