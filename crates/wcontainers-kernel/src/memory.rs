//! A process's address space as the kernel keeps it: the ranges it has mapped and what each allows.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::Cell;

use rangemap::RangeSet;
use wcontainers_abi::{Cpu, LOWEST_ADDRESS, MemoryId, PAGE_SIZE, Permissions};

use crate::fs::Node;
use crate::linux::general::{RLIMIT_AS, RLIMIT_DATA, RLIMIT_MEMLOCK, RLIMIT_STACK};
use crate::process::{LIMITS, Limit};
use crate::system::{AccessKind, MemoryError, WatchesFor};
use crate::{Errno, errno};

pub(crate) fn page_down(address: u64) -> u64 {
    address & !(PAGE_SIZE - 1)
}

pub(crate) fn page_up(address: u64) -> u64 {
    page_down(address.saturating_add(PAGE_SIZE - 1))
}

/// Linux's `PAGE_ALIGN` of a length a program passes: it wraps, so the last page's lengths become 0.
pub(crate) fn page_align(length: u64) -> u64 {
    page_down(length.wrapping_add(PAGE_SIZE - 1))
}

/// Linux's default stack limit, `RLIMIT_STACK`, which places a new program's mappings.
pub(crate) const STACK_LIMIT: u64 = 8 << 20;

/// The gap Linux keeps below a stack that may grow (`stack_guard_gap`, 256 pages).
pub(crate) const STACK_GUARD_GAP: u64 = 256 * PAGE_SIZE;

/// The least gap Linux leaves between the stack's top and the mmap base (`MIN_GAP`, mm/util.c).
const MMAP_GAP_MIN: u64 = 128 << 20;

/// The greatest gap, as a fraction of the stack's top (`MAX_GAP`, five sixths, mm/util.c).
const MMAP_GAP_MAX_SIXTHS: u64 = 5;
const SIXTHS: u64 = 6;

/// The gap below a stack's top at `top` before the mmap base, as Linux's `mmap_base` sizes it for
/// the default stack limit, so an address space of any size keeps room for mappings.
fn mmap_gap(top: u64) -> u64 {
    let gap = STACK_LIMIT + STACK_GUARD_GAP;
    let max = top / SIXTHS * MMAP_GAP_MAX_SIXTHS;
    if gap < MMAP_GAP_MIN && MMAP_GAP_MIN < max {
        MMAP_GAP_MIN
    } else {
        gap.min(max)
    }
}

/// What a range holds, for `/proc/<pid>/maps` and for what happens past the end of a file.
#[derive(Clone)]
pub(crate) enum Backing {
    Anonymous,
    /// Zeroed memory that names a file: shared anonymous memory and shared mappings of `/dev/zero`,
    /// which Linux backs with a removed file on its internal tmpfs, or a private mapping of
    /// `/dev/zero`, which is anonymous memory that keeps the device's file. The file's device, inode
    /// and the offset in it, and the name `/proc/<pid>/maps` shows.
    Zero {
        device: u64,
        ino: u64,
        offset: u64,
        name: Vec<u8>,
    },
    /// A file's pages from `offset`, shown from the file's block of shared pages. Pages wholly past
    /// the file's end, from `file_end` on, are kept with no access, and touching one is `SIGBUS` as on
    /// Linux. A private mapping's pages in `copied` (file page numbers) hold its own copy instead,
    /// made when the page was first written (Linux's copy on write); until then a page shows the
    /// file's bytes as they change.
    File {
        node: Node,
        path: Vec<u8>,
        offset: u64,
        file_end: u64,
        copied: BTreeSet<u64>,
    },
    /// Private anonymous memory whose pages Linux may drop under memory pressure (`MAP_DROPPABLE`):
    /// never locked, wiped on fork.
    Droppable,
    Heap,
    Stack,
    /// Pages the kernel itself provides, named in `/proc/<pid>/maps` (`[vvar]`, `[vdso]`); never
    /// merged with others.
    Special(&'static str),
}

#[derive(Clone)]
pub(crate) struct Mapping {
    pub start: u64,
    pub end: u64,
    /// What the program asked for.
    pub permissions: Permissions,
    /// What `mprotect` may give it (Linux's `VM_MAYREAD`, `VM_MAYWRITE` and `VM_MAYEXEC`).
    pub allowed: Permissions,
    pub shared: bool,
    /// Whether `mlock` holds it in memory (Linux's `VM_LOCKED`).
    pub locked: Lock,
    /// Whether a fault below it grows it, with a guard gap kept free below (Linux's `VM_GROWSDOWN`):
    /// the stack, and `MAP_GROWSDOWN` memory.
    pub grows_down: bool,
    /// What a fork gives the child of it (`MADV_DONTFORK`, `MADV_WIPEONFORK`).
    pub on_fork: OnFork,
    pub backing: Backing,
}

/// What a fork does with a mapping: by default the child gets a copy (or, shared, the same pages).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct OnFork {
    /// The child does not get it (`VM_DONTCOPY`).
    pub skip: bool,
    /// The child gets it zeroed (`VM_WIPEONFORK`).
    pub wipe: bool,
}

/// Whether a mapping is locked in memory, and if so whether its pages are brought in at once or as
/// they are touched (`VM_LOCKONFAULT`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lock {
    #[default]
    None,
    Now,
    OnFault,
}

impl Mapping {
    /// Whether the mapping counts against `RLIMIT_DATA`: private and writable, and not a stack
    /// (Linux's `is_data_mapping`).
    pub fn is_data(&self) -> bool {
        is_data(self.permissions, self.shared, self.grows_down)
    }

    /// Where the mapping's reach starts: below a mapping that grows down, its guard gap is kept free
    /// (Linux's `vm_start_gap`).
    pub fn start_gap(&self) -> u64 {
        if self.grows_down {
            self.start.saturating_sub(STACK_GUARD_GAP)
        } else {
            self.start
        }
    }

    /// What the page at `address` allows the program (before copy on write).
    pub fn page_permissions(&self, address: u64) -> Permissions {
        match &self.backing {
            Backing::File { file_end, .. } if address >= *file_end => Permissions::empty(),
            _ => cpu_permissions(self.permissions),
        }
    }

    /// Whether it maps a secret memory file, which stays locked and which no other task may reach.
    pub fn is_secret(&self) -> bool {
        matches!(&self.backing, Backing::File { node, .. } if node.borrow().is_secret())
    }

    /// Whether `mlock` and `munlock` leave the mapping as it is (`mlock_fixup`): secret memory stays
    /// locked, droppable memory and `VM_SPECIAL` mappings are never locked.
    pub fn keeps_lock(&self) -> bool {
        self.is_secret() || self.is_special() || matches!(self.backing, Backing::Droppable)
    }

