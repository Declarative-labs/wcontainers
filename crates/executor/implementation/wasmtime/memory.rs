//! Guest memories the executor maps itself and hands to wasmtime, and where things sit in a process's
//! state memory.

use std::ffi::c_void;
use std::ops::Range;
use std::sync::atomic::AtomicU32;

use rustix::mm::{MapFlags, ProtFlags, mmap_anonymous, mprotect, munmap};
use wasmtime::{LinearMemory, MemoryCreator, MemoryType, SharedMemory};
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

#[expect(
    unsafe_code,
    reason = "a raw pointer keeps the memory from being Send by itself; the mapping is plain memory owned by this value, and wasmtime synchronizes its own use"
)]
// SAFETY: as the reason says.
unsafe impl Send for HostMemory {}
#[expect(
    unsafe_code,
    reason = "a raw pointer keeps the memory from being Sync by itself; the mapping is plain memory owned by this value, and wasmtime synchronizes its own use"
)]
// SAFETY: as the reason says.
unsafe impl Sync for HostMemory {}

#[expect(
    unsafe_code,
    reason = "wasmtime's trait for a memory the embedder makes is unsafe: the executor makes its own so it can replace and copy the pages of a guest memory"
)]
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
        self.open(new_size).map_err(wasmtime::Error::msg)?;
        self.size = new_size;
        Ok(())
    }

    fn as_ptr(&self) -> *mut u8 {
        self.base
    }
}

impl HostMemory {
    /// Lets the first bytes of the mapping, as many as the size, be read and written.
    #[expect(
        unsafe_code,
        reason = "mprotect of the value's own mapping, which rustix takes as a raw pointer"
    )]
    fn open(&self, size: usize) -> Result<(), String> {
        if size == 0 {
            return Ok(());
        }
        assert!(
            size <= self.mapped,
            "a memory opens only inside its mapping"
        );
        use rustix::mm::MprotectFlags;
        // SAFETY: the start of this value's own mapping, which holds at least `size` bytes.
        unsafe {
            mprotect(
                self.base.cast(),
                size,
                MprotectFlags::READ | MprotectFlags::WRITE,
            )
        }
        .map_err(|error| format!("opening a memory: {error}"))
    }
}

impl Drop for HostMemory {
    #[expect(
        unsafe_code,
        reason = "munmap of the value's own mapping, which rustix takes as a raw pointer"
    )]
    fn drop(&mut self) {
        // SAFETY: the whole mapping this value made, no longer used by anything.
        unsafe { munmap(self.base.cast(), self.mapped) }.expect("unmapping a guest memory");
    }
}

#[expect(
    unsafe_code,
    reason = "wasmtime's trait for making the memories of an engine is unsafe: the executor makes its own so it can replace and copy the pages of a guest memory"
)]
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
        let base = reserve(mapped).map_err(|error| format!("reserving a guest memory: {error}"))?;
        let memory = HostMemory {
            base: base.cast(),
            mapped,
            capacity,
            size: minimum,
        };
        memory.open(minimum)?;
        Ok(Box::new(memory))
    }
}

/// Returns the start of a new mapping of the length that allows nothing.
#[expect(
    unsafe_code,
    reason = "mmap of a new anonymous mapping, which rustix declares unsafe for the address hint it takes"
)]
fn reserve(length: usize) -> rustix::io::Result<*mut c_void> {
    // SAFETY: a new mapping where nothing else is.
    unsafe {
        mmap_anonymous(
            std::ptr::null_mut(),
            length,
            ProtFlags::empty(),
            MapFlags::PRIVATE,
        )
    }
}

/// The host's page size.
pub fn host_page() -> u64 {
    rustix::param::page_size() as u64
}

/// Host memory the executor reaches by offset: a process's guest memory or its state memory.
///
/// Guest code may change its bytes, so they are copied in and out and never lent as a slice.
#[derive(Clone)]
pub(crate) struct Region {
    base: *mut u8,
    length: usize,
    /// The state memory the region is, which the region keeps alive; none for a guest memory, which its store keeps.
    shared: Option<SharedMemory>,
}

