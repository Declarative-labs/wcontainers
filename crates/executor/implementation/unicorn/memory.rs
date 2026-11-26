//! One address space of the stand-in: a unicorn instance holding the pages it maps, and host memory for the rest.
//!
//! Unicorn owns the bytes of the pages it maps; a page that allows nothing is not mapped, so it faults by being absent,
//! and its bytes wait in host memory. A range that shows shared pages reads and writes them in host memory between
//! turns; unicorn holds a copy for a turn, taken before it when the shared pages changed and given back after it.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use memmap2::MmapMut;
use unicorn_engine::{
    Arch, Arm64CpuModel, Context, HookType, MemType, Mode, Prot, RegisterARM64, Unicorn, uc_error,
};
use wcontainers_abi::{
    Access, CpuError, Exit, PAGE_SIZE, Permissions, RegisterBlock, SYSTEM_CALL_ARGUMENT_COUNT,
    SystemCallNumber, SystemCallRequest,
};
use wcontainers_processor_architecture_arm64::INSTRUCTION_SIZE;
use wcontainers_processor_architecture_arm64::encoding::IMM16;

use crate::standin::Unsupported;

// QEMU's exception numbers, as unicorn's interrupt hook reports them.
/// [EXCP_UDEF](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_UNDEFINED: u32 = 1;
/// [EXCP_SWI](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_SYSCALL: u32 = 2;
/// [EXCP_DATA_ABORT](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_DATA_ABORT: u32 = 4;
/// [EXCP_BKPT](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_BREAKPOINT: u32 = 7;
/// [EXCP_HVC](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_HYPERVISOR_CALL: u32 = 11;
/// [EXCP_SMC](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_SECURE_MONITOR_CALL: u32 = 13;

/// The most bytes a space maps in unicorn as one region, a level 2 block of pages.
///
/// A change of one page's permissions maps the regions it touches anew, so it costs this much at most.
const LARGEST_REGION: u64 = PAGE_SIZE * (PAGE_SIZE / size_of::<u64>() as u64);

/// The count of instructions or of microseconds that unicorn takes as no limit.
const UNICORN_NO_LIMIT: usize = 0;
const UNICORN_NO_TIMEOUT: u64 = 0;
/// The slice that runs one instruction, at one instruction a nanosecond.
pub(crate) const ONE_INSTRUCTION: u64 = 1;

/// A page's length as an index into host memory.
const PAGE_BYTES: usize = PAGE_SIZE as usize;
/// The pages one bit of a touched-page word stands for.
const PAGES_PER_WORD: u64 = u64::BITS as u64;

/// What stopped the emulator, recorded by its hooks.
#[derive(Default)]
pub(crate) struct Stopped {
    exception: Option<u32>,
    fault: Option<(u64, Access)>,
    /// Instructions of the blocks entered, for how long the turn took.
    instructions: u64,
    /// When a turn on the machine's clock ends, whatever it has run: on the clock at the
    /// resolution of the machine's tick, which is cheap to read at every block, as Linux looks at
    /// CPU time at its tick.
    deadline: Option<coarsetime::Instant>,
    /// Raised from another thread to end the turn at the next block.
    interrupt: Option<Arc<AtomicBool>>,
}

/// Pages that may hold something other than zeros, one bit each.
#[derive(Clone)]
struct TouchedPages {
    words: Vec<u64>,
}

impl TouchedPages {
    fn new(pages: u64) -> Self {
        Self {
            words: vec![u64::MIN; to_index(pages.div_ceil(PAGES_PER_WORD))],
        }
    }

    fn touch(&mut self, pages: Range<u64>) {
        for page in pages {
            self.words[to_index(page / PAGES_PER_WORD)] |= 1 << (page % PAGES_PER_WORD);
        }
    }

    fn forget(&mut self, page: u64) {
        self.words[to_index(page / PAGES_PER_WORD)] &= !(1 << (page % PAGES_PER_WORD));
    }

    fn contains(&self, page: u64) -> bool {
        self.words[to_index(page / PAGES_PER_WORD)] & (1 << (page % PAGES_PER_WORD)) != 0
    }

    /// Returns the touched pages among the pages, in order.
    fn list(&self, pages: Range<u64>) -> Vec<u64> {
        pages.filter(|page| self.contains(*page)).collect()
    }
}

/// Pages that several spaces may show, in host memory that lives while any shows them.
pub(crate) struct SharedPages {
    length: u64,
    bytes: RefCell<MmapMut>,
    touched: RefCell<TouchedPages>,
    /// Counts the changes to the bytes, so a space's copy for a turn is taken again only after one.
    version: Cell<u64>,
}

impl SharedPages {
    /// Returns new zeroed pages for the number of bytes.
    pub(crate) fn new(length: u64) -> Result<Self, CpuError> {
        // A mapping of no bytes is refused, so an empty block holds one page no one reaches.
        let mapped = usize::try_from(length.max(PAGE_SIZE)).map_err(|_| CpuError::OutOfMemory)?;
        let bytes = MmapMut::map_anon(mapped).map_err(|_| CpuError::OutOfMemory)?;
        Ok(Self {
            length,
            bytes: RefCell::new(bytes),
            touched: RefCell::new(TouchedPages::new(length.div_ceil(PAGE_SIZE))),
            version: Cell::new(u64::MIN),
        })
    }

