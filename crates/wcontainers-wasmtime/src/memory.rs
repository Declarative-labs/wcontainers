//! Guest memories the back end maps itself and hands to wasmtime, and where things sit in a process's
//! state memory.

use std::ffi::c_void;

use rustix::mm::{MapFlags, ProtFlags, mmap_anonymous, mprotect, munmap};
use wasmtime::{LinearMemory, MemoryCreator, MemoryType};
use wcontainers_translator::contract;

/// Makes every linear memory of the engine as one anonymous mapping of ours: the bytes the memory
/// has can be read and written, and the rest of the reservation and the guard fault.
pub struct HostMemories;

struct HostMemory {
    base: *mut u8,
    mapped: usize,
    /// Bytes it may grow to without moving.
    capacity: usize,
    size: usize,
}

// SAFETY: the mapping is plain memory owned by this value; wasmtime synchronises its own use.
unsafe impl Send for HostMemory {}
// SAFETY: as above.
unsafe impl Sync for HostMemory {}

// SAFETY: the memory never moves and stays mapped until dropped; `byte_size` bytes from `as_ptr` are
// readable and writable, zero when made.
unsafe impl LinearMemory for HostMemory {
    fn byte_size(&self) -> usize {
        self.size
    }

    fn byte_capacity(&self) -> usize {
        self.capacity
    }

    fn grow_to(&mut self, new_size: usize) -> wasmtime::Result<()> {
        wasmtime::ensure!(
            new_size <= self.capacity,
            "a memory grows only inside its reservation"
        );
        // SAFETY: the start of this value's own mapping.
        unsafe { open(self.base, new_size) }.map_err(wasmtime::Error::msg)?;
        self.size = new_size;
        Ok(())
    }

    fn as_ptr(&self) -> *mut u8 {
        self.base
    }
}

impl Drop for HostMemory {
    fn drop(&mut self) {
        // SAFETY: the whole mapping this value made, no longer used by anything.
        unsafe { munmap(self.base.cast(), self.mapped) }.expect("unmapping a guest memory");
    }
}

// SAFETY: each memory is a fresh mapping of at least the size, reservation and guard asked for.
unsafe impl MemoryCreator for HostMemories {
    fn new_memory(
        &self,
        _ty: MemoryType,
        minimum: usize,
        maximum: Option<usize>,
        reserved: Option<usize>,
        guard: usize,
    ) -> Result<Box<dyn LinearMemory>, String> {
        let capacity = reserved
            .unwrap_or(minimum)
            .max(maximum.unwrap_or(minimum))
            .max(minimum);
        let mapped = capacity + guard;
        // SAFETY: a new mapping where nothing else is.
        let base = unsafe {
            mmap_anonymous(
                std::ptr::null_mut(),
                mapped,
                ProtFlags::empty(),
                MapFlags::PRIVATE,
            )
        }
        .map_err(|error| format!("reserving a guest memory: {error}"))?;
        // SAFETY: the start of the mapping just made.
        unsafe { open(base.cast(), minimum) }?;
        Ok(Box::new(HostMemory {
            base: base.cast(),
            mapped,
            capacity,
            size: minimum,
        }))
    }
}

/// Lets the first `size` bytes of a mapping be read and written.
///
/// # Safety
/// `base` is the start of a mapping made by [`HostMemories`] of at least `size` bytes.
unsafe fn open(base: *mut u8, size: usize) -> Result<(), String> {
    if size == 0 {
        return Ok(());
    }
    use rustix::mm::MprotectFlags;
    // SAFETY: as the caller promises.
    unsafe {
        mprotect(
            base.cast(),
            size,
            MprotectFlags::READ | MprotectFlags::WRITE,
        )
    }
    .map_err(|error| format!("opening a memory: {error}"))
}

/// Replaces whole host pages of a guest memory with fresh zero pages, which also gives their memory
/// back to the host.
///
/// # Safety
/// `at` and `length` are multiples of the host's page size inside a memory made by [`HostMemories`],
/// and nothing reads or writes the range meanwhile.
pub unsafe fn fresh_pages(at: *mut u8, length: usize) {
    // SAFETY: as the caller promises; the new pages replace the old ones in place.
    unsafe {
        mmap_anonymous(
            at.cast::<c_void>(),
            length,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::PRIVATE | MapFlags::FIXED,
        )
    }
    .expect("fresh pages in a guest memory");
}

/// Bytes of the state memory given to thread blocks.
const BLOCKS_BYTES: u64 = 15 << 20;

/// Where things sit in a process's state memory, and the address ceiling that follows.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// The addresses a process may use on this back end: guest memory, the permission table and the
    /// array of slots all cover 0 to this.
    pub ceiling: u64,
    /// Bytes of one thread's state: its block and the CPU side's own words, a multiple of 16.
    pub thread_bytes: u32,
    pub permissions_at: u32,
    pub permission_pages: u64,
    pub slots_at: u32,
    /// Pages of 64 KiB of the state memory.
    pub state_pages: u32,
}

impl Layout {
    /// The layout for a guest: thread blocks, then the permission table (one byte per page and a zero
    /// one after), then the array of slots (one i32 per `G::ALIGN` bytes), in a 32 bit state memory,
    /// with the largest ceiling, a multiple of 4 MiB, that fits.
    pub fn of<G: wcontainers_guest_cpu::Guest>() -> Self {
        const STEP: u64 = 4 << 20;
        let page = wcontainers_abi::PAGE_SIZE;
        let fits = |ceiling: u64| {
            let slots_at = (BLOCKS_BYTES + ceiling / page + 1).next_multiple_of(4);
            (slots_at + 4 * (ceiling / G::ALIGN) <= 1 << 32).then_some(slots_at)
        };
        let mut ceiling = 1u64 << 32;
        while fits(ceiling).is_none() {
            ceiling -= STEP;
        }
        let slots_at = fits(ceiling).expect("fits");
        let end = slots_at + 4 * (ceiling / G::ALIGN);
        Layout {
            ceiling,
            thread_bytes: contract::thread_state_size::<G>().next_multiple_of(16) as u32,
            permissions_at: BLOCKS_BYTES as u32,
            permission_pages: ceiling / page,
            slots_at: slots_at as u32,
            state_pages: end.div_ceil(65536) as u32,
        }
    }

    /// How many thread blocks fit.
    pub fn blocks(&self) -> u32 {
        (BLOCKS_BYTES / u64::from(self.thread_bytes)) as u32
    }
}
