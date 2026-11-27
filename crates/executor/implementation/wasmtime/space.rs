//! A guest's address space: its bytes in a memory file mapped into one host memory, which every store's guest memory
//! is; what each page allows; the state memory its threads share; and the gate that stops them for a change.

use std::collections::BTreeMap;
use std::ops::Range;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rustix::fs::{FallocateFlags, MemfdFlags, SealFlags, SeekFrom};
use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_arm64::Arm64;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_guest_cpu::Guest;
use wcontainers_translator::contract::{
    self, SLOT_BYTES, WASM_PAGE_BYTES, permission_byte, progress_count_offset,
    progress_limit_offset, thread_state_size,
};
use wcontainers_volatile_bytes::VolatileBytes;
use wcontainers_wasmtime::Wasmtime;
use wcontainers_wasmtime_direct::{DirectBytes, DirectHostMemory};

use crate::WasmtimeError;
use crate::engine::{Shared, Translated};
use crate::state_memory::StateMemory;

/// The bytes of guest address a space has: what a 32-bit offset reaches, the most whose code the array of slots in a
/// 32-bit state memory can cover besides the table and the blocks.
pub(crate) const GUEST_BYTES: u64 = 1 << u32::BITS;
/// The inaccessible bytes after a guest memory, which wasmtime's compiled code may rely on to fault: its default guard.
pub(crate) const GUEST_GUARD_BYTES: u64 = 32 << 20;
/// The pages of a space.
const GUEST_PAGES: u64 = GUEST_BYTES / PAGE_SIZE;
/// The bytes of the state memory: all a 32-bit memory has.
const STATE_BYTES: u64 = 1 << u32::BITS;
/// The bytes of the state memory given to thread blocks, from its start.
const BLOCKS_BYTES: u64 = 1 << 20;
/// Thread blocks start at a multiple of the guest's widest register, a 128-bit vector register.
const THREAD_ALIGN: u64 = size_of::<u128>() as u64;
/// The bytes of one thread's state in the state memory.
pub(crate) const THREAD_BYTES: u64 =
    (thread_state_size::<Arm64>() as u64).next_multiple_of(THREAD_ALIGN);
/// Where the permission table starts: one byte for each page, then a zero byte.
pub(crate) const PERMISSIONS_AT: u64 = BLOCKS_BYTES;
/// Where the array of slots starts, after the permission table.
pub(crate) const SLOTS_AT: u64 =
    (PERMISSIONS_AT + GUEST_PAGES + 1).next_multiple_of(SLOT_BYTES as u64);
/// The guest addresses from zero whose code the array of slots covers, one entry for each instruction.
pub(crate) const SLOTS_COVER: u64 = (STATE_BYTES - SLOTS_AT) / SLOT_BYTES as u64 * Arm64::ALIGN;
/// The WebAssembly pages of a guest memory and of a state memory.
pub(crate) const GUEST_WASM_PAGES: u64 = GUEST_BYTES / WASM_PAGE_BYTES;
const STATE_WASM_PAGES: u64 = STATE_BYTES / WASM_PAGE_BYTES;
/// The name a space's memory file has, which only debugging shows.
const FILE_NAME: &str = "wcontainers-guest";
/// Pages of records kept together, allocated when one of them is first given anything.
const CHUNK_PAGES: usize = 1 << 12;
/// The bytes copied at once between two memory files.
const COPY_BYTES: usize = 1 << 20;

/// A page record's bits beside its permissions (`Permissions::bits`): it was given permissions, so it is mapped.
const MAPPED: u8 = 1 << 3;
/// The page holds code a load was translated from, so the table takes writing from it.
const CODE: u8 = 1 << 4;
/// The bits of a record that are its permissions.
const PERMISSION_BITS: u8 = Permissions::all().bits();

/// A space's pages, its file, its state memory and its code, shared by its memory, its table and every thread on it.
pub(crate) struct Space {
    pub(crate) shared: Arc<Shared>,
    file: Arc<OwnedFd>,
    pub(crate) host: DirectHostMemory,
    pub(crate) state: StateMemory,
    pages: Mutex<Pages>,
    pub(crate) code: Mutex<Code>,
    pub(crate) gate: Gate,
    /// Free thread blocks, and where the next new one goes.
    blocks: Mutex<(Vec<u32>, u64)>,
    /// Counts the changes no piece in progress may go on past: code dropped, permissions narrowed.
    pub(crate) narrowed: AtomicU64,
    /// How many loads `code` holds, so that a thread sees cheaply that it has some to instantiate.
    pub(crate) load_count: AtomicUsize,
}

