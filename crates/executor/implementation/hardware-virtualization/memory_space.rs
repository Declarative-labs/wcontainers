//! The executor's guest memory: spaces whose pages take frames from one pool, which its virtual machine maps whole.
//!
//! A space keeps what its pages allow, their frames and its own translation tables in the pool. A page without a
//! frame reads as zeros.

use std::collections::BTreeMap;
use std::ops::Range;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use wcontainers_abi::{Access, PAGE_MASK, PAGE_SIZE, Permissions};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace,
};
use wcontainers_executor_hardware_virtualization_inside::layout::{
    ENTRIES, ENTRY_BYTES, LEAF_LEVEL, MEMORY_BASE, TOP_LEVEL, level_index,
};
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::{Arm64, DataAccess, InstructionFetch};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::single_step::{CpuStopper, MEMORY_BYTES, to_page_entry, to_table_entry, to_unmapped_entry};
use crate::{Memory, Result, Stopper};

/// Where the pool lies in guest-physical memory: right after the machine's own memory.
pub(crate) const POOL_BASE: u64 = MEMORY_BASE + MEMORY_BYTES;
/// Bytes of the pool: as many as the machine's own memory.
pub(crate) const POOL_BYTES: u64 = MEMORY_BYTES;
/// The number of frames the pool holds.
const FRAMES: u64 = POOL_BYTES / PAGE_SIZE;
/// What a frame just taken is held by: the one record or table it is taken for.
const ONE_HOLDER: u32 = 1;
/// How long a change of the running space's tables waits before it stops the virtual CPU again.
///
/// A stop that comes between two runs may be forgotten: [hv_vcpus_exit](https://developer.apple.com/documentation/hypervisor/hv_vcpus_exit(_:_:)) does not say it is kept.
const STOP_AGAIN_AFTER: Duration = Duration::from_millis(1);
/// The address bits a translation uses: those below the top byte, which [TBI0](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TCR-EL1--Translation-Control-Register--EL1-#:~:text=TBI0%2C%20bit%20%5B37%5D) leaves as a tag.
const UNTAGGED: u64 = u64::MAX >> u8::BITS;

/// Returns the guest-physical address of a frame of the pool.
const fn to_physical(frame: u64) -> u64 {
    POOL_BASE + frame * PAGE_SIZE
}

/// Returns the frame of the pool at a guest-physical address.
const fn to_frame(physical: u64) -> u64 {
    (physical - POOL_BASE) / PAGE_SIZE
}

/// The pool's memory and who holds each of its frames.
struct Frames {
    memory: Memory,
    /// Frames given back, taken before those never used.
    free: Vec<u64>,
    /// How many page records and tables hold each frame taken so far, by its number.
    holders: Vec<u32>,
}

impl Frames {
    /// Takes a frame of zeros, held once, or returns `None` when the pool has none left.
    fn take(&mut self) -> Option<u64> {
        let frame = match self.free.pop() {
            Some(frame) => frame,
            None => {
                let frame = self.holders.len() as u64;
                if frame == FRAMES {
                    return None;
                }
                self.holders.push(u32::MIN);
                frame
            }
        };
        self.holders[frame as usize] = ONE_HOLDER;
        self.bytes_mut(frame, u64::MIN, PAGE_SIZE).fill(u8::MIN);
        Some(frame)
    }

    fn hold(&mut self, frame: u64) {
        self.holders[frame as usize] += 1;
    }

    fn release(&mut self, frame: u64) {
        let holders = &mut self.holders[frame as usize];
        *holders -= 1;
        if *holders == u32::MIN {
            self.free.push(frame);
        }
    }

    fn bytes(&self, frame: u64, offset: u64, length: u64) -> &[u8] {
        self.memory
            .bytes(MEMORY_BASE + frame * PAGE_SIZE + offset, length)
    }

    fn bytes_mut(&mut self, frame: u64, offset: u64, length: u64) -> &mut [u8] {
        self.memory
            .bytes_mut(MEMORY_BASE + frame * PAGE_SIZE + offset, length)
    }