    /// Whether Linux marks it `VM_SPECIAL`: a TCP socket's mapping (`tcp_mmap`'s `VM_MIXEDMAP`
    /// and `VM_DONTEXPAND`), never locked and never grown.
    pub fn is_special(&self) -> bool {
        matches!(&self.backing, Backing::File { node, .. }
            if matches!(node.borrow().content, crate::fs::Content::Socket(_)))
    }

    /// The mapping in runs of pages, each with whether the page holds the mapping's own bytes: every
    /// page but those of a private file mapping not yet copied.
    pub fn runs(&self) -> Vec<(u64, u64, bool)> {
        let Backing::File { offset, copied, .. } = &self.backing else {
            return alloc::vec![(self.start, self.end, true)];
        };
        if self.shared {
            return alloc::vec![(self.start, self.end, true)];
        }
        let first = offset / PAGE_SIZE;
        let address = |page: u64| self.start + (page - first) * PAGE_SIZE;
        let mut runs = Vec::new();
        let mut at = self.start;
        let last = first + (self.end - self.start) / PAGE_SIZE;
        for page in copied.range(first..last) {
            let from = address(*page);
            if from > at {
                runs.push((at, from, false));
            }
            match runs.last_mut() {
                Some((_, end, true)) if *end == from => *end = from + PAGE_SIZE,
                _ => runs.push((from, from + PAGE_SIZE, true)),
            }
            at = from + PAGE_SIZE;
        }
        if at < self.end {
            runs.push((at, self.end, false));
        }
        runs
    }

    /// Whether a write to the page at `address` makes the mapping's own copy of it first: a private
    /// file page the mapping may write, before the file's end, not copied yet.
    pub fn copies_on_write(&self, address: u64) -> bool {
        match &self.backing {
            Backing::File {
                offset,
                file_end,
                copied,
                ..
            } if !self.shared => {
                self.permissions.contains(Permissions::WRITE)
                    && address < *file_end
                    && !copied.contains(&(offset / PAGE_SIZE + (address - self.start) / PAGE_SIZE))
            }
            _ => false,
        }
    }

    /// The same mapping cut to `[start, end)`.
    pub fn cut_public(&self, start: u64, end: u64) -> Mapping {
        self.cut(start, end)
    }

    fn cut(&self, start: u64, end: u64) -> Mapping {
        let mut cut = self.clone();
        match &mut cut.backing {
            Backing::File { offset, copied, .. } => {
                *offset += start - self.start;
                let first = *offset / PAGE_SIZE;
                let pages = first..first + (end - start) / PAGE_SIZE;
                copied.retain(|page| pages.contains(page));
            }
            Backing::Zero { offset, .. } => *offset += start - self.start,
            _ => {}
        }
        cut.start = start;
        cut.end = end;
        cut
    }
}

/// Whether a mapping with these properties counts against `RLIMIT_DATA`.
pub(crate) fn is_data(permissions: Permissions, shared: bool, grows_down: bool) -> bool {
    permissions.contains(Permissions::WRITE) && !shared && !grows_down
}

/// Every mapped range of one process, and where the next ones go.
#[derive(Clone)]
pub(crate) struct AddressMap {
    /// By start address; never overlapping.
    mappings: BTreeMap<u64, Mapping>,
    /// Ranges that have been mapped at some time, so may hold old bytes. Start to end.
    used: BTreeMap<u64, u64>,
    /// One past the highest address: the container's address width.
    pub end: u64,
    /// One past the highest address the executor can give now; nothing is placed above it, and a
    /// request that would reach past it has outgrown it.
    pub ceiling: u64,
    /// New mappings with no address go below this, highest first.
    pub mmap_base: u64,
    pub brk_start: u64,
    pub brk: u64,
    /// The program's data as Linux measures it (`end_data - start_data`), which `RLIMIT_DATA` counts
    /// together with the heap.
    pub data_size: u64,
    /// How new mappings are locked, as `mlockall(MCL_FUTURE)` sets (Linux's `def_flags`).
    pub future_lock: Lock,
    /// `membarrier` registrations, as Linux's `MEMBARRIER_STATE_*` bits.
    pub membarrier: u32,
    /// Protection keys allocated, one bit each; key 0 from the start.
    pub protection_keys: u32,
    /// `PR_SET_DUMPABLE`, kept with the memory as Linux keeps it in `mm->flags`; cleared when a
    /// user's ids change.
    pub dumpable: bool,
    /// The embedder's watches on these pages and its record of the pages written.
    pub watches: Watches,
    /// What the map notes for the kernels its processes are on.
    notes: Vec<Rc<MapNotes>>,
}

/// What address maps note for a kernel, one value shared by every map on it.
#[derive(Debug, Default)]
pub(crate) struct MapNotes {
    /// The length a placement asked for that found no room below the ceiling.
    pub outgrown: Cell<Option<u64>>,
    /// A shared mapping was put in or taken out of some map.
    pub sharing_changed: Cell<bool>,
}

/// How much of a page's access a watch brings back: none, writes, everything.
fn watch_rank(watch: WatchesFor) -> u8 {
    match watch {
        WatchesFor::None => 0,
        WatchesFor::Writes => 1,
        WatchesFor::All => 2,
    }
}

/// The embedder's watches on one memory's pages, and the pages written since it last asked.
#[derive(Clone, Default)]
pub(crate) struct Watches {
    /// Watched pages by address; never `WatchesFor::None`.
    pages: BTreeMap<u64, WatchesFor>,
    /// Pages written since the embedder last asked; none until it first asks.
    written: Option<RangeSet<u64>>,
    /// The first watched page a kernel copy for the running call found, and how it touched it.
    pub touched: Option<(u64, AccessKind)>,
    /// The ranges the running call's description named and how, against which its copies are
    /// checked.
    pub described: Option<Vec<(u64, u64, AccessKind)>>,
    /// The first address a copy of the running call reached outside its description.
    pub undescribed: Option<u64>,
}

impl Watches {
    /// Notes a copy of `[start, end)` as `access` that the running call's description did not
    /// name: a write must lie in ranges named as written, which meet every watch.
    pub fn check_described(&mut self, start: u64, end: u64, access: AccessKind) {
        let Some(described) = &self.described else {
            return;
        };
        let mut at = start;
        while at < end {
            match described.iter().find(|(from, to, named)| {
                (*from..*to).contains(&at)
                    && (access != AccessKind::Write || *named == AccessKind::Write)
            }) {
                Some((_, to, _)) => at = *to,
                None => {
                    self.undescribed.get_or_insert(at);
                    return;
                }
            }
        }
    }

