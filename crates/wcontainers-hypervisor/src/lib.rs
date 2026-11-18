//! The native back end's monitor: the host program's part that makes the virtual machine, loads the inside,
//! runs its virtual CPUs and answers what the inside asks through the link.
//!
//! One interface of ours, [`Hypervisor`], is over KVM and Hypervisor.framework; nothing above it names a
//! platform.

use std::io;
use std::ptr::{self, NonNull};

use object::{Object, ObjectSegment};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_inside::boot::{BOOT_MAP_BYTES, BootRegisters, boot_registers, write_boot_map};
use wcontainers_inside::layout::{IMAGE, MEMORY_BASE, linear};
use wcontainers_inside::link::{Link, Start};

#[cfg(target_os = "macos")]
pub mod hvf;

/// The host's hypervisor.
#[cfg(target_os = "macos")]
pub type Host = hvf::HypervisorFramework;

/// The inside's programs, built for the host's architecture.
pub mod programs {
    include!(concat!(env!("OUT_DIR"), "/programs.rs"));
}

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
    /// ID_AA64DFR0_EL1.
    DebugFeatures,
    /// HCR_EL2, in a machine with EL2.
    HypervisorControl,
}

/// The interrupt numbers of a machine.
#[derive(Clone, Copy, Debug)]
pub struct Interrupts {
    pub timer: u32,
    pub counter: u32,
    pub first_raised: u32,
}

/// A way of making hardware virtual machines.
pub trait Hypervisor {
    type Machine: Machine;
    /// Makes the process's one machine; `el2` gives its virtual CPUs EL2.
    fn machine(el2: bool) -> Result<Self::Machine>;
    /// Whether a machine can give its virtual CPUs EL2.
    fn el2_supported() -> bool;
}

/// One virtual machine.
pub trait Machine: Send + Sync {
    type Cpu: VirtualCpu;
    /// Maps `length` bytes of host memory at `host` at guest-physical `at`.
    fn map(&self, host: NonNull<u8>, at: u64, length: u64) -> Result<()>;
    /// Unmaps a guest-physical range.
    fn unmap(&self, at: u64, length: u64) -> Result<()>;
    /// Raises or lowers a host interrupt.
    fn raise(&self, interrupt: u32, level: bool) -> Result<()>;
    /// Makes a virtual CPU on the calling thread, which alone may run it.
    fn cpu(&self) -> Result<Self::Cpu>;
    /// The machine's interrupt numbers.
    fn interrupts(&self) -> Result<Interrupts>;
    /// Virtual CPUs a machine can have.
    fn max_cpus(&self) -> Result<u64>;
}

/// One virtual CPU.
pub trait VirtualCpu {
    type Stopper: Stopper;
    /// Runs until an exit.
    fn run(&mut self) -> Result<Exit>;
    fn get(&self, register: Register) -> Result<u64>;
    fn set(&mut self, register: Register, value: u64) -> Result<()>;
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
// and the monitor's protocol.
unsafe impl Send for Memory {}
// SAFETY: as above.
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
        // SAFETY: inside the reservation.
        unsafe { self.host.as_ptr().add((at - MEMORY_BASE) as usize) }
    }

    /// Gives back the host pages of a range: unmapped from the machine, replaced by fresh memory, mapped again.
    pub fn give_back<M: Machine>(&self, machine: &M, at: u64, length: u64) -> Result<()> {
        machine.unmap(at, length)?;
        let host = NonNull::new(self.host(at)).expect("host address");
        fresh(Some(host), length)?;
        machine.map(host, at, length)
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        // SAFETY: the reservation is ours and nothing maps it any more.
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
    // SAFETY: an anonymous mapping, at an address of our own reservation when fixed.
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
    // SAFETY: the vector holds at least the image's bytes.
    unsafe {
        ptr::copy_nonoverlapping(
            image.as_ptr(),
            aligned.as_mut_ptr().cast::<u8>(),
            image.len(),
        )
    };
    // SAFETY: the vector's bytes, the image's length.
    let image = unsafe { std::slice::from_raw_parts(aligned.as_ptr().cast::<u8>(), image.len()) };
    let file = object::File::parse(image).map_err(|e| Error::Image(e.to_string()))?;
    let mut end = MEMORY_BASE;
    for segment in file.segments() {
        let data = segment.data().map_err(|e| Error::Image(e.to_string()))?;
        let at = segment.address() - linear(0);
        if segment.address() < IMAGE {
            return Err(Error::Image(format!(
                "segment at {:#x} is below the image's address",
                segment.address()
            )));
        }
        // SAFETY: the segment lies in guest memory, which is ours.
        unsafe { ptr::copy_nonoverlapping(data.as_ptr(), memory.host(at), data.len()) };
        end = end.max(at + segment.size());
    }
    let boot_root = end.next_multiple_of(PAGE_SIZE);
    write_boot_map(boot_root, memory.length(), |at, entry| {
        // SAFETY: the boot map lies in guest memory, after the image.
        unsafe { ptr::write(memory.host(at).cast::<u64>(), entry) }
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
    // SAFETY: the start record's page lies in guest memory and is this CPU's.
    unsafe { ptr::write(memory.host(page).cast::<Start>(), start) };
    let features = cpu.get(Register::MemoryModelFeatures)?;
    let BootRegisters {
        mair,
        tcr,
        sctlr,
        cpacr,
        ttbr1,
        pstate,
    } = boot_registers(loaded.boot_root, features);
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
pub fn link(memory: &Memory, loaded: &Loaded, index: u64) -> &'static mut Link {
    // SAFETY: the link page lies in guest memory, which lives as long as the monitor; the inside writes it
    // only before the doorbell, while the monitor reads it only after.
    unsafe { &mut *memory.host(loaded.links + index * PAGE_SIZE).cast::<Link>() }
}