    fn read_entry(&self, physical: u64) -> u64 {
        let frame = to_frame(physical);
        let offset = physical - to_physical(frame);
        let mut entry = [u8::MIN; ENTRY_BYTES as usize];
        entry.copy_from_slice(self.bytes(frame, offset, ENTRY_BYTES));
        u64::from_le_bytes(entry)
    }

    fn write_entry(&mut self, physical: u64, entry: u64) {
        let frame = to_frame(physical);
        let offset = physical - to_physical(frame);
        self.bytes_mut(frame, offset, ENTRY_BYTES)
            .copy_from_slice(&entry.to_le_bytes());
    }

    fn fill_table(&mut self, table: u64, entry: u64) {
        for index in 0..ENTRIES {
            self.write_entry(to_physical(table) + index * ENTRY_BYTES, entry);
        }
    }
}

/// A page of a space that has a frame.
#[derive(Clone, Copy)]
struct Backed {
    frame: u64,
    /// Whether the frame came from another space's page by `map_from`, which `unmap_from` ends.
    mapped: bool,
}

/// One space's records and translation tables.
struct Space {
    /// The top-level table's frame.
    root: u64,
    /// Every table's frame, the top-level one first.
    tables: Vec<u64>,
    /// What each range of pages allows, by its first address; ranges that allow nothing are left out.
    permissions: BTreeMap<u64, (u64, Permissions)>,
    /// The frame of each page that has one, by the page's address.
    backed: BTreeMap<u64, Backed>,
}

impl Space {
    /// Returns what the page at the address allows.
    fn permissions(&self, page: u64) -> Permissions {
        match self.permissions.range(..=page).next_back() {
            Some((_, &(end, allowed))) if page < end => allowed,
            _ => Permissions::empty(),
        }
    }

    /// Returns each range inside the addresses and what it allows, cut to the addresses.
    fn list_permissions(&self, addresses: Range<u64>) -> Vec<(Range<u64>, Permissions)> {
        let first = match self.permissions.range(..=addresses.start).next_back() {
            Some((&start, _)) => start,
            None => addresses.start,
        };
        self.permissions
            .range(first..addresses.end)
            .filter(|&(_, &(end, _))| end > addresses.start)
            .map(|(&start, &(end, allowed))| {
                (start.max(addresses.start)..end.min(addresses.end), allowed)
            })
            .collect()
    }

    /// Makes the addresses allow `allowed`, whatever they allowed before.
    fn set_permissions(&mut self, addresses: Range<u64>, allowed: Permissions) {
        let overlapping: Vec<(u64, u64, Permissions)> = {
            let first = match self.permissions.range(..addresses.start).next_back() {
                Some((&start, &(end, _))) if end > addresses.start => start,
                _ => addresses.start,
            };
            self.permissions
                .range(first..addresses.end)
                .map(|(&start, &(end, allowed))| (start, end, allowed))
                .collect()
        };
        for (start, end, kept) in overlapping {
            self.permissions.remove(&start);
            if start < addresses.start {
                self.permissions.insert(start, (addresses.start, kept));
            }
            if end > addresses.end {
                self.permissions.insert(addresses.end, (end, kept));
            }
        }
        if !allowed.is_empty() {
            self.permissions
                .insert(addresses.start, (addresses.end, allowed));
        }
    }

    /// Returns the leaf entry the page's records give: its frame with what it allows, or none.
    fn to_leaf_entry(&self, page: u64) -> u64 {
        let allowed = self.permissions(page);
        match self.backed.get(&page) {
            Some(backed) if !allowed.is_empty() => {
                let (data_access, instruction_fetch) = to_page_permission(allowed);
                to_page_entry(to_physical(backed.frame), data_access, instruction_fetch)
            }
            _ => to_unmapped_entry(LEAF_LEVEL),
        }
    }

    /// Makes the tables that lead to the page's leaf entry, or returns `None` when the pool has no frame for one.
    fn make_tables(&mut self, frames: &mut Frames, page: u64) -> Option<()> {
        let mut table = self.root;
        for level in TOP_LEVEL..LEAF_LEVEL {
            let at = to_physical(table) + level_index(page, level) * ENTRY_BYTES;
            let entry = frames.read_entry(at);
            table = if entry == to_unmapped_entry(level) {
                let own = frames.take()?;
                frames.fill_table(own, to_unmapped_entry(level + 1));
                frames.write_entry(at, to_table_entry(to_physical(own)));
                self.tables.push(own);
                own
            } else {
                to_frame(entry & !to_table_entry(u64::MIN))
            };
        }
        Some(())
    }