    /// Returns new pages holding these pages' bytes, which take host memory only for the pages that hold any.
    pub(crate) fn copy(&self) -> Result<Self, CpuError> {
        let length = self.len();
        let copy = Self::new(length)?;
        let touched = self.touched.borrow().list(0..length.div_ceil(PAGE_SIZE));
        let bytes = self.bytes.borrow();
        for page in touched {
            let range = page_bytes(page, length);
            copy.write(
                range.start,
                &bytes[to_index(range.start)..to_index(range.end)],
            );
        }
        Ok(copy)
    }

    pub(crate) fn len(&self) -> u64 {
        self.length
    }

    /// Returns how many bytes of the page from the offset lie inside the pages.
    fn count_from(&self, offset: u64) -> usize {
        to_index(self.length.saturating_sub(offset).min(PAGE_SIZE))
    }

    /// Copies the bytes from the offset into the buffer, which must lie inside the pages.
    pub(crate) fn read(&self, offset: u64, into: &mut [u8]) {
        let start = to_index(offset);
        into.copy_from_slice(&self.bytes.borrow()[start..start + into.len()]);
    }

    /// Copies the buffer to the offset, which must lie inside the pages.
    pub(crate) fn write(&self, offset: u64, from: &[u8]) {
        if from.is_empty() {
            return;
        }
        let start = to_index(offset);
        self.bytes.borrow_mut()[start..start + from.len()].copy_from_slice(from);
        let end = offset + from.len() as u64;
        self.touched
            .borrow_mut()
            .touch(offset / PAGE_SIZE..end.div_ceil(PAGE_SIZE));
        self.version.set(self.version.get().wrapping_add(1));
    }

    fn is_touched(&self, page: u64) -> bool {
        self.touched.borrow().contains(page)
    }
}

/// A range of a space that shows shared pages.
#[derive(Clone)]
pub(crate) struct Shown {
    end: u64,
    pub(crate) pages: Rc<SharedPages>,
    /// Where in the shared pages the range's start lies.
    offset: u64,
    /// Whether the range was mapped from elsewhere, so a copy shows the pages too instead of their bytes.
    pub(crate) mapped: bool,
    /// The version of the shared pages unicorn's copy holds, none when it holds none.
    synced: Option<u64>,
}

/// A part of a space: its own pages, or shared pages it shows.
pub(crate) enum Part {
    Own,
    Shown {
        pages: Rc<SharedPages>,
        offset: u64,
        mapped: bool,
    },
}

pub(crate) struct Memory {
    unicorn: Unicorn<'static, Stopped>,
    /// The CPU state as made, with the exclusive monitor clear, put back before each turn: a return to
    /// a program clears the monitor (`ClearExclusiveLocal` at exception return), and nothing of one
    /// thread's turn may reach another's.
    fresh: Context,
    /// The bytes of the space's own pages that unicorn does not map.
    hidden: MmapMut,
    /// What each page allows, as the kernel last said.
    pages: Vec<Permissions>,
    /// The ranges currently mapped in unicorn: start to (end, permissions).
    mapped: BTreeMap<u64, (u64, Permissions)>,
    /// Ranges that show shared pages, by their start.
    shared: BTreeMap<u64, Shown>,
    /// Own pages that may hold something other than zeros, so a copy copies only those.
    touched: TouchedPages,
    /// Whether its translated blocks count instructions. Unicorn adds its counting hook without
    /// translating again, so blocks made without it would never stop at a turn's end.
    counting: bool,
    /// Where its address space ends.
    end: u64,
}

impl Memory {
    pub(crate) fn new(end: u64) -> Result<Self, CpuError> {
        let mut unicorn = Unicorn::new_with_data(Arch::ARM64, Mode::ARM, Stopped::default())
            .map_err(|_| CpuError::NotSupported)?;
        unicorn
            .ctl_set_cpu_model(Arm64CpuModel::MAX as i32)
            .expect("unicorn has the max ARM64 model");
        unicorn
            .add_intr_hook(|unicorn, exception| {
                unicorn.get_data_mut().exception = Some(exception);
                unicorn.emu_stop().expect("stopping unicorn");
            })
            .expect("adding the interrupt hook");
        unicorn
            .add_block_hook(1, 0, |unicorn, _, size| {
                let stopped = unicorn.get_data_mut();
                stopped.instructions += u64::from(size) / INSTRUCTION_SIZE;
                if stopped
                    .deadline
                    .is_some_and(|deadline| coarsetime::Instant::now() >= deadline)
                    || stopped
                        .interrupt
                        .as_ref()
                        .is_some_and(|interrupt| interrupt.load(Ordering::Acquire))
                {
                    unicorn.emu_stop().expect("stopping unicorn");
                }
            })
            .expect("adding the block hook");
        let invalid = HookType(
            HookType::MEM_READ_UNMAPPED.0
                | HookType::MEM_WRITE_UNMAPPED.0
                | HookType::MEM_FETCH_UNMAPPED.0
                | HookType::MEM_READ_PROT.0
                | HookType::MEM_WRITE_PROT.0
                | HookType::MEM_FETCH_PROT.0,
        );
        unicorn
            .add_mem_hook(invalid, 1, 0, |unicorn, kind, address, _, _| {
                let access = match kind {
                    MemType::WRITE_UNMAPPED | MemType::WRITE_PROT => Access::Write,
                    MemType::FETCH_UNMAPPED | MemType::FETCH_PROT => Access::Execute,
                    _ => Access::Read,
                };
                // QEMU stores an access that crosses pages one byte at a time and goes on past the first
                // byte that faults, so the first is the instruction's fault.
                unicorn
                    .get_data_mut()
                    .fault
                    .get_or_insert((address, access));
                false
            })
            .expect("adding the fault hook");
        let fresh = unicorn
            .context_init()
            .expect("saving unicorn's fresh CPU state");
        let hidden = MmapMut::map_anon(usize::try_from(end).map_err(|_| CpuError::OutOfMemory)?)
            .map_err(|_| CpuError::OutOfMemory)?;
        Ok(Self {
            unicorn,
            fresh,
            hidden,
            pages: vec![Permissions::empty(); to_index(end / PAGE_SIZE)],
            mapped: BTreeMap::new(),
            shared: BTreeMap::new(),
            touched: TouchedPages::new(end / PAGE_SIZE),
            counting: false,
            end,
        })
    }

