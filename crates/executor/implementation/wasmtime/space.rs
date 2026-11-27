//! A guest's address space: its memory file and host memory, its page permissions, its state memory and its gate.
//!
//! Its 2^48 bytes of addresses are in windows of 4 GiB (the translator's address windows); the guest memory holds
//! [`HOST_WINDOWS`] of them, window 0 first, each followed by a page that shows the next window's first page.

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
    self, SLOT_BYTES, VIRTUAL_ADDRESS_BITS, WASM_PAGE_BYTES, WINDOW_BYTES, WINDOW_TABLE_BYTES,
    WINDOWS, permission_byte, progress_count_offset, progress_limit_offset, thread_state_size,
};
use wcontainers_volatile_bytes::VolatileBytes;
use wcontainers_wasmtime::Wasmtime;
use wcontainers_wasmtime_direct::{DirectBytes, DirectHostMemory};

use crate::WasmtimeError;
use crate::engine::{Shared, Translated};
use crate::state_memory::StateMemory;

/// The bytes of address a space has: those of a 48-bit virtual address.
pub(crate) const SPACE_BYTES: u64 = 1 << VIRTUAL_ADDRESS_BITS;
/// The pages of address one window holds.
const WINDOW_PAGES: u64 = WINDOW_BYTES / PAGE_SIZE;
/// Tuning number: the windows of addresses a space's guest memory holds at once, window 0 among them.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
pub(crate) const HOST_WINDOWS: u64 = 8;
/// The bytes after each window's place that show the next window's first page: a WebAssembly page.
///
/// A WebAssembly page keeps every window's place whole host pages on every host.
const SPILL_BYTES: u64 = WASM_PAGE_BYTES;
/// The pages from one window's place to the next one's.
const STRIDE_PAGES: u64 = (WINDOW_BYTES + SPILL_BYTES) / PAGE_SIZE;
/// The bytes of a space's guest memory: its windows' places, each with the bytes after it.
pub(crate) const GUEST_BYTES: u64 = HOST_WINDOWS * STRIDE_PAGES * PAGE_SIZE;
/// The inaccessible bytes after a guest memory, which wasmtime's compiled code may rely on to fault: its default guard.
///
/// <https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html#method.memory_guard_size>
pub(crate) const GUEST_GUARD_BYTES: u64 = 32 << 20;
/// The pages of a space's guest memory, which the permission table has a byte for each of.
pub(crate) const GUEST_PAGES: u64 = GUEST_BYTES / PAGE_SIZE;
/// The window table's place for a window the guest memory does not hold: past the permission table's pages.
const NO_PLACE: u64 = GUEST_BYTES;
/// What [`Pages::windows`] holds for a window not given a place.
const NO_WINDOW: u8 = u8::MAX;
/// The bytes of the state memory: all a 32-bit memory has.
const STATE_BYTES: u64 = 1 << u32::BITS;
/// Tuning number: the bytes of the state memory given to thread blocks, from its start.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const BLOCKS_BYTES: u64 = 1 << 20;
/// Thread blocks start at a multiple of the guest's widest register, a 128-bit vector register.
const THREAD_ALIGN: u64 = size_of::<u128>() as u64;
/// The bytes of one thread's state in the state memory.
pub(crate) const THREAD_BYTES: u64 =
    (thread_state_size::<Arm64>() as u64).next_multiple_of(THREAD_ALIGN);
/// Where the window table starts: the place of each window, then the place of an address above them all.
const WINDOW_TABLE_AT: u64 = BLOCKS_BYTES;
/// Where the permission table starts, where the window table ends: one byte for each page, then a zero byte.
pub(crate) const PERMISSIONS_AT: u64 = WINDOW_TABLE_AT + WINDOW_TABLE_BYTES;
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
/// Tuning number: the pages of records kept together, allocated when one of them is first given anything.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const CHUNK_PAGES: usize = 1 << 12;
/// Tuning number: the bytes copied at once between two memory files.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const COPY_BYTES: usize = 1 << 20;

/// The bits of a record that are its permissions.
const PERMISSION_BITS: u8 = Permissions::all().bits();
/// A page record's bit above its permissions (`Permissions::bits`): it was given permissions, so it is mapped.
const MAPPED: u8 = PERMISSION_BITS + 1;
/// The page holds code a load was translated from, so the table takes writing from it.
const CODE: u8 = MAPPED << 1;

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