    /// Writes the page's leaf entry from its records, where tables lead to it.
    fn write_leaf_entry(&self, frames: &mut Frames, page: u64) {
        let mut table = self.root;
        for level in TOP_LEVEL..LEAF_LEVEL {
            let entry = frames.read_entry(to_physical(table) + level_index(page, level) * ENTRY_BYTES);
            if entry == to_unmapped_entry(level) {
                return;
            }
            table = to_frame(entry & !to_table_entry(u64::MIN));
        }
        let leaf = to_physical(table) + level_index(page, LEAF_LEVEL) * ENTRY_BYTES;
        frames.write_entry(leaf, self.to_leaf_entry(page));
    }

    /// Gives the page the frame, adding the one it had to `released`, or returns `None` when no frame is left for its tables.
    fn back(&mut self, frames: &mut Frames, page: u64, backed: Backed, released: &mut Vec<u64>) -> Option<()> {
        if self.make_tables(frames, page).is_none() {
            frames.release(backed.frame);
            return None;
        }
        if let Some(old) = self.backed.insert(page, backed) {
            released.push(old.frame);
        }
        self.write_leaf_entry(frames, page);
        Some(())
    }

    /// Takes the page's frame away, adding it to `released`, so that the page reads as zeros.
    fn unback(&mut self, frames: &mut Frames, page: u64, released: &mut Vec<u64>) {
        if let Some(old) = self.backed.remove(&page) {
            self.write_leaf_entry(frames, page);
            released.push(old.frame);
        }
    }

    /// Returns the page's frame, taking one when it has none, or `None` when the pool has none left.
    fn find_or_take_frame(&mut self, frames: &mut Frames, page: u64) -> Option<u64> {
        if let Some(backed) = self.backed.get(&page) {
            return Some(backed.frame);
        }
        let frame = frames.take()?;
        // A page without a frame had none to let go of.
        self.back(
            frames,
            page,
            Backed {
                frame,
                mapped: false,
            },
            &mut Vec::new(),
        )?;
        Some(frame)
    }
}

/// Returns what a page that allows `allowed` gives EL0, where nothing may be written that cannot be read.
fn to_page_permission(allowed: Permissions) -> (DataAccess, InstructionFetch) {
    let data_access = if allowed.contains(Permissions::WRITE) {
        DataAccess::ReadWrite
    } else if allowed.contains(Permissions::READ) {
        DataAccess::ReadOnly
    } else {
        DataAccess::Forbidden
    };
    let instruction_fetch = if allowed.contains(Permissions::EXECUTE) {
        InstructionFetch::Allowed
    } else {
        InstructionFetch::Forbidden
    };
    (data_access, instruction_fetch)
}

/// Returns whether a page that allows `allowed` lets the guest's level make the access.
fn allows(allowed: Permissions, access: Access) -> bool {
    let (data_access, instruction_fetch) = to_page_permission(allowed);
    match access {
        Access::Read => data_access != DataAccess::Forbidden,
        Access::Write => data_access == DataAccess::ReadWrite,
        Access::Execute => instruction_fetch == InstructionFetch::Allowed,
    }
}

/// The executor's virtual CPU, as a change of the tables of the space it runs waits on it.
struct Runner {
    /// Whether the virtual CPU may run a space's guest code.
    in_guest: AtomicBool,
    /// Counts the runs of a space's guest that have ended.
    ended: AtomicU64,
    /// Set by a change of the running space's tables, so that the virtual CPU leaves the guest and forgets its translations.
    forget: AtomicBool,
    stopper: CpuStopper,
}