    pub(crate) fn end(&self) -> u64 {
        self.end
    }

    /// Returns what the page at the address allows, nothing past the end of the space.
    pub(crate) fn read_permissions(&self, address: u64) -> Permissions {
        usize::try_from(address / PAGE_SIZE)
            .ok()
            .and_then(|page| self.pages.get(page).copied())
            .unwrap_or_else(Permissions::empty)
    }

    /// Returns the addresses as host indices if they lie inside the space.
    pub(crate) fn range(&self, address: u64, length: u64) -> Result<Range<usize>, CpuError> {
        let end = address.checked_add(length).ok_or(CpuError::BadAddress)?;
        if end > self.end {
            return Err(CpuError::BadAddress);
        }
        Ok(to_index(address)..to_index(end))
    }

    /// Returns the shared range holding the address, with its start.
    fn find_shown(&self, address: u64) -> Option<(u64, &Shown)> {
        self.shared
            .range(..=address)
            .next_back()
            .filter(|(_, shown)| address < shown.end)
            .map(|(start, shown)| (*start, shown))
    }

    /// Returns how many bytes from the address lie in one page and one part of the space.
    fn piece_length(&self, address: u64, left: u64) -> u64 {
        let page_end = (address / PAGE_SIZE + 1) * PAGE_SIZE;
        let part_end = match self.find_shown(address) {
            Some((_, shown)) => shown.end,
            None => self
                .shared
                .range(address..)
                .next()
                .map_or(self.end, |(start, _)| *start),
        };
        left.min(page_end - address).min(part_end - address)
    }

    /// Copies the bytes at the address into the buffer, whatever the pages allow.
    pub(crate) fn read(&self, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        self.range(address, into.len() as u64)?;
        let mut done = usize::MIN;
        while done < into.len() {
            let at = address + done as u64;
            let count = to_index(self.piece_length(at, (into.len() - done) as u64));
            let piece = &mut into[done..done + count];
            match self.find_shown(at) {
                Some((start, shown)) => shown.pages.read(shown.offset + (at - start), piece),
                None if self.is_mapped(at / PAGE_SIZE) => self
                    .unicorn
                    .mem_read(at, piece)
                    .expect("unicorn maps the pages that allow something"),
                None => piece.copy_from_slice(&self.hidden[to_index(at)..to_index(at) + count]),
            }
            done += count;
        }
        Ok(())
    }

    /// Copies the buffer to the address, whatever the pages allow.
    pub(crate) fn write(&mut self, address: u64, from: &[u8]) -> Result<(), CpuError> {
        self.range(address, from.len() as u64)?;
        let mut done = usize::MIN;
        while done < from.len() {
            let at = address + done as u64;
            let count = to_index(self.piece_length(at, (from.len() - done) as u64));
            let piece = &from[done..done + count];
            match self.find_shown(at) {
                Some((start, shown)) => shown.pages.write(shown.offset + (at - start), piece),
                None => {
                    if self.is_mapped(at / PAGE_SIZE) {
                        self.unicorn
                            .mem_write(at, piece)
                            .expect("unicorn maps the pages that allow something");
                    } else {
                        self.hidden[to_index(at)..to_index(at) + count].copy_from_slice(piece);
                    }
                    self.touched.touch(at / PAGE_SIZE..at / PAGE_SIZE + 1);
                }
            }
            done += count;
        }
        Ok(())
    }

    /// Sets the bytes over the addresses to zero and ends any sharing there.
    pub(crate) fn zero(&mut self, addresses: Range<u64>) -> Result<(), CpuError> {
        self.range(addresses.start, addresses.end - addresses.start)?;
        self.unshare(addresses.clone());
        let zeros = [u8::MIN; PAGE_BYTES];
        let mut page = addresses.start / PAGE_SIZE;
        while page * PAGE_SIZE < addresses.end {
            let bytes = page_bytes(page, self.end);
            let cut = bytes.start.max(addresses.start)..bytes.end.min(addresses.end);
            if cut == bytes {
                self.forget_page(page);
            } else {
                self.write(cut.start, &zeros[..to_index(cut.end - cut.start)])?;
            }
            page += 1;
        }
        self.remap(addresses);
        Ok(())
    }