    /// Whether any page is watched.
    pub fn any(&self) -> bool {
        !self.pages.is_empty()
    }

    /// Whether anything narrows what the CPU side allows.
    pub fn narrows(&self) -> bool {
        !self.pages.is_empty() || self.written.is_some()
    }

    /// What the CPU side allows of a page that would allow `full`.
    fn narrowed(&self, page: u64, full: Permissions) -> Permissions {
        let mut allowed = full;
        match self.pages.get(&page) {
            Some(WatchesFor::All) => allowed = Permissions::empty(),
            Some(WatchesFor::Writes) => allowed -= Permissions::WRITE,
            Some(WatchesFor::None) | None => {}
        }
        if self
            .written
            .as_ref()
            .is_some_and(|written| !written.contains(&page))
        {
            allowed -= Permissions::WRITE;
        }
        allowed
    }

    /// The watch on the page holding `address`.
    pub fn watch_at(&self, address: u64) -> WatchesFor {
        self.pages
            .get(&page_down(address))
            .copied()
            .unwrap_or(WatchesFor::None)
    }

    /// Whether the record of written pages holds back writing the page of `address`.
    pub fn holds_back_write(&self, address: u64) -> bool {
        self.written
            .as_ref()
            .is_some_and(|written| !written.contains(&page_down(address)))
    }

    /// The first address in `[start, end)` whose page is watched for `access`.
    pub fn first_watched(&self, start: u64, end: u64, access: AccessKind) -> Option<u64> {
        if self.pages.is_empty() || start >= end {
            return None;
        }
        self.pages
            .range(page_down(start)..end)
            .find(|(_, watch)| access == AccessKind::Write || **watch == WatchesFor::All)
            .map(|(page, _)| (*page).max(start))
    }

    /// How much of the `reached` bytes from `start` a kernel copy of `length` bytes takes before a
    /// watched page, which is not there for it until the watch is met; the copy is checked against
    /// the running call's description.
    pub fn copy_reach(&mut self, start: u64, length: u64, reached: u64, access: AccessKind) -> u64 {
        self.check_described(start, start.saturating_add(length), access);
        match self.first_watched(start, start + reached, access) {
            Some(at) => {
                self.touched.get_or_insert((at, access));
                at - start
            }
            None => reached,
        }
    }

    /// Notes that the process wrote `[start, end)`, if the embedder keeps a record.
    pub fn note_written(&mut self, start: u64, end: u64) -> bool {
        let Some(written) = &mut self.written else {
            return false;
        };
        let pages = page_down(start)..page_up(end);
        let fresh = !pages.is_empty() && written.gaps(&pages).next().is_some();
        if fresh {
            written.insert(pages);
        }
        fresh
    }

    /// Takes the watches of `[start, end)`, by their pages' offsets from `start`.
    fn take(&mut self, start: u64, end: u64) -> Vec<(u64, WatchesFor)> {
        let taken: Vec<(u64, WatchesFor)> = self
            .pages
            .range(start..end)
            .map(|(page, watch)| (*page - start, *watch))
            .collect();
        for (offset, _) in &taken {
            self.pages.remove(&(start + offset));
        }
        taken
    }

    /// Forgets the watches and the record of `[start, end)`, which is unmapped.
    fn forget(&mut self, start: u64, end: u64) {
        if !self.narrows() {
            return;
        }
        let gone: Vec<u64> = self
            .pages
            .range(start..end)
            .map(|(page, _)| *page)
            .collect();
        for page in gone {
            self.pages.remove(&page);
        }
        if let Some(written) = &mut self.written
            && start < end
        {
            written.remove(start..end);
        }
    }
}

impl AddressMap {
    /// A map of `end`'s width whose stack's top is at the ceiling.
    pub fn new(end: u64, ceiling: u64, notes: Rc<MapNotes>) -> Self {
        Self {
            mappings: BTreeMap::new(),
            used: BTreeMap::new(),
            end,
            ceiling,
            mmap_base: page_down(ceiling - mmap_gap(ceiling)),
            brk_start: 0,
            brk: 0,
            data_size: 0,
            future_lock: Lock::None,
            membarrier: 0,
            protection_keys: 1,
            dumpable: true,
            watches: Watches::default(),
            notes: alloc::vec![notes],
        }
    }

    fn note(&self, change: impl Fn(&MapNotes)) {
        for notes in &self.notes {
            change(notes);
        }
    }

    /// Ok if all of a range is mapped and none of it is a backend file's page not read yet.
    pub fn readable_whole(&self, range: core::ops::Range<u64>) -> Result<(), MemoryError> {
        if range.start >= range.end {
            return Ok(());
        }
        if !self.fully_mapped(range.start, range.end) {
            return Err(MemoryError::NotMapped);
        }
        let mut at = range.start;
        while at < range.end {
            let mapping = self.find(at).ok_or(MemoryError::NotMapped)?;
            let to = mapping.end.min(range.end);
            if let Backing::File { node, offset, .. } = &mapping.backing {
                let page = |address: u64| (offset + (address - mapping.start)) / PAGE_SIZE;
                if !crate::shared::unfilled(node, page(page_down(at))..page(page_up(to))).is_empty()
                {
                    return Err(MemoryError::NotRead);
                }
            }
            at = to;
        }
        Ok(())
    }

    /// Notes what changes for a kernel it is put on, too.
    pub fn note_to(&mut self, notes: Rc<MapNotes>) {
        if !self.notes.iter().any(|known| Rc::ptr_eq(known, &notes)) {
            self.notes.push(notes);
        }
    }

    /// Stops noting for a kernel no process of which runs in it any more.
    pub fn stop_noting_to(&mut self, notes: &Rc<MapNotes>) {
        self.notes.retain(|known| !Rc::ptr_eq(known, notes));
    }

    /// Notes for no kernel, as a copy not yet put on one.
    pub fn notes_to_none(&mut self) {
        self.notes.clear();
    }