impl Runner {
    /// Waits until the virtual CPU runs no guest code on translations it had before the call.
    fn wait_until_forgotten(&self) {
        let ended = self.ended.load(Ordering::SeqCst);
        if !self.in_guest.load(Ordering::SeqCst) {
            return;
        }
        self.forget.store(true, Ordering::SeqCst);
        loop {
            // The stopper refuses only once the virtual CPU is gone, and then the guest has left already.
            self.stopper.stop().ok();
            let stopped = Instant::now();
            while stopped.elapsed() < STOP_AGAIN_AFTER {
                if self.ended.load(Ordering::SeqCst) != ended {
                    return;
                }
                std::thread::yield_now();
            }
        }
    }
}

/// The frames and every space of one executor.
pub(crate) struct Pool {
    frames: Frames,
    spaces: BTreeMap<u64, Space>,
    next_space: u64,
    runner: Arc<Runner>,
    /// The space whose guest the virtual CPU runs, if any.
    running: Option<u64>,
}

impl Pool {
    /// Reserves the pool's memory, which the caller maps at [`POOL_BASE`], for the spaces the stopper's CPU runs.
    pub(crate) fn new(stopper: CpuStopper) -> Result<Pool> {
        Ok(Pool {
            frames: Frames {
                memory: Memory::reserve(POOL_BYTES)?,
                free: Vec::new(),
                holders: Vec::new(),
            },
            spaces: BTreeMap::new(),
            next_space: u64::MIN,
            runner: Arc::new(Runner {
                in_guest: AtomicBool::new(false),
                ended: AtomicU64::new(u64::MIN),
                forget: AtomicBool::new(false),
                stopper,
            }),
            running: None,
        })
    }

    /// Returns the host address of the pool's first byte.
    pub(crate) fn host(&self) -> NonNull<u8> {
        NonNull::new(self.frames.memory.host(MEMORY_BASE)).expect("the reservation is not at zero")
    }

    fn space(&self, id: u64) -> &Space {
        self.spaces.get(&id).expect("a space lives as long as its handle")
    }

    /// Returns the frames and one space, each to change.
    fn split(&mut self, id: u64) -> (&mut Frames, &mut Space) {
        let space = self
            .spaces
            .get_mut(&id)
            .expect("a space lives as long as its handle");
        (&mut self.frames, space)
    }
}

/// The pool has no frame left for a new space's top-level table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutOfFrames;

/// One address space of the executor's guest memory.
pub struct GuestMemorySpace {
    pool: Arc<Mutex<Pool>>,
    id: u64,
}

impl GuestMemorySpace {
    /// Makes a space of the pool that allows nothing and reads as zeros.
    pub(crate) fn new(pool: &Arc<Mutex<Pool>>) -> std::result::Result<Self, OutOfFrames> {
        let mut locked = lock(pool);
        let root = locked.frames.take().ok_or(OutOfFrames)?;
        locked.frames.fill_table(root, to_unmapped_entry(TOP_LEVEL));
        let id = locked.next_space;
        locked.next_space += 1;
        locked.spaces.insert(
            id,
            Space {
                root,
                tables: vec![root],
                permissions: BTreeMap::new(),
                backed: BTreeMap::new(),
            },
        );
        Ok(GuestMemorySpace {
            pool: Arc::clone(pool),
            id,
        })
    }

    /// Returns whether the space's frames are those of the pool.
    pub(crate) fn is_of(&self, pool: &Arc<Mutex<Pool>>) -> bool {
        Arc::ptr_eq(&self.pool, pool)
    }

    /// Returns the guest-physical address of the space's top-level table.
    pub(crate) fn find_root(&self) -> u64 {
        to_physical(lock(&self.pool).space(self.id).root)
    }

    /// Gives the page a frame where its permissions allow the access, and returns whether the guest may make it now.
    pub(crate) fn back_for(&self, address: u64, access: Access) -> bool {
        let page = address & UNTAGGED & !PAGE_MASK;
        let mut pool = lock(&self.pool);
        let (frames, space) = pool.split(self.id);
        if !allows(space.permissions(page), access) {
            return false;
        }
        if space.find_or_take_frame(frames, page).is_none() {
            return false;
        }
        space.write_leaf_entry(frames, page);
        true
    }

