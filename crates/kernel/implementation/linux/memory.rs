//! A process's address space as the kernel keeps it: the ranges it has mapped and what each allows.

use crate::guest_machine::GuestMachine;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;
use crossbeam_utils::atomic::AtomicCell;
use wcontainers_executor::Executor;

use rangemap::RangeSet;
use wcontainers_abi::{LOWEST_ADDRESS, MemoryId, PAGE_SIZE, Permissions};

use crate::fs::Node;
use crate::linux::general::{RLIMIT_AS, RLIMIT_DATA, RLIMIT_MEMLOCK, RLIMIT_STACK};
use crate::process::{LIMITS, Limit, Tid};
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

/// The least room Linux leaves between the top of the address space and the mapping base
/// (`MIN_GAP`).
const MIN_GAP: u64 = 128 << 20;

/// `kernel.randomize_va_space`: the stack, the mappings, the vDSO and the heap placed at random.
pub(crate) const RANDOMIZE_VA_SPACE: u32 = 2;

/// ARM64 Linux's smallest address space for 64-bit tasks (39 bits with 4 KiB pages), which its
/// 64-bit randomization values are made for; a smaller space is placed at random as Linux places
/// 32-bit tasks.
const SMALLEST_64_BIT_SPACE: u64 = 1 << 39;

/// Linux's `MAX_GAP`, the most room left above the mapping base, as a share of the space.
const MAX_GAP_SHARE: (u64, u64) = (5, 6);

/// Linux's `TASK_UNMAPPED_BASE`, where its bottom-up search starts, as the space divided by it.
const UNMAPPED_BASE_DIVISOR: u64 = 4;

/// `CONFIG_ARCH_MMAP_RND_BITS` and `CONFIG_ARCH_MMAP_RND_COMPAT_BITS`, at ARM64's defaults for
/// 4 KiB pages.
const MMAP_RANDOM_BITS: u32 = 18;
const MMAP_RANDOM_BITS_32_BIT: u32 = 11;

/// ARM64's `STACK_RND_MASK`, in pages, for 64-bit and 32-bit tasks.
const STACK_RANDOM_PAGES: u64 = 0x3ffff;
const STACK_RANDOM_PAGES_32_BIT: u64 = 0x7ff;

/// ARM64's `arch_randomize_brk` ranges above a program's end, for 64-bit and 32-bit tasks.
const BRK_RANDOM_RANGE: u64 = 1 << 30;
const BRK_RANDOM_RANGE_32_BIT: u64 = 32 << 20;

/// How far `execve` moves a program's parts at random (Linux's `PF_RANDOMIZE` layout). An address
/// space smaller than any ARM64 Linux gives 64-bit tasks takes Linux's values for 32-bit tasks,
/// so that randomness spends a small share of it.
#[derive(Clone, Copy)]
pub(crate) struct LayoutRandomization {
    /// `vm.mmap_rnd_bits`: pages of randomness below the mapping base and above the base of a
    /// program that can be placed anywhere.
    pub mmap_bits: u32,
    /// The pages the stack's top moves down by, at most, as a mask.
    pub stack_pages: u64,
    /// The range above a program's end where its heap starts.
    pub brk_range: u64,
}

impl LayoutRandomization {
    pub(crate) fn for_space(end: u64) -> Self {
        if end < SMALLEST_64_BIT_SPACE {
            Self {
                mmap_bits: MMAP_RANDOM_BITS_32_BIT,
                stack_pages: STACK_RANDOM_PAGES_32_BIT,
                brk_range: BRK_RANDOM_RANGE_32_BIT,
            }
        } else {
            Self {
                mmap_bits: MMAP_RANDOM_BITS,
                stack_pages: STACK_RANDOM_PAGES,
                brk_range: BRK_RANDOM_RANGE,
            }
        }
    }

    /// `arch_mmap_rnd`: a page-aligned offset from `random`.
    pub(crate) fn mmap_offset(&self, random: u64) -> u64 {
        (random & ((1 << self.mmap_bits) - 1)) * PAGE_SIZE
    }

    /// `randomize_stack_top`'s offset down from the top, from `random`.
    pub(crate) fn stack_offset(&self, random: u64) -> u64 {
        (random & self.stack_pages) * PAGE_SIZE
    }

    /// `randomize_page`: a page in the heap's range above `start`, from `random`.
    pub(crate) fn brk_start(&self, start: u64, random: u64) -> u64 {
        let start = page_up(start);
        start + random % (self.brk_range / PAGE_SIZE) * PAGE_SIZE
    }
}

/// The tops of the ranges placement fills in turn, lowest first, below the container's address
/// space's end: a mapping goes above one only when the program asks for an address there or the
/// range below is full. They are the ceilings of a 32 bit memory and of a browser's 64 bit one.
pub const PLACEMENT_TIERS: [u64; 2] = [1 << 32, 1 << 34];

/// The size of a PMD's huge page, which anonymous mappings of whole ones are aligned to.
const PMD_SIZE: u64 = crate::hugetlb::DEFAULT_SIZE * crate::hugetlb::KIB;

/// The top of the lowest range placement fills, which the stack and the mmap base are placed
/// under: the lowest tier, or the executor's ceiling where that is lower.
pub(crate) fn first_tier_top(ceiling: u64) -> u64 {
    PLACEMENT_TIERS[0].min(ceiling)
}