    /// Sets the embedder's watch on the mapped pages of `[start, end)` (page aligned) and tells
    /// the CPU side; whether any page's watch changed, and how many pages' were raised (set, or
    /// widened to `All`).
    pub fn set_watch(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        range: core::ops::Range<u64>,
        watch: WatchesFor,
    ) -> (bool, u64) {
        // Only watched pages are visited to end watches, and only mapped ones to set them.
        let mut changed = false;
        let mut raised = 0;
        if range.is_empty() {
            return (changed, raised);
        }
        if watch == WatchesFor::None {
            let ended: Vec<u64> = self
                .watches
                .pages
                .range(range.clone())
                .map(|(page, _)| *page)
                .collect();
            for page in &ended {
                self.watches.pages.remove(page);
            }
            changed = !ended.is_empty();
        } else {
            let spans: Vec<(u64, u64)> = self
                .mappings
                .range(..range.end)
                .map(|(_, mapping)| mapping)
                .filter(|mapping| mapping.end > range.start)
                .map(|mapping| (mapping.start.max(range.start), mapping.end.min(range.end)))
                .collect();
            for (from, to) in spans {
                for page in (from..to).step_by(PAGE_SIZE as usize) {
                    let before = self.watches.pages.insert(page, watch);
                    changed |= before != Some(watch);
                    if watch_rank(watch) > before.map_or(0, watch_rank) {
                        raised += 1;
                    }
                }
            }
        }
        if changed {
            self.reapply(cpu, memory, range.start, range.end);
        }
        (changed, raised)
    }

    /// The pages written since the embedder last asked, as ranges, and a fresh record from now
    /// (the first time: every page mapped writable).
    pub fn take_dirty(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
    ) -> Vec<core::ops::Range<u64>> {
        let written = match self.watches.written.replace(RangeSet::new()) {
            Some(written) => written,
            None => self
                .mappings
                .values()
                .filter(|mapping| mapping.permissions.contains(Permissions::WRITE))
                .map(|mapping| mapping.start..mapping.end)
                .collect(),
        };
        let ranges = written.into_iter().collect();
        let starts: Vec<(u64, u64)> = self.mappings.values().map(|m| (m.start, m.end)).collect();
        for (start, end) in starts {
            self.reapply(cpu, memory, start, end);
        }
        ranges
    }

    /// A fork's child's copy: no record of written pages, and nothing of its parent's running
    /// call.
    pub fn start_child(&mut self, cpu: &mut dyn Cpu, memory: MemoryId) {
        self.note_sharers_changed();
        self.watches.touched = None;
        self.watches.described = None;
        self.watches.undescribed = None;
        if self.watches.written.take().is_some() {
            let starts: Vec<(u64, u64)> =
                self.mappings.values().map(|m| (m.start, m.end)).collect();
            for (start, end) in starts {
                self.reapply(cpu, memory, start, end);
            }
        }
    }

    /// Tells the CPU side again what every mapping's pages over `[start, end)` allow.
    pub fn reapply(&mut self, cpu: &mut dyn Cpu, memory: MemoryId, start: u64, end: u64) {
        let (start, end) = (page_down(start), page_up(end));
        let overlapping: Vec<Mapping> = self
            .mappings
            .range(..end)
            .map(|(_, mapping)| mapping)
            .filter(|mapping| mapping.end > start)
            .cloned()
            .collect();
        for mapping in overlapping {
            // The ranges were mapped, so the CPU side takes them.
            let _ = apply_permissions_in(cpu, memory, &mapping, start, end, &self.watches);
        }
    }

    /// Notes that the process's bytes in `[start, end)` changed, and lets the CPU side write the
    /// pages it now may.
    pub fn note_written(&mut self, cpu: &mut dyn Cpu, memory: MemoryId, start: u64, end: u64) {
        if self.watches.note_written(start, end) {
            self.reapply(cpu, memory, start, end);
        }
    }

    /// Takes the watches of `[start, end)` along to pages a move is about to take there.
    pub fn take_watches(&mut self, start: u64, end: u64) -> Vec<(u64, WatchesFor)> {
        if self.watches.pages.is_empty() {
            return Vec::new();
        }
        self.watches.take(start, end)
    }

    /// Pages moved to `[start, end)`: the watches taken along with them are set here, and the
    /// pages are noted as written.
    pub fn moved_here(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
        watches: Vec<(u64, WatchesFor)>,
    ) {
        for (offset, watch) in watches {
            self.watches.pages.insert(start + offset, watch);
        }
        if self.watches.narrows() {
            self.watches.note_written(start, end);
            self.reapply(cpu, memory, start, end);
        }
    }

    /// Ends the watches of `[start, end)`, whose pages a fork's child gets fresh.
    pub fn forget_watches(&mut self, cpu: &mut dyn Cpu, memory: MemoryId, start: u64, end: u64) {
        if self.watches.pages.range(start..end).next().is_some() {
            self.watches.forget(start, end);
            self.reapply(cpu, memory, start, end);
        }
    }