    fn lock(&self) -> MutexGuard<'_, Pool> {
        lock(&self.pool)
    }

    /// Marks the space as the one whose guest the virtual CPU runs, until the returned mark drops.
    pub(crate) fn start_running(&self) -> Running<'_> {
        let mut pool = self.lock();
        pool.running = Some(self.id);
        Running {
            space: self,
            runner: Arc::clone(&pool.runner),
        }
    }

    /// Ends a change of the space's tables: once no guest runs on translations from before it, lets go of the frames.
    fn finish_change<'space>(&'space self, mut pool: MutexGuard<'space, Pool>, released: Vec<u64>) {
        if pool.running == Some(self.id) {
            let runner = Arc::clone(&pool.runner);
            drop(pool);
            runner.wait_until_forgotten();
            pool = self.lock();
        }
        for frame in released {
            pool.frames.release(frame);
        }
    }

    /// Panics when the other space takes its frames from another executor's pool, which this machine does not map.
    fn check_same_pool(&self, other: &Self) {
        assert!(
            Arc::ptr_eq(&self.pool, &other.pool),
            "a space maps and copies only spaces of the executor that made it"
        );
    }
}

impl Drop for GuestMemorySpace {
    fn drop(&mut self) {
        let mut pool = self.lock();
        let Some(space) = pool.spaces.remove(&self.id) else {
            return;
        };
        for backed in space.backed.values() {
            pool.frames.release(backed.frame);
        }
        for &table in &space.tables {
            pool.frames.release(table);
        }
    }
}

fn lock(pool: &Mutex<Pool>) -> MutexGuard<'_, Pool> {
    pool.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Returns the addresses if they lie inside what a program can address.
