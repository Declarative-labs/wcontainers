//! The embedder's views of the stubs' memfds: windows of 2 MiB mapped at first touch into one reservation, the
//! least recently used one unmapped beyond a bound, so neither the stubs' number nor their sizes bound our address
//! space.

use std::collections::HashMap;
use std::os::fd::BorrowedFd;
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::io::Errno;
use rustix::mm::{MapFlags, ProtFlags};
use wcontainers_abi::KIB;

/// A window's bytes, and the alignment of its offset in its file.
pub const WINDOW_SIZE: usize = 2048 * KIB as usize;

/// The windows mapped at most: 32 GiB of views, against `vm.max_map_count`'s default of 65,530 host mappings.
pub const WINDOWS: usize = 16_384;

/// Which memfd a window views: a number given once per memfd in the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileKey(u64);

impl FileKey {
    /// A key no other memfd of this process has.
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for FileKey {
    fn default() -> Self {
        Self::new()
    }
}

/// One place in the reservation, and whether it was touched since the clock's hand last passed it.
#[derive(Clone, Copy)]
struct Slot {
    holds: Option<(FileKey, u64)>,
    touched: bool,
}

/// The cache; an executor call holds it `&mut`, so nothing it handed out is in use when it evicts.
pub struct WindowCache {
    /// The reservation's address, kept as a number so the cache is `Send`; `Drop` unmaps it.
    base: usize,
    table: HashMap<(FileKey, u64), usize>,
    slots: Vec<Slot>,
    /// The clock's hand: eviction takes the first slot from here not touched since the hand last passed it, an
    /// approximation of least recently used that costs one store per touch.
    hand: usize,
    evictions: u64,
}

impl WindowCache {
    /// Reserves room for `capacity` windows.
    pub fn new(capacity: usize) -> Result<Self, Errno> {
        assert!(capacity > 0);
        #[expect(
            unsafe_code,
            reason = "reserves address space where the host chooses, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: a new inaccessible mapping at an address the host picks overlaps nothing; `Drop` unmaps it.
        let base = unsafe {
            rustix::mm::mmap_anonymous(
                core::ptr::null_mut(),
                capacity * WINDOW_SIZE,
                ProtFlags::empty(),
                MapFlags::PRIVATE | MapFlags::NORESERVE,
            )
        }?;
        Ok(Self {
            base: base as usize,
            table: HashMap::new(),
            slots: vec![
                Slot {
                    holds: None,
                    touched: false
                };
                capacity
            ],
            hand: 0,
            evictions: 0,
        })
    }

    /// How many windows were unmapped to make room ("view window evicted").
    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    /// How many windows are mapped.
    pub fn mapped(&self) -> usize {
        self.table.len()
    }

    /// Copies bytes from `file` at `offset` into `into`.
    pub fn read(
        &mut self,
        file: BorrowedFd<'_>,
        key: FileKey,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), Errno> {
        let mut done = 0;
        while done < into.len() {
            let (at, length) = self.span(file, key, offset + done as u64, into.len() - done)?;
            #[expect(
                unsafe_code,
                reason = "copies out of a window, which a stub may write at the same time"
            )]
            // SAFETY: `at` starts `length` mapped bytes of one window, which stays mapped while `self` is borrowed;
            // `into` is ours. A stub writing them meanwhile gives some mix of its bytes, never an invalid value.
            unsafe {
                core::ptr::copy_nonoverlapping(at as *const u8, into[done..].as_mut_ptr(), length);
            }
            done += length;
        }
        Ok(())
    }

    /// Copies `bytes` into `file` at `offset`.
    pub fn write(
        &mut self,
        file: BorrowedFd<'_>,
        key: FileKey,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), Errno> {
        let mut done = 0;
        while done < bytes.len() {
            let (at, length) = self.span(file, key, offset + done as u64, bytes.len() - done)?;
            #[expect(
                unsafe_code,
                reason = "copies into a window, which a stub may read at the same time"
            )]
            // SAFETY: as in `read`, with the window writable.
            unsafe {
                core::ptr::copy_nonoverlapping(bytes[done..].as_ptr(), at as *mut u8, length);
            }
            done += length;
        }
        Ok(())
    }

    /// Unmaps every window of a file, which its owner is about to close.
    pub fn forget(&mut self, key: FileKey) {
        for slot in 0..self.slots.len() {
            if let Some((file, window)) = self.slots[slot].holds
                && file == key
            {
                self.table.remove(&(file, window));
                self.slots[slot].holds = None;
                // A failure leaves the old view in a slot no key finds, and the next map there replaces it.
                let _ = self.map_slot(slot, None);
            }
        }
    }

    /// The address of `offset` in its window, mapped if it was not, and how many of `wanted` bytes the window holds.
    fn span(
        &mut self,
        file: BorrowedFd<'_>,
        key: FileKey,
        offset: u64,
        wanted: usize,
    ) -> Result<(usize, usize), Errno> {
        let window = offset / WINDOW_SIZE as u64;
        let within = (offset % WINDOW_SIZE as u64) as usize;
        let slot = match self.table.get(&(key, window)) {
            Some(slot) => *slot,
            None => {
                let slot = self.free_slot();
                self.map_slot(slot, Some((file, window)))?;
                self.slots[slot].holds = Some((key, window));
                self.table.insert((key, window), slot);
                slot
            }
        };
        self.slots[slot].touched = true;
        Ok((
            self.base + slot * WINDOW_SIZE + within,
            wanted.min(WINDOW_SIZE - within),
        ))
    }

    /// A slot to map into: an empty one, or the clock's choice, whose window is unmapped by the map over it.
    fn free_slot(&mut self) -> usize {
        loop {
            let slot = self.hand;
            self.hand = (self.hand + 1) % self.slots.len();
            match self.slots[slot] {
                Slot { holds: None, .. } => return slot,
                Slot { touched: true, .. } => self.slots[slot].touched = false,
                Slot {
                    holds: Some(held),
                    touched: false,
                } => {
                    self.table.remove(&held);
                    self.slots[slot].holds = None;
                    self.evictions += 1;
                    return slot;
                }
            }
        }
    }

    /// Maps a window of a file into a slot, or makes the slot inaccessible again.
    fn map_slot(&self, slot: usize, window: Option<(BorrowedFd<'_>, u64)>) -> Result<(), Errno> {
        let at = (self.base + slot * WINDOW_SIZE) as *mut core::ffi::c_void;
        #[expect(
            unsafe_code,
            reason = "maps over one slot of our own reservation, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: the slot lies inside the reservation, which only this value maps into, and no reference into it
        // outlives the call that made it.
        let mapped = unsafe {
            match window {
                Some((file, window)) => rustix::mm::mmap(
                    at,
                    WINDOW_SIZE,
                    ProtFlags::READ | ProtFlags::WRITE,
                    MapFlags::SHARED | MapFlags::FIXED,
                    file,
                    window * WINDOW_SIZE as u64,
                ),
                None => rustix::mm::mmap_anonymous(
                    at,
                    WINDOW_SIZE,
                    ProtFlags::empty(),
                    MapFlags::PRIVATE | MapFlags::FIXED | MapFlags::NORESERVE,
                ),
            }
        }?;
        if mapped == at {
            Ok(())
        } else {
            Err(Errno::FAULT)
        }
    }
}

impl Drop for WindowCache {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "unmaps the reservation made by `new`")]
        // SAFETY: the reservation is this value's alone, and every copy through it has finished.
        let _ = unsafe {
            rustix::mm::munmap(
                self.base as *mut core::ffi::c_void,
                self.slots.len() * WINDOW_SIZE,
            )
        };
    }
}