/// Each page's record and backing, by its place in the guest memory, and the place of each window.
struct Pages {
    chunks: Vec<Option<Box<[u8; CHUNK_PAGES]>>>,
    /// By page of the guest memory, where a page mapped from another space's pages shows them from.
    backing: BTreeMap<u64, Backing>,
    /// By window of addresses, which of the guest memory's windows it is shown in, or [`NO_WINDOW`].
    windows: Vec<u8>,
    /// How many of the guest memory's windows are given.
    given: u8,
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

/// The guest memory a [`WasmtimeExecutor`](crate::WasmtimeExecutor) makes: a space of 2^48 bytes of addresses.
///
/// At most [`HOST_WINDOWS`] windows of 4 GiB of them hold bytes or permissions at once.
pub struct WasmtimeMemory {
    pub(crate) space: Arc<Space>,
}

/// The one page table of a [`WasmtimeMemory`], made with it.
pub struct WasmtimePageTable {
    space: Arc<Space>,
}

/// Addresses of a [`WasmtimeMemory`], which a guest may be changing while they are held.
pub struct WasmtimeBytes<'space> {
    /// The bytes in one part for each window they touch.
    parts: Vec<DirectBytes>,
    space: &'space Space,
    addresses: Range<u64>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Returns the pages the addresses touch, after checking they lie in the space.
fn pages_of(addresses: &Range<u64>) -> Result<Range<u64>, InvalidAddressError> {
    if addresses.start > addresses.end || addresses.end > SPACE_BYTES {
        return Err(InvalidAddressError);
    }
    Ok(addresses.start / PAGE_SIZE..addresses.end.div_ceil(PAGE_SIZE))
}

/// Returns the pages split where a window ends, so that each part lies in one window.
fn split_at_windows(pages: Range<u64>) -> impl Iterator<Item = Range<u64>> {
    let mut start = pages.start;
    std::iter::from_fn(move || {
        if start >= pages.end {
            return None;
        }
        let end = pages.end.min((start / WINDOW_PAGES + 1) * WINDOW_PAGES);
        let part = start..end;
        start = end;
        Some(part)
    })
}

/// Returns the first page of the guest memory's window.
const fn first_place_page(host_window: u8) -> u64 {
    host_window as u64 * STRIDE_PAGES
}

/// Returns whether two ranges share an address.
fn overlaps(first: &Range<u64>, second: &Range<u64>) -> bool {
    first.start < second.end && second.start < first.end
}

impl Pages {
    /// Returns the page of the guest memory the page of address is shown in, if its window is given a place.
    fn place(&self, page: u64) -> Option<u64> {
        let window = usize::try_from(page / WINDOW_PAGES).ok()?;
        let host_window = *self.windows.get(window)?;
        (host_window != NO_WINDOW).then(|| first_place_page(host_window) + page % WINDOW_PAGES)
    }

    /// Returns the page of address a page of the guest memory shows, if it shows one.
    ///
    /// A page of a window's place shows a page of the window, and the page after the place the next window's first.
    fn address_page(&self, place: u64) -> Option<u64> {
        let host_window = u8::try_from(place / STRIDE_PAGES).ok()?;
        let window = self
            .windows
            .iter()
            .position(|&given| given == host_window)? as u64;
        Some(window * WINDOW_PAGES + place % STRIDE_PAGES)
    }

    fn record(&self, page: u64) -> u8 {
        let Some(place) = self.place(page) else {
            return u8::MIN;
        };
        let place = usize::try_from(place).unwrap_or(usize::MAX);
        self.chunks
            .get(place / CHUNK_PAGES)
            .and_then(Option::as_ref)
            .map_or(u8::MIN, |chunk| chunk[place % CHUNK_PAGES])
    }

    /// Changes the record of a page whose window has a place.
    fn change(&mut self, page: u64, change: impl FnOnce(u8) -> u8) {
        let place = self.place(page).expect("a page of a window with a place");
        let place = usize::try_from(place).expect("a page of the guest memory");
        let chunk = self.chunks[place / CHUNK_PAGES]
            .get_or_insert_with(|| Box::new([u8::MIN; CHUNK_PAGES]));
        let record = &mut chunk[place % CHUNK_PAGES];
        *record = change(*record);
    }

