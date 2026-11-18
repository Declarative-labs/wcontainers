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

/// The host's page size.
pub fn host_page() -> u64 {
    rustix::param::page_size() as u64
}

/// Copies whole host pages from one anonymous mapping to another exactly: afterwards the destination
/// holds what the source holds now, whatever the host has done with the source's pages (compressed,
/// swapped out), and later writes to either are not seen by the other.
///
/// # Safety
/// `from`, `to` and `length` are multiples of the host's page size; both ranges are inside anonymous
/// private mappings that can be read and written, they do not overlap, nothing else uses either
/// meanwhile, and the destination holds zeros.
pub unsafe fn copy_pages(from: *const u8, to: *mut u8, length: usize) {
    if length == 0 {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        // The system shares the pages and copies each on its first write in either mapping.
        // SAFETY: as the caller promises; the task is this process.
        let result = unsafe {
            mach2::vm::mach_vm_copy(
                mach2::traps::mach_task_self(),
                from as u64,
                length as u64,
                to as u64,
            )
        };
        assert_eq!(result, mach2::kern_return::KERN_SUCCESS, "mach_vm_copy");
    }
    #[cfg(target_os = "linux")]
    {
        // Only pages the system shows present or swapped out can hold anything but zeros.
        use std::os::unix::fs::FileExt;
        static PAGEMAP: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();
        let pagemap = PAGEMAP
            .get_or_init(|| std::fs::File::open("/proc/self/pagemap").expect("/proc/self/pagemap"));
        let page = host_page() as usize;
        let pages = length / page;
        let mut entries = vec![0u8; pages * 8];
        pagemap
            .read_exact_at(&mut entries, (from as usize / page * 8) as u64)
            .expect("reading /proc/self/pagemap");
        let held = |index: usize| {
            let entry =
                u64::from_le_bytes(entries[index * 8..index * 8 + 8].try_into().expect("8"));
            entry & (3 << 62) != 0
        };
        let mut index = 0;
        while index < pages {
            if !held(index) {
                index += 1;
                continue;
            }
            let first = index;
            while index < pages && held(index) {
                index += 1;
            }
            // SAFETY: inside both ranges, as the caller promises.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    from.add(first * page),
                    to.add(first * page),
                    (index - first) * page,
                )
            }
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    // SAFETY: as the caller promises.
    unsafe {
        std::ptr::copy_nonoverlapping(from, to, length)
    }
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

#[cfg(test)]
mod tests {
    use super::{copy_pages, host_page};
    use rustix::mm::{MapFlags, ProtFlags, mmap_anonymous, munmap};

    fn mapping(length: usize) -> *mut u8 {
        // SAFETY: a new mapping where nothing else is.
        unsafe {
            mmap_anonymous(
                std::ptr::null_mut(),
                length,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::PRIVATE,
            )
        }
        .expect("a mapping")
        .cast()
    }

    /// The copy holds every page written, also one the system has paged out, and pages never written
    /// stay zero; writes after the copy are not seen across.
    #[test]
    fn page_copies_are_exact() {
        let page = host_page() as usize;
        let length = 64 * page;
        let (from, to) = (mapping(length), mapping(length));
        // SAFETY: both mappings are this test's own, `length` bytes each.
        let (source, copy) = unsafe {
            (
                std::slice::from_raw_parts_mut(from, length),
                std::slice::from_raw_parts_mut(to, length),
            )
        };
        for index in (0..64).filter(|i| i % 3 != 0) {
            source[index * page..(index + 1) * page].fill(index as u8 + 1);
        }
        #[cfg(target_os = "linux")]
        {
            let out = &mut source[4 * page..8 * page];
            // SAFETY: pages of this test's own mapping.
            unsafe {
                rustix::mm::madvise(
                    out.as_mut_ptr().cast(),
                    out.len(),
                    rustix::mm::Advice::LinuxPageOut,
                )
            }
            .expect("paging out");
            use std::os::unix::fs::FileExt;
            let mut entry = [0u8; 8];
            std::fs::File::open("/proc/self/pagemap")
                .unwrap()
                .read_exact_at(&mut entry, (out.as_ptr() as usize / page * 8) as u64)
                .unwrap();
            let entry = u64::from_le_bytes(entry);
            eprintln!(
                "a paged out page: present {}, swapped {}",
                entry >> 63,
                entry >> 62 & 1
            );
        }
        let expected = source.to_vec();
        // SAFETY: whole pages of the two mappings, the destination all zeros.
        unsafe { copy_pages(from, to, length) };
        assert!(copy == expected.as_slice());
        source[page] = 0xee;
        copy[2 * page] = 0xdd;
        assert_eq!(copy[page], 2);
        assert_eq!(source[2 * page], 3);
        // SAFETY: the mappings made above, no longer used.
        unsafe {
            munmap(from.cast(), length).unwrap();
            munmap(to.cast(), length).unwrap();
        }
    }
}