    /// Makes an own page read as zeros once the regions over it are mapped again.
    fn forget_page(&mut self, page: u64) {
        if self.touched.contains(page) {
            let bytes = page_bytes(page, self.end);
            self.hidden[to_index(bytes.start)..to_index(bytes.end)].fill(u8::MIN);
            self.touched.forget(page);
        }
    }

    pub(crate) fn set_permissions(&mut self, addresses: Range<u64>, allowed: Permissions) {
        let pages = to_index(addresses.start / PAGE_SIZE)..to_index(addresses.end / PAGE_SIZE);
        self.pages[pages].fill(allowed);
        self.remap(addresses);
    }

    /// Returns what the pages over the addresses allow, one entry a page.
    pub(crate) fn list_permissions(&self, addresses: Range<u64>) -> Vec<Permissions> {
        self.pages[to_index(addresses.start / PAGE_SIZE)..to_index(addresses.end / PAGE_SIZE)]
            .to_vec()
    }

    /// Sets what the pages from the address allow, one entry a page, without mapping them anew.
    pub(crate) fn copy_permissions(&mut self, start: u64, permissions: &[Permissions]) {
        let first = to_index(start / PAGE_SIZE);
        self.pages[first..first + permissions.len()].copy_from_slice(permissions);
    }

    /// Drops unicorn's translations of the addresses, after their code changed.
    pub(crate) fn forget_code(&mut self, addresses: Range<u64>) {
        self.unicorn
            .ctl_remove_cache(addresses.start, addresses.end)
            .expect("dropping unicorn's translations");
    }

    /// Returns the parts of the space over the addresses, each cut to them, with its start and end.
    pub(crate) fn list_parts(&self, addresses: Range<u64>) -> Vec<(Range<u64>, Part)> {
        let mut parts = Vec::new();
        let mut at = addresses.start;
        while at < addresses.end {
            match self.find_shown(at) {
                Some((start, shown)) => {
                    let end = shown.end.min(addresses.end);
                    parts.push((
                        at..end,
                        Part::Shown {
                            pages: Rc::clone(&shown.pages),
                            offset: shown.offset + (at - start),
                            mapped: shown.mapped,
                        },
                    ));
                    at = end;
                }
                None => {
                    let end = self
                        .shared
                        .range(at..)
                        .next()
                        .map_or(addresses.end, |(start, _)| *start)
                        .min(addresses.end);
                    parts.push((at..end, Part::Own));
                    at = end;
                }
            }
        }
        parts
    }

    /// Returns whether the page may hold something other than zeros.
    pub(crate) fn is_touched(&self, page: u64) -> bool {
        match self.find_shown(page * PAGE_SIZE) {
            Some((start, shown)) => shown
                .pages
                .is_touched((shown.offset + (page * PAGE_SIZE - start)) / PAGE_SIZE),
            None => self.touched.contains(page),
        }
    }

    /// Takes the addresses out of the shared ranges, splitting those at its ends, and returns the parts taken.
    ///
    /// The addresses taken read as whatever the space's own pages last held there; unicorn's copy stays until a remap.
    pub(crate) fn unshare(&mut self, addresses: Range<u64>) -> Vec<(Range<u64>, Shown)> {
        let overlapping: Vec<u64> = self
            .shared
            .range(..addresses.end)
            .filter(|(_, shown)| shown.end > addresses.start)
            .map(|(start, _)| *start)
            .collect();
        let mut taken = Vec::new();
        for range_start in overlapping {
            let shown = self.shared.remove(&range_start).expect("a shared range");
            if range_start < addresses.start {
                self.shared.insert(
                    range_start,
                    Shown {
                        end: addresses.start,
                        ..shown.clone()
                    },
                );
            }
            if shown.end > addresses.end {
                self.shared.insert(
                    addresses.end,
                    Shown {
                        offset: shown.offset + (addresses.end - range_start),
                        ..shown.clone()
                    },
                );
            }
            let from = range_start.max(addresses.start);
            let to = shown.end.min(addresses.end);
            let offset = shown.offset + (from - range_start);
            taken.push((
                from..to,
                Shown {
                    end: to,
                    offset,
                    ..shown
                },
            ));
        }
        taken
    }

    /// Makes the addresses show the shared pages from the offset, without mapping them anew in unicorn.
    pub(crate) fn show_unmapped(
        &mut self,
        addresses: Range<u64>,
        pages: Rc<SharedPages>,
        offset: u64,
        mapped: bool,
    ) {
        self.unshare(addresses.clone());
        self.shared.insert(
            addresses.start,
            Shown {
                end: addresses.end,
                pages,
                offset,
                mapped,
                synced: None,
            },
        );
    }

    /// Makes the addresses show the shared pages from the offset.
    pub(crate) fn show(
        &mut self,
        addresses: Range<u64>,
        pages: Rc<SharedPages>,
        offset: u64,
        mapped: bool,
    ) {
        self.show_unmapped(addresses.clone(), pages, offset, mapped);
        self.remap(addresses);
    }