/// Linux's `mmap_base` for a top-down layout: below the stack's limit, its guard gap and, when
/// randomized, its random move, with at least `MIN_GAP` and at most five sixths of the space left
/// above, then `offset` lower.
pub(crate) fn mmap_base(
    end: u64,
    stack_limit: u64,
    randomization: Option<LayoutRandomization>,
    offset: u64,
) -> u64 {
    let pad = STACK_GUARD_GAP + randomization.map_or(0, |random| random.stack_pages * PAGE_SIZE);
    let gap = stack_limit.checked_add(pad).unwrap_or(stack_limit);
    let most = end / MAX_GAP_SHARE.1 * MAX_GAP_SHARE.0;
    let gap = if gap < MIN_GAP && MIN_GAP < most {
        MIN_GAP
    } else {
        gap.min(most)
    };
    page_up(end.saturating_sub(gap).saturating_sub(offset))
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
    /// Whether Linux keeps it as anonymous memory (`vma_is_anonymous`: no operations of its own),
    /// as a private mapping of `/dev/zero` is too.
    pub(crate) fn is_anonymous(&self) -> bool {
        match self.backing {
            Backing::Anonymous | Backing::Droppable | Backing::Heap | Backing::Stack => true,
            Backing::Zero { .. } => !self.shared,
            Backing::File { .. } | Backing::Special(_) => false,
        }
    }

    /// Whether the mapping counts against `RLIMIT_DATA`: private and writable, and not a stack
    /// (Linux's `is_data_mapping`).
    pub(crate) fn is_data(&self) -> bool {
        is_data(self.permissions, self.shared, self.grows_down)
    }

    /// Where the mapping's reach starts: below a mapping that grows down, its guard gap is kept free
    /// (Linux's `vm_start_gap`).
    pub(crate) fn start_gap(&self) -> u64 {
        if self.grows_down {
            self.start.saturating_sub(STACK_GUARD_GAP)
        } else {
            self.start
        }
    }

    /// What the page at `address` allows the program (before copy on write).
    pub(crate) fn page_permissions(&self, address: u64) -> Permissions {
        match &self.backing {
            Backing::File { file_end, .. } if address >= *file_end => Permissions::empty(),
            _ => cpu_permissions(self.permissions),
        }
    }

    /// Whether it maps a secret memory file, which stays locked and which no other task may reach.
    pub(crate) fn is_secret(&self) -> bool {
        matches!(&self.backing, Backing::File { node, .. } if node.borrow().is_secret())
    }

    /// Whether `mlock` and `munlock` leave the mapping as it is (`mlock_fixup`): secret memory stays
    /// locked, droppable memory and `VM_SPECIAL` mappings are never locked.
    pub(crate) fn keeps_lock(&self) -> bool {
        self.is_secret() || self.is_special() || matches!(self.backing, Backing::Droppable)
    }

    /// Whether Linux marks it `VM_SPECIAL`: a TCP socket's mapping (`tcp_mmap`'s `VM_MIXEDMAP`
    /// and `VM_DONTEXPAND`), never locked and never grown.
    pub(crate) fn is_special(&self) -> bool {
        matches!(&self.backing, Backing::File { node, .. }
            if matches!(node.borrow().content, crate::fs::Content::Socket(_)))
    }

    /// The mapping in runs of pages, each with whether the page holds the mapping's own bytes: every
    /// page but those of a private file mapping not yet copied.
    pub(crate) fn runs(&self) -> Vec<(u64, u64, bool)> {
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
    pub(crate) fn copies_on_write(&self, address: u64) -> bool {
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
    pub(crate) fn cut_public(&self, start: u64, end: u64) -> Mapping {
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
    /// Guard pages (`MADV_GUARD_INSTALL`, Linux's guard markers) by address: never accessible,
    /// through any change of protection, until removed, unmapped or mapped over.
    guards: BTreeSet<u64>,
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
    notes: Vec<Arc<MapNotes>>,
}

/// What address maps note for a kernel, one value shared by every map on it.
#[derive(Debug, Default)]
pub(crate) struct MapNotes {
    /// The length a placement asked for that found no room below the ceiling.
    pub outgrown: AtomicCell<Option<u64>>,
    /// A shared mapping was put in or taken out of some map.
    pub sharing_changed: AtomicCell<bool>,
}

/// How much of a page's access a watch brings back: none, writes, everything.
fn watch_rank(watch: WatchesFor) -> u8 {
    match watch {
        WatchesFor::None => 0,
        WatchesFor::Writes => 1,
        WatchesFor::All => 2,
    }
}

/// A watch taken along with its page: its thread (none: every thread), its page's offset, the watch.
pub(crate) type TakenWatch = (Option<Tid>, u64, WatchesFor);

/// Takes the watched pages of `[start, end)` out of `pages`, by their offsets from `start`.
fn take_pages(
    pages: &mut BTreeMap<u64, WatchesFor>,
    start: u64,
    end: u64,
) -> Vec<(u64, WatchesFor)> {
    let taken: Vec<(u64, WatchesFor)> = pages
        .range(start..end)
        .map(|(page, watch)| (*page - start, *watch))
        .collect();
    for (offset, _) in &taken {
        pages.remove(&(start + offset));
    }
    taken
}

/// Sets `watch` on each page of `spans` in `pages`, and says what changed.
///
/// `None` ends the watches of `range`'s pages. The answer is whether any page's watch changed, and
/// how many pages' were raised (set, or widened to `All`).
fn set_pages(
    pages: &mut BTreeMap<u64, WatchesFor>,
    spans: &[(u64, u64)],
    range: core::ops::Range<u64>,
    watch: WatchesFor,
) -> (bool, u64) {
    if watch == WatchesFor::None {
        let ended = take_pages(pages, range.start, range.end);
        return (!ended.is_empty(), 0);
    }
    let mut changed = false;
    let mut raised = 0;
    for &(from, to) in spans {
        for page in (from..to).step_by(PAGE_SIZE as usize) {
            let before = pages.insert(page, watch);
            changed |= before != Some(watch);
            if watch_rank(watch) > before.map_or(0, watch_rank) {
                raised += 1;
            }
        }
    }
    (changed, raised)
}

/// The embedder's watches on one memory's pages, and the pages written since it last asked.
#[derive(Clone, Default)]
pub(crate) struct Watches {
    /// Watched pages by address; never `WatchesFor::None`.
    pages: BTreeMap<u64, WatchesFor>,
    /// Each thread's own watched pages, which only that thread's accesses meet.
    ///
    /// Never an empty set, never `WatchesFor::None`.
    threads: BTreeMap<Tid, BTreeMap<u64, WatchesFor>>,
    /// The thread whose own watches the CPU side holds: the one readied for its turn last.
    applied: Option<Tid>,
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
    pub(crate) fn check_described(&mut self, start: u64, end: u64, access: AccessKind) {
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

    /// Tells whether any page is watched for thread `tid`, for every thread or for it alone.
    pub(crate) fn any_for(&self, tid: Tid) -> bool {
        !self.pages.is_empty() || self.threads.contains_key(&tid)
    }

    /// Whether anything narrows what the CPU side allows.
    pub(crate) fn narrows(&self) -> bool {
        !self.pages.is_empty() || !self.threads.is_empty() || self.written.is_some()
    }

    /// Returns the watch thread `by` meets on `page`: every thread's or its own, whichever is wider.
    fn watch_for(&self, by: Option<Tid>, page: u64) -> WatchesFor {
        let every = self.pages.get(&page).copied().unwrap_or(WatchesFor::None);
        let own = by
            .and_then(|tid| self.threads.get(&tid))
            .and_then(|pages| pages.get(&page))
            .copied()
            .unwrap_or(WatchesFor::None);
        if watch_rank(own) > watch_rank(every) {
            own
        } else {
            every
        }
    }

    /// What the CPU side allows of a page that would allow `full`.
    fn narrowed(&self, page: u64, full: Permissions) -> Permissions {
        let mut allowed = full;
        match self.watch_for(self.applied, page) {
            WatchesFor::All => allowed = Permissions::empty(),
            WatchesFor::Writes => allowed -= Permissions::WRITE,
            WatchesFor::None => {}
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

    /// Returns the watch thread `by` meets on the page holding `address`.
    pub(crate) fn watch_at(&self, by: Tid, address: u64) -> WatchesFor {
        self.watch_for(Some(by), page_down(address))
    }

    /// Whether the record of written pages holds back writing the page of `address`.
    pub(crate) fn holds_back_write(&self, address: u64) -> bool {
        self.written
            .as_ref()
            .is_some_and(|written| !written.contains(&page_down(address)))
    }

    /// Finds the first address in `[start, end)` where thread `by` meets a watch for `access`.
    pub(crate) fn first_watched(
        &self,
        by: Tid,
        start: u64,
        end: u64,
        access: AccessKind,
    ) -> Option<u64> {
        if start >= end {
            return None;
        }
        let first = |pages: &BTreeMap<u64, WatchesFor>| {
            pages
                .range(page_down(start)..end)
                .find(|(_, watch)| access == AccessKind::Write || **watch == WatchesFor::All)
                .map(|(page, _)| *page)
        };
        let every = first(&self.pages);
        let own = self.threads.get(&by).and_then(first);
        every
            .into_iter()
            .chain(own)
            .min()
            .map(|page| page.max(start))
    }

    /// Returns how much of the `reached` bytes from `start` a kernel copy for thread `by` takes.
    ///
    /// The copy of `length` bytes stops before a watched page, which is not there for it until the
    /// watch is met; the copy is checked against the running call's description.
    pub(crate) fn copy_reach(
        &mut self,
        by: Tid,
        start: u64,
        length: u64,
        reached: u64,
        access: AccessKind,
    ) -> u64 {
        self.check_described(start, start.saturating_add(length), access);
        match self.first_watched(by, start, start + reached, access) {
            Some(at) => {
                self.touched.get_or_insert((at, access));
                at - start
            }
            None => reached,
        }
    }

    /// Notes that the process wrote `[start, end)`, if the embedder keeps a record.
    pub(crate) fn note_written(&mut self, start: u64, end: u64) -> bool {
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

    /// Takes the watches of `[start, end)` with their threads, by their pages' offsets from `start`.
    fn take(&mut self, start: u64, end: u64) -> Vec<TakenWatch> {
        let mut taken: Vec<TakenWatch> = take_pages(&mut self.pages, start, end)
            .into_iter()
            .map(|(offset, watch)| (None, offset, watch))
            .collect();
        for (tid, pages) in &mut self.threads {
            taken.extend(
                take_pages(pages, start, end)
                    .into_iter()
                    .map(|(offset, watch)| (Some(*tid), offset, watch)),
            );
        }
        self.threads.retain(|_, pages| !pages.is_empty());
        taken
    }

    /// Sets each watch taken by [`Self::take`] on the page at its offset from `start`.
    fn put(&mut self, start: u64, taken: Vec<TakenWatch>) {
        for (tid, offset, watch) in taken {
            let pages = match tid {
                Some(tid) => self.threads.entry(tid).or_default(),
                None => &mut self.pages,
            };
            pages.insert(start + offset, watch);
        }
    }

    /// Forgets the watches and the record of `[start, end)`, which is unmapped.
    fn forget(&mut self, start: u64, end: u64) {
        if !self.narrows() {
            return;
        }
        take_pages(&mut self.pages, start, end);
        for pages in self.threads.values_mut() {
            take_pages(pages, start, end);
        }
        self.threads.retain(|_, pages| !pages.is_empty());
        if let Some(written) = &mut self.written
            && start < end
        {
            written.remove(start..end);
        }
    }
}

impl AddressMap {
    /// A map of `end`'s width whose stack's top is at the ceiling.
    pub(crate) fn new(end: u64, ceiling: u64, notes: Arc<MapNotes>) -> Self {
        Self {
            mappings: BTreeMap::new(),
            used: BTreeMap::new(),
            guards: BTreeSet::new(),
            end,
            ceiling,
            mmap_base: mmap_base(first_tier_top(ceiling), STACK_LIMIT, None, 0),
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
    pub(crate) fn readable_whole(&self, range: core::ops::Range<u64>) -> Result<(), MemoryError> {
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
    pub(crate) fn note_to(&mut self, notes: Arc<MapNotes>) {
        if !self.notes.iter().any(|known| Arc::ptr_eq(known, &notes)) {
            self.notes.push(notes);
        }
    }

    /// Stops noting for a kernel no process of which runs in it any more.
    pub(crate) fn stop_noting_to(&mut self, notes: &Arc<MapNotes>) {
        self.notes.retain(|known| !Arc::ptr_eq(known, notes));
    }

    /// Notes for no kernel, as a copy not yet put on one.
    pub(crate) fn notes_to_none(&mut self) {
        self.notes.clear();
    }

    /// Sets the embedder's watch on the mapped pages of `[start, end)` (page aligned) and tells
    /// the CPU side; whether any page's watch changed, and how many pages' were raised (set, or
    /// widened to `All`).
    pub(crate) fn set_watch<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        range: core::ops::Range<u64>,
        watch: WatchesFor,
    ) -> (bool, u64) {
        // Only watched pages are visited to end watches, and only mapped ones to set them.
        if range.is_empty() {
            return (false, 0);
        }
        let spans = self.mapped_spans(&range, watch);
        let (changed, raised) = set_pages(&mut self.watches.pages, &spans, range.clone(), watch);
        if changed {
            self.reapply(cpu, memory, range.start, range.end);
        }
        (changed, raised)
    }

    /// Sets the embedder's watch for thread `tid` alone on the mapped pages of a page aligned range.
    ///
    /// The CPU side is told if that thread's watches are the ones it holds. The answer says what
    /// changed, as [`Self::set_watch`]'s does.
    pub(crate) fn set_thread_watch<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        tid: Tid,
        range: core::ops::Range<u64>,
        watch: WatchesFor,
    ) -> (bool, u64) {
        if range.is_empty() {
            return (false, 0);
        }
        let spans = self.mapped_spans(&range, watch);
        let pages = self.watches.threads.entry(tid).or_default();
        let (changed, raised) = set_pages(pages, &spans, range.clone(), watch);
        if pages.is_empty() {
            self.watches.threads.remove(&tid);
        }
        if changed && self.watches.applied == Some(tid) {
            self.reapply(cpu, memory, range.start, range.end);
        }
        (changed, raised)
    }

    /// Returns the mapped parts of `range` a watch `watch` is set on: none when it ends watches.
    fn mapped_spans(&self, range: &core::ops::Range<u64>, watch: WatchesFor) -> Vec<(u64, u64)> {
        if watch == WatchesFor::None {
            return Vec::new();
        }
        self.mappings
            .range(..range.end)
            .map(|(_, mapping)| mapping)
            .filter(|mapping| mapping.end > range.start)
            .map(|mapping| (mapping.start.max(range.start), mapping.end.min(range.end)))
            .collect()
    }

    /// Readies the memory for a turn of thread `tid`: the CPU side holds its own watches.
    ///
    /// Those of the thread readied before it are let go of.
    pub(crate) fn ready_for<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        tid: Tid,
    ) {
        if self.watches.applied == Some(tid) {
            return;
        }
        let before = self.watches.applied.replace(tid);
        self.reapply_own(cpu, memory, before.into_iter().chain([tid]));
    }

    /// Ends thread `tid`'s own watches, as the thread has ended.
    pub(crate) fn end_thread_watches<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        tid: Tid,
    ) {
        let Some(pages) = self.watches.threads.remove(&tid) else {
            return;
        };
        if self.watches.applied == Some(tid) {
            for page in pages.keys() {
                self.reapply(cpu, memory, *page, page + PAGE_SIZE);
            }
        }
    }

    /// Tells the CPU side again what the pages watched for any of `threads` alone allow.
    fn reapply_own<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        threads: impl Iterator<Item = Tid>,
    ) {
        let mut pages: Vec<u64> = threads
            .filter_map(|tid| self.watches.threads.get(&tid))
            .flat_map(|pages| pages.keys().copied())
            .collect();
        pages.sort_unstable();
        pages.dedup();
        // Runs of neighboring pages are told at once.
        let mut at = 0;
        while at < pages.len() {
            let start = pages[at];
            let mut end = start + PAGE_SIZE;
            at += 1;
            while at < pages.len() && pages[at] == end {
                end += PAGE_SIZE;
                at += 1;
            }
            self.reapply(cpu, memory, start, end);
        }
    }

    /// The pages written since the embedder last asked, as ranges, and a fresh record from now
    /// (the first time: every page mapped writable).
    pub(crate) fn take_dirty<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
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

    /// Makes a fork's child's copy: no written record, no parent thread's watch, no running call.
    ///
    /// The child's thread is a new one, so the watches of its parent's threads are not its own.
    pub(crate) fn start_child<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
    ) {
        self.note_sharers_changed();
        self.watches.touched = None;
        self.watches.described = None;
        self.watches.undescribed = None;
        // The copy's pages allow what they did for the parent's thread that ran last.
        if let Some(applied) = self.watches.applied.take() {
            self.reapply_own(cpu, memory, [applied].into_iter());
        }
        self.watches.threads.clear();
        if self.watches.written.take().is_some() {
            let starts: Vec<(u64, u64)> =
                self.mappings.values().map(|m| (m.start, m.end)).collect();
            for (start, end) in starts {
                self.reapply(cpu, memory, start, end);
            }
        }
    }

    /// Tells the CPU side again what every mapping's pages over `[start, end)` allow.
    pub(crate) fn reapply<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) {
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
            let _ = apply_permissions_in(
                cpu,
                memory,
                &mapping,
                &self.guards,
                start,
                end,
                &self.watches,
            );
        }
    }

    /// Notes that the process's bytes in `[start, end)` changed, and lets the CPU side write the
    /// pages it now may.
    pub(crate) fn note_written<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) {
        if self.watches.note_written(start, end) {
            self.reapply(cpu, memory, start, end);
        }
    }

    /// Takes the watches of `[start, end)` along to pages a move is about to take there.
    pub(crate) fn take_watches(&mut self, start: u64, end: u64) -> Vec<TakenWatch> {
        if self.watches.pages.is_empty() && self.watches.threads.is_empty() {
            return Vec::new();
        }
        self.watches.take(start, end)
    }

    /// Pages moved to `[start, end)`: the watches taken along with them are set here, and the
    /// pages are noted as written.
    pub(crate) fn moved_here<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        start: u64,
        end: u64,
        watches: Vec<TakenWatch>,
    ) {
        self.watches.put(start, watches);
        if self.watches.narrows() {
            self.watches.note_written(start, end);
            self.reapply(cpu, memory, start, end);
        }
    }

    /// Ends the watches of `[start, end)`, whose pages a fork's child gets fresh.
    pub(crate) fn forget_watches<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) {
        let watched = |pages: &BTreeMap<u64, WatchesFor>| pages.range(start..end).next().is_some();
        if watched(&self.watches.pages) || self.watches.threads.values().any(watched) {
            self.watches.forget(start, end);
            self.reapply(cpu, memory, start, end);
        }
    }

    /// A write fault on a page the record of written pages held back: noted, and the page may be
    /// written from now on. Whether that was the fault's reason.
    pub(crate) fn written_fault<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        address: u64,
    ) -> bool {
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

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Mapping> {
        self.mappings.values()
    }

    /// Whether `pages` more pages, data pages if `data`, fit the limits on the address space and on
    /// data, as Linux's `may_expand_vm`. `freed` pages of the same kind are given up at the same time.
    pub(crate) fn may_expand(
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
    pub(crate) fn pages_in(&self, start: u64, end: u64) -> (u64, u64) {
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
    pub(crate) fn find(&self, address: u64) -> Option<&Mapping> {
        let (_, mapping) = self.mappings.range(..=address).next_back()?;
        (address < mapping.end).then_some(mapping)
    }

    /// The first mapping that ends above `address`.
    pub(crate) fn next_after(&self, address: u64) -> Option<&Mapping> {
        if let Some(mapping) = self.find(address) {
            return Some(mapping);
        }
        self.mappings
            .range(address..)
            .next()
            .map(|(_, mapping)| mapping)
    }

    /// How many bytes from `address` on, up to `length`, the program may access as `wanted`.
    pub(crate) fn accessible(&self, address: u64, length: u64, wanted: Permissions) -> u64 {
        let end = address.saturating_add(length);
        let mut at = address;
        while at < end {
            let Some(mapping) = self.find(at) else { break };
            if !mapping.page_permissions(at).contains(wanted) || self.guarded(at) {
                break;
            }
            let reach = match &mapping.backing {
                Backing::File { file_end, .. } if at < *file_end => (*file_end).min(mapping.end),
                _ => mapping.end,
            };
            at = self.guard_from(at, reach).min(end);
        }
        at - address
    }

    /// Whether the page holding `address` is a guard page.
    pub(crate) fn guarded(&self, address: u64) -> bool {
        self.guards.contains(&page_down(address))
    }

    /// The first guard page in `[from, to)`, or `to`.
    pub(crate) fn guard_from(&self, from: u64, to: u64) -> u64 {
        self.guards
            .range(page_down(from)..to)
            .next()
            .copied()
            .unwrap_or(to)
            .max(from)
    }

    /// The guard pages of `[start, end)`, taken away.
    pub(crate) fn take_guards(&mut self, start: u64, end: u64) -> Vec<u64> {
        let taken: Vec<u64> = self.guards.range(start..end).copied().collect();
        for page in &taken {
            self.guards.remove(page);
        }
        taken
    }

    /// Makes `pages` guard pages, which the CPU side then allows nothing.
    pub(crate) fn add_guards<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        pages: impl IntoIterator<Item = u64>,
    ) -> Result<(), Errno> {
        for page in pages {
            self.guards.insert(page);
            cpu.set_permissions(memory, page, PAGE_SIZE, Permissions::empty())
                .map_err(errno::from_cpu)?;
        }
        Ok(())
    }

    /// Takes the guard pages of `[start, end)` away and tells the CPU side what their mappings
    /// allow again.
    pub(crate) fn remove_guards<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        for page in self.take_guards(start, end) {
            self.refresh(cpu, memory, page, page + PAGE_SIZE)?;
        }
        Ok(())
    }

    /// How many bytes from `address` on, up to `length`, another task's access may reach, as Linux's
    /// `get_user_pages` without `FOLL_FORCE`: by the mapping's own read or write right (not what the
    /// CPU allows), never wholly past a file's end, and never growing a stack.
    pub(crate) fn pinnable(&self, address: u64, length: u64, write: bool) -> u64 {
        let wanted = if write {
            Permissions::WRITE
        } else {
            Permissions::READ
        };
        let end = address.saturating_add(length);
        let mut at = address;
        while at < end {
            let Some(mapping) = self.find(at) else { break };
            if !mapping.permissions.contains(wanted) || mapping.is_secret() || self.guarded(at) {
                break;
            }
            let reach = match &mapping.backing {
                Backing::File { file_end, .. } if at >= *file_end => break,
                Backing::File { file_end, .. } => (*file_end).min(mapping.end),
                _ => mapping.end,
            };
            at = self.guard_from(at, reach).min(end);
        }
        at - address
    }

    /// How many bytes from `address` on, up to `length`, a forced access reaches (`get_user_pages`
    /// with `FOLL_FORCE`, as `/proc/<pid>/mem` and debuggers use): a page the mapping may read
    /// (`VM_MAYREAD`), and for writing one it may write, or a private one it may be allowed to
    /// write, whose page is then copied as on a write fault. Never secret memory, never wholly past
    /// a file's end, never `[vvar]`, never growing a stack.
    pub(crate) fn forced_reach(&self, address: u64, length: u64, write: bool) -> u64 {
        let end = address.saturating_add(length);
        let mut at = address;
        while at < end {
            let Some(mapping) = self.find(at) else { break };
            let allowed = if write {
                mapping.permissions.contains(Permissions::WRITE)
                    || (!mapping.shared && mapping.allowed.contains(Permissions::WRITE))
            } else {
                (mapping.permissions | mapping.allowed).contains(Permissions::READ)
            };
            if !allowed
                || mapping.is_secret()
                || matches!(mapping.backing, Backing::Special("[vvar]"))
                || self.guarded(at)
            {
                break;
            }
            let reach = match &mapping.backing {
                Backing::File { file_end, .. } if at >= *file_end => break,
                Backing::File { file_end, .. } => (*file_end).min(mapping.end),
                _ => mapping.end,
            };
            at = self.guard_from(at, reach).min(end);
        }
        at - address
    }

    /// Whether every page of the range is mapped.
    pub(crate) fn fully_mapped(&self, start: u64, end: u64) -> bool {
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
    pub(crate) fn any_mapped(&self, start: u64, end: u64) -> bool {
        self.next_after(start)
            .is_some_and(|mapping| mapping.start < end)
    }

    /// Whether a new mapping may be placed at the range without being asked to: nothing mapped there,
    /// nor in the guard gap below a mapping that grows down.
    pub(crate) fn free_for_placement(&self, start: u64, end: u64) -> bool {
        self.next_after(start)
            .is_none_or(|mapping| mapping.start_gap() >= end)
    }

    /// Bytes new mappings with no address can still take in the lowest range: the gaps below the
    /// mmap base and those the search falls back to above it.
    pub(crate) fn placeable(&self) -> u64 {
        let (fallback_floor, top) = self.fallback_range();
        self.free_in(LOWEST_ADDRESS, self.mmap_base)
            + self.free_in(fallback_floor.max(self.mmap_base), top)
    }

    /// The free bytes in `[floor, top)`, the guard gap below a mapping that grows down not free.
    fn free_in(&self, floor: u64, top: u64) -> u64 {
        let mut free = 0;
        let mut at = floor;
        for mapping in self
            .mappings
            .range(..top.saturating_add(STACK_GUARD_GAP))
            .map(|(_, mapping)| mapping)
        {
            if mapping.end <= at {
                continue;
            }
            free += mapping.start_gap().min(top).saturating_sub(at);
            at = mapping.end;
        }
        free + top.saturating_sub(at)
    }

    /// The range Linux's top-down search falls back to searching bottom-up when there is no room
    /// below the mmap base: from `TASK_UNMAPPED_BASE`, a quarter of the way up, to the top of the
    /// lowest range, which takes the gap between the mmap base and the stack.
    fn fallback_range(&self) -> (u64, u64) {
        let top = first_tier_top(self.ceiling);
        (page_up(top / UNMAPPED_BASE_DIVISOR), top)
    }

    /// Whether a request for `length` bytes ending at `end` reaches past the ceiling; if so it is
    /// noted as outgrown.
    pub(crate) fn crosses_ceiling(&self, end: u64, length: u64) -> bool {
        if end <= self.ceiling {
            return false;
        }
        self.note(|notes| notes.outgrown.store(Some(length)));
        true
    }

    /// Free space for `length` bytes, the highest below the mmap base, as Linux's top-down search,
    /// which goes on below the program and its heap down to the lowest address, then bottom-up
    /// above the mmap base as Linux falls back to, then in each higher tier in turn, top-down from
    /// its top.
    pub(crate) fn find_free(&self, length: u64) -> Option<u64> {
        self.find_free_beside(length, &[])
    }

    /// Free space as [`AddressMap::find_free`] finds it with `taken` in use too: where the loader
    /// will place a new program's parts.
    pub(crate) fn find_free_beside(
        &self,
        length: u64,
        taken: &[core::ops::Range<u64>],
    ) -> Option<u64> {
        self.search(length, taken, false)
    }

    /// Free space as [`AddressMap::find_free`] finds it for anonymous private memory placed with
    /// no hint: a length of whole PMDs starts on a PMD's boundary where its range has room for
    /// that, as Linux places it for its transparent huge pages (`__thp_get_unmapped_area`).
    pub(crate) fn find_free_anonymous(&self, length: u64) -> Option<u64> {
        self.search(length, &[], length.is_multiple_of(PMD_SIZE))
    }

    /// The search of [`AddressMap::find_free_beside`], range by range, and on a PMD's boundary
    /// first if `on_boundary`; when it finds no space below a ceiling it is noted as outgrown.
    fn search(
        &self,
        length: u64,
        taken: &[core::ops::Range<u64>],
        on_boundary: bool,
    ) -> Option<u64> {
        let mut ranges = alloc::vec![(LOWEST_ADDRESS, self.mmap_base)];
        let mut floor = first_tier_top(self.ceiling);
        for tier in PLACEMENT_TIERS.into_iter().chain([self.end]) {
            let top = tier.min(self.end);
            if top > floor {
                ranges.push((floor, top));
                floor = top;
            }
        }
        for (index, (floor, top)) in ranges.into_iter().enumerate() {
            // A range the executor cannot give is never filled in part, so that placement is the
            // same on every executor.
            if top > self.ceiling {
                break;
            }
            // Linux's search: top-down, and in the lowest range, which holds the stack, bottom-up
            // above the mmap base when that finds nothing.
            let find = |length: u64| {
                self.find_free_between(floor, top, length, taken)
                    .or_else(|| {
                        let (floor, top) = self.fallback_range();
                        let lowest = index == 0;
                        lowest
                            .then(|| self.find_lowest_free_between(floor, top, length, taken))
                            .flatten()
                    })
            };
            // Linux's search for one PMD more, then the boundary in that space, or one PMD higher
            // when the space starts on one, as its layout is top-down.
            let padded = length.checked_add(PMD_SIZE).filter(|_| on_boundary);
            if let Some(start) = padded.and_then(find) {
                let to_boundary = start.wrapping_neg() & (PMD_SIZE - 1);
                return Some(if to_boundary == 0 {
                    start + PMD_SIZE
                } else {
                    start + to_boundary
                });
            }
            if let Some(start) = find(length) {
                return Some(start);
            }
        }
        if self.ceiling < self.end {
            self.note(|notes| notes.outgrown.store(Some(length)));
        }
        None
    }

    /// The highest free space for `length` bytes in `[floor, top)`, with `taken` in use too.
    fn find_free_between(
        &self,
        floor: u64,
        top: u64,
        length: u64,
        taken: &[core::ops::Range<u64>],
    ) -> Option<u64> {
        let mut ceiling = top;
        loop {
            let start = ceiling
                .checked_sub(length)
                .filter(|start| *start >= floor)?;
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

    /// The lowest free space for `length` bytes in `[floor, top)`, with `taken` in use too.
    fn find_lowest_free_between(
        &self,
        floor: u64,
        top: u64,
        length: u64,
        taken: &[core::ops::Range<u64>],
    ) -> Option<u64> {
        let mut start = floor;
        loop {
            let end = start.checked_add(length).filter(|end| *end <= top)?;
            // The highest mapping that, with the guard gap below one that grows down, overlaps
            // [start, end): nothing below its end is free for the length.
            let mapped = self
                .mappings
                .range(..end.saturating_add(STACK_GUARD_GAP))
                .rev()
                .map(|(_, mapping)| mapping)
                .find(|mapping| mapping.start_gap() < end)
                .filter(|mapping| mapping.end > start)
                .map(|mapping| mapping.end);
            let reserved = taken
                .iter()
                .filter(|range| range.start < end && range.end > start)
                .map(|range| range.end)
                .max();
            match mapped.max(reserved) {
                Some(above) => start = above,
                None => return Some(start),
            }
        }
    }

    /// Takes a range out of the table, splitting mappings at its ends, and returns what was there;
    /// its pages' watches and record go with them.
    pub(crate) fn remove(&mut self, start: u64, end: u64) -> Vec<Mapping> {
        self.watches.forget(start, end);
        let removed = self.split_out(start, end);
        if removed.iter().any(|mapping| mapping.shared) {
            self.note(|notes| notes.sharing_changed.store(true));
        }
        removed
    }

    /// Whether any mapping shows shared memory.
    pub(crate) fn has_shared(&self) -> bool {
        self.mappings.values().any(|mapping| mapping.shared)
    }

    /// Notes that the processes using this map changed, if it shows shared memory.
    pub(crate) fn note_sharers_changed(&self) {
        if self.has_shared() {
            self.note(|notes| notes.sharing_changed.store(true));
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
    pub(crate) fn insert<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        mapping: Mapping,
    ) -> Result<(), Errno> {
        let removed = self.remove(mapping.start, mapping.end);
        self.take_guards(mapping.start, mapping.end);
        // New pages are new bytes, as Linux marks a new mapping soft-dirty.
        self.watches.note_written(mapping.start, mapping.end);
        let had_code = removed
            .iter()
            .any(|old| old.permissions.contains(Permissions::EXECUTE));
        // A replaced file mapping lets go of the file's pages first, as `unmap` does: else the
        // clearing below writes zeros into the file, and the new pages stay a view of it.
        for old in &removed {
            if matches!(old.backing, Backing::File { .. }) {
                cpu.unmap_shared(memory, old.start, old.end - old.start)
                    .map_err(errno::from_cpu)?;
            }
        }
        self.clear_used(cpu, memory, mapping.start, mapping.end)?;
        self.apply(cpu, memory, &mapping)?;
        if had_code && !mapping.permissions.contains(Permissions::EXECUTE) {
            cpu.note_changed_code(memory, mapping.start, mapping.end - mapping.start, &[]);
        }
        if mapping.shared {
            self.note(|notes| notes.sharing_changed.store(true));
        }
        self.mappings.insert(mapping.start, mapping);
        self.merge_around();
        Ok(())
    }

    /// Unmaps a range, as `munmap` does.
    pub(crate) fn unmap<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        let removed = self.remove(start, end);
        self.take_guards(start, end);
        for old in &removed {
            cpu.set_permissions(memory, old.start, old.end - old.start, Permissions::empty())
                .map_err(errno::from_cpu)?;
            if matches!(old.backing, Backing::File { .. }) {
                cpu.unmap_shared(memory, old.start, old.end - old.start)
                    .map_err(errno::from_cpu)?;
            }
            if old.permissions.contains(Permissions::EXECUTE) {
                cpu.note_changed_code(memory, old.start, old.end - old.start, &[]);
            }
        }
        Ok(())
    }

    /// Changes what a fully mapped range allows, as `mprotect` does.
    pub(crate) fn protect<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
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
                cpu.note_changed_code(memory, mapping.start, mapping.end - mapping.start, &[]);
            }
            self.mappings.insert(mapping.start, mapping);
        }
        self.merge_around();
        Ok(())
    }

    /// Moves where the mappings of a file stop being reachable to the file's new end, `file_pages`
    /// bytes from its start. Private copies wholly past it go, as Linux's truncation unmaps even
    /// copied pages (`unmap_mapping_range` with `even_cows`), so they show the file's block again.
    pub(crate) fn set_file_end<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        node: &Node,
        file_pages: u64,
    ) -> Result<(), Errno> {
        let affected: Vec<u64> = self
            .mappings
            .values()
            .filter(|mapping| {
                matches!(&mapping.backing, Backing::File { node: mapped, .. } if alloc::sync::Arc::ptr_eq(mapped, node))
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
    pub(crate) fn copy_on_write<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
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
                cpu.unmap_shared(memory, *from, to - from)
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
    pub(crate) fn drop_copies<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
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
    pub(crate) fn set_lock(&mut self, start: u64, end: u64, lock: Lock) {
        for mut mapping in self.split_out(start, end) {
            if !mapping.keeps_lock() {
                mapping.locked = lock;
            }
            self.mappings.insert(mapping.start, mapping);
        }
        self.merge_around();
    }

    /// Sets what a fork does with a range, every page of which is mapped.
    pub(crate) fn set_on_fork(&mut self, start: u64, end: u64, change: impl Fn(&mut OnFork)) {
        for mut mapping in self.split_out(start, end) {
            change(&mut mapping.on_fork);
            self.mappings.insert(mapping.start, mapping);
        }
        self.merge_around();
    }

    /// Locks or unlocks every mapping.
    pub(crate) fn set_lock_all(&mut self, lock: Lock) {
        for mapping in self.mappings.values_mut() {
            if !mapping.keeps_lock() {
                mapping.locked = lock;
            }
        }
        self.merge_around();
    }

    /// The pages locked in memory (Linux's `locked_vm`), and those of them within a range.
    pub(crate) fn locked_pages(&self, start: u64, end: u64) -> (u64, u64) {
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
    pub(crate) fn may_lock(
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
    fn apply<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        mapping: &Mapping,
    ) -> Result<(), Errno> {
        self.apply_permissions(cpu, memory, mapping)?;
        if mapping.permissions.contains(Permissions::EXECUTE) {
            cpu.note_changed_code(memory, mapping.start, mapping.end - mapping.start, &[]);
        }
        Ok(())
    }

    /// Tells the CPU side again what the pages of `[start, end)` in the mapping holding `start`
    /// allow, once pages of its file came that it showed with no access.
    pub(crate) fn refresh<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        match self.find(start).cloned() {
            Some(mapping) => {
                let (lo, hi) = (page_down(start), page_up(end));
                apply_permissions_in(cpu, memory, &mapping, &self.guards, lo, hi, &self.watches)
            }
            None => Ok(()),
        }
    }

    /// Tells the CPU side what a mapping's pages allow.
    fn apply_permissions<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
        memory: MemoryId,
        mapping: &Mapping,
    ) -> Result<(), Errno> {
        apply_permissions_in(
            cpu,
            memory,
            mapping,
            &self.guards,
            mapping.start,
            mapping.end,
            &self.watches,
        )
    }

    /// Zeroes the parts of a range that held something before, and marks it all as used.
    fn clear_used<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
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
                        && alloc::sync::Arc::ptr_eq(node, next_node)
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
    pub(crate) fn grow_stack<C: Executor + 'static>(
        &mut self,
        cpu: &mut GuestMachine<C>,
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
fn drop_copies<C: Executor + 'static>(
    cpu: &mut GuestMachine<C>,
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
        cpu.map_shared(
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
/// it, no writing where a private file page is not copied yet, so the first write comes back
/// to copy it, and nothing on a guard page; the embedder's watches narrow each page further.
fn apply_permissions_in<C: Executor + 'static>(
    cpu: &mut GuestMachine<C>,
    memory: MemoryId,
    mapping: &Mapping,
    guards: &BTreeSet<u64>,
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
        if mapping.shared && mapping.permissions.contains(Permissions::WRITE) {
            crate::shared::backend_writable(node);
        }
        let first = offset / PAGE_SIZE;
        let address = |page: u64| mapping.start + (page - first) * PAGE_SIZE;
        let page = |address: u64| first + (address - mapping.start) / PAGE_SIZE;
        if lo < split.min(hi) {
            for run in crate::shared::unfilled(node, page(lo)..page(split.min(hi))) {
                set(address(run.start), address(run.end), Permissions::empty())?;
            }
        }
    }
    for &guard in guards.range(page_down(lo)..hi) {
        set(guard, guard + PAGE_SIZE, Permissions::empty())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page with a page on either side.
    const WATCHED: u64 = 16 * PAGE_SIZE;
    /// The thread a watch is set for, and one of its siblings.
    const OWNER: Tid = 7;
    const SIBLING: Tid = 8;

    /// Returns `OWNER`'s own watch on writes to `WATCHED`, and every thread's on the next page.
    fn own_and_shared() -> Watches {
        let mut watches = Watches::default();
        watches
            .threads
            .entry(OWNER)
            .or_default()
            .insert(WATCHED, WatchesFor::Writes);
        watches.pages.insert(WATCHED + PAGE_SIZE, WatchesFor::All);
        watches
    }

    #[test]
    fn a_threads_own_watch_is_met_by_that_thread_alone() {
        let mut watches = own_and_shared();
        let both = (WATCHED, WATCHED + 2 * PAGE_SIZE);
        let first =
            |watches: &Watches, by, access| watches.first_watched(by, both.0, both.1, access);
        assert_eq!(first(&watches, OWNER, AccessKind::Write), Some(WATCHED));
        assert_eq!(
            first(&watches, OWNER, AccessKind::Read),
            Some(WATCHED + PAGE_SIZE)
        );
        assert_eq!(
            first(&watches, SIBLING, AccessKind::Write),
            Some(WATCHED + PAGE_SIZE)
        );
        assert_eq!(watches.watch_at(OWNER, WATCHED + 1), WatchesFor::Writes);
        assert_eq!(watches.watch_at(SIBLING, WATCHED + 1), WatchesFor::None);
        // A copy for the owner stops at its own page; one for a sibling goes on to the shared one.
        let length = 2 * PAGE_SIZE;
        assert_eq!(
            watches.copy_reach(OWNER, WATCHED, length, length, AccessKind::Write),
            0
        );
        assert_eq!(
            watches.copy_reach(SIBLING, WATCHED, length, length, AccessKind::Write),
            PAGE_SIZE
        );
    }

    #[test]
    fn the_cpu_side_holds_the_own_watches_of_the_thread_readied_last() {
        let mut watches = own_and_shared();
        let full = Permissions::READ | Permissions::WRITE;
        assert!(watches.narrows());
        assert_eq!(watches.narrowed(WATCHED, full), full);
        watches.applied = Some(OWNER);
        assert_eq!(watches.narrowed(WATCHED, full), Permissions::READ);
        watches.applied = Some(SIBLING);
        assert_eq!(watches.narrowed(WATCHED, full), full);
        // Every thread's watch narrows the page whoever runs.
        assert_eq!(
            watches.narrowed(WATCHED + PAGE_SIZE, full),
            Permissions::empty()
        );
    }

    #[test]
    fn a_threads_own_watches_move_and_end_with_their_pages() {
        let mut watches = own_and_shared();
        let taken = watches.take(WATCHED, WATCHED + 2 * PAGE_SIZE);
        assert!(watches.threads.is_empty() && watches.pages.is_empty());
        let moved = 64 * PAGE_SIZE;
        watches.put(moved, taken);
        assert_eq!(watches.watch_at(OWNER, moved), WatchesFor::Writes);
        assert_eq!(watches.watch_at(SIBLING, moved), WatchesFor::None);
        assert_eq!(
            watches.watch_at(SIBLING, moved + PAGE_SIZE),
            WatchesFor::All
        );
        watches.forget(moved, moved + 2 * PAGE_SIZE);
        assert!(!watches.narrows());
        assert!(!watches.any_for(OWNER));
    }

    #[test]
    fn setting_a_threads_own_watch_says_what_changed() {
        let mut pages = BTreeMap::new();
        let spans = [(WATCHED, WATCHED + 2 * PAGE_SIZE)];
        let range = spans[0].0..spans[0].1;
        assert_eq!(
            set_pages(&mut pages, &spans, range.clone(), WatchesFor::Writes),
            (true, 2)
        );
        assert_eq!(
            set_pages(&mut pages, &spans, range.clone(), WatchesFor::Writes),
            (false, 0)
        );
        assert_eq!(
            set_pages(&mut pages, &spans, range.clone(), WatchesFor::All),
            (true, 2)
        );
        assert_eq!(
            set_pages(&mut pages, &[], range.clone(), WatchesFor::None),
            (true, 0)
        );
        assert!(pages.is_empty());
    }

    #[test]
    fn the_mmap_gap_is_linuxs_for_an_address_space_of_any_size() {
        let gap = |top: u64| top - mmap_base(top, STACK_LIMIT, None, 0);
        let large = 1 << 32;
        assert_eq!(gap(large), MIN_GAP);
        // Five sixths of a top below the least gap is under it: the stack's own gap is kept.
        let small = MIN_GAP;
        assert_eq!(gap(small), STACK_LIMIT + STACK_GUARD_GAP);
        let tiny = MAX_GAP_SHARE.1 * PAGE_SIZE;
        assert_eq!(gap(tiny), MAX_GAP_SHARE.0 * PAGE_SIZE);
        for top in [large, small, tiny, 0] {
            assert!(gap(top) <= top);
        }
    }

    /// Places up to `count` mappings of `length` bytes with no address, until placement fails,
    /// and returns where each went.
    fn fill(map: &mut AddressMap, length: u64, count: usize) -> Vec<u64> {
        let mut placed = Vec::new();
        while placed.len() < count
            && let Some(start) = map.find_free(length)
        {
            place(map, start, length);
            placed.push(start);
        }
        placed
    }

    #[test]
    fn anonymous_memory_of_whole_pmds_starts_on_a_boundary() {
        let notes = Arc::new(MapNotes::default());
        let mut map = AddressMap::new(PLACEMENT_TIERS[1], PLACEMENT_TIERS[1], notes);
        // Below an odd page, as below a vDSO.
        let odd = map.find_free(PAGE_SIZE).expect("space");
        place(&mut map, odd, PAGE_SIZE);
        // Each goes below the one before (what an alignment leaves above it is too small for the
        // next); the last follows one that starts on a boundary, so the space below ends on one.
        let mut top = odd;
        for (length, aligned) in [
            (PMD_SIZE, true),
            (PMD_SIZE + PAGE_SIZE, false),
            (2 * PMD_SIZE, true),
            (PMD_SIZE, true),
        ] {
            let start = map.find_free_anonymous(length).expect("space");
            // The highest start below what is mapped, on a boundary for whole PMDs.
            let highest = top - length;
            let expected = if aligned {
                highest & !(PMD_SIZE - 1)
            } else {
                highest
            };
            assert_eq!(start, expected, "{length:#x}");
            place(&mut map, start, length);
            top = start;
        }
    }

    /// The lowest range's room for a PMD but not for one on a boundary keeps the mapping there
    /// unaligned, under any ceiling, rather than sending it to a higher range.
    #[test]
    fn a_pmd_stays_in_the_lowest_range_without_its_boundary() {
        let hole = PMD_SIZE + PAGE_SIZE;
        for ceiling in [PLACEMENT_TIERS[0], PLACEMENT_TIERS[1]] {
            let notes = Arc::new(MapNotes::default());
            let mut map = AddressMap::new(PLACEMENT_TIERS[1], ceiling, notes);
            let base = map.mmap_base;
            place(
                &mut map,
                LOWEST_ADDRESS + hole,
                base - LOWEST_ADDRESS - hole,
            );
            let top = first_tier_top(ceiling);
            place(&mut map, base, top - base);
            let start = map.find_free_anonymous(PMD_SIZE);
            assert_eq!(
                start,
                Some(LOWEST_ADDRESS + hole - PMD_SIZE),
                "{ceiling:#x}"
            );
        }
    }

    /// With no room below the mmap base, placement goes bottom-up in the lowest range above it,
    /// the gap Linux leaves for the stack, before any higher range; and Linux's search there for a
    /// PMD more gives a boundary.
    #[test]
    fn a_full_space_below_the_mmap_base_falls_back_to_the_gap_above_it() {
        for ceiling in [PLACEMENT_TIERS[0], PLACEMENT_TIERS[1]] {
            let notes = Arc::new(MapNotes::default());
            let mut map = AddressMap::new(PLACEMENT_TIERS[1], ceiling, notes);
            let base = map.mmap_base;
            let top = first_tier_top(ceiling);
            place(&mut map, LOWEST_ADDRESS, base - LOWEST_ADDRESS);
            let stack = top - STACK_LIMIT;
            place(&mut map, stack, STACK_LIMIT);
            map.mappings.get_mut(&stack).unwrap().grows_down = true;
            let before = map.placeable();
            assert_eq!(before, stack - STACK_GUARD_GAP - base, "{ceiling:#x}");
            assert_eq!(map.find_free(PAGE_SIZE), Some(base), "{ceiling:#x}");
            let aligned = (base + PMD_SIZE) & !(PMD_SIZE - 1);
            assert_eq!(
                map.find_free_anonymous(PMD_SIZE),
                Some(aligned),
                "{ceiling:#x}"
            );
            // The gap below the stack's guard gap is filled to its end, never into it.
            let room = before & !(PAGE_SIZE - 1);
            assert_eq!(map.find_free(room), Some(base), "{ceiling:#x}");
            let higher = (ceiling > top).then_some(PLACEMENT_TIERS[1] - room - PAGE_SIZE);
            assert_eq!(map.find_free(room + PAGE_SIZE), higher, "{ceiling:#x}");
        }
    }

    /// Maps `length` bytes of anonymous memory at `start`.
    fn place(map: &mut AddressMap, start: u64, length: u64) {
        let mapping = Mapping {
            start,
            end: start + length,
            permissions: Permissions::READ,
            allowed: Permissions::all(),
            shared: false,
            locked: Lock::None,
            grows_down: false,
            on_fork: OnFork::default(),
            backing: Backing::Anonymous,
        };
        map.mappings.insert(start, mapping);
    }

    #[test]
    fn placement_fills_the_lowest_tier_first_whatever_the_ceiling_and_width() {
        let length = PLACEMENT_TIERS[0] / 8;
        let count = 64;
        let lowest = |map: &AddressMap| map.mmap_base;
        for width in [1 << 36, 1 << 40, crate::user::ADDRESS_LIMIT] {
            let mut first = None;
            for ceiling in [PLACEMENT_TIERS[0], PLACEMENT_TIERS[1], width] {
                let notes = Arc::new(MapNotes::default());
                let mut map = AddressMap::new(width, ceiling, notes.clone());
                let placed = fill(&mut map, length, count);
                assert!(
                    placed
                        .iter()
                        .all(|start| start + length <= ceiling.min(width))
                );
                // Under the lowest tier the mappings go where they go under any ceiling.
                let low: Vec<u64> = placed
                    .iter()
                    .copied()
                    .filter(|start| *start < PLACEMENT_TIERS[0])
                    .collect();
                assert!(low.iter().all(|start| start + length <= lowest(&map)));
                assert_eq!(*first.get_or_insert(low.clone()), low, "width {width:#x}");
                // A higher tier is used only once the one below is full.
                let tier_of =
                    |start: u64| PLACEMENT_TIERS.iter().filter(|top| start >= **top).count();
                assert!(
                    placed
                        .windows(2)
                        .all(|pair| tier_of(pair[0]) <= tier_of(pair[1]))
                );
                let ended = placed.len() < count;
                assert_eq!(notes.outgrown.load().is_some(), ended && ceiling < width);
                assert_eq!(
                    ended,
                    ceiling < width,
                    "width {width:#x}, ceiling {ceiling:#x}"
                );
            }
        }
    }
}