/// Each page's record, and the pages shown from another file.
struct Pages {
    chunks: Vec<Option<Box<[u8; CHUNK_PAGES]>>>,
    /// By page, where a page mapped from another space's pages shows them from.
    backing: BTreeMap<u64, Backing>,
}

/// A file and the offset in it a page shows.
#[derive(Clone)]
struct Backing {
    file: Arc<OwnedFd>,
    offset: u64,
}

/// The code translated in a space, in the order it was loaded.
pub(crate) struct Code {
    pub(crate) loads: Vec<Arc<Load>>,
    /// The lookup table's next free slot, the same in every thread.
    next_slot: u32,
}

/// A translation placed in a space: at a load address, its pieces in the lookup table from a slot.
pub(crate) struct Load {
    pub(crate) translated: Arc<Translated>,
    pub(crate) load_address: u64,
    pub(crate) slot_base: u32,
    /// Code was written over what it was read from: new threads do not instantiate it.
    pub(crate) dropped: AtomicBool,
}

/// Lets a change wait until no thread of the space runs guest code, and holds the threads back until it is made.
pub(crate) struct Gate {
    /// Threads inside guest code.
    running: AtomicUsize,
    /// A change waits for the threads to leave.
    wanted: AtomicBool,
    /// Held for the whole of a change.
    changing: Mutex<()>,
    /// The blocks of every thread made on the space, whose stop flags a change raises.
    blocks: Mutex<Vec<u32>>,
}

/// The guest memory a [`WasmtimeExecutor`](crate::WasmtimeExecutor) makes: a space of 4 GiB of addresses from zero.
pub struct WasmtimeMemory {
    pub(crate) space: Arc<Space>,
}

/// The one page table of a [`WasmtimeMemory`], made with it.
pub struct WasmtimePageTable {
    space: Arc<Space>,
}

/// Addresses of a [`WasmtimeMemory`], which a guest may be changing while they are held.
pub struct WasmtimeBytes<'space> {
    bytes: DirectBytes,
    space: &'space Space,
    addresses: Range<u64>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Returns the pages the addresses touch, after checking they lie in the space.
fn pages_of(addresses: &Range<u64>) -> Result<Range<u64>, InvalidAddressError> {
    if addresses.start > addresses.end || addresses.end > GUEST_BYTES {
        return Err(InvalidAddressError);
    }
    Ok(addresses.start / PAGE_SIZE..addresses.end.div_ceil(PAGE_SIZE))
}

/// Returns whether two ranges share an address.
fn overlaps(first: &Range<u64>, second: &Range<u64>) -> bool {
    first.start < second.end && second.start < first.end
}

impl Pages {
    fn record(&self, page: u64) -> u8 {
        let page = usize::try_from(page).unwrap_or(usize::MAX);
        self.chunks
            .get(page / CHUNK_PAGES)
            .and_then(Option::as_ref)
            .map_or(0, |chunk| chunk[page % CHUNK_PAGES])
    }

    fn change(&mut self, page: u64, change: impl FnOnce(u8) -> u8) {
        let page = usize::try_from(page).expect("a page of the space");
        let chunk =
            self.chunks[page / CHUNK_PAGES].get_or_insert_with(|| Box::new([0; CHUNK_PAGES]));
        let record = &mut chunk[page % CHUNK_PAGES];
        *record = change(*record);
    }

    /// What the table says a page allows: what it was given, without writing while it holds code.
    fn effective(&self, page: u64) -> Permissions {
        let record = self.record(page);
        let allowed = Permissions::from_bits_truncate(record & PERMISSION_BITS);
        if record & CODE != 0 {
            allowed - Permissions::WRITE
        } else {
            allowed
        }
    }
}