    /// Ends the sharing over the addresses, the space keeping as its own what the shared pages held.
    pub(crate) fn keep_shared_bytes(&mut self, addresses: Range<u64>) {
        for (taken, shown) in self.unshare(addresses.clone()) {
            let mut bytes = vec![u8::MIN; to_index(taken.end - taken.start)];
            shown.pages.read(shown.offset, &mut bytes);
            self.write(taken.start, &bytes)
                .expect("a shared range lies inside the space");
        }
        self.remap(addresses);
    }

    /// Ends the sharing over the addresses where they were mapped from elsewhere, those parts reading as zeros.
    pub(crate) fn drop_mappings(&mut self, addresses: Range<u64>) {
        let mapped: Vec<Range<u64>> = self
            .list_parts(addresses.clone())
            .into_iter()
            .filter_map(|(range, part)| match part {
                Part::Shown { mapped: true, .. } => Some(range),
                _ => None,
            })
            .collect();
        for range in mapped {
            self.unshare(range.clone());
            for page in range.start / PAGE_SIZE..range.end.div_ceil(PAGE_SIZE) {
                self.forget_page(page);
            }
        }
        self.remap(addresses);
    }

    /// Makes the space's own pages over the addresses shared pages it shows, and returns every part as shared pages.
    pub(crate) fn share_out(
        &mut self,
        addresses: Range<u64>,
    ) -> Result<Vec<(Range<u64>, Rc<SharedPages>, u64)>, CpuError> {
        let mut shared = Vec::new();
        let mut converted = false;
        for (range, part) in self.list_parts(addresses.clone()) {
            match part {
                Part::Shown { pages, offset, .. } => shared.push((range, pages, offset)),
                Part::Own => {
                    let pages = Rc::new(SharedPages::new(range.end - range.start)?);
                    let mut bytes = [u8::MIN; PAGE_BYTES];
                    for page in self
                        .touched
                        .list(range.start / PAGE_SIZE..range.end / PAGE_SIZE)
                    {
                        let at = page * PAGE_SIZE;
                        self.read(at, &mut bytes)?;
                        pages.write(at - range.start, &bytes);
                    }
                    self.show_unmapped(range.clone(), Rc::clone(&pages), u64::MIN, false);
                    shared.push((range, pages, u64::MIN));
                    converted = true;
                }
            }
        }
        if converted {
            self.remap(addresses);
        }
        Ok(shared)
    }

    /// Points every shared range at the pages `to_copy` gives for its pages.
    pub(crate) fn retarget(
        &mut self,
        mut to_copy: impl FnMut(&Rc<SharedPages>) -> Rc<SharedPages>,
    ) {
        for shown in self.shared.values_mut() {
            shown.pages = to_copy(&shown.pages);
            shown.synced = None;
        }
        self.remap(0..self.end);
    }

    /// Returns whether the page lies in a range that shows shared pages.
    fn is_shared(&self, page: u64) -> bool {
        self.find_shown(page * PAGE_SIZE).is_some()
    }

    fn is_mapped(&self, page: u64) -> bool {
        self.pages
            .get(to_index(page))
            .is_some_and(|permissions| !permissions.is_empty())
    }

    /// A copy with the same bytes, permissions and shared ranges, on a new unicorn instance.
    pub(crate) fn duplicate(&self) -> Result<Self, CpuError> {
        let mut copy = Self::new(self.end)?;
        let mut bytes = [u8::MIN; PAGE_BYTES];
        for page in self.touched.list(0..self.end / PAGE_SIZE) {
            if self.is_shared(page) {
                continue;
            }
            let at = page * PAGE_SIZE;
            self.read(at, &mut bytes)?;
            copy.hidden[to_index(at)..to_index(at) + PAGE_BYTES].copy_from_slice(&bytes);
        }
        copy.touched.clone_from(&self.touched);
        copy.pages.clone_from(&self.pages);
        copy.shared = self
            .shared
            .iter()
            .map(|(start, shown)| {
                (
                    *start,
                    Shown {
                        synced: None,
                        ..shown.clone()
                    },
                )
            })
            .collect();
        copy.remap(0..self.end);
        Ok(copy)
    }

    /// Returns the pages that may hold something other than zeros, the shared ranges' pages among them, in order.
    #[cfg(feature = "harness")]
    pub(crate) fn list_held_pages(&self) -> Vec<u64> {
        let mut pages: std::collections::BTreeSet<u64> = self
            .touched
            .list(0..self.end / PAGE_SIZE)
            .into_iter()
            .collect();
        for (start, shown) in &self.shared {
            pages.extend(start / PAGE_SIZE..shown.end.div_ceil(PAGE_SIZE));
        }
        pages.into_iter().collect()
    }