fn check(addresses: &Range<u64>) -> std::result::Result<(), InvalidAddressError> {
    if addresses.start <= addresses.end && addresses.end <= Arm64::ADDRESSABLE.end {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

/// Returns the addresses if they lie inside what a program can address and are whole pages.
fn check_pages(addresses: &Range<u64>) -> std::result::Result<(), InvalidAddressError> {
    check(addresses)?;
    if addresses.start & PAGE_MASK == u64::MIN && addresses.end & PAGE_MASK == u64::MIN {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

/// Returns the other space's addresses that pair with these, from its start, if they are whole pages inside it.
fn pair(
    addresses: &Range<u64>,
    other_start: u64,
) -> std::result::Result<Range<u64>, InvalidAddressError> {
    let other_end = other_start
        .checked_add(addresses.end - addresses.start)
        .ok_or(InvalidAddressError)?;
    let other = other_start..other_end;
    check_pages(&other)?;
    Ok(other)
}

/// Returns each page the addresses touch: the page, the offset in it, and the bytes of the addresses in it.
fn list_pieces(addresses: Range<u64>) -> impl Iterator<Item = (u64, u64, u64)> {
    let mut at = addresses.start;
    std::iter::from_fn(move || {
        if at >= addresses.end {
            return None;
        }
        let page = at & !PAGE_MASK;
        let end = addresses.end.min(page + PAGE_SIZE);
        let piece = (page, at - page, end - at);
        at = end;
        Some(piece)
    })
}

fn list_pages(addresses: Range<u64>) -> impl Iterator<Item = u64> {
    addresses.step_by(PAGE_SIZE as usize)
}

impl MemorySpace for GuestMemorySpace {
    type Bytes<'space> = GuestBytes<'space>;

    fn borrow(&self, addresses: Range<u64>) -> std::result::Result<GuestBytes<'_>, InvalidAddressError> {
        check(&addresses)?;
        Ok(GuestBytes {
            space: self,
            addresses,
        })
    }

    /// Leaves a page without a frame as it is, since it reads as zeros already.
    fn zero(&self, addresses: Range<u64>) -> std::result::Result<(), InvalidAddressError> {
        check(&addresses)?;
        let mut pool = self.lock();
        let (frames, space) = pool.split(self.id);
        for (page, offset, length) in list_pieces(addresses) {
            if let Some(backed) = space.backed.get(&page) {
                frames.bytes_mut(backed.frame, offset, length).fill(u8::MIN);
            }
        }
        Ok(())
    }

    /// Takes whole pages only.
    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> std::result::Result<(), InvalidAddressError> {
        check_pages(&addresses)?;
        let mut pool = self.lock();
        let (frames, space) = pool.split(self.id);
        space.set_permissions(addresses.clone(), allowed);
        let pages: Vec<u64> = space.backed.range(addresses).map(|(&page, _)| page).collect();
        for page in pages {
            space.write_leaf_entry(frames, page);
        }
        self.finish_change(pool, Vec::new());
        Ok(())
    }

    /// Takes whole pages, which keep what this space allows; a want of frames leaves the pages before it mapped.
    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> std::result::Result<(), MapMemoryError> {
        check_pages(&addresses).map_err(MapMemoryError::InvalidAddress)?;
        let others = pair(&addresses, other_start).map_err(MapMemoryError::InvalidAddress)?;
        self.check_same_pool(other);
        let mut pool = self.lock();
        let mut released = Vec::new();
        let mut outcome = Ok(());
        for (page, other_page) in list_pages(addresses).zip(list_pages(others)) {
            let (frames, source) = pool.split(other.id);
            let Some(frame) = source.find_or_take_frame(frames, other_page) else {
                outcome = Err(MapMemoryError::OutOfMemory);
                break;
            };
            frames.hold(frame);
            let (frames, space) = pool.split(self.id);
            if space
                .back(frames, page, Backed { frame, mapped: true }, &mut released)
                .is_none()
            {
                outcome = Err(MapMemoryError::OutOfMemory);
                break;
            }
        }
        self.finish_change(pool, released);
        outcome
    }

    /// Takes whole pages only; pages that were not mapped from another space keep their bytes.
    fn unmap_from(&self, addresses: Range<u64>) -> std::result::Result<(), InvalidAddressError> {
        check_pages(&addresses)?;
        let mut pool = self.lock();
        let (frames, space) = pool.split(self.id);
        let mapped: Vec<u64> = space
            .backed
            .range(addresses)
            .filter(|(_, backed)| backed.mapped)
            .map(|(&page, _)| page)
            .collect();
        let mut released = Vec::new();
        for page in mapped {
            space.unback(frames, page, &mut released);
        }
        self.finish_change(pool, released);
        Ok(())
    }

    /// Takes whole pages and copies each page's bytes at once, where a copy on write would share them until a write.
    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> std::result::Result<(), CopyMemoryError> {
        check_pages(&addresses).map_err(CopyMemoryError::InvalidAddress)?;
        let others = pair(&addresses, other_start).map_err(CopyMemoryError::InvalidAddress)?;
        self.check_same_pool(other);
        let shift = |address: u64| address - others.start + addresses.start;
        let mut pool = self.lock();
        let source = pool.space(other.id);
        let permissions = source.list_permissions(others.clone());
        let sources: Vec<(u64, Backed)> = source
            .backed
            .range(others.clone())
            .map(|(&page, &backed)| (page, backed))
            .collect();
        let mut copies: Vec<(u64, Backed)> = Vec::with_capacity(sources.len());
        let mut buffer = [u8::MIN; PAGE_SIZE as usize];
        for (page, backed) in sources {
            let frame = if backed.mapped {
                pool.frames.hold(backed.frame);
                backed.frame
            } else {
                let Some(frame) = pool.frames.take() else {
                    for (_, copied) in copies {
                        pool.frames.release(copied.frame);
                    }
                    return Err(CopyMemoryError::OutOfMemory);
                };
                buffer.copy_from_slice(pool.frames.bytes(backed.frame, u64::MIN, PAGE_SIZE));
                pool.frames.bytes_mut(frame, u64::MIN, PAGE_SIZE).copy_from_slice(&buffer);
                frame
            };
            copies.push((
                shift(page),
                Backed {
                    frame,
                    mapped: backed.mapped,
                },
            ));
        }
        let (frames, space) = pool.split(self.id);
        let old: Vec<u64> = space.backed.range(addresses.clone()).map(|(&page, _)| page).collect();
        let mut released = Vec::new();
        for page in old {
            space.unback(frames, page, &mut released);
        }
        space.set_permissions(addresses.clone(), Permissions::empty());
        for (range, allowed) in permissions {
            space.set_permissions(shift(range.start)..shift(range.end), allowed);
        }
        let mut outcome = Ok(());
        let mut copies = copies.into_iter();
        while let Some((page, backed)) = copies.next() {
            if space.back(frames, page, backed, &mut released).is_none() {
                // The copies not placed yet are held by nothing else.
                for (_, unplaced) in copies.by_ref() {
                    frames.release(unplaced.frame);
                }
                outcome = Err(CopyMemoryError::OutOfMemory);
            }
        }
        self.finish_change(pool, released);
        outcome
    }
}

/// The mark of a space whose guest the virtual CPU runs, which counts the virtual CPU's runs of it.
pub(crate) struct Running<'space> {
    space: &'space GuestMemorySpace,
    runner: Arc<Runner>,
}

impl Running<'_> {
    /// Counts the start of a run that enters the guest through the handler's entry, which forgets every translation.
    pub(crate) fn enter(&self) {
        self.runner.in_guest.store(true, Ordering::SeqCst);
    }

    /// Counts the end of a run: the virtual CPU runs no guest code until the next start.
    pub(crate) fn leave(&self) {
        self.runner.ended.fetch_add(1, Ordering::SeqCst);
        self.runner.in_guest.store(false, Ordering::SeqCst);
    }

    /// Returns whether a change of the space's tables asked the guest to forget its translations since the last call.
    pub(crate) fn take_forget(&self) -> bool {
        self.runner.forget.swap(false, Ordering::SeqCst)
    }
}

impl Drop for Running<'_> {
    /// Ends a run left in the middle, as by a panic, so that no change of the tables waits on it.
    fn drop(&mut self) {
        if self.runner.in_guest.load(Ordering::SeqCst) {
            self.leave();
        }
        self.space.lock().running = None;
    }
}