impl Space {
    pub(crate) fn create(shared: Arc<Shared>) -> Result<Space, WasmtimeError> {
        let refused = |error: rustix::io::Errno| WasmtimeError::HostRefused(error.to_string());
        let file =
            rustix::fs::memfd_create(FILE_NAME, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
                .map_err(refused)?;
        rustix::fs::ftruncate(&file, GUEST_BYTES).map_err(refused)?;
        rustix::fs::fcntl_add_seals(&file, SealFlags::SHRINK | SealFlags::GROW).map_err(refused)?;
        let host = shared
            .wasmtime
            .new_host_memory(
                file.as_fd(),
                usize::try_from(GUEST_BYTES).expect("a space fits the host's addresses"),
                usize::try_from(GUEST_GUARD_BYTES).expect("a guard fits the host's addresses"),
            )
            .map_err(refused)?;
        let state = StateMemory::new(
            &shared.engine,
            u32::try_from(STATE_WASM_PAGES).expect("a 32-bit memory's pages"),
        )
        .map_err(|error| WasmtimeError::HostRefused(error.to_string()))?;
        let chunks = usize::try_from(GUEST_PAGES)
            .expect("a count of pages")
            .div_ceil(CHUNK_PAGES);
        Ok(Space {
            shared,
            file: Arc::new(file),
            host,
            state,
            pages: Mutex::new(Pages {
                chunks: (0..chunks).map(|_| None).collect(),
                backing: BTreeMap::new(),
            }),
            code: Mutex::new(Code {
                loads: Vec::new(),
                next_slot: contract::EXACT_SLOT as u32 + 1,
            }),
            gate: Gate {
                running: AtomicUsize::new(0),
                wanted: AtomicBool::new(false),
                changing: Mutex::new(()),
                blocks: Mutex::new(Vec::new()),
            },
            blocks: Mutex::new((Vec::new(), 0)),
            narrowed: AtomicU64::new(0),
            load_count: AtomicUsize::new(0),
        })
    }

    /// Returns the bytes at the addresses; none outside the space.
    pub(crate) fn bytes(&self, addresses: Range<u64>) -> Option<DirectBytes> {
        if addresses.start > addresses.end || addresses.end > GUEST_BYTES {
            return None;
        }
        self.shared
            .wasmtime
            .host_memory_bytes(&self.host, addresses)
    }

    /// Copies the bytes at the address into the buffer; false when they do not all lie in the space.
    pub(crate) fn read(&self, address: u64, into: &mut [u8]) -> bool {
        let Some(end) = address.checked_add(into.len() as u64) else {
            return false;
        };
        self.bytes(address..end)
            .is_some_and(|bytes| bytes.read(into) == into.len())
    }

    /// What the page was given; nothing for a page outside the space.
    pub(crate) fn permissions(&self, page: u64) -> Permissions {
        Permissions::from_bits_truncate(lock(&self.pages).record(page) & PERMISSION_BITS)
    }

    /// Whether the page was given permissions, so that a fault on it is one of permission rather than translation.
    pub(crate) fn is_mapped(&self, page: u64) -> bool {
        lock(&self.pages).record(page) & MAPPED != 0
    }

    /// Whether the table holds writing back from the page because code was translated from it.
    pub(crate) fn holds_code(&self, page: u64) -> bool {
        lock(&self.pages).record(page) & CODE != 0
    }

    /// Returns the run of pages around the address that allow running code, if its own page does.
    pub(crate) fn executable_run(&self, address: u64) -> Option<Range<u64>> {
        let pages = lock(&self.pages);
        let executable = |page: u64| {
            page < GUEST_PAGES
                && Permissions::from_bits_truncate(pages.record(page) & PERMISSION_BITS)
                    .contains(Permissions::EXECUTE)
        };
        let page = address / PAGE_SIZE;
        if !executable(page) {
            return None;
        }
        let first = (0..page)
            .rev()
            .take_while(|&p| executable(p))
            .last()
            .unwrap_or(page);
        let last = (page + 1..GUEST_PAGES)
            .take_while(|&p| executable(p))
            .last()
            .unwrap_or(page);
        Some(first * PAGE_SIZE..(last + 1) * PAGE_SIZE)
    }

    /// Gives the pages permissions, or with none leaves them unmapped; a narrowing waits until no thread runs.
    pub(crate) fn set_permissions(&self, pages: Range<u64>, allowed: Option<Permissions>) {
        let new = allowed.map_or(0, |allowed| allowed.bits() | MAPPED);
        let narrows = {
            let records = lock(&self.pages);
            pages.clone().any(|page| {
                let old = records.record(page);
                let lost = old & PERMISSION_BITS & !new;
                lost & (Permissions::READ | Permissions::WRITE).bits() != 0
                    || (lost & Permissions::EXECUTE.bits() != 0 && old & CODE != 0)
            })
        };
        let apply = || {
            let losing_code: Vec<u64> = {
                let records = lock(&self.pages);
                pages
                    .clone()
                    .filter(|&page| {
                        records.record(page) & CODE != 0 && new & Permissions::EXECUTE.bits() == 0
                    })
                    .collect()
            };
            for page in losing_code {
                self.drop_code_locked(page * PAGE_SIZE..(page + 1) * PAGE_SIZE);
            }
            {
                let mut records = lock(&self.pages);
                for page in pages.clone() {
                    records.change(page, |record| new | (record & CODE));
                }
            }
            self.write_permission_bytes(pages.clone());
        };
        if narrows {
            self.change(None, || {
                apply();
                self.narrowed.fetch_add(1, Ordering::SeqCst);
            });
        } else {
            apply();
        }
    }

    /// Writes the permission table's bytes of the pages and of the page before them, each of which says what the page
    /// after it allows too.
    pub(crate) fn write_permission_bytes(&self, pages: Range<u64>) {
        let first = pages.start.saturating_sub(1);
        let end = pages.end.min(GUEST_PAGES);
        if first >= end {
            return;
        }
        let bytes: Vec<u8> = {
            let records = lock(&self.pages);
            (first..end)
                .map(|page| {
                    let next = if page + 1 < GUEST_PAGES {
                        records.effective(page + 1)
                    } else {
                        Permissions::empty()
                    };
                    permission_byte(records.effective(page), next)
                })
                .collect()
        };
        let at = usize::try_from(PERMISSIONS_AT + first).expect("an offset in the state memory");
        self.state.write(at, &bytes);
    }

    /// Makes a change once no thread of the space runs guest code, holding them back until it is made.
    ///
    /// Every thread's stop flag but the given block's is raised, so that each leaves guest code at its next stop
    /// check. The caller runs no guest code of the space itself.
    pub(crate) fn change<Returned>(
        &self,
        except: Option<u32>,
        change: impl FnOnce() -> Returned,
    ) -> Returned {
        let _changing = lock(&self.gate.changing);
        self.gate.wanted.store(true, Ordering::SeqCst);
        for block in lock(&self.gate.blocks)
            .iter()
            .filter(|&&block| Some(block) != except)
        {
            self.raise_stop_flag(*block);
        }
        while self.gate.running.load(Ordering::SeqCst) != 0 {
            std::thread::yield_now();
        }
        let changed = change();
        self.gate.wanted.store(false, Ordering::SeqCst);
        changed
    }

    /// Counts the calling thread in guest code, once no change is being made.
    pub(crate) fn enter(&self) {
        loop {
            self.gate.running.fetch_add(1, Ordering::SeqCst);
            if !self.gate.wanted.load(Ordering::SeqCst) {
                return;
            }
            self.gate.running.fetch_sub(1, Ordering::SeqCst);
            drop(lock(&self.gate.changing));
        }
    }

    /// Counts the calling thread out of guest code.
    pub(crate) fn leave(&self) {
        self.gate.running.fetch_sub(1, Ordering::SeqCst);
    }

    /// Makes the thread of the block call the stop hook at its next stop check: its progress limit becomes zero.
    pub(crate) fn raise_stop_flag(&self, block: u32) {
        self.set_progress_limit(block, u64::MIN);
    }

    /// Sets the progress count at which the thread of the block calls the stop hook at its next stop check.
    pub(crate) fn set_progress_limit(&self, block: u32, limit: u64) {
        let at = usize::try_from(block).expect("a block offset") + progress_limit_offset::<Arm64>();
        self.state.store_u64(at, limit);
    }

    /// Returns the progress count the thread of the block saved last.
    pub(crate) fn progress_count(&self, block: u32) -> u64 {
        let at = usize::try_from(block).expect("a block offset") + progress_count_offset::<Arm64>();
        let mut count = [0; size_of::<u64>()];
        self.state.read(at, &mut count);
        u64::from_le_bytes(count)
    }

    /// Returns a zeroed thread block for a new thread, whose stop flag every change raises; none when all are taken.
    pub(crate) fn allocate_block(&self) -> Option<u32> {
        let block = {
            let mut blocks = lock(&self.blocks);
            match blocks.0.pop() {
                Some(block) => block,
                None => {
                    let next = blocks.1;
                    if next + THREAD_BYTES > BLOCKS_BYTES {
                        return None;
                    }
                    blocks.1 = next + THREAD_BYTES;
                    u32::try_from(next).expect("a block inside a 32-bit memory")
                }
            }
        };
        let at = usize::try_from(block).expect("a block offset");
        self.state.fill(
            at,
            usize::try_from(THREAD_BYTES).expect("a block's bytes"),
            0,
        );
        lock(&self.gate.blocks).push(block);
        Some(block)
    }

    pub(crate) fn free_block(&self, block: u32) {
        lock(&self.gate.blocks).retain(|&kept| kept != block);
        lock(&self.blocks).0.push(block);
    }

    /// Returns the slot the array gives the address; 0 for none or an address it does not cover.
    pub(crate) fn slot(&self, address: u64) -> u32 {
        let Some(at) = slot_entry(address) else {
            return 0;
        };
        let mut entry = [0; SLOT_BYTES];
        self.state.read(at, &mut entry);
        u32::from_le_bytes(entry)
    }

    fn set_slot(&self, address: u64, slot: u32) {
        if let Some(at) = slot_entry(address) {
            self.state.write(at, &slot.to_le_bytes());
        }
    }

    /// Places a translation of the code at the load address: gives its pieces slots, takes writing from the pages it was
    /// read from, gives each piece's address its slot, and returns the load and whether a page lost writing.
    ///
    /// Called only inside a change, before which no thread may reach the new slots.
    pub(crate) fn add_load_locked(
        &self,
        translated: Arc<Translated>,
        load_address: u64,
        lock_pages: bool,
    ) -> (Arc<Load>, bool) {
        let pieces = u32::try_from(translated.pieces()).expect("a count of pieces");
        let load = {
            let mut code = lock(&self.code);
            let load = Arc::new(Load {
                translated,
                load_address,
                slot_base: code.next_slot,
                dropped: AtomicBool::new(false),
            });
            code.next_slot += pieces;
            code.loads.push(load.clone());
            self.load_count.store(code.loads.len(), Ordering::SeqCst);
            load
        };
        let mut narrowed = false;
        if lock_pages {
            let mut newly = Vec::new();
            {
                let mut records = lock(&self.pages);
                for read in &load.translated.read {
                    let start = load_address.saturating_add(read.start);
                    let end = load_address.saturating_add(read.end).min(GUEST_BYTES);
                    for page in start / PAGE_SIZE..end.div_ceil(PAGE_SIZE) {
                        let record = records.record(page);
                        if record & CODE == 0 {
                            narrowed |= record & Permissions::WRITE.bits() != 0;
                            records.change(page, |record| record | CODE);
                            newly.push(page);
                        }
                    }
                }
            }
            for page in newly {
                self.write_permission_bytes(page..page + 1);
            }
        }
        let mut slot = load.slot_base;
        for (_, offsets) in &load.translated.modules {
            for offset in offsets {
                self.set_slot(load_address + offset, slot);
                slot += 1;
            }
        }
        (load, narrowed)
    }

    /// Drops every load read from the addresses, so the lookup finds none of its pieces, and gives writing back to the
    /// pages; whether there was any. Called only inside a change.
    pub(crate) fn drop_code_locked(&self, addresses: Range<u64>) -> bool {
        let Ok(pages) = pages_of(&addresses) else {
            return false;
        };
        let held: Vec<u64> = {
            let records = lock(&self.pages);
            pages
                .filter(|&page| records.record(page) & CODE != 0)
                .collect()
        };
        if held.is_empty() {
            return false;
        }
        let touched = |load: &Load| {
            load.translated.read.iter().any(|read| {
                let read = load.load_address.saturating_add(read.start)
                    ..load.load_address.saturating_add(read.end);
                held.iter()
                    .any(|page| overlaps(&read, &(page * PAGE_SIZE..(page + 1) * PAGE_SIZE)))
            })
        };
        let loads: Vec<Arc<Load>> = lock(&self.code).loads.clone();
        for load in loads
            .iter()
            .filter(|load| !load.dropped.load(Ordering::SeqCst) && touched(load))
        {
            load.dropped.store(true, Ordering::SeqCst);
            let mut slot = load.slot_base;
            for (_, offsets) in &load.translated.modules {
                for offset in offsets {
                    let address = load.load_address + offset;
                    if self.slot(address) == slot {
                        self.set_slot(address, 0);
                    }
                    slot += 1;
                }
            }
        }
        {
            let mut records = lock(&self.pages);
            for &page in &held {
                records.change(page, |record| record & !CODE);
            }
        }
        for &page in &held {
            self.write_permission_bytes(page..page + 1);
        }
        self.narrowed.fetch_add(1, Ordering::SeqCst);
        true
    }

    /// Drops every load, so that the lookup finds none of their pieces. Called only while no thread runs.
    pub(crate) fn drop_every_load(&self) {
        let loads: Vec<Arc<Load>> = lock(&self.code).loads.clone();
        for load in loads
            .iter()
            .filter(|load| !load.dropped.swap(true, Ordering::SeqCst))
        {
            let mut slot = load.slot_base;
            for (_, offsets) in &load.translated.modules {
                for offset in offsets {
                    let address = load.load_address + offset;
                    if self.slot(address) == slot {
                        self.set_slot(address, 0);
                    }
                    slot += 1;
                }
            }
        }
    }

    /// Drops the code read from the addresses, if any, once no thread runs.
    pub(crate) fn drop_code(&self, addresses: Range<u64>) {
        let Ok(pages) = pages_of(&addresses) else {
            return;
        };
        let any = {
            let records = lock(&self.pages);
            pages.clone().any(|page| records.record(page) & CODE != 0)
        };
        if any {
            self.change(None, || self.drop_code_locked(addresses));
        }
    }

    /// Returns where each page shows its bytes from: its own file at its address, or the backing it was mapped from.
    fn backing(&self, page: u64) -> Backing {
        lock(&self.pages)
            .backing
            .get(&page)
            .cloned()
            .unwrap_or_else(|| Backing {
                file: self.file.clone(),
                offset: page * PAGE_SIZE,
            })
    }

    /// Maps each run of the pages from its backing, and records it. Called only inside a change.
    fn show_locked(&self, mapped: Vec<(u64, Backing)>) -> Result<(), rustix::io::Errno> {
        let mut index = 0;
        while index < mapped.len() {
            let (first, ref backing) = mapped[index];
            let mut end = index + 1;
            while end < mapped.len()
                && mapped[end].0 == first + (end - index) as u64
                && Arc::ptr_eq(&mapped[end].1.file, &backing.file)
                && mapped[end].1.offset == backing.offset + (end - index) as u64 * PAGE_SIZE
            {
                end += 1;
            }
            let last = first + (end - index) as u64;
            self.shared.wasmtime.map_file(
                &self.host,
                first * PAGE_SIZE..last * PAGE_SIZE,
                backing.file.as_fd(),
                backing.offset,
            )?;
            index = end;
        }
        let mut records = lock(&self.pages);
        for (page, backing) in mapped {
            if Arc::ptr_eq(&backing.file, &self.file) && backing.offset == page * PAGE_SIZE {
                records.backing.remove(&page);
            } else {
                records.backing.insert(page, backing);
            }
        }
        Ok(())
    }

    /// Shows the pages from this space's own file again, zeroed. Called only inside a change.
    fn own_zeroed_locked(&self, pages: Range<u64>) -> Result<(), rustix::io::Errno> {
        if pages.is_empty() {
            return Ok(());
        }
        rustix::fs::fallocate(
            &*self.file,
            FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
            pages.start * PAGE_SIZE,
            (pages.end - pages.start) * PAGE_SIZE,
        )?;
        let mapped: Vec<u64> = lock(&self.pages)
            .backing
            .range(pages)
            .map(|(&page, _)| page)
            .collect();
        let own = mapped
            .into_iter()
            .map(|page| {
                let backing = Backing {
                    file: self.file.clone(),
                    offset: page * PAGE_SIZE,
                };
                (page, backing)
            })
            .collect();
        self.show_locked(own)
    }

    /// Copies the data of the other space's own file at the pages into this space's own file at the same offset less the
    /// shift, skipping what the other file holds no data at. Called only inside a change.
    fn copy_own_locked(
        &self,
        other: &Space,
        pages: Range<u64>,
        into: u64,
    ) -> Result<(), rustix::io::Errno> {
        let end = pages.end * PAGE_SIZE;
        let mut at = pages.start * PAGE_SIZE;
        let mut buffer = vec![0; COPY_BYTES];
        while at < end {
            let data = match rustix::fs::seek(&*other.file, SeekFrom::Data(at)) {
                Ok(data) => data,
                Err(rustix::io::Errno::NXIO) => break,
                Err(error) => return Err(error),
            };
            if data >= end {
                break;
            }
            let hole = rustix::fs::seek(&*other.file, SeekFrom::Hole(data))?.min(end);
            let mut from = data;
            while from < hole {
                let length = usize::try_from((hole - from).min(COPY_BYTES as u64))
                    .expect("a buffer's length");
                let read = rustix::io::pread(&*other.file, &mut buffer[..length], from)?;
                if read == 0 {
                    break;
                }
                let written = from - pages.start * PAGE_SIZE + into * PAGE_SIZE;
                let mut done = 0;
                while done < read {
                    done += rustix::io::pwrite(
                        &*self.file,
                        &buffer[done..read],
                        written + done as u64,
                    )?;
                }
                from += read as u64;
            }
            at = hole;
        }
        Ok(())
    }

    /// Whether any page of the addresses holds code.
    fn any_code(&self, addresses: &Range<u64>) -> bool {
        let Ok(pages) = pages_of(addresses) else {
            return false;
        };
        let records = lock(&self.pages);
        pages
            .into_iter()
            .any(|page| records.record(page) & CODE != 0)
    }
}

/// Returns where in the state memory the array holds the slot of the address, if it covers it.
fn slot_entry(address: u64) -> Option<usize> {
    (address < SLOTS_COVER && address.is_multiple_of(Arm64::ALIGN)).then(|| {
        usize::try_from(SLOTS_AT + contract::slot_entry::<Arm64>(address))
            .expect("an offset in the state memory")
    })
}

impl MemorySpace for WasmtimeMemory {
    type Bytes<'space> = WasmtimeBytes<'space>;
    type PageTable = WasmtimePageTable;