    /// Makes unicorn's mappings over the addresses match the pages' permissions again.
    pub(crate) fn remap(&mut self, addresses: Range<u64>) {
        let mut low = addresses.start - addresses.start % PAGE_SIZE;
        let mut high = addresses.end.div_ceil(PAGE_SIZE) * PAGE_SIZE;
        let overlapping: Vec<u64> = self
            .mapped
            .range(..high)
            .filter(|(_, (region_end, _))| *region_end > low)
            .map(|(region_start, _)| *region_start)
            .collect();
        for region_start in overlapping {
            let (region_end, _) = self.mapped.remove(&region_start).expect("a region");
            self.save_own_pages(region_start..region_end);
            self.unicorn
                .mem_unmap(region_start, region_end - region_start)
                .expect("unmapping a region unicorn has");
            low = low.min(region_start);
            high = high.max(region_end);
        }
        let last = high / PAGE_SIZE;
        let mut page = low / PAGE_SIZE;
        while page < last {
            let permissions = self.pages[to_index(page)];
            let run_start = page;
            let run_last = last.min(page + LARGEST_REGION / PAGE_SIZE);
            while page < run_last && self.pages[to_index(page)] == permissions {
                page += 1;
            }
            if permissions.is_empty() {
                continue;
            }
            let region = run_start * PAGE_SIZE..page * PAGE_SIZE;
            self.unicorn
                .mem_map(region.start, region.end - region.start, prot(permissions))
                .expect("mapping a region in unicorn");
            self.mapped.insert(region.start, (region.end, permissions));
            // Its guest may write the pages while it runs, which no touched bit would record.
            if permissions.contains(Permissions::WRITE) {
                self.touched.touch(run_start..page);
            }
            self.restore_own_pages(region);
        }
        // Unicorn's copy of a shared range over the regions mapped anew went with them.
        let starts: Vec<u64> = self.shared.range(..high).map(|(start, _)| *start).collect();
        for start in starts {
            if let Some(shown) = self.shared.get_mut(&start)
                && shown.end > low
            {
                shown.synced = None;
            }
        }
    }

    /// Copies the own pages unicorn maps over the region into host memory, before the region goes.
    fn save_own_pages(&mut self, region: Range<u64>) {
        for page in self
            .touched
            .list(region.start / PAGE_SIZE..region.end / PAGE_SIZE)
        {
            if self.is_shared(page) {
                continue;
            }
            let bytes = page_bytes(page, self.end);
            self.unicorn
                .mem_read(
                    bytes.start,
                    &mut self.hidden[to_index(bytes.start)..to_index(bytes.end)],
                )
                .expect("the region is mapped in unicorn");
        }
    }

    /// Copies the own pages over a region just mapped from host memory into unicorn.
    fn restore_own_pages(&mut self, region: Range<u64>) {
        for page in self
            .touched
            .list(region.start / PAGE_SIZE..region.end / PAGE_SIZE)
        {
            if self.is_shared(page) {
                continue;
            }
            let bytes = page_bytes(page, self.end);
            self.unicorn
                .mem_write(
                    bytes.start,
                    &self.hidden[to_index(bytes.start)..to_index(bytes.end)],
                )
                .expect("the region is mapped in unicorn");
        }
    }

    /// Gives unicorn a copy of every shared range's pages it maps whose copy is not current.
    fn copy_shared_in(&mut self) {
        let starts: Vec<u64> = self.shared.keys().copied().collect();
        for start in starts {
            let shown = &self.shared[&start];
            let version = shown.pages.version.get();
            if shown.synced == Some(version) {
                continue;
            }
            let shown = shown.clone();
            let mut bytes = [u8::MIN; PAGE_BYTES];
            for page in start / PAGE_SIZE..shown.end / PAGE_SIZE {
                let at = page * PAGE_SIZE;
                let offset = shown.offset + (at - start);
                let count = shown.pages.count_from(offset);
                if count == usize::MIN
                    || !self.is_mapped(page)
                    || !shown.pages.is_touched(offset / PAGE_SIZE)
                {
                    continue;
                }
                shown.pages.read(offset, &mut bytes[..count]);
                self.unicorn
                    .mem_write(at, &bytes[..count])
                    .expect("unicorn maps the pages that allow something");
            }
            if let Some(kept) = self.shared.get_mut(&start) {
                kept.synced = Some(version);
            }
        }
    }

    /// Gives the shared pages back the bytes the guest wrote to unicorn's copy of them.
    ///
    /// Every writable page is compared with the shared pages before any is given back, so a range that shows the same
    /// pages as another and was not written leaves what the other wrote.
    fn copy_shared_out(&mut self) {
        let mut written = Vec::new();
        let mut copied = [u8::MIN; PAGE_BYTES];
        let mut held = [u8::MIN; PAGE_BYTES];
        for (start, shown) in &self.shared {
            for page in start / PAGE_SIZE..shown.end / PAGE_SIZE {
                if !self.pages[to_index(page)].contains(Permissions::WRITE) {
                    continue;
                }
                let at = page * PAGE_SIZE;
                let offset = shown.offset + (at - start);
                let count = shown.pages.count_from(offset);
                self.unicorn
                    .mem_read(at, &mut copied[..count])
                    .expect("unicorn maps the pages that allow something");
                shown.pages.read(offset, &mut held[..count]);
                if copied[..count] != held[..count] {
                    for (bytes, place) in changed_runs(&held[..count], &copied[..count]) {
                        written.push((Rc::clone(&shown.pages), offset + place, bytes));
                    }
                }
            }
        }
        for (pages, offset, bytes) in written {
            pages.write(offset, &bytes);
        }
        for shown in self.shared.values_mut() {
            shown.synced = Some(shown.pages.version.get());
        }
    }