/// Addresses borrowed from a space, read and written a copy at a time under the pool's lock.
pub struct GuestBytes<'space> {
    space: &'space GuestMemorySpace,
    addresses: Range<u64>,
}

impl GuestBytes<'_> {
    /// Returns a buffer as long as the borrow, which its bytes pass through on their way to a sink or from a source.
    fn make_buffer(&self) -> Vec<u8> {
        vec![u8::MIN; usize::try_from(self.len()).unwrap_or(usize::MAX)]
    }

    /// Returns the addresses of the first `count` bytes, as many as the borrow holds at most.
    fn to_first(&self, count: usize) -> Range<u64> {
        let count = u64::try_from(count).unwrap_or(u64::MAX).min(self.len());
        self.addresses.start..self.addresses.start + count
    }
}

impl VolatileBytes for GuestBytes<'_> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let pool = self.space.lock();
        let space = pool.space(self.space.id);
        let mut done = usize::MIN;
        for (page, offset, length) in list_pieces(self.to_first(into.len())) {
            let piece = &mut into[done..done + length as usize];
            match space.backed.get(&page) {
                Some(backed) => {
                    piece.copy_from_slice(pool.frames.bytes(backed.frame, offset, length));
                }
                None => piece.fill(u8::MIN),
            }
            done += piece.len();
        }
        done
    }

    /// Returns fewer bytes than asked only when the pool has no frame left for a page never written before.
    fn write(&self, bytes: &[u8]) -> usize {
        let mut pool = self.space.lock();
        let (frames, space) = pool.split(self.space.id);
        let mut done = usize::MIN;
        for (page, offset, length) in list_pieces(self.to_first(bytes.len())) {
            let Some(frame) = space.find_or_take_frame(frames, page) else {
                return done;
            };
            let piece = &bytes[done..done + length as usize];
            frames.bytes_mut(frame, offset, length).copy_from_slice(piece);
            done += piece.len();
        }
        done
    }

    /// Copies the bytes into a buffer of their own first, since consecutive pages need not lie on consecutive frames.
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> std::result::Result<usize, Sink::Error> {
        let mut buffer = self.make_buffer();
        let count = self.read(&mut buffer);
        sink.write(&buffer[..count])
    }

    /// Reads the source into a buffer of the bytes' length first, since consecutive pages need not lie on consecutive frames.
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> std::result::Result<usize, Source::Error> {
        let mut buffer = self.make_buffer();
        let count = source.read(&mut buffer)?;
        Ok(self.write(&buffer[..count]))
    }
}