    fn borrow(&self, addresses: Range<u64>) -> Result<WasmtimeBytes<'_>, InvalidAddressError> {
        pages_of(&addresses)?;
        let bytes = self
            .space
            .bytes(addresses.clone())
            .ok_or(InvalidAddressError)?;
        Ok(WasmtimeBytes {
            bytes,
            space: &self.space,
            addresses,
        })
    }

    /// Gives whole pages of the space's own file back to the host, and writes zeros over the rest.
    fn zero(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        let pages = pages_of(&addresses)?;
        let whole = addresses.start.div_ceil(PAGE_SIZE)..addresses.end / PAGE_SIZE;
        let space = &self.space;
        let own_whole: Vec<u64> = {
            let records = lock(&space.pages);
            whole
                .clone()
                .filter(|page| !records.backing.contains_key(page))
                .collect()
        };
        for page in pages.clone() {
            let bytes = page * PAGE_SIZE..(page + 1) * PAGE_SIZE;
            let cleared = addresses.start.max(bytes.start)..addresses.end.min(bytes.end);
            if cleared.is_empty() || own_whole.binary_search(&page).is_ok() {
                continue;
            }
            let zeros =
                vec![0; usize::try_from(cleared.end - cleared.start).expect("part of a page")];
            if let Some(view) = space.bytes(cleared) {
                view.write(&zeros);
            }
        }
        for page in own_whole {
            let punched = rustix::fs::fallocate(
                &*space.file,
                FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
                page * PAGE_SIZE,
                PAGE_SIZE,
            );
            if punched.is_err()
                && let Some(view) = space.bytes(page * PAGE_SIZE..(page + 1) * PAGE_SIZE)
            {
                view.write(&[0; PAGE_SIZE as usize]);
            }
        }
        space.drop_code(addresses);
        Ok(())
    }
}