impl Region {
    /// Returns the region of a memory of a store, at its base and length.
    ///
    /// # Safety
    /// The bytes are a memory made by [`HostMemories`] that never moves or shrinks, and the region and
    /// every clone of it are dropped before the store that holds the memory.
    #[expect(
        unsafe_code,
        reason = "the region outlives no borrow of the store, so its maker promises the store outlives it"
    )]
    pub(crate) unsafe fn of_store_memory(base: *mut u8, length: usize) -> Self {
        Self {
            base,
            length,
            shared: None,
        }
    }

    /// Returns the region of a shared memory, which it keeps alive.
    pub(crate) fn of_shared_memory(memory: &SharedMemory) -> Self {
        let data = memory.data();
        Self {
            base: data.as_ptr().cast_mut().cast(),
            length: data.len(),
            shared: Some(memory.clone()),
        }
    }

    /// Returns the offsets of the bytes, after checking they lie in the region.
    fn within(&self, at: usize, length: usize) -> Range<usize> {
        let end = at
            .checked_add(length)
            .filter(|end| *end <= self.length)
            .unwrap_or_else(|| {
                panic!(
                    "{length} bytes at {at:#x} lie outside a region of {:#x}",
                    self.length
                )
            });
        at..end
    }

    /// Copies the bytes at the offset into the buffer, as many as it holds.
    #[expect(
        unsafe_code,
        reason = "a copy out of memory that guest code reaches too, which no safe call of wasmtime gives without its store"
    )]
    pub(crate) fn read(&self, at: usize, into: &mut [u8]) {
        let bytes = self.within(at, into.len());
        // SAFETY: inside the region, which lives as long as `self`; no guest code of the process runs
        // while the executor copies.
        unsafe {
            std::ptr::copy_nonoverlapping(self.base.add(bytes.start), into.as_mut_ptr(), into.len())
        }
    }

    /// Copies the buffer to the bytes at the offset.
    #[expect(
        unsafe_code,
        reason = "a copy into memory that guest code reaches too, which no safe call of wasmtime gives without its store"
    )]
    pub(crate) fn write(&self, at: usize, from: &[u8]) {
        let bytes = self.within(at, from.len());
        // SAFETY: as for `read`.
        unsafe {
            std::ptr::copy_nonoverlapping(from.as_ptr(), self.base.add(bytes.start), from.len())
        }
    }

    /// Sets the bytes at the offset to zero, as many as the length.
    #[expect(
        unsafe_code,
        reason = "a fill of memory that guest code reaches too, which no safe call of wasmtime gives without its store"
    )]
    pub(crate) fn zero(&self, at: usize, length: usize) {
        let bytes = self.within(at, length);
        // SAFETY: as for `read`.
        unsafe { std::ptr::write_bytes(self.base.add(bytes.start), u8::MIN, length) }
    }

    /// Replaces whole host pages of a guest memory with fresh zero pages, which also gives their memory
    /// back to the host.
    #[expect(
        unsafe_code,
        reason = "mmap over pages of the guest memory's own mapping, which rustix takes as a raw pointer"
    )]
    pub(crate) fn fresh_pages(&self, at: usize, length: usize) {
        let page = host_page() as usize;
        assert!(
            self.shared.is_none() && at.is_multiple_of(page) && length.is_multiple_of(page),
            "only whole host pages of a guest memory are replaced"
        );
        let bytes = self.within(at, length);
        // SAFETY: whole host pages inside a private anonymous mapping made by `HostMemories`, which
        // nothing reads or writes meanwhile; the new pages replace the old ones in place.
        unsafe {
            mmap_anonymous(
                self.base.add(bytes.start).cast::<c_void>(),
                length,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::PRIVATE | MapFlags::FIXED,
            )
        }
        .expect("fresh pages in a guest memory");
    }

    /// Returns the word at the offset as the stop flag the runtime holds for the run in progress.
    #[expect(
        unsafe_code,
        reason = "the runtime's stop flag takes the word as 'static; the state memory outlives every run, and the runtime lets go of the word when the run leaves"
    )]
    pub(crate) fn stop_flag(&self, at: usize) -> &'static AtomicU32 {
        let word = self.within(at, size_of::<AtomicU32>());
        let address = self.base.wrapping_add(word.start);
        assert!(
            address.cast::<AtomicU32>().is_aligned(),
            "a stop flag is an aligned word"
        );
        // SAFETY: an aligned word inside the state memory, which lives as long as the process; it is
        // reached only through atomics or the guest's own loads.
        unsafe { &*address.cast::<AtomicU32>() }
    }

    /// Copies runs of whole host pages, given as offsets, into the same offsets of the other region
    /// exactly: afterwards the other holds what this holds now, whatever the host has done with its
    /// pages (compressed, swapped out), and later writes to either are not seen by the other.
    ///
    /// The runs are in order and do not overlap, and the other's runs hold zeros, or what they held
    /// is not all replaced.
    pub(crate) fn copy_pages_to(&self, other: &Region, runs: &[Range<usize>]) {
        let page = host_page() as usize;
        let mut end = 0;
        for run in runs {
            assert!(
                run.start >= end && run.start.is_multiple_of(page) && run.end.is_multiple_of(page),
                "runs of whole host pages, in order"
            );
            self.within(run.start, run.end - run.start);
            other.within(run.start, run.end - run.start);
            end = run.end;
        }
        assert!(
            self.base != other.base,
            "a region's pages are copied into another region"
        );
        copy_checked_pages(self.base, other.base, runs);
    }
}