    pub(crate) fn load_registers(&mut self, registers: &RegisterBlock) -> Result<(), uc_error> {
        let unicorn = &mut self.unicorn;
        unicorn.context_restore(&self.fresh)?;
        for (index, value) in registers.x.iter().enumerate() {
            unicorn.reg_write(x_register(index), *value)?;
        }
        unicorn.reg_write(RegisterARM64::SP, registers.sp)?;
        unicorn.reg_write(RegisterARM64::PC, registers.pc)?;
        unicorn.reg_write(RegisterARM64::NZCV, registers.pstate)?;
        for (index, value) in registers.v.iter().enumerate() {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&value[0].to_le_bytes());
            bytes[8..].copy_from_slice(&value[1].to_le_bytes());
            unicorn.reg_write_long(q_register(index), &bytes)?;
        }
        unicorn.reg_write(RegisterARM64::FPSR, u64::from(registers.fpsr))?;
        unicorn.reg_write(RegisterARM64::FPCR, u64::from(registers.fpcr))?;
        unicorn.reg_write(RegisterARM64::TPIDR_EL0, registers.tpidr_el0)?;
        Ok(())
    }

    fn save_registers(&self, registers: &mut RegisterBlock) -> Result<(), uc_error> {
        let unicorn = &self.unicorn;
        for (index, value) in registers.x.iter_mut().enumerate() {
            *value = unicorn.reg_read(x_register(index))?;
        }
        registers.sp = unicorn.reg_read(RegisterARM64::SP)?;
        registers.pc = unicorn.reg_read(RegisterARM64::PC)?;
        registers.pstate = unicorn.reg_read(RegisterARM64::NZCV)? & wcontainers_abi::pstate::NZCV;
        for (index, value) in registers.v.iter_mut().enumerate() {
            let bytes = unicorn.reg_read_long(q_register(index))?;
            value[0] = u64::from_le_bytes(bytes[..8].try_into().expect("16 bytes"));
            value[1] = u64::from_le_bytes(bytes[8..16].try_into().expect("16 bytes"));
        }
        registers.fpsr = unicorn.reg_read(RegisterARM64::FPSR)? as u32;
        registers.fpcr = unicorn.reg_read(RegisterARM64::FPCR)? as u32;
        registers.tpidr_el0 = unicorn.reg_read(RegisterARM64::TPIDR_EL0)?;
        Ok(())
    }

    /// Runs the registers on this memory and returns why they came back with the number of instructions run.
    ///
    /// Instructions are counted in whole blocks. With a slice, it stops after that many instructions; with a
    /// deadline, also once the machine's clock passes it; with an interrupt, also once it is raised.
    pub(crate) fn run_turn(
        &mut self,
        registers: &mut RegisterBlock,
        slice_nanoseconds: Option<u64>,
        deadline: Option<coarsetime::Instant>,
        interrupt: Option<Arc<AtomicBool>>,
    ) -> (Result<Exit, Unsupported>, u64) {
        self.copy_shared_in();
        self.load_registers(registers)
            .expect("loading registers into unicorn");
        *self.unicorn.get_data_mut() = Stopped {
            deadline,
            interrupt,
            ..Stopped::default()
        };
        let count = slice_nanoseconds.map_or(UNICORN_NO_LIMIT, |nanoseconds| {
            usize::try_from(nanoseconds.max(ONE_INSTRUCTION)).unwrap_or(usize::MAX)
        });
        if count > UNICORN_NO_LIMIT && !self.counting {
            self.unicorn
                .ctl_flush_tb()
                .expect("flushing unicorn's translated blocks");
        }
        // Unicorn drops its blocks itself when counting stops.
        self.counting = count > UNICORN_NO_LIMIT;
        let result = self
            .unicorn
            .emu_start(registers.pc, u64::MAX, UNICORN_NO_TIMEOUT, count);
        self.save_registers(registers)
            .expect("reading registers from unicorn");
        self.copy_shared_out();
        let stopped = std::mem::take(self.unicorn.get_data_mut());
        // QEMU leaves the program counter after a call to a higher level; at EL0 the call is undefined and the
        // program counter stays on it.
        if matches!(
            stopped.exception,
            Some(EXCEPTION_HYPERVISOR_CALL | EXCEPTION_SECURE_MONITOR_CALL)
        ) {
            registers.pc -= INSTRUCTION_SIZE;
        }
        (
            self.to_exit(result, &stopped, registers),
            stopped.instructions,
        )
    }

    /// Returns what the hooks recorded and unicorn's result as the exit they are.
    fn to_exit(
        &self,
        result: Result<(), uc_error>,
        stopped: &Stopped,
        registers: &RegisterBlock,
    ) -> Result<Exit, Unsupported> {
        // A jump to an address that is not a multiple of 4 faults on alignment before any fetch.
        if let Some((address, access)) = stopped.fault {
            if access == Access::Execute && !address.is_multiple_of(INSTRUCTION_SIZE) {
                return Ok(Exit::AlignmentFault { address });
            }
            return Ok(Exit::MemoryFault { address, access });
        }
        let pc = registers.pc;
        Ok(match (result, stopped.exception) {
            (_, Some(EXCEPTION_SYSCALL)) => {
                let x = &registers.x;
                Exit::Syscall(SystemCallRequest {
                    number: SystemCallNumber(x[wcontainers_abi::SYSTEM_CALL_NUMBER_REGISTER]),
                    arguments: x[..SYSTEM_CALL_ARGUMENT_COUNT]
                        .try_into()
                        .expect("a system call's arguments are registers"),
                })
            }
            (_, Some(EXCEPTION_BREAKPOINT)) => {
                let mut instruction = [u8::MIN; INSTRUCTION_SIZE as usize];
                self.read(pc, &mut instruction)
                    .expect("the breakpoint's instruction lies inside the space");
                let comment = u16::try_from(IMM16.read(u32::from_le_bytes(instruction)))
                    .expect("a breakpoint's comment is 16 bits");
                Exit::Breakpoint { comment }
            }
            // QEMU raises a fetch from a pc that is not a multiple of 4 as undefined.
            (_, Some(EXCEPTION_UNDEFINED)) if !pc.is_multiple_of(INSTRUCTION_SIZE) => {
                Exit::AlignmentFault { address: pc }
            }
            // Unicorn runs at EL1, where HVC and SMC may reach a higher level; at EL0 both are undefined.
            (
                _,
                Some(
                    EXCEPTION_UNDEFINED | EXCEPTION_HYPERVISOR_CALL | EXCEPTION_SECURE_MONITOR_CALL,
                ),
            )
            | (Err(uc_error::INSN_INVALID), None) => Exit::IllegalInstruction,
            // Permission and mapping faults come through the fault hook, so a data abort here is QEMU's
            // alignment check.
            (_, Some(EXCEPTION_DATA_ABORT)) => return Err(Unsupported::AlignmentFault { pc }),
            (Ok(()), None) => Exit::Stop,
            (result, exception) => {
                return Err(Unsupported::UnknownStop {
                    pc,
                    result,
                    exception,
                });
            }
        })
    }
}