impl WasmtimeMemory {
    pub(crate) fn create(
        shared: Arc<Shared>,
    ) -> Result<(WasmtimeMemory, WasmtimePageTable), WasmtimeError> {
        let space = Arc::new(Space::create(shared)?);
        Ok((
            WasmtimeMemory {
                space: space.clone(),
            },
            WasmtimePageTable { space },
        ))
    }
}

impl WasmtimePageTable {
    /// Gives the pages the permissions of a state, or with none leaves them unmapped, for a run of one instruction.
    pub(crate) fn assign(
        &mut self,
        addresses: Range<u64>,
        allowed: Option<Permissions>,
    ) -> Result<(), InvalidAddressError> {
        let pages = pages_of(&addresses)?;
        self.space.set_permissions(pages, allowed);
        Ok(())
    }
}

impl PageTable for WasmtimePageTable {
    fn set_permissions(
        &mut self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError> {
        self.assign(addresses, Some(allowed))
    }

    fn map_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError> {
        let pages = pages_of(&addresses).map_err(MapMemoryError::InvalidAddress)?;
        let other_end = other_start
            .checked_add(addresses.end - addresses.start)
            .ok_or(MapMemoryError::InvalidAddress(InvalidAddressError))?;
        let other_pages =
            pages_of(&(other_start..other_end)).map_err(MapMemoryError::InvalidAddress)?;
        let mapped: Vec<(u64, Backing)> = pages
            .clone()
            .zip(other_pages)
            .map(|(page, other_page)| (page, other.space.backing(other_page)))
            .collect();
        let space = &self.space;
        space
            .change(None, || {
                space.drop_code_locked(addresses.clone());
                space.show_locked(mapped)
            })
            .map_err(|_| MapMemoryError::OutOfMemory)
    }

    fn unmap_from(&mut self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        let pages = pages_of(&addresses)?;
        let space = &self.space;
        let mapped: Vec<u64> = lock(&space.pages)
            .backing
            .range(pages)
            .map(|(&page, _)| page)
            .collect();
        if mapped.is_empty() {
            return Ok(());
        }
        space.change(None, || {
            space.drop_code_locked(addresses.clone());
            for page in mapped {
                // The page showed another space's bytes: it shows its own again, zeroed.
                space
                    .own_zeroed_locked(page..page + 1)
                    .expect("the host gives back a space's own page");
            }
        });
        Ok(())
    }

    fn copy_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        let pages = pages_of(&addresses).map_err(CopyMemoryError::InvalidAddress)?;
        let other_end = other_start
            .checked_add(addresses.end - addresses.start)
            .ok_or(CopyMemoryError::InvalidAddress(InvalidAddressError))?;
        let other_pages =
            pages_of(&(other_start..other_end)).map_err(CopyMemoryError::InvalidAddress)?;
        let (records, backing): (Vec<u8>, Vec<(u64, Backing)>) = {
            let other_records = lock(&other.space.pages);
            let records = other_pages
                .clone()
                .map(|page| other_records.record(page))
                .collect();
            let backing = pages
                .clone()
                .zip(other_pages.clone())
                .filter_map(|(page, other_page)| {
                    other_records
                        .backing
                        .get(&other_page)
                        .map(|backing| (page, backing.clone()))
                })
                .collect();
            (records, backing)
        };
        let space = &self.space;
        let copied = space.change(None, || -> Result<(), rustix::io::Errno> {
            space.drop_code_locked(addresses.clone());
            space.own_zeroed_locked(pages.clone())?;
            space.copy_own_locked(&other.space, other_pages.clone(), pages.start)?;
            space.show_locked(backing)?;
            {
                let mut own = lock(&space.pages);
                for (page, record) in pages.clone().zip(records) {
                    own.change(page, |_| record & !CODE);
                }
            }
            space.write_permission_bytes(pages.clone());
            space.narrowed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        copied.map_err(|_| CopyMemoryError::OutOfMemory)
    }
}

impl WasmtimeBytes<'_> {
    /// Drops the code the bytes held after a write to them.
    fn written(&self) {
        if self.space.any_code(&self.addresses) {
            self.space.drop_code(self.addresses.clone());
        }
    }
}

impl VolatileBytes for WasmtimeBytes<'_> {
    fn len(&self) -> u64 {
        self.bytes.len()
    }

    fn read(&self, into: &mut [u8]) -> usize {
        self.bytes.read(into)
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let written = self.bytes.write(bytes);
        self.written();
        written
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        self.bytes.read_to(sink)
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let written = self.bytes.write_from(source);
        self.written();
        written
    }
}