/// Copies runs of whole host pages from one region to another, checked by [`Region::copy_pages_to`].
#[cfg(target_os = "macos")]
fn copy_checked_pages(from: *const u8, to: *mut u8, runs: &[Range<usize>]) {
    for run in runs {
        // SAFETY: whole pages inside both regions, which are distinct mappings; the task is this process.
        #[expect(
            unsafe_code,
            reason = "mach_vm_copy of the process's own pages, which the system shares and copies on the first write in either region"
        )]
        let result = unsafe {
            mach2::vm::mach_vm_copy(
                mach2::traps::mach_task_self(),
                from.wrapping_add(run.start) as u64,
                (run.end - run.start) as u64,
                to.wrapping_add(run.start) as u64,
            )
        };
        assert_eq!(result, mach2::kern_return::KERN_SUCCESS, "mach_vm_copy");
    }
}

/// Copies runs of whole host pages from one region to another, checked by [`Region::copy_pages_to`].
///
/// Only pages the system shows present or swapped out can hold anything but zeros. The page map is
/// opened for each copy: an open one stays bound to the process that opened it, which after a fork of
/// the host process is not this one.
#[cfg(target_os = "linux")]
fn copy_checked_pages(from: *const u8, to: *mut u8, runs: &[Range<usize>]) {
    use std::os::unix::fs::FileExt;
    use zerocopy::IntoBytes;
    let pagemap = std::fs::File::open(PAGEMAP).expect(PAGEMAP);
    let page = host_page() as usize;
    let mut entries: Vec<u64> = Vec::new();
    for run in runs {
        let pages = (run.end - run.start) / page;
        entries.resize(pages, 0);
        pagemap
            .read_exact_at(
                entries.as_mut_bytes(),
                ((from as usize + run.start) / page * PAGEMAP_ENTRY) as u64,
            )
            .unwrap_or_else(|error| panic!("reading {PAGEMAP}: {error}"));
        let held =
            |index: usize| u64::from_le(entries[index]) & (PAGEMAP_PRESENT | PAGEMAP_SWAPPED) != 0;
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
            let at = run.start + first * page;
            // SAFETY: whole pages inside both regions, which are distinct mappings that no guest code
            // runs on meanwhile.
            #[expect(
                unsafe_code,
                reason = "a copy of the pages the page map shows held, between two regions guest code reaches too"
            )]
            unsafe {
                std::ptr::copy_nonoverlapping(from.add(at), to.add(at), (index - first) * page)
            };
        }
    }
}

/// Copies runs of whole host pages from one region to another, checked by [`Region::copy_pages_to`].
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn copy_checked_pages(from: *const u8, to: *mut u8, runs: &[Range<usize>]) {
    for run in runs {
        // SAFETY: whole pages inside both regions, which are distinct mappings that no guest code runs
        // on meanwhile.
        #[expect(
            unsafe_code,
            reason = "a copy of whole pages between two regions guest code reaches too"
        )]
        unsafe {
            std::ptr::copy_nonoverlapping(
                from.add(run.start),
                to.add(run.start),
                run.end - run.start,
            )
        };
    }
}

/// Bytes of one entry of Linux's `/proc/self/pagemap`, and its bits for a page present in memory and
/// one swapped out (Linux's Documentation/admin-guide/mm/pagemap.rst).
#[cfg(target_os = "linux")]
const PAGEMAP: &str = "/proc/self/pagemap";
#[cfg(target_os = "linux")]
const PAGEMAP_ENTRY: usize = size_of::<u64>();
#[cfg(target_os = "linux")]
const PAGEMAP_PRESENT: u64 = 1 << 63;
#[cfg(target_os = "linux")]
const PAGEMAP_SWAPPED: u64 = 1 << 62;

/// Bytes of one entry of the array of slots, an i32 (the translator's contract).
pub(crate) const SLOT_BYTES: u64 = size_of::<i32>() as u64;

/// Bytes a 32 bit memory addresses, the state memory's size at most.
const STATE_MEMORY_BYTES: u64 = 1 << u32::BITS;

/// Thread blocks start at a multiple of the guest's widest register, a 128 bit vector register.
const THREAD_ALIGN: usize = size_of::<u128>();

/// Bytes of a WebAssembly page at the default page size, the unit memories are sized in.
pub(crate) fn wasm_page() -> u64 {
    MemoryType::new(0, None).page_size()
}

/// Bytes of the state memory given to thread blocks.
const BLOCKS_BYTES: u64 = 15 << 20;