/// Returns each run of bytes that differs between two copies of a page, with its place in the page.
fn changed_runs(before: &[u8], after: &[u8]) -> Vec<(Vec<u8>, u64)> {
    let mut runs = Vec::new();
    let mut place = usize::MIN;
    while place < after.len() {
        if before[place] == after[place] {
            place += 1;
            continue;
        }
        let start = place;
        while place < after.len() && before[place] != after[place] {
            place += 1;
        }
        runs.push((after[start..place].to_vec(), start as u64));
    }
    runs
}

fn prot(permissions: Permissions) -> Prot {
    let mut prot = Prot::NONE.0;
    if permissions.contains(Permissions::READ) {
        prot |= Prot::READ.0;
    }
    if permissions.contains(Permissions::WRITE) {
        prot |= Prot::WRITE.0;
    }
    if permissions.contains(Permissions::EXECUTE) {
        prot |= Prot::EXEC.0;
    }
    Prot(prot)
}

/// Unicorn's number of the general register `X<index>`; X0 to X28 are numbered in a run, X29 and X30 apart.
pub(crate) fn x_register(index: usize) -> i32 {
    match index {
        0..=28 => RegisterARM64::X0 as i32 + index as i32,
        29 => RegisterARM64::X29 as i32,
        30 => RegisterARM64::X30 as i32,
        _ => panic!("no general register X{index}"),
    }
}

/// Unicorn's number of the vector register `Q<index>`; Q0 to Q31 are numbered in a run.
pub(crate) fn q_register(index: usize) -> i32 {
    assert!(index < 32, "no vector register Q{index}");
    RegisterARM64::Q0 as i32 + index as i32
}

/// Returns the addresses of the page, cut at the end given.
fn page_bytes(page: u64, end: u64) -> Range<u64> {
    let start = page * PAGE_SIZE;
    start..(start + PAGE_SIZE).min(end.max(start))
}

/// Returns the number as an index into host memory.
fn to_index(number: u64) -> usize {
    usize::try_from(number).expect("an address of a space fits in host memory")
}

#[cfg(test)]
mod tests {
    use unicorn_engine::RegisterARM64;

    use super::{changed_runs, q_register, x_register};

    #[test]
    fn register_numbers_end_where_unicorn_numbers_them() {
        assert_eq!(x_register(28), RegisterARM64::X28 as i32);
        assert_eq!(x_register(29), RegisterARM64::X29 as i32);
        assert_eq!(x_register(30), RegisterARM64::X30 as i32);
        assert_eq!(q_register(31), RegisterARM64::Q31 as i32);
    }

    /// Before, this index became a value of unicorn's register enum that names no register, which
    /// is undefined behavior from a safe function.
    #[test]
    #[should_panic(expected = "no general register X31")]
    fn a_general_register_past_x30_is_refused() {
        x_register(31);
    }

    /// As for the general registers.
    #[test]
    #[should_panic(expected = "no vector register Q32")]
    fn a_vector_register_past_q31_is_refused() {
        q_register(32);
    }

    #[test]
    fn changed_runs_are_the_bytes_that_differ() {
        let before = [0, 1, 2, 3, 4, 5];
        let after = [0, 9, 9, 3, 4, 8];
        assert_eq!(
            changed_runs(&before, &after),
            vec![(vec![9, 9], 1), (vec![8], 5)]
        );
    }
}