    /// Returns what the table says a page allows: what it was given, without writing while it holds code.
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
            u32::try_from(STATE_WASM_PAGES).expect("the pages of the state memory"),
        )
        .map_err(|error| WasmtimeError::HostRefused(error.to_string()))?;
        let chunks = usize::try_from(GUEST_PAGES)
            .expect("a count of pages")
            .div_ceil(CHUNK_PAGES);
        // Window 0 is the first window of the guest memory, at 0; every other has no place yet.
        let mut windows = vec![NO_WINDOW; usize::try_from(WINDOWS).expect("a count of windows")];
        windows[usize::MIN] = u8::MIN;
        let given = u8::try_from(windows.iter().filter(|&&host| host != NO_WINDOW).count())
            .expect("a count of the guest memory's windows");
        let no_place = NO_PLACE.to_le_bytes();
        let table: Vec<u8> = (0..=WINDOWS)
            .flat_map(|window| {
                if window == u64::MIN {
                    u64::MIN.to_le_bytes()
                } else {
                    no_place
                }
            })
            .collect();
        state.write(
            usize::try_from(WINDOW_TABLE_AT).expect("an offset in the state memory"),
            &table,
        );
        Ok(Space {
            shared,
            file: Arc::new(file),
            host,
            state,
            pages: Mutex::new(Pages {
                chunks: (0..chunks).map(|_| None).collect(),
                backing: BTreeMap::new(),
                windows,
                given,
            }),
            code: Mutex::new(Code {
                loads: Vec::new(),
                next_slot: contract::EXACT_SLOT as u32 + 1,
            }),
            gate: Gate {
                running: AtomicUsize::new(usize::MIN),
                wanted: AtomicBool::new(false),
                changing: Mutex::new(()),
                blocks: Mutex::new(Vec::new()),
            },
            blocks: Mutex::new((Vec::new(), u64::MIN)),
            narrowed: AtomicU64::new(u64::MIN),
            load_count: AtomicUsize::new(usize::MIN),
        })
    }

    /// Gives the windows of the pages that have no place in the guest memory one; false when none is left.
    fn give_places(&self, pages: Range<u64>) -> bool {
        let mut records = lock(&self.pages);
        for part in split_at_windows(pages) {
            let window = part.start / WINDOW_PAGES;
            let index = usize::try_from(window).expect("a window of the space");
            if records.windows[index] != NO_WINDOW {
                continue;
            }
            if u64::from(records.given) == HOST_WINDOWS {
                return false;
            }
            let host_window = records.given;
            records.given += 1;
            records.windows[index] = host_window;
            let place = first_place_page(host_window) * PAGE_SIZE;
            let entry = WINDOW_TABLE_AT + window * size_of::<u64>() as u64;
            self.state.store_u64(
                usize::try_from(entry).expect("an offset in the state memory"),
                place,
            );
            drop(records);
            // The window before shows this one's first page after its place, and this one the next one's.
            if let Some(before) = window.checked_sub(1) {
                self.show_spill(before);
            }
            self.show_spill(window);
            records = lock(&self.pages);
        }
        true
    }

    /// Returns how many windows of the guest memory are given to windows of addresses.
    pub(crate) fn windows_given(&self) -> u64 {
        u64::from(lock(&self.pages).given)
    }

    /// Returns the address of a place in the guest memory; the place itself for one no window shows.
    pub(crate) fn address_of_place(&self, place: u64) -> u64 {
        let page = lock(&self.pages).address_page(place / PAGE_SIZE);
        page.map_or(place, |page| page * PAGE_SIZE + place % PAGE_SIZE)
    }

    /// Returns the bytes at the addresses in one part for each window they touch, giving their windows a place.
    ///
    /// None outside the space or when the guest memory has no window left for them.
    pub(crate) fn bytes(&self, addresses: Range<u64>) -> Option<Vec<DirectBytes>> {
        let pages = pages_of(&addresses).ok()?;
        if !self.give_places(pages) {
            return None;
        }
        let records = lock(&self.pages);
        let mut parts = Vec::new();
        let mut start = addresses.start;
        while start < addresses.end {
            let end = addresses.end.min((start / WINDOW_BYTES + 1) * WINDOW_BYTES);
            let place = records.place(start / PAGE_SIZE)? * PAGE_SIZE + start % PAGE_SIZE;
            parts.push(
                self.shared
                    .wasmtime
                    .host_memory_bytes(&self.host, place..place + (end - start))?,
            );
            start = end;
        }
        Some(parts)
    }

    /// Copies the bytes at the address into the buffer; false when they do not all lie in the space.
    pub(crate) fn read(&self, address: u64, into: &mut [u8]) -> bool {
        let Some(end) = address.checked_add(into.len() as u64) else {
            return false;
        };
        let Some(parts) = self.bytes(address..end) else {
            return false;
        };
        let mut done = usize::MIN;
        for part in parts {
            done += part.read(&mut into[done..]);
        }
        done == into.len()
    }

    /// Returns what the page was given, or nothing for a page outside the space.
    pub(crate) fn permissions(&self, page: u64) -> Permissions {
        Permissions::from_bits_truncate(lock(&self.pages).record(page) & PERMISSION_BITS)
    }

    /// Returns whether the page was given permissions, so that a fault on it is one of permission, not translation.
    pub(crate) fn is_mapped(&self, page: u64) -> bool {
        lock(&self.pages).record(page) & MAPPED != 0
    }

    /// Returns whether the table holds writing back from the page because code was translated from it.
    pub(crate) fn holds_code(&self, page: u64) -> bool {
        lock(&self.pages).record(page) & CODE != 0
    }

    /// Returns the run of pages around the address that allow running code, if its own page does.
    ///
    /// A page past the space has no record, so it allows nothing and ends the run.
    pub(crate) fn find_executable_run(&self, address: u64) -> Option<Range<u64>> {
        let pages = lock(&self.pages);
        let executable = |page: u64| {
            Permissions::from_bits_truncate(pages.record(page) & PERMISSION_BITS)
                .contains(Permissions::EXECUTE)
        };
        let page = address / PAGE_SIZE;
        if !executable(page) {
            return None;
        }
        // The page itself allows running code, so each search finds it at least.
        let first = (0..=page)
            .rev()
            .take_while(|&p| executable(p))
            .last()
            .unwrap_or(page);
        let last = (page..)
            .take_while(|&p| executable(p))
            .last()
            .unwrap_or(page);
        Some(first * PAGE_SIZE..(last + 1) * PAGE_SIZE)
    }

    /// Returns the parts of the pages, one for each window they touch, whose window has a place.
    fn placed_parts(&self, pages: Range<u64>) -> Vec<Range<u64>> {
        let records = lock(&self.pages);
        split_at_windows(pages)
            .filter(|part| records.place(part.start).is_some())
            .collect()
    }

    /// Gives the pages permissions, or with none leaves them unmapped; false when no window is left for them.
    ///
    /// A narrowing waits until no thread runs; a refusal changes nothing.
    pub(crate) fn set_permissions(&self, pages: Range<u64>, allowed: Option<Permissions>) -> bool {
        if allowed.is_some() && !self.give_places(pages.clone()) {
            return false;
        }
        let parts = self.placed_parts(pages);
        let new = allowed.map_or(u8::MIN, |allowed| allowed.bits() | MAPPED);
        let narrows = {
            let records = lock(&self.pages);
            parts.iter().cloned().flatten().any(|page| {
                let old = records.record(page);
                let lost = old & PERMISSION_BITS & !new;
                lost & (Permissions::READ | Permissions::WRITE).bits() != 0
                    || (lost & Permissions::EXECUTE.bits() != 0 && old & CODE != 0)
            })
        };
        let apply = || {
            let losing_code: Vec<u64> = {
                let records = lock(&self.pages);
                parts
                    .iter()
                    .cloned()
                    .flatten()
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
                for page in parts.iter().cloned().flatten() {
                    records.change(page, |record| new | (record & CODE));
                }
            }
            for part in &parts {
                self.write_permission_bytes(part.clone());
            }
        };
        if narrows {
            self.change(None, || {
                apply();
                self.narrowed.fetch_add(1, Ordering::SeqCst);
            });
        } else {
            apply();
        }
        true
    }

    /// Writes the permission table's bytes of the pages and of the page before them.
    ///
    /// Each byte says what the page after it allows too: for a window's last page, what the next window's first page
    /// allows, which the guest memory shows after the window's place.
    ///
    /// The pages lie in the space, and the page past the space has no record, so it allows nothing.
    pub(crate) fn write_permission_bytes(&self, pages: Range<u64>) {
        // The pages of one window have places one after another, so each window's bytes are written at once.
        let runs: Vec<(u64, Vec<u8>)> = {
            let records = lock(&self.pages);
            split_at_windows(pages.start.saturating_sub(1)..pages.end)
                .filter_map(|part| {
                    let place = records.place(part.start)?;
                    let bytes = part
                        .map(|page| {
                            permission_byte(records.effective(page), records.effective(page + 1))
                        })
                        .collect();
                    Some((place, bytes))
                })
                .collect()
        };
        for (place, bytes) in runs {
            let at =
                usize::try_from(PERMISSIONS_AT + place).expect("an offset in the state memory");
            self.state.write(at, &bytes);
        }
    }

    /// Shows the next window's first page after the window's place, and writes the byte of the window's last page.
    ///
    /// The page is shown from where it shows its bytes, and the byte says what it allows; nothing is done while either
    /// window has no place.
    fn show_spill(&self, window: u64) {
        let last = (window + 1) * WINDOW_PAGES - 1;
        let next = last + 1;
        let (spill, backing) = {
            let records = lock(&self.pages);
            let (Some(last_place), Some(next_place)) = (records.place(last), records.place(next))
            else {
                return;
            };
            let backing = records
                .backing
                .get(&next_place)
                .cloned()
                .unwrap_or_else(|| Backing {
                    file: self.file.clone(),
                    offset: next_place * PAGE_SIZE,
                });
            (last_place + 1, backing)
        };
        self.shared
            .wasmtime
            .map_file(
                &self.host,
                spill * PAGE_SIZE..(spill + 1) * PAGE_SIZE,
                backing.file.as_fd(),
                backing.offset,
            )
            .expect("the host maps a page of the space's own memory again");
        self.write_permission_bytes(next..next);
    }

    /// Shows the first page of each window among the pages after the place of the window before it again.
    fn show_spills(&self, pages: Range<u64>) {
        let first_window = pages.start.div_ceil(WINDOW_PAGES).max(1);
        let end_window = pages.end.div_ceil(WINDOW_PAGES);
        for window in first_window..end_window {
            self.show_spill(window - 1);
        }
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
        let mut count = [u8::MIN; size_of::<u64>()];
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
                    u32::try_from(next).expect("a block inside the state memory")
                }
            }
        };
        let at = usize::try_from(block).expect("a block offset");
        self.state.fill(
            at,
            usize::try_from(THREAD_BYTES).expect("a block's bytes"),
            u8::MIN,
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
            return contract::EMPTY_SLOT.cast_unsigned();
        };
        let mut entry = [u8::MIN; SLOT_BYTES];
        self.state.read(at, &mut entry);
        u32::from_le_bytes(entry)
    }

    fn set_slot(&self, address: u64, slot: u32) {
        if let Some(at) = slot_entry(address) {
            self.state.write(at, &slot.to_le_bytes());
        }
    }

    /// Places a translation of the code at the load address, and returns the load and whether a page lost writing.
    ///
    /// Its pieces get slots, the pages it was read from lose writing, and each piece's address gets its slot.
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
                    let end = load_address.saturating_add(read.end).min(SLOTS_COVER);
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

    /// Drops every load read from the addresses and gives writing back to their pages.
    ///
    /// The lookup then finds none of its pieces; it is called only inside a change.
    pub(crate) fn drop_code_locked(&self, addresses: Range<u64>) {
        let Ok(pages) = pages_of(&addresses).map(code_pages) else {
            return;
        };
        let held: Vec<u64> = {
            let records = lock(&self.pages);
            pages
                .filter(|&page| records.record(page) & CODE != 0)
                .collect()
        };
        if held.is_empty() {
            return;
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
                        self.set_slot(address, contract::EMPTY_SLOT.cast_unsigned());
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
    }

    /// Drops every load, so that the lookup finds none of their pieces, while no thread runs.
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
                        self.set_slot(address, contract::EMPTY_SLOT.cast_unsigned());
                    }
                    slot += 1;
                }
            }
        }
    }

    /// Drops the code read from the addresses, if any, once no thread runs.
    pub(crate) fn drop_code(&self, addresses: Range<u64>) {
        let Ok(pages) = pages_of(&addresses).map(code_pages) else {
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

    /// Returns where a page of a window with a place shows its bytes from: its own file or its mapping's backing.
    fn backing(&self, page: u64) -> Backing {
        let records = lock(&self.pages);
        let place = records
            .place(page)
            .expect("a page of a window with a place");
        records
            .backing
            .get(&place)
            .cloned()
            .unwrap_or_else(|| Backing {
                file: self.file.clone(),
                offset: place * PAGE_SIZE,
            })
    }

    /// Maps each run of the pages, whose windows have a place, from its backing and records it, inside a change.
    fn show_locked(&self, mapped: Vec<(u64, Backing)>) -> Result<(), rustix::io::Errno> {
        let Some(pages) = mapped
            .first()
            .zip(mapped.last())
            .map(|(&(first, _), &(last, _))| first..last + 1)
        else {
            return Ok(());
        };
        let placed: Vec<(u64, Backing)> = {
            let records = lock(&self.pages);
            mapped
                .into_iter()
                .map(|(page, backing)| {
                    let place = records
                        .place(page)
                        .expect("a page of a window with a place");
                    (place, backing)
                })
                .collect()
        };
        let mut index = usize::MIN;
        while index < placed.len() {
            let (first, ref backing) = placed[index];
            let mut end = index + 1;
            while end < placed.len()
                && placed[end].0 == first + (end - index) as u64
                && Arc::ptr_eq(&placed[end].1.file, &backing.file)
                && placed[end].1.offset == backing.offset + (end - index) as u64 * PAGE_SIZE
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
        {
            let mut records = lock(&self.pages);
            for (place, backing) in placed {
                if Arc::ptr_eq(&backing.file, &self.file) && backing.offset == place * PAGE_SIZE {
                    records.backing.remove(&place);
                } else {
                    records.backing.insert(place, backing);
                }
            }
        }
        self.show_spills(pages);
        Ok(())
    }

    /// Shows the pages from this space's own file again, zeroed, inside a change.
    ///
    /// Pages of a window with no place are zero already.
    fn own_zeroed_locked(&self, pages: Range<u64>) -> Result<(), rustix::io::Errno> {
        for part in self.placed_parts(pages) {
            let place = lock(&self.pages)
                .place(part.start)
                .expect("a part of a window with a place");
            let places = place..place + (part.end - part.start);
            rustix::fs::fallocate(
                &*self.file,
                FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
                places.start * PAGE_SIZE,
                (places.end - places.start) * PAGE_SIZE,
            )?;
            let mapped: Vec<u64> = lock(&self.pages)
                .backing
                .range(places.clone())
                .map(|(&place, _)| part.start + (place - places.start))
                .collect();
            let own = mapped
                .into_iter()
                .map(|page| {
                    let backing = Backing {
                        file: self.file.clone(),
                        offset: (places.start + (page - part.start)) * PAGE_SIZE,
                    };
                    (page, backing)
                })
                .collect();
            self.show_locked(own)?;
        }
        Ok(())
    }

    /// Copies the data of the other space's own file at the pages into this space's own file, inside a change.
    ///
    /// This space's windows of the pages from `into` must have a place.
    ///
    /// Each page's bytes land at the page as far past `into` as it is past the first page, and what the other file
    /// holds no data at is skipped.
    fn copy_own_locked(
        &self,
        other: &Space,
        pages: Range<u64>,
        into: u64,
    ) -> Result<(), rustix::io::Errno> {
        let mut page = pages.start;
        while page < pages.end {
            let target = into + (page - pages.start);
            let end = pages
                .end
                .min((page / WINDOW_PAGES + 1) * WINDOW_PAGES)
                .min(page + ((target / WINDOW_PAGES + 1) * WINDOW_PAGES - target));
            let from = lock(&other.pages).place(page);
            if let Some(from) = from {
                let to = lock(&self.pages)
                    .place(target)
                    .expect("a page of a window with a place");
                copy_file_data(
                    &other.file,
                    from * PAGE_SIZE..(from + (end - page)) * PAGE_SIZE,
                    &self.file,
                    to * PAGE_SIZE,
                )?;
            }
            page = end;
        }
        Ok(())
    }
}

/// Copies the data of the file at the offsets to the other file from the offset, skipping the file's holes.
///
/// Both are memory files of a space, which cannot shrink: a read inside one gives bytes, and a write takes them all
/// or fails.
fn copy_file_data(
    file: &OwnedFd,
    offsets: Range<u64>,
    into: &OwnedFd,
    at: u64,
) -> Result<(), rustix::io::Errno> {
    let mut buffer = vec![u8::MIN; COPY_BYTES];
    let mut next = offsets.start;
    // Data found at or past the end, or none at all, ends the copy.
    loop {
        let data = match rustix::fs::seek(file, SeekFrom::Data(next)) {
            Ok(data) if data < offsets.end => data,
            Ok(_) | Err(rustix::io::Errno::NXIO) => return Ok(()),
            Err(error) => return Err(error),
        };
        let hole = rustix::fs::seek(file, SeekFrom::Hole(data))?.min(offsets.end);
        let mut from = data;
        while from < hole {
            let length =
                usize::try_from((hole - from).min(COPY_BYTES as u64)).expect("a buffer's length");
            let read = rustix::io::pread(file, &mut buffer[..length], from)?;
            assert_ne!(read, usize::MIN, "a memory file reads inside its size");
            let written = rustix::io::pwrite(into, &buffer[..read], from - offsets.start + at)?;
            if written != read {
                return Err(rustix::io::Errno::NOSPC);
            }
            from += read as u64;
        }
        next = hole;
    }
}

/// Returns those of the pages code may be read from: the ones the array of slots covers, all in window 0.
fn code_pages(pages: Range<u64>) -> Range<u64> {
    let covered = SLOTS_COVER.div_ceil(PAGE_SIZE);
    pages.start.min(covered)..pages.end.min(covered)
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

    /// Borrows the bytes, giving their windows a place, or refuses them when no window is left for them.
    fn borrow(&self, addresses: Range<u64>) -> Result<WasmtimeBytes<'_>, InvalidAddressError> {
        pages_of(&addresses)?;
        let parts = self
            .space
            .bytes(addresses.clone())
            .ok_or(InvalidAddressError)?;
        Ok(WasmtimeBytes {
            parts,
            space: &self.space,
            addresses,
        })
    }

    /// Gives whole pages of the space's own file back to the host, and writes zeros over the rest.
    ///
    /// A window with no place is zero already.
    fn zero(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        let pages = pages_of(&addresses)?;
        let space = &self.space;
        for page in space.placed_parts(pages).into_iter().flatten() {
            let bytes = page * PAGE_SIZE..(page + 1) * PAGE_SIZE;
            let cleared = addresses.start.max(bytes.start)..addresses.end.min(bytes.end);
            // A whole page of its own file goes back to the host; any other bytes are written over.
            let own_place = {
                let records = lock(&space.pages);
                records
                    .place(page)
                    .filter(|place| !records.backing.contains_key(place))
            };
            match own_place {
                Some(place) if cleared == bytes => {
                    rustix::fs::fallocate(
                        &*space.file,
                        FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
                        place * PAGE_SIZE,
                        PAGE_SIZE,
                    )
                    .expect("a memory file gives a page back");
                }
                _ => {
                    let zeros = vec![
                        u8::MIN;
                        usize::try_from(cleared.end - cleared.start)
                            .expect("part of a page")
                    ];
                    for view in space.bytes(cleared).into_iter().flatten() {
                        view.write(&zeros[..usize::try_from(view.len()).expect("part of a page")]);
                    }
                }
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
    ///
    /// Pages whose windows the guest memory has no room left for are refused as outside the space.
    pub(crate) fn assign(
        &mut self,
        addresses: Range<u64>,
        allowed: Option<Permissions>,
    ) -> Result<(), InvalidAddressError> {
        let pages = pages_of(&addresses)?;
        if self.space.set_permissions(pages, allowed) {
            Ok(())
        } else {
            Err(InvalidAddressError)
        }
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
        if !self.space.give_places(pages.clone()) || !other.space.give_places(other_pages.clone()) {
            return Err(MapMemoryError::OutOfMemory);
        }
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
        let parts = space.placed_parts(pages);
        let mapped: Vec<u64> = {
            let records = lock(&space.pages);
            parts
                .into_iter()
                .flatten()
                .filter(|&page| {
                    records
                        .place(page)
                        .is_some_and(|place| records.backing.contains_key(&place))
                })
                .collect()
        };
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
        // Only the windows the other space gives a place hold anything to copy; this space's take one.
        let held: Vec<(Range<u64>, Range<u64>)> = {
            let other_records = lock(&other.space.pages);
            pages
                .clone()
                .zip(other_pages.clone())
                .filter(|&(_, other_page)| other_records.place(other_page).is_some())
                .fold(
                    Vec::new(),
                    |mut runs: Vec<(Range<u64>, Range<u64>)>, (page, other_page)| {
                        match runs.last_mut() {
                            Some((run, other_run))
                                if run.end == page && other_run.end == other_page =>
                            {
                                run.end += 1;
                                other_run.end += 1;
                            }
                            _ => runs.push((page..page + 1, other_page..other_page + 1)),
                        }
                        runs
                    },
                )
        };
        if held
            .iter()
            .any(|(run, _)| !self.space.give_places(run.clone()))
        {
            return Err(CopyMemoryError::OutOfMemory);
        }
        let (records, backing): (Vec<(u64, u8)>, Vec<(u64, Backing)>) = {
            let other_records = lock(&other.space.pages);
            let records = held
                .iter()
                .flat_map(|(run, other_run)| run.clone().zip(other_run.clone()))
                .map(|(page, other_page)| (page, other_records.record(other_page)))
                .collect();
            let backing = held
                .iter()
                .flat_map(|(run, other_run)| run.clone().zip(other_run.clone()))
                .filter_map(|(page, other_page)| {
                    let other_place = other_records.place(other_page)?;
                    other_records
                        .backing
                        .get(&other_place)
                        .map(|backing| (page, backing.clone()))
                })
                .collect();
            (records, backing)
        };
        let space = &self.space;
        let copied = space.change(None, || -> Result<(), rustix::io::Errno> {
            space.drop_code_locked(addresses.clone());
            space.own_zeroed_locked(pages.clone())?;
            for (run, other_run) in &held {
                space.copy_own_locked(&other.space, other_run.clone(), run.start)?;
            }
            space.show_locked(backing)?;
            let parts = space.placed_parts(pages.clone());
            {
                let mut own = lock(&space.pages);
                for &(page, record) in &records {
                    own.change(page, |_| record & !CODE);
                }
                // Pages the other space never gave a place hold nothing and allow nothing here either.
                for page in parts.iter().cloned().flatten() {
                    if records
                        .binary_search_by_key(&page, |&(held, _)| held)
                        .is_err()
                    {
                        own.change(page, |_| u8::MIN);
                    }
                }
            }
            for part in parts {
                space.write_permission_bytes(part);
            }
            space.narrowed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        copied.map_err(|_| CopyMemoryError::OutOfMemory)
    }
}

impl WasmtimeBytes<'_> {
    /// Drops the code the bytes held after a write to them; [`Space::drop_code`] does nothing where none is held.
    fn written(&self) {
        self.space.drop_code(self.addresses.clone());
    }
}

impl VolatileBytes for WasmtimeBytes<'_> {
    fn len(&self) -> u64 {
        self.parts.iter().map(DirectBytes::len).sum()
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let mut done = usize::MIN;
        for part in &self.parts {
            let read = part.read(&mut into[done..]);
            done += read;
            if u64::try_from(read).ok() != Some(part.len()) {
                break;
            }
        }
        done
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let mut done = usize::MIN;
        for part in &self.parts {
            let written = part.write(&bytes[done..]);
            done += written;
            if u64::try_from(written).ok() != Some(part.len()) {
                break;
            }
        }
        self.written();
        done
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut done = usize::MIN;
        for part in &self.parts {
            let read = part.read_to(sink)?;
            done += read;
            if u64::try_from(read).ok() != Some(part.len()) {
                break;
            }
        }
        Ok(done)
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut done = usize::MIN;
        let mut failed = None;
        for part in &self.parts {
            match part.write_from(source) {
                Ok(written) => {
                    done += written;
                    if u64::try_from(written).ok() != Some(part.len()) {
                        break;
                    }
                }
                Err(error) => {
                    failed = Some(error);
                    break;
                }
            }
        }
        self.written();
        failed.map_or(Ok(done), Err)
    }
}