    /// A write fault on a page the record of written pages held back: noted, and the page may be
    /// written from now on. Whether that was the fault's reason.
    pub fn written_fault(&mut self, cpu: &mut dyn Cpu, memory: MemoryId, address: u64) -> bool {
        let page = page_down(address);
        let writable = self
            .find(address)
            .is_some_and(|mapping| mapping.permissions.contains(Permissions::WRITE));
        if !writable || !self.watches.note_written(page, page + PAGE_SIZE) {
            return false;
        }
        self.reapply(cpu, memory, page, page + PAGE_SIZE);
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mapping> {
        self.mappings.values()
    }

    /// Whether `pages` more pages, data pages if `data`, fit the limits on the address space and on
    /// data, as Linux's `may_expand_vm`. `freed` pages of the same kind are given up at the same time.
    pub fn may_expand(
        &self,
        limits: &[Limit; LIMITS],
        data: bool,
        pages: u64,
        freed: (u64, u64),
    ) -> bool {
        let (total, data_pages) = self.iter().fold((0, 0), |(total, data), mapping| {
            let pages = (mapping.end - mapping.start) / PAGE_SIZE;
            (
                total + pages,
                data + if mapping.is_data() { pages } else { 0 },
            )
        });
        let total = total - freed.0;
        let data_pages = data_pages - freed.1;
        if total + pages > limits[RLIMIT_AS as usize].current / PAGE_SIZE {
            return false;
        }
        let data_limit = limits[RLIMIT_DATA as usize];
        if data && data_pages + pages > data_limit.current / PAGE_SIZE {
            // Linux lets a zero limit through up to the hard limit, for Valgrind.
            return data_limit.current == 0 && data_pages + pages <= data_limit.maximum / PAGE_SIZE;
        }
        true
    }

    /// The pages of the range that are mapped, and how many of them are data.
    pub fn pages_in(&self, start: u64, end: u64) -> (u64, u64) {
        let mut total = 0;
        let mut data = 0;
        for mapping in self.mappings.values() {
            let from = mapping.start.max(start);
            let to = mapping.end.min(end);
            if from < to {
                total += (to - from) / PAGE_SIZE;
                if mapping.is_data() {
                    data += (to - from) / PAGE_SIZE;
                }
            }
        }
        (total, data)
    }

    /// The mapping holding `address`.
    pub fn find(&self, address: u64) -> Option<&Mapping> {
        let (_, mapping) = self.mappings.range(..=address).next_back()?;
        (address < mapping.end).then_some(mapping)
    }

    /// The first mapping that ends above `address`.
    pub fn next_after(&self, address: u64) -> Option<&Mapping> {
        if let Some(mapping) = self.find(address) {
            return Some(mapping);
        }
        self.mappings
            .range(address..)
            .next()
            .map(|(_, mapping)| mapping)
    }

    /// How many bytes from `address` on, up to `length`, the program may access as `wanted`.
    pub fn accessible(&self, address: u64, length: u64, wanted: Permissions) -> u64 {
        let end = address.saturating_add(length);
        let mut at = address;
        while at < end {
            let Some(mapping) = self.find(at) else { break };
            if !mapping.page_permissions(at).contains(wanted) {
                break;
            }
            let reach = match &mapping.backing {
                Backing::File { file_end, .. } if at < *file_end => (*file_end).min(mapping.end),
                _ => mapping.end,
            };
            at = reach.min(end);
        }
        at - address
    }

    /// How many bytes from `address` on, up to `length`, another task's access may reach, as Linux's
    /// `get_user_pages` without `FOLL_FORCE`: by the mapping's own read or write right (not what the
    /// CPU allows), never wholly past a file's end, and never growing a stack.
    pub fn pinnable(&self, address: u64, length: u64, write: bool) -> u64 {
        let wanted = if write {
            Permissions::WRITE
        } else {
            Permissions::READ
        };
        let end = address.saturating_add(length);
        let mut at = address;
        while at < end {
            let Some(mapping) = self.find(at) else { break };
            if !mapping.permissions.contains(wanted) || mapping.is_secret() {
                break;
            }
            let reach = match &mapping.backing {
                Backing::File { file_end, .. } if at >= *file_end => break,
                Backing::File { file_end, .. } => (*file_end).min(mapping.end),
                _ => mapping.end,
            };
            at = reach.min(end);
        }
        at - address
    }

    /// Whether every page of the range is mapped.
    pub fn fully_mapped(&self, start: u64, end: u64) -> bool {
        let mut at = start;
        while at < end {
            match self.find(at) {
                Some(mapping) => at = mapping.end,
                None => return false,
            }
        }
        true
    }

    /// Whether any page of the range is mapped.
    pub fn any_mapped(&self, start: u64, end: u64) -> bool {
        self.next_after(start)
            .is_some_and(|mapping| mapping.start < end)
    }

    /// Whether a new mapping may be placed at the range without being asked to: nothing mapped there,
    /// nor in the guard gap below a mapping that grows down.
    pub fn free_for_placement(&self, start: u64, end: u64) -> bool {
        self.next_after(start)
            .is_none_or(|mapping| mapping.start_gap() >= end)
    }

    /// Bytes new mappings with no address can still take: the gaps between the heap's end and the
    /// mmap base.
    pub fn placeable(&self) -> u64 {
        let floor = self.brk.max(LOWEST_ADDRESS);
        let mut free = 0;
        let mut at = floor;
        for mapping in self
            .mappings
            .range(..self.mmap_base)
            .map(|(_, mapping)| mapping)
        {
            if mapping.end <= at {
                continue;
            }
            free += mapping.start_gap().min(self.mmap_base).saturating_sub(at);
            at = mapping.end;
        }
        free + self.mmap_base.saturating_sub(at)
    }

    /// Whether a request for `length` bytes ending at `end` reaches past the ceiling; if so it is
    /// noted as outgrown.
    pub fn crosses_ceiling(&self, end: u64, length: u64) -> bool {
        if end <= self.ceiling {
            return false;
        }
        self.note(|notes| notes.outgrown.set(Some(length)));
        true
    }

    /// Free space for `length` bytes, the highest below the mmap base, as Linux's top-down search.
    pub fn find_free(&self, length: u64) -> Option<u64> {
        self.find_free_beside(length, self.brk, &[])
    }

    /// Free space as [`AddressMap::find_free`] finds it with the heap ending at `brk` and `taken`
    /// in use too: where the loader will place a new program's parts.
    pub fn find_free_beside(
        &self,
        length: u64,
        brk: u64,
        taken: &[core::ops::Range<u64>],
    ) -> Option<u64> {
        let mut ceiling = self.mmap_base;
        let floor = brk.max(LOWEST_ADDRESS);
        loop {
            let start = ceiling.checked_sub(length).filter(|start| *start >= floor);
            let Some(start) = start else {
                if self.ceiling < self.end {
                    self.note(|notes| notes.outgrown.set(Some(length)));
                }
                return None;
            };
            // The highest mapping that, with the guard gap below one that grows down, overlaps
            // [start, ceiling).
            let mapped = self
                .mappings
                .range(..ceiling.saturating_add(STACK_GUARD_GAP))
                .rev()
                .map(|(_, mapping)| mapping)
                .find(|mapping| mapping.start_gap() < ceiling)
                .filter(|mapping| mapping.end > start)
                .map(Mapping::start_gap);
            let reserved = taken
                .iter()
                .filter(|range| range.start < ceiling && range.end > start)
                .map(|range| range.start)
                .max();
            match mapped.max(reserved) {
                Some(below) => ceiling = below,
                None => return Some(start),
            }
        }
    }

    /// Takes a range out of the table, splitting mappings at its ends, and returns what was there;
    /// its pages' watches and record go with them.
    pub fn remove(&mut self, start: u64, end: u64) -> Vec<Mapping> {
        self.watches.forget(start, end);
        let removed = self.split_out(start, end);
        if removed.iter().any(|mapping| mapping.shared) {
            self.note(|notes| notes.sharing_changed.set(true));
        }
        removed
    }

    /// Whether any mapping shows shared memory.
    pub fn has_shared(&self) -> bool {
        self.mappings.values().any(|mapping| mapping.shared)
    }

    /// Notes that the processes using this map changed, if it shows shared memory.
    pub fn note_sharers_changed(&self) {
        if self.has_shared() {
            self.note(|notes| notes.sharing_changed.set(true));
        }
    }

    /// Takes the mappings of a range out of the table to be put back changed, splitting them at
    /// its ends; the pages, their watches and their record stay.
    fn split_out(&mut self, start: u64, end: u64) -> Vec<Mapping> {
        let overlapping: Vec<u64> = self
            .mappings
            .range(..end)
            .filter(|(_, mapping)| mapping.end > start)
            .map(|(key, _)| *key)
            .collect();
        let mut removed = Vec::new();
        for key in overlapping {
            let mapping = self.mappings.remove(&key).expect("mapping");
            if mapping.start < start {
                let before = mapping.cut(mapping.start, start);
                self.mappings.insert(before.start, before);
            }
            if mapping.end > end {
                let after = mapping.cut(end, mapping.end);
                self.mappings.insert(after.start, after);
            }
            removed.push(mapping.cut(mapping.start.max(start), mapping.end.min(end)));
        }
        removed
    }

    /// Puts a mapping in the table, replacing whatever it covers, and tells the CPU side. New anonymous
    /// memory is zero: a range that has been used before is cleared first.
    pub fn insert(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        mapping: Mapping,
    ) -> Result<(), Errno> {
        let removed = self.remove(mapping.start, mapping.end);
        // New pages are new bytes, as Linux marks a new mapping soft-dirty.
        self.watches.note_written(mapping.start, mapping.end);
        let had_code = removed
            .iter()
            .any(|old| old.permissions.contains(Permissions::EXECUTE));
        self.clear_used(cpu, memory, mapping.start, mapping.end)?;
        self.apply(cpu, memory, &mapping)?;
        if had_code && !mapping.permissions.contains(Permissions::EXECUTE) {
            cpu.code_changed(memory, mapping.start, mapping.end - mapping.start, &[]);
        }
        if mapping.shared {
            self.note(|notes| notes.sharing_changed.set(true));
        }
        self.mappings.insert(mapping.start, mapping);
        self.merge_around();
        Ok(())
    }

    /// Unmaps a range, as `munmap` does.
    pub fn unmap(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        let removed = self.remove(start, end);
        for old in &removed {
            cpu.set_permissions(memory, old.start, old.end - old.start, Permissions::empty())
                .map_err(errno::from_cpu)?;
            if matches!(old.backing, Backing::File { .. }) {
                cpu.shared_unmap(memory, old.start, old.end - old.start)
                    .map_err(errno::from_cpu)?;
            }
            if old.permissions.contains(Permissions::EXECUTE) {
                cpu.code_changed(memory, old.start, old.end - old.start, &[]);
            }
        }
        Ok(())
    }

    /// Changes what a fully mapped range allows, as `mprotect` does.
    pub fn protect(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
        permissions: Permissions,
    ) -> Result<(), Errno> {
        let changed = self.split_out(start, end);
        for mut mapping in changed {
            let had_code = mapping.permissions.contains(Permissions::EXECUTE);
            mapping.permissions = permissions;
            self.apply(cpu, memory, &mapping)?;
            if had_code && !permissions.contains(Permissions::EXECUTE) {
                cpu.code_changed(memory, mapping.start, mapping.end - mapping.start, &[]);
            }
            self.mappings.insert(mapping.start, mapping);
        }
        self.merge_around();
        Ok(())
    }

    /// Moves where the mappings of a file stop being reachable to the file's new end, `file_pages`
    /// bytes from its start. Private copies wholly past it go, as Linux's truncation unmaps even
    /// copied pages (`unmap_mapping_range` with `even_cows`), so they show the file's block again.
    pub fn set_file_end(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        node: &Node,
        file_pages: u64,
    ) -> Result<(), Errno> {
        let affected: Vec<u64> = self
            .mappings
            .values()
            .filter(|mapping| {
                matches!(&mapping.backing, Backing::File { node: mapped, .. } if alloc::rc::Rc::ptr_eq(mapped, node))
            })
            .map(|mapping| mapping.start)
            .collect();
        for start in affected {
            let mut mapping = self.mappings.remove(&start).expect("mapping");
            let new_end = match &mut mapping.backing {
                Backing::File {
                    offset, file_end, ..
                } => {
                    *file_end = mapping.start
                        + file_pages
                            .saturating_sub(*offset)
                            .min(mapping.end - mapping.start);
                    *file_end
                }
                _ => unreachable!("a file mapping"),
            };
            let end = mapping.end;
            drop_copies(cpu, memory, &mut mapping, new_end, end)?;
            self.apply_permissions(cpu, memory, &mapping)?;
            self.mappings.insert(start, mapping);
        }
        Ok(())
    }

    /// Makes the pages of private file mappings in `[start, end)` that still show the file, up to
    /// each file's end, the mappings' own copies, and lets them be written as the mappings allow: a
    /// write fault's copy on write, done ahead for the kernel's own writes and for populating.
    pub fn copy_on_write(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        let (start, end) = (page_down(start), page_up(end));
        let affected: Vec<u64> = self
            .mappings
            .range(..end)
            .filter(|(_, mapping)| {
                mapping.end > start
                    && !mapping.shared
                    && matches!(mapping.backing, Backing::File { .. })
            })
            .map(|(key, _)| *key)
            .collect();
        for key in affected {
            let mut mapping = self.mappings.remove(&key).expect("mapping");
            let from = start.max(mapping.start);
            let mut copies = Vec::new();
            if let Backing::File {
                node,
                offset,
                file_end,
                copied,
                ..
            } = &mut mapping.backing
            {
                let to = end.min(*file_end);
                let first = *offset / PAGE_SIZE;
                let page = |at: u64| first + (at - mapping.start) / PAGE_SIZE;
                // A page not read from the backend yet is copied when it has come.
                let unread = crate::shared::unfilled(node, page(from)..page(to.max(from)));
                let mut at = from;
                while at < to {
                    let number = page(at);
                    if unread.iter().any(|run| run.contains(&number)) {
                        at += PAGE_SIZE;
                        continue;
                    }
                    if copied.insert(number) {
                        match copies.last_mut() {
                            Some((_, run_end)) if *run_end == at => *run_end = at + PAGE_SIZE,
                            _ => copies.push((at, at + PAGE_SIZE)),
                        }
                    }
                    at += PAGE_SIZE;
                }
            }
            for (from, to) in &copies {
                cpu.shared_unmap(memory, *from, to - from)
                    .map_err(errno::from_cpu)?;
            }
            if !copies.is_empty() {
                self.apply_permissions(cpu, memory, &mapping)?;
            }
            self.mappings.insert(key, mapping);
        }
        Ok(())
    }

    /// Drops the private file pages copied in `[start, end)`, so they show the file again, as
    /// `MADV_DONTNEED` zaps a private mapping's copies.
    pub fn drop_copies(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        let affected: Vec<u64> = self
            .mappings
            .range(..end)
            .filter(|(_, mapping)| mapping.end > start)
            .map(|(key, _)| *key)
            .collect();
        for key in affected {
            let mut mapping = self.mappings.remove(&key).expect("mapping");
            drop_copies(cpu, memory, &mut mapping, start, end)?;
            self.apply_permissions(cpu, memory, &mapping)?;
            self.mappings.insert(key, mapping);
        }
        Ok(())
    }

    /// Locks or unlocks a range, every page of which is mapped.
    pub fn set_lock(&mut self, start: u64, end: u64, lock: Lock) {
        for mut mapping in self.split_out(start, end) {
            if !mapping.keeps_lock() {
                mapping.locked = lock;
            }
            self.mappings.insert(mapping.start, mapping);
        }
        self.merge_around();
    }

    /// Sets what a fork does with a range, every page of which is mapped.
    pub fn set_on_fork(&mut self, start: u64, end: u64, change: impl Fn(&mut OnFork)) {
        for mut mapping in self.split_out(start, end) {
            change(&mut mapping.on_fork);
            self.mappings.insert(mapping.start, mapping);
        }
        self.merge_around();
    }

    /// Locks or unlocks every mapping.
    pub fn set_lock_all(&mut self, lock: Lock) {
        for mapping in self.mappings.values_mut() {
            if !mapping.keeps_lock() {
                mapping.locked = lock;
            }
        }
        self.merge_around();
    }

    /// The pages locked in memory (Linux's `locked_vm`), and those of them within a range.
    pub fn locked_pages(&self, start: u64, end: u64) -> (u64, u64) {
        self.mappings
            .values()
            .filter(|mapping| mapping.locked != Lock::None)
            .fold((0, 0), |(all, within), mapping| {
                let from = mapping.start.max(start);
                let to = mapping.end.min(end);
                let inside = if from < to {
                    (to - from) / PAGE_SIZE
                } else {
                    0
                };
                (
                    all + (mapping.end - mapping.start) / PAGE_SIZE,
                    within + inside,
                )
            })
    }

    /// Whether `pages` more locked pages fit `RLIMIT_MEMLOCK` (Linux's `mlock_future_ok`).
    pub fn may_lock(
        &self,
        limits: &[Limit; LIMITS],
        lock: Lock,
        pages: u64,
        powerful: bool,
    ) -> bool {
        lock == Lock::None
            || powerful
            || self.locked_pages(0, 0).0 + pages
                <= limits[RLIMIT_MEMLOCK as usize].current / PAGE_SIZE
    }

    /// Tells the CPU side what a mapping's pages allow, and that its code may run.
    fn apply(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        mapping: &Mapping,
    ) -> Result<(), Errno> {
        self.apply_permissions(cpu, memory, mapping)?;
        if mapping.permissions.contains(Permissions::EXECUTE) {
            cpu.code_changed(memory, mapping.start, mapping.end - mapping.start, &[]);
        }
        Ok(())
    }

    /// Tells the CPU side again what the pages of `[start, end)` in the mapping holding `start`
    /// allow, once pages of its file came that it showed with no access.
    pub fn refresh(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        match self.find(start).cloned() {
            Some(mapping) => {
                let (lo, hi) = (page_down(start), page_up(end));
                apply_permissions_in(cpu, memory, &mapping, lo, hi, &self.watches)
            }
            None => Ok(()),
        }
    }

    /// Tells the CPU side what a mapping's pages allow.
    fn apply_permissions(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        mapping: &Mapping,
    ) -> Result<(), Errno> {
        apply_permissions_in(
            cpu,
            memory,
            mapping,
            mapping.start,
            mapping.end,
            &self.watches,
        )
    }

    /// Zeroes the parts of a range that held something before, and marks it all as used.
    fn clear_used(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        let overlapping: Vec<(u64, u64)> = self
            .used
            .range(..end)
            .filter(|(_, used_end)| **used_end > start)
            .map(|(used_start, used_end)| (*used_start, *used_end))
            .collect();
        let mut low = start;
        let mut high = end;
        for (used_start, used_end) in overlapping {
            let from = used_start.max(start);
            let to = used_end.min(end);
            cpu.zero(memory, from, to - from).map_err(errno::from_cpu)?;
            self.used.remove(&used_start);
            low = low.min(used_start);
            high = high.max(used_end);
        }
        self.used.insert(low, high);
        Ok(())
    }

    /// Joins neighbouring mappings that are the same, as Linux merges areas: anonymous ones, and shared
    /// ones of the same file at following offsets except System V segments' (private file pages may have their own copies, which
    /// Linux merges only in cases not modelled here).
    fn merge_around(&mut self) {
        let mut merged: BTreeMap<u64, Mapping> = BTreeMap::new();
        for (_, mapping) in core::mem::take(&mut self.mappings) {
            if let Some((_, last)) = merged.iter_mut().next_back()
                && last.end == mapping.start
                && last.permissions == mapping.permissions
                && last.shared == mapping.shared
                && last.allowed == mapping.allowed
                && last.locked == mapping.locked
                && last.grows_down == mapping.grows_down
                && last.on_fork == mapping.on_fork
            {
                let file_end = match (&last.backing, &mapping.backing) {
                    (Backing::Anonymous, Backing::Anonymous)
                    | (Backing::Droppable, Backing::Droppable)
                    | (Backing::Heap, Backing::Heap)
                    | (Backing::Stack, Backing::Stack) => Some(None),
                    (
                        Backing::File {
                            node,
                            offset,
                            file_end,
                            ..
                        },
                        Backing::File {
                            node: next_node,
                            offset: next_offset,
                            file_end: next_file_end,
                            ..
                        },
                    ) if mapping.shared
                        && alloc::rc::Rc::ptr_eq(node, next_node)
                        && !node.borrow().is_segment()
                        && offset + (last.end - last.start) == *next_offset
                        && (*file_end == last.end || *next_file_end == mapping.start) =>
                    {
                        Some(Some((*file_end).max(*next_file_end)))
                    }
                    (
                        Backing::Zero {
                            device,
                            ino,
                            offset,
                            ..
                        },
                        Backing::Zero {
                            device: next_device,
                            ino: next_ino,
                            offset: next_offset,
                            ..
                        },
                    ) if (device, ino) == (next_device, next_ino)
                        && offset + (last.end - last.start) == *next_offset =>
                    {
                        Some(None)
                    }
                    _ => None,
                };
                if let Some(file_end) = file_end {
                    last.end = mapping.end;
                    if let (Some(new_end), Backing::File { file_end, .. }) =
                        (file_end, &mut last.backing)
                    {
                        *file_end = new_end;
                    }
                    continue;
                }
            }
            merged.insert(mapping.start, mapping);
        }
        self.mappings = merged;
    }

    /// Grows the mapping above `address` down to cover it if that mapping grows down, as Linux does on
    /// a fault below it (`expand_downwards`). Returns whether it grew.
    pub fn grow_stack(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        address: u64,
        limits: &[Limit; LIMITS],
        may_lock_any: bool,
    ) -> Result<bool, Errno> {
        let Some(stack) = self
            .mappings
            .range(address..)
            .next()
            .map(|(_, mapping)| mapping.clone())
            .filter(|mapping| mapping.grows_down)
        else {
            return Ok(false);
        };
        let new_start = page_down(address);
        if new_start < LOWEST_ADDRESS {
            return Ok(false);
        }
        // The guard gap is kept free of the mapping below, unless that one grows down too or cannot be
        // reached at all.
        if let Some((_, below)) = self.mappings.range(..stack.start).next_back()
            && !below.grows_down
            && !below.permissions.is_empty()
            && new_start - below.end < STACK_GUARD_GAP
        {
            return Ok(false);
        }
        // Linux's `acct_stack_growth`: the growth within the address space's limit, the mapping's
        // size within the stack's, a locked mapping's growth within the locked memory limit.
        let pages = (stack.start - new_start) / PAGE_SIZE;
        if !self.may_expand(limits, false, pages, (0, 0))
            || stack.end - new_start > limits[RLIMIT_STACK as usize].current
            || !self.may_lock(limits, stack.locked, pages, may_lock_any)
        {
            return Ok(false);
        }
        let grown = Mapping {
            start: new_start,
            end: stack.start,
            ..stack
        };
        self.insert(cpu, memory, grown)?;
        Ok(true)
    }
}

/// Drops a private file mapping's copies of the pages in `[start, end)`: they show the file's block
/// again.
fn drop_copies(
    cpu: &mut dyn Cpu,
    memory: MemoryId,
    mapping: &mut Mapping,
    start: u64,
    end: u64,
) -> Result<(), Errno> {
    if mapping.shared {
        return Ok(());
    }
    let mapping_start = mapping.start;
    let (from, to) = (start.max(mapping.start), end.min(mapping.end));
    let Backing::File {
        node,
        offset,
        copied,
        ..
    } = &mut mapping.backing
    else {
        return Ok(());
    };
    if from >= to {
        return Ok(());
    }
    let first = *offset / PAGE_SIZE;
    let pages =
        first + (from - mapping_start) / PAGE_SIZE..first + (to - mapping_start) / PAGE_SIZE;
    let dropped: Vec<u64> = copied.range(pages).copied().collect();
    if dropped.is_empty() {
        return Ok(());
    }
    let block = node
        .borrow()
        .content
        .block()
        .expect("a mapped file has a block");
    for page in dropped {
        copied.remove(&page);
        cpu.shared_map(
            block.id,
            page * PAGE_SIZE,
            memory,
            mapping_start + (page - first) * PAGE_SIZE,
            PAGE_SIZE,
        )
        .map_err(errno::from_cpu)?;
    }
    Ok(())
}

/// What the CPU side enforces for what a program asked: on ARM64 writing implies reading, while a page
/// that may only run cannot be read (execute-only, as on the reference's CPU, measured: a load faults
/// and the kernel's copies give EFAULT).
pub(crate) fn cpu_permissions(asked: Permissions) -> Permissions {
    if asked.contains(Permissions::WRITE) {
        asked | Permissions::READ
    } else {
        asked
    }
}

/// Tells the CPU side what a mapping's pages in `[lo, hi)` allow: nothing wholly past a file's
/// end, nothing where a backend file's page is not read yet, so the first touch comes back to read
/// it, and no writing where a private file page is not copied yet, so the first write comes back
/// to copy it; the embedder's watches narrow each page further.
fn apply_permissions_in(
    cpu: &mut dyn Cpu,
    memory: MemoryId,
    mapping: &Mapping,
    lo: u64,
    hi: u64,
    watches: &Watches,
) -> Result<(), Errno> {
    let (lo, hi) = (lo.max(mapping.start), hi.min(mapping.end));
    let narrows = watches.narrows();
    let mut set = |from: u64, to: u64, permissions: Permissions| {
        let (from, to) = (from.max(lo), to.min(hi));
        if from >= to {
            return Ok::<(), Errno>(());
        }
        if !narrows {
            return cpu
                .set_permissions(memory, from, to - from, permissions)
                .map_err(errno::from_cpu);
        }
        // Runs of pages the watches leave alike.
        let mut start = from;
        while start < to {
            let allowed = watches.narrowed(page_down(start), permissions);
            let mut end = page_down(start) + PAGE_SIZE;
            while end < to && watches.narrowed(end, permissions) == allowed {
                end += PAGE_SIZE;
            }
            let end = end.min(to);
            cpu.set_permissions(memory, start, end - start, allowed)
                .map_err(errno::from_cpu)?;
            start = end;
        }
        Ok(())
    };
    let split = match &mapping.backing {
        Backing::File { file_end, .. } => (*file_end).clamp(mapping.start, mapping.end),
        _ => mapping.end,
    };
    let full = cpu_permissions(mapping.permissions);
    for (from, to, own) in mapping.runs() {
        let to = to.min(split);
        if from >= to {
            break;
        }
        set(from, to, if own { full } else { full - Permissions::WRITE })?;
    }
    if mapping.end > split {
        set(split, mapping.end, Permissions::empty())?;
    }
    if let Backing::File { node, offset, .. } = &mapping.backing {
        let first = offset / PAGE_SIZE;
        let address = |page: u64| mapping.start + (page - first) * PAGE_SIZE;
        let page = |address: u64| first + (address - mapping.start) / PAGE_SIZE;
        if lo < split.min(hi) {
            for run in crate::shared::unfilled(node, page(lo)..page(split.min(hi))) {
                set(address(run.start), address(run.end), Permissions::empty())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mmap_gap_is_linuxs_for_an_address_space_of_any_size() {
        let large = 1 << 32;
        assert_eq!(mmap_gap(large), MMAP_GAP_MIN);
        // Five sixths of a top below the least gap is under it: the stack's own gap is kept.
        let small = MMAP_GAP_MIN;
        assert_eq!(mmap_gap(small), STACK_LIMIT + STACK_GUARD_GAP);
        let tiny = SIXTHS * PAGE_SIZE;
        assert_eq!(mmap_gap(tiny), MMAP_GAP_MAX_SIXTHS * PAGE_SIZE);
        for top in [large, small, tiny, 0] {
            assert!(mmap_gap(top) <= top);
        }
    }
}