/// Where things sit in a process's state memory, and the address ceiling that follows.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// The addresses a process may use on this executor: guest memory, the permission table and the
    /// array of slots all cover 0 to this.
    pub ceiling: u64,
    /// Bytes of one thread's state: its block and the CPU side's own words, a multiple of
    /// `THREAD_ALIGN`.
    pub thread_bytes: u32,
    pub permissions_at: u32,
    pub permission_pages: u64,
    pub slots_at: u32,
    /// WebAssembly pages of the state memory.
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
            let slots_at = (BLOCKS_BYTES + ceiling / page + 1).next_multiple_of(SLOT_BYTES);
            (slots_at + SLOT_BYTES * (ceiling / G::ALIGN) <= STATE_MEMORY_BYTES).then_some(slots_at)
        };
        let mut ceiling = STATE_MEMORY_BYTES;
        while fits(ceiling).is_none() {
            ceiling -= STEP;
        }
        let slots_at = fits(ceiling).expect("fits");
        let end = slots_at + SLOT_BYTES * (ceiling / G::ALIGN);
        Layout {
            ceiling,
            thread_bytes: contract::thread_state_size::<G>().next_multiple_of(THREAD_ALIGN) as u32,
            permissions_at: BLOCKS_BYTES as u32,
            permission_pages: ceiling / page,
            slots_at: slots_at as u32,
            state_pages: end.div_ceil(wasm_page()) as u32,
        }
    }

    /// How many thread blocks fit.
    pub fn blocks(&self) -> u32 {
        (BLOCKS_BYTES / u64::from(self.thread_bytes)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::{HostMemories, Region, host_page};
    use wasmtime::{LinearMemory, MemoryCreator, MemoryType};

    /// The host pages each memory of the test holds.
    const PAGES: usize = 64;

    /// Returns a memory of the length made as the engine makes them, and its region.
    fn memory(length: usize) -> (Box<dyn LinearMemory>, Region) {
        let memory = HostMemories
            .new_memory(MemoryType::new(0, None), length, Some(length), None, 0)
            .expect("a memory");
        // SAFETY: a memory made by `HostMemories`, which never moves and is dropped after the region.
        #[expect(
            unsafe_code,
            reason = "the test's region of a memory it holds itself, as a process holds its store's"
        )]
        let region = unsafe { Region::of_store_memory(memory.as_ptr(), memory.byte_size()) };
        (memory, region)
    }

    /// The copy holds every page written, also one the system has paged out, and pages never written
    /// stay zero; writes after the copy are not seen across.
    #[test]
    fn page_copies_are_exact() {
        let page = host_page() as usize;
        let length = PAGES * page;
        let ((_from, source), (_to, copy)) = (memory(length), memory(length));
        for index in (0..PAGES).filter(|i| i % 3 != 0) {
            source.write(index * page, &vec![index as u8 + 1; page]);
        }
        #[cfg(target_os = "linux")]
        {
            let out = 4 * page..8 * page;
            // SAFETY: pages of the test's own memory.
            #[expect(
                unsafe_code,
                reason = "madvise of the test's own pages, which rustix takes as a raw pointer"
            )]
            let advised = unsafe {
                rustix::mm::madvise(
                    _from.as_ptr().add(out.start).cast(),
                    out.len(),
                    rustix::mm::Advice::LinuxPageOut,
                )
            };
            advised.expect("paging out");
            use super::{PAGEMAP, PAGEMAP_ENTRY, PAGEMAP_PRESENT, PAGEMAP_SWAPPED};
            use std::os::unix::fs::FileExt;
            let mut entry = [0u8; PAGEMAP_ENTRY];
            std::fs::File::open(PAGEMAP)
                .unwrap()
                .read_exact_at(
                    &mut entry,
                    ((_from.as_ptr() as usize + out.start) / page * PAGEMAP_ENTRY) as u64,
                )
                .unwrap();
            let entry = u64::from_le_bytes(entry);
            eprintln!(
                "a paged out page: present {}, swapped {}",
                entry & PAGEMAP_PRESENT != 0,
                entry & PAGEMAP_SWAPPED != 0
            );
        }
        let mut expected = vec![0; length];
        source.read(0, &mut expected);
        source.copy_pages_to(&copy, &[0..length / 2, length / 2..length]);
        let mut copied = vec![0; length];
        copy.read(0, &mut copied);
        assert!(copied == expected);
        source.write(page, &[0xee]);
        copy.write(2 * page, &[0xdd]);
        let mut byte = [0];
        copy.read(page, &mut byte);
        assert_eq!(byte, [2]);
        source.read(2 * page, &mut byte);
        assert_eq!(byte, [3]);
    }
}
