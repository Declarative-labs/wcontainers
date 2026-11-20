//! The native back end's CPU side inside the machine (docs/native.md sections 5.2, 5.3 and 6.3): a process is
//! an address space with its own identifier, its pages kept as the kernel says; shared blocks; threads; pieces
//! translated through the link and loaded into the code area; and a thread run to its next exit.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::hash::Hasher as _;
use core::ops::Range;
use core::ptr;

use aarch64_cpu::asm::barrier::{self, ISH, ISHST, SY};
use aarch64_cpu::registers::{ESR_EL1, FAR_EL1, MDSCR_EL1, Read, SPSR_EL1, TPIDR_EL0, Write as _};
use aarch64_paging::descriptor::El1Attributes;
use arm_sysregs::el0::registers::CtrEl0;
use rangemap::RangeMap;
use rustc_hash::FxHasher;
use wcontainers_abi::{
    Access, AddressSpace as Addresses, Cpu, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions,
    RegisterBlock, Resume, SharedId, Syscall, ThreadId, Warning,
};
use wcontainers_arm64::CACHE_TYPE;
use wcontainers_arm64::mirror::{
    self, LAST_TARGET_BRANCH, LAST_TARGET_CALL, LAST_TARGET_TEST, MIRROR, REACH_BITS,
    SLOT_MOVE_BITS, SLOT_MOVES,
};
use wcontainers_arm64::thread_state::{FPCR_BITS, RegisterPage};
use wcontainers_guest_cpu::position::{NativeReason, Span};
use wcontainers_runtime::{Answer, Carry, Execution, Turn};

use crate::floor::{
    self, AddressSpace, FIRST_PROCESS_IDENTIFIER, Frames, Full, IDLE_IDENTIFIER, Table, Taken,
    UserRegisters, code, enter_user, guest_no_access, guest_read_only, guest_read_write,
    inside_code, make_code_visible, process_code, with_frames,
};
use crate::layout::{BLOCK_SIZE, Region, linear};
use crate::link::LINK_BYTES;
use crate::reason::{
    ASKED_THE_HOST, CODE_AREA_EMPTIED, CODE_CHANGED_AFTER_IT_RAN, EXACT_FLOATING_POINT_REQUESTED,
    GUEST_PHYSICAL_SPACE_FULL, IDENTIFIERS_RAN_OUT, LARGE_ENTRY_SPLIT, MEMORY_COPIED_AT_FORK,
    MEMORY_FAULT, NOT_YET_TRANSLATED, RESERVATION_BROKEN, TURN_LIMIT_STEPPED,
};
use crate::routines::{FloatControl, Placed, Routines, Vectors};
use crate::stop::{
    self, Exception, FloatStatus, GENERAL, GuestState, Interrupted, Resolved, SYSCALL_NUMBER, Stop,
};
use crate::translation::{self, Field, LinkWords, Linking, Translate};

const _: () = assert!(
    MIRROR >> mirror::SLOT_MOVED_BITS == Region::Code.base() >> mirror::SLOT_MOVED_BITS,
    "a slot's moves leave the bits a piece's address shares with the slot's"
);

/// The bytes one announcement of changed code covers: the instruction cache line of the CTR_EL0 we present.
const CODE_LINE: u64 = (size_of::<u32>() as u64) << CtrEl0::from_bits_retain(CACHE_TYPE).iminline();

/// A guest page that has a frame. A page without one reads as zeros and gets one at its first touch.
#[derive(Clone, Copy, Debug)]
struct Page {
    frame: u64,
    /// The block whose frame this is, when shared.
    shared: Option<SharedId>,
}

/// A range that shows a shared block: the block, and the block's offset of address 0, the same along the range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shown {
    block: SharedId,
    origin: u64,
}

impl Shown {
    /// The block's offset that `address` shows.
    fn offset(&self, address: u64) -> u64 {
        address.wrapping_add(self.origin)
    }
}

/// The frames of a shared block's pages, each taken at its first touch.
type Block = Vec<Option<u64>>;

/// What dropping the pieces of a range found.
#[derive(Default)]
struct Forgotten {
    dropped: bool,
    /// The guest code of a dropped piece is no longer what it was translated from.
    changed: bool,
}

/// A piece loaded into the code area.
struct Loaded {
    base: u64,
    length: usize,
    spans: Vec<Span>,
    /// The guest code it was translated from.
    source: Vec<u8>,
    /// Its direct exits: each one's guest target and host address.
    exits: Vec<(u64, u64)>,
}

/// A direct exit: its first word's host address and that word as translated, which a link writes over.
#[derive(Clone, Copy)]
struct Site {
    at: u64,
    word: u32,
    /// The offset bits of a branch pointed at the target's piece itself; none when a direct branch replaces the
    /// word.
    retarget: Option<u32>,
}

struct Memory {
    space: AddressSpace,
    /// What each range allows; outside them the guest's level may touch nothing.
    allowed: RangeMap<u64, Permissions>,
    /// The ranges that show a shared block.
    shown: RangeMap<u64, Shown>,
    /// The pages that have a frame, by address.
    pages: BTreeMap<u64, Page>,
    /// Loaded pieces by the guest address they start at.
    pieces: BTreeMap<u64, Loaded>,
    /// The guest address of each loaded piece, by its host address.
    hosts: BTreeMap<u64, u64>,
    /// The frames of the entry mirror's pages, by their address.
    slots: BTreeMap<u64, u64>,
    /// The direct exits of loaded pieces, by the guest address they go to: linked while a piece is loaded there.
    exits_to: BTreeMap<u64, Vec<Site>>,
    /// The runs of frames reserved for block-sized ranges, by the range's address.
    reserved: BTreeMap<u64, Reservation>,
}

/// The pages of a block-sized range.
const BLOCK_PAGES: u64 = BLOCK_SIZE / PAGE_SIZE;

/// The words of a reservation's bits, one for each page.
const TAKEN_WORDS: usize = BLOCK_PAGES.div_ceil(u64::BITS as u64) as usize;

const _: () = assert!(
    BLOCK_PAGES.is_multiple_of(u64::BITS as u64),
    "a complete reservation has every bit of every word set"
);

/// A run of frames held for one block-sized range of a memory, each page given the frame at its own offset, so
/// that the range can be mapped by one block entry once every page has its frame (docs/native.md section 5.2).
struct Reservation {
    run: u64,
    /// Which pages hold their frame, one bit each.
    taken: [u64; TAKEN_WORDS],
    /// When it was made, so that the oldest is broken first.
    made: u64,
}

impl Reservation {
    /// The word and bit of the page at `offset` in the range.
    fn bit(offset: u64) -> (usize, u64) {
        let page = offset / PAGE_SIZE;
        (
            (page / u64::from(u64::BITS)) as usize,
            1 << (page % u64::from(u64::BITS)),
        )
    }

    /// Gives the page at `offset` its frame.
    fn take(&mut self, offset: u64) -> u64 {
        let (word, bit) = Self::bit(offset);
        assert_eq!(self.taken[word] & bit, 0, "a reserved frame taken twice");
        self.taken[word] |= bit;
        self.run + offset
    }

    /// Whether the page at `offset` holds `frame`, its frame of the run.
    fn holds(&self, offset: u64, frame: u64) -> bool {
        let (word, bit) = Self::bit(offset);
        frame == self.run + offset && self.taken[word] & bit != 0
    }

    /// Takes the frame back from the page at `offset`.
    fn give(&mut self, offset: u64) {
        let (word, bit) = Self::bit(offset);
        self.taken[word] &= !bit;
    }

    fn complete(&self) -> bool {
        self.taken.iter().all(|&word| word == u64::MAX)
    }

    fn empty(&self) -> bool {
        self.taken.iter().all(|&word| word == 0)
    }

    /// The frames no page holds.
    fn untaken(&self) -> impl Iterator<Item = u64> + '_ {
        (0..BLOCK_PAGES)
            .map(|page| page * PAGE_SIZE)
            .filter(|&offset| {
                let (word, bit) = Self::bit(offset);
                self.taken[word] & bit == 0
            })
            .map(|offset| self.run + offset)
    }
}

/// The block-sized range `address` is in.
fn block_of(address: u64) -> u64 {
    address & !(BLOCK_SIZE - 1)
}

/// The block-sized ranges that lie wholly in `range`.
fn whole_blocks(range: &Range<u64>) -> impl Iterator<Item = u64> + use<> {
    let first = range.start.next_multiple_of(BLOCK_SIZE);
    (first..block_of(range.end)).step_by(BLOCK_SIZE as usize)
}

impl Memory {
    fn new(space: AddressSpace) -> Memory {
        Memory {
            space,
            allowed: RangeMap::new(),
            shown: RangeMap::new(),
            pages: BTreeMap::new(),
            pieces: BTreeMap::new(),
            hosts: BTreeMap::new(),
            slots: BTreeMap::new(),
            exits_to: BTreeMap::new(),
            reserved: BTreeMap::new(),
        }
    }

    /// What every page of the block-sized range at `base` allows, when they allow the same and some access, and
    /// none shows a shared block: a range one block entry can map.
    fn alike(&self, base: u64) -> Option<Permissions> {
        let range = base..base + BLOCK_SIZE;
        if self.shown.overlaps(&range) {
            return None;
        }
        let (whole, &allowed) = self.allowed.get_key_value(&base)?;
        (whole.end >= range.end && !allowed.is_empty()).then_some(allowed)
    }

    /// What the page at `page` allows.
    fn allows(&self, page: u64) -> Permissions {
        self.allowed
            .get(&page)
            .copied()
            .unwrap_or(Permissions::empty())
    }

    /// The frame holding the page at `page`: its own, else the frame of the block page it shows, if any.
    fn frame(&self, blocks: &BTreeMap<SharedId, Block>, page: u64) -> Option<u64> {
        match self.pages.get(&page) {
            Some(p) => Some(p.frame),
            None => {
                let shown = self.shown.get(&page)?;
                *blocks
                    .get(&shown.block)?
                    .get((shown.offset(page) / PAGE_SIZE) as usize)?
            }
        }
    }

    /// Reads guest memory, pages with no frame as zeros.
    fn read(&self, blocks: &BTreeMap<SharedId, Block>, address: u64, into: &mut [u8]) {
        read_pages(|page| self.frame(blocks, page), address, into);
    }

    /// The loaded piece whose host code holds `pc`; a pc in the entry mirror is in none, a piece of no length.
    fn piece_at(&self, pc: u64) -> stop::Piece<'_> {
        let found = self
            .hosts
            .range(..=pc)
            .next_back()
            .and_then(|(&base, guest)| {
                let loaded = &self.pieces[guest];
                (pc - base < loaded.length as u64).then_some(stop::Piece {
                    base,
                    length: loaded.length,
                    spans: &loaded.spans,
                })
            });
        found.unwrap_or_else(|| {
            assert!(
                pc >= MIRROR,
                "translated code ran outside every piece, at {pc:#x}"
            );
            stop::Piece {
                base: pc,
                length: 0,
                spans: &[],
            }
        })
    }
}

/// Writes host words at `at` in the code area or the running process's entry mirror, and makes them visible.
fn write_code(at: u64, words: &[u32]) {
    // SAFETY: both are mapped writable at this level, and no virtual CPU runs these words while they change.
    unsafe { ptr::copy_nonoverlapping(words.as_ptr(), at as *mut u32, words.len()) };
    make_code_visible(at, size_of_val(words) as u64);
}

impl Loaded {
    /// Whether the guest code this piece was translated from overlaps `range`.
    fn overlaps(&self, at: u64, range: &core::ops::Range<u64>) -> bool {
        let end = at + self.source.len() as u64;
        at < range.end && range.start < end
    }
}

struct Thread {
    memory: MemoryId,
    state: GuestState,
    float: FloatStatus,
    page: RegisterPage,
    /// The register page's address and frame.
    register_page: u64,
    page_frame: u64,
    /// The vector registers while another thread has the hardware's.
    vectors: Vectors,
    tpidr: u64,
}

/// The native CPU side of one container.
pub struct NativeCpu {
    global: Table,
    memories: BTreeMap<MemoryId, Memory>,
    shared: BTreeMap<SharedId, Block>,
    /// Boxed, so that a run takes its thread out and puts it back without copying it.
    threads: BTreeMap<ThreadId, Box<Thread>>,
    next_id: u32,
    /// The next address-space identifier, none when they ran out.
    next_asid: Option<u16>,
    /// The address space switched to while none of a process may be: the global tables and no guest page.
    idle: AddressSpace,
    /// The floor's counts last reported: link exits, frames refused, blocks split.
    asked_reported: u64,
    full_reported: u64,
    splits_reported: u64,
    /// Runs of frames that no reservation holds, kept whole for the next one.
    runs: Vec<u64>,
    /// How many reservations were made.
    reservations_made: u64,
    /// The code area's next free byte and the end of what is mapped of it.
    code_next: u64,
    code_mapped: u64,
    /// Whether pieces carry the progress count (the process asks for exact replay).
    exact_replay: bool,
    /// The words written to fill entry slots and link exits.
    words: LinkWords,
    /// The thread whose vector registers the hardware holds.
    live: Option<ThreadId>,
    /// The float state routines.
    read_float_control: extern "C" fn() -> FloatControl,
    write_float_control: extern "C" fn(u64, u64),
    save_vectors: extern "C" fn(*mut Vectors),
    load_vectors: extern "C" fn(*const Vectors),
    invalidate_whole_tlb: extern "C" fn(),
    counters: BTreeMap<&'static str, u64>,
    /// Slow paths not yet handed to a run's `warn`: each reason's latest count, which holds every one before it, and detail.
    unsent: BTreeMap<&'static str, (u64, u64)>,
    /// How many times a thread stopped, and a hash of the position, count and pc, of each stop in order.
    stops: u64,
    positions: FxHasher,
    /// The count of the thread that stopped last, at that stop.
    last_count: u64,
}

fn out_of_memory(_: Full) -> CpuError {
    CpuError::OutOfMemory
}

fn page_of(address: u64) -> u64 {
    address & !(PAGE_SIZE - 1)
}

/// A frame's bytes through the linear map.
fn frame_bytes(frame: u64) -> &'static mut [u8] {
    // SAFETY: the frame is the CPU side's, and only reached through the linear map.
    unsafe { core::slice::from_raw_parts_mut(linear(frame) as *mut u8, PAGE_SIZE as usize) }
}

/// The table attributes that give the guest's level what `permissions` allow of data. Code is never run from
/// guest pages: a page that may be run is read for translating.
fn attributes(permissions: Permissions) -> El1Attributes {
    if permissions.contains(Permissions::WRITE) {
        guest_read_write()
    } else if permissions.contains(Permissions::READ) {
        guest_read_only()
    } else {
        guest_no_access()
    }
}

/// The pages of a page-aligned range.
fn pages(address: u64, length: u64) -> impl Iterator<Item = u64> {
    (address..address + length).step_by(PAGE_SIZE as usize)
}

/// Reads guest memory through `frame`, which gives a page's frame, pages with none as zeros.
fn read_pages(frame: impl Fn(u64) -> Option<u64>, mut address: u64, into: &mut [u8]) {
    let mut done = 0;
    while done < into.len() {
        let offset = (address - page_of(address)) as usize;
        let take = (PAGE_SIZE as usize - offset).min(into.len() - done);
        let part = &mut into[done..done + take];
        match frame(page_of(address)) {
            Some(frame) => part.copy_from_slice(&frame_bytes(frame)[offset..offset + take]),
            None => part.fill(0),
        }
        done += take;
        address += take as u64;
    }
}

impl NativeCpu {
    /// The CPU side over the global tables `global`; pieces carry the progress count when `exact_replay`.
    pub fn new(global: Table, exact_replay: bool) -> Result<NativeCpu, Full> {
        let frame = with_frames(|frames| frames.take())?;
        let placed: Placed = floor::ask(&Routines { at: frame }, &[]);
        let at = Region::Code.base();
        with_frames(|frames| global.map(frames, at, frame, inside_code()))?;
        make_code_visible(at, PAGE_SIZE);
        let words: LinkWords = floor::ask(&Linking {}, &[]);
        let idle = AddressSpace {
            user: with_frames(Table::new)?,
            inside: global,
            asid: IDLE_IDENTIFIER,
        };
        // SAFETY: the monitor wrote each routine at its place, with the C calling convention's signature
        // `Placed` names, and the frame is mapped for this level to run.
        let (
            read_float_control,
            write_float_control,
            save_vectors,
            load_vectors,
            invalidate_whole_tlb,
        ) = unsafe {
            (
                core::mem::transmute::<u64, extern "C" fn() -> FloatControl>(
                    at + placed.read_float_control,
                ),
                core::mem::transmute::<u64, extern "C" fn(u64, u64)>(
                    at + placed.write_float_control,
                ),
                core::mem::transmute::<u64, extern "C" fn(*mut Vectors)>(at + placed.save_vectors),
                core::mem::transmute::<u64, extern "C" fn(*const Vectors)>(
                    at + placed.load_vectors,
                ),
                core::mem::transmute::<u64, extern "C" fn()>(at + placed.invalidate_whole_tlb),
            )
        };
        Ok(NativeCpu {
            global,
            memories: BTreeMap::new(),
            shared: BTreeMap::new(),
            threads: BTreeMap::new(),
            next_id: 0,
            next_asid: Some(FIRST_PROCESS_IDENTIFIER),
            idle,
            asked_reported: 0,
            full_reported: 0,
            splits_reported: 0,
            runs: Vec::new(),
            reservations_made: 0,
            code_next: Region::Code.base() + PAGE_SIZE,
            code_mapped: Region::Code.base() + PAGE_SIZE,
            exact_replay,
            words,
            live: None,
            read_float_control,
            write_float_control,
            save_vectors,
            load_vectors,
            invalidate_whole_tlb,
            counters: BTreeMap::new(),
            unsent: BTreeMap::new(),
            stops: 0,
            positions: FxHasher::default(),
            last_count: 0,
        })
    }

    /// Every slow path taken so far, by reason.
    pub fn counters(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        self.counters
            .iter()
            .map(|(&reason, &count)| (reason, count))
    }

    fn slow(&mut self, reason: &'static str, times: u64, detail: u64) {
        let count = self.counters.entry(reason).or_default();
        *count += times;
        self.unsent.insert(reason, (*count, detail));
    }

    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn memory(&mut self, memory: MemoryId) -> Result<&mut Memory, CpuError> {
        self.memories.get_mut(&memory).ok_or(CpuError::BadAddress)
    }

    fn check(&self, address: u64, length: u64) -> Result<(), CpuError> {
        match address.checked_add(length) {
            Some(end) if end <= self.address_space().end => Ok(()),
            _ => Err(CpuError::BadAddress),
        }
    }

    /// The frame of a shared block's page at `offset`, taken at its first touch.
    fn block_frame(&mut self, block: SharedId, offset: u64) -> Result<u64, CpuError> {
        let index = (offset / PAGE_SIZE) as usize;
        let held = *self
            .shared
            .get(&block)
            .and_then(|frames| frames.get(index))
            .ok_or(CpuError::BadAddress)?;
        if let Some(frame) = held {
            return Ok(frame);
        }
        let frame = self.guest_frame()?;
        self.shared.get_mut(&block).expect("looked up above")[index] = Some(frame);
        Ok(frame)
    }

    /// A zeroed frame for the guest's memory, reservations broken for it when no other is left.
    fn guest_frame(&mut self) -> Result<u64, CpuError> {
        while !with_frames(|frames| frames.has_room_for_guest()) && self.reclaim() {}
        with_frames(Frames::take_for_guest).map_err(out_of_memory)
    }

    /// Gives frames back from reservations: a whole run no reservation holds, else the untaken frames of the
    /// oldest reservation that has any, which is broken. Whether any came back.
    fn reclaim(&mut self) -> bool {
        if let Some(run) = self.runs.pop() {
            with_frames(|frames| {
                (0..BLOCK_PAGES).for_each(|page| frames.give(run + page * PAGE_SIZE));
            });
            return true;
        }
        let oldest = self
            .memories
            .iter()
            .flat_map(|(&id, m)| {
                m.reserved
                    .iter()
                    .filter(|(_, r)| !r.complete())
                    .map(move |(&base, r)| (r.made, id, base))
            })
            .min();
        let Some((_, id, base)) = oldest else {
            return false;
        };
        let broken = self
            .memories
            .get_mut(&id)
            .and_then(|m| m.reserved.remove(&base));
        let broken = broken.expect("found above");
        let given =
            with_frames(|frames| broken.untaken().map(|frame| frames.give(frame)).count() as u64);
        self.slow(RESERVATION_BROKEN, 1, given);
        true
    }

    /// A zeroed frame for the private page at `page`: the frame at its offset in the run reserved for its
    /// block-sized range, reserved now if it has none, else, when no run is left, a frame of its own
    /// (docs/native.md section 5.2).
    fn private_frame(&mut self, memory: MemoryId, page: u64) -> Result<u64, CpuError> {
        let base = block_of(page);
        if !self.memory(memory)?.reserved.contains_key(&base)
            && let Some(run) = self
                .runs
                .pop()
                .or_else(|| with_frames(Frames::take_run_for_guest))
        {
            self.reservations_made += 1;
            let made = self.reservations_made;
            self.memory(memory)?.reserved.insert(
                base,
                Reservation {
                    run,
                    taken: [0; TAKEN_WORDS],
                    made,
                },
            );
        }
        match self.memory(memory)?.reserved.get_mut(&base) {
            Some(reservation) => {
                let frame = reservation.take(page - base);
                floor::tables::clear(frame);
                Ok(frame)
            }
            None => self.guest_frame(),
        }
    }

    /// Takes back the frame a private page of `memory` no longer holds: into its reservation, whose run is kept
    /// whole once no page holds a frame of it, else into the free frames.
    fn release(&mut self, memory: MemoryId, page: u64, frame: u64) {
        let base = block_of(page);
        let reservation = self
            .memories
            .get_mut(&memory)
            .and_then(|m| m.reserved.get_mut(&base))
            .filter(|r| r.holds(page - base, frame));
        let Some(reservation) = reservation else {
            with_frames(|frames| frames.give(frame));
            return;
        };
        reservation.give(page - base);
        if reservation.empty() {
            let run = reservation.run;
            self.memories
                .get_mut(&memory)
                .expect("found above")
                .reserved
                .remove(&base);
            self.runs.push(run);
        }
    }

    /// Maps the block-sized range at `base` with one block entry when every page of it holds its frame of the
    /// range's reservation and they are alike.
    fn promote(&mut self, memory: MemoryId, base: u64) -> Result<(), CpuError> {
        let m = self.memory(memory)?;
        let (Some(reservation), Some(allowed)) = (m.reserved.get(&base), m.alike(base)) else {
            return Ok(());
        };
        if !reservation.complete() || m.space.user.is_block(base) {
            return Ok(());
        }
        let (table, run) = (m.space.user, reservation.run);
        with_frames(|frames| table.promote(frames, base, run, attributes(allowed)));
        Ok(())
    }

    /// Maps the page at `page`, which nothing maps, to `frame`, breaking reservations for the tables it needs.
    fn map_page(&mut self, memory: MemoryId, page: u64, frame: u64) -> Result<(), CpuError> {
        let m = self.memory(memory)?;
        let (table, allowed) = (m.space.user, attributes(m.allows(page)));
        loop {
            match with_frames(|frames| table.map(frames, page, frame, allowed)) {
                Ok(()) => return Ok(()),
                Err(full) if !self.reclaim() => return Err(out_of_memory(full)),
                Err(_) => {}
            }
        }
    }

    /// Gives the page at `page`, which has no frame, its frame: the block's when it shows one, else a private
    /// one, mapped with what the page allows.
    fn back(&mut self, memory: MemoryId, page: u64) -> Result<u64, CpuError> {
        let shown = self.memory(memory)?.shown.get(&page).copied();
        let (frame, shared) = match shown {
            Some(shown) => (
                self.block_frame(shown.block, shown.offset(page))?,
                Some(shown.block),
            ),
            None => (self.private_frame(memory, page)?, None),
        };
        if let Err(error) = self.map_page(memory, page, frame) {
            if shared.is_none() {
                self.release(memory, page, frame);
            }
            return Err(error);
        }
        self.memory(memory)?
            .pages
            .insert(page, Page { frame, shared });
        self.promote(memory, block_of(page))?;
        Ok(frame)
    }

    /// The frame of a guest page, given one when it has none.
    fn frame_for(&mut self, memory: MemoryId, page: u64) -> Result<u64, CpuError> {
        match self.memory(memory)?.pages.get(&page) {
            Some(p) => Ok(p.frame),
            None => self.back(memory, page),
        }
    }

    /// Backs the page a fault at `address` was refused at when this was its first touch: it has no frame and
    /// allows the access. Whether it did.
    fn first_touch(
        &mut self,
        memory: MemoryId,
        address: u64,
        write: bool,
    ) -> Result<bool, CpuError> {
        let m = self.memory(memory)?;
        let page = page_of(address);
        let needed = if write {
            Permissions::WRITE
        } else {
            Permissions::READ
        };
        if m.pages.contains_key(&page) || !m.allows(page).contains(needed) {
            return Ok(false);
        }
        self.back(memory, page).map(|_| true)
    }

    /// Points a guest page at another frame, mapped with what it allows.
    fn replace(&mut self, memory: MemoryId, page: u64, with: Page) -> Result<(), CpuError> {
        let m = self.memory(memory)?;
        let table = m.space.user;
        let attributes = attributes(m.allows(page));
        match m.pages.get(&page) {
            Some(_) => with_frames(|frames| table.replace(frames, page, with.frame, attributes)),
            None => with_frames(|frames| table.map(frames, page, with.frame, attributes)),
        }
        .map_err(out_of_memory)?;
        m.pages.insert(page, with);
        Ok(())
    }

    /// Takes the frame from a guest page, which then reads as zeros until its next touch, giving it back when
    /// it is the page's own.
    fn unback(&mut self, memory: MemoryId, page: u64) -> Result<(), CpuError> {
        let m = self.memory(memory)?;
        let Some(p) = m.pages.remove(&page) else {
            return Ok(());
        };
        let table = m.space.user;
        let split = with_frames(|frames| table.split_block(frames, page));
        if let Err(full) = split {
            self.memory(memory)?.pages.insert(page, p);
            return Err(out_of_memory(full));
        }
        table.unmap(page);
        if p.shared.is_none() {
            self.release(memory, page, p.frame);
        }
        Ok(())
    }

    /// Unmaps every block entry of `memory` that lies wholly in `range`, its pages reading as zeros until their
    /// next touch and its run kept whole.
    fn unback_blocks(&mut self, memory: MemoryId, range: &Range<u64>) -> Result<(), CpuError> {
        let m = self.memories.get_mut(&memory).ok_or(CpuError::BadAddress)?;
        for base in whole_blocks(range).filter(|&base| m.space.user.is_block(base)) {
            m.space.user.unmap_block(base);
            let mut from_block = m.pages.split_off(&base);
            let mut after = from_block.split_off(&(base + BLOCK_SIZE));
            m.pages.append(&mut after);
            let reservation = m
                .reserved
                .remove(&base)
                .expect("a block maps a reserved run");
            self.runs.push(reservation.run);
        }
        Ok(())
    }

    /// Makes `copy` hold what `source` holds, its private pages copied at once, giving how many were.
    fn copy_memory(&mut self, source: MemoryId, copy: MemoryId) -> Result<u64, CpuError> {
        let from = self.memory(source)?;
        let pages: Vec<(u64, Page)> = from.pages.iter().map(|(&a, &p)| (a, p)).collect();
        let (allowed, shown) = (from.allowed.clone(), from.shown.clone());
        let to = self.memory(copy)?;
        to.allowed = allowed;
        to.shown = shown;
        let mut copied = 0;
        for (at, page) in pages {
            let frame = match page.shared {
                Some(_) => page.frame,
                None => {
                    copied += 1;
                    let frame = self.private_frame(copy, at)?;
                    frame_bytes(frame).copy_from_slice(frame_bytes(page.frame));
                    frame
                }
            };
            if let Err(error) = self.map_page(copy, at, frame) {
                if page.shared.is_none() {
                    self.release(copy, at, frame);
                }
                return Err(error);
            }
            self.memory(copy)?.pages.insert(at, Page { frame, ..page });
            self.promote(copy, block_of(at))?;
        }
        Ok(copied)
    }

    /// Ends the sharing of the pages of `range`, page aligned: each keeps a private copy of what its block's page
    /// held, except those `zeroed` names, which are about to read as zeros.
    fn unshare(
        &mut self,
        memory: MemoryId,
        range: Range<u64>,
        zeroed: impl Fn(u64) -> bool,
    ) -> Result<(), CpuError> {
        if range.is_empty() {
            return Ok(());
        }
        let shown: Vec<Range<u64>> = self
            .memory(memory)?
            .shown
            .overlapping(range.clone())
            .map(|(r, _)| r.start.max(range.start)..r.end.min(range.end))
            .collect();
        for part in shown {
            for at in pages(part.start, part.end - part.start) {
                let m = &self.memories[&memory];
                let own = m.pages.get(&at).copied();
                let held = m.frame(&self.shared, at);
                match held {
                    Some(frame) if !zeroed(at) => {
                        let copy = self.private_frame(memory, at)?;
                        frame_bytes(copy).copy_from_slice(frame_bytes(frame));
                        let private = Page {
                            frame: copy,
                            shared: None,
                        };
                        if let Err(error) = self.replace(memory, at, private) {
                            self.release(memory, at, copy);
                            return Err(error);
                        }
                        self.promote(memory, block_of(at))?;
                    }
                    _ if own.is_some() => self.unback(memory, at)?,
                    _ => {}
                }
            }
        }
        self.memory(memory)?.shown.remove(range);
        Ok(())
    }

    /// Drops the pieces translated from a range.
    fn forget_code(&mut self, memory: MemoryId, address: u64, length: u64) -> Forgotten {
        let mut forgotten = Forgotten::default();
        let Some(m) = self.memories.get(&memory) else {
            return forgotten;
        };
        let range = address..address + length;
        let overlapping: Vec<u64> = m
            .pieces
            .iter()
            .filter(|(at, loaded)| loaded.overlaps(**at, &range))
            .map(|(&at, _)| at)
            .collect();
        for &at in &overlapping {
            let loaded = &m.pieces[&at];
            let mut now = vec![0; loaded.source.len()];
            m.read(&self.shared, at, &mut now);
            forgotten.dropped = true;
            forgotten.changed |= now != loaded.source;
        }
        let m = self.memories.get_mut(&memory).expect("the memory is there");
        for at in overlapping {
            drop_piece(m, at, self.words.empty);
        }
        forgotten
    }

    /// Drops the pieces of a range the kernel changed, counting "code changed after it ran" when the guest code
    /// of one is no longer what it was translated from.
    fn forget_changed_code(&mut self, memory: MemoryId, address: u64, length: u64) {
        if self.forget_code(memory, address, length).changed {
            self.slow(CODE_CHANGED_AFTER_IT_RAN, 1, address);
        }
    }

    /// The guest said the code at `address` changed: drops what was translated from its line, in every memory
    /// that shows the line's frame.
    fn code_line_changed(&mut self, memory: MemoryId, address: u64) {
        let line = address & !(CODE_LINE - 1);
        let mut lines = Vec::from([(memory, line)]);
        if let Some(&shown) = self.memories[&memory].shown.get(&line) {
            let offset = shown.offset(line);
            for (&other, m) in &self.memories {
                lines.extend(
                    m.shown
                        .iter()
                        .filter(|(_, s)| s.block == shown.block)
                        .map(|(range, s)| (range, offset.wrapping_sub(s.origin)))
                        .filter(|(range, at)| range.contains(at))
                        .map(|(_, at)| (other, at)),
                );
            }
        }
        lines.sort_unstable();
        lines.dedup();
        for (other, line) in lines {
            if self.forget_code(other, line, CODE_LINE).dropped {
                self.slow(CODE_CHANGED_AFTER_IT_RAN, 1, line);
            }
        }
    }

    /// Whether the guest may read the byte at `address` (an announcement of changed code checks it).
    fn readable(&self, memory: MemoryId, address: u64) -> bool {
        self.check(address, 1).is_ok()
            && self.memories[&memory]
                .allows(page_of(address))
                .contains(Permissions::READ)
    }

    /// Drops every loaded piece and starts the code area again after its routines. Only between pieces: no
    /// piece is running, and none is linked to another.
    fn empty_code_area(&mut self) {
        let mut dropped = 0;
        for m in self.memories.values_mut() {
            dropped += m.pieces.len() as u64;
            let all: Vec<u64> = m.pieces.keys().copied().collect();
            for at in all {
                drop_piece(m, at, self.words.empty);
            }
        }
        self.code_next = Region::Code.base() + PAGE_SIZE;
        self.slow(CODE_AREA_EMPTIED, 1, dropped);
    }

    /// Gives every live memory a new identifier after one invalidation of every identifier, and the next free
    /// one, or none when there are more memories than identifiers.
    fn new_generation(&mut self) -> Option<u16> {
        barrier::dsb(ISHST);
        (self.invalidate_whole_tlb)();
        barrier::dsb(ISH);
        barrier::isb(SY);
        let mut next = IDLE_IDENTIFIER;
        for m in self.memories.values_mut() {
            next = next.checked_add(1)?;
            m.space.asid = next;
        }
        self.slow(IDENTIFIERS_RAN_OUT, 1, self.memories.len() as u64);
        next.checked_add(1)
    }

    fn save_vectors(&mut self, thread: ThreadId) {
        let mut vectors: Vectors = Default::default();
        (self.save_vectors)(&mut vectors);
        if let Some(t) = self.threads.get_mut(&thread) {
            t.vectors = vectors;
        }
    }

    /// Gives `thread` the hardware's vector registers.
    fn take_hardware(&mut self, thread: ThreadId, vectors: &Vectors) {
        if self.live == Some(thread) {
            return;
        }
        if let Some(previous) = self.live {
            self.save_vectors(previous);
        }
        (self.load_vectors)(vectors);
        self.live = Some(thread);
    }

    /// Writes host code into the code area and makes it visible, giving its address.
    fn place(&mut self, code_bytes: &[u8]) -> Result<u64, Full> {
        let start = self.code_next;
        let end = start + code_bytes.len() as u64;
        let global = self.global;
        while self.code_mapped < end {
            let at = self.code_mapped;
            with_frames(|frames| {
                let frame = frames.take()?;
                global.map(frames, at, frame, code())
            })?;
            self.code_mapped += PAGE_SIZE;
        }
        // SAFETY: the code area is mapped writable at this level, and nothing runs this range yet.
        unsafe {
            ptr::copy_nonoverlapping(code_bytes.as_ptr(), start as *mut u8, code_bytes.len())
        };
        make_code_visible(start, end - start);
        self.code_next = end;
        Ok(start)
    }

    /// Makes sure a piece starting at guest `pc` is loaded, or gives the exit that running from there is.
    fn load(&mut self, memory: MemoryId, pc: u64) -> Result<(), Exit> {
        let m = &self.memories[&memory];
        if m.pieces.contains_key(&pc) {
            return Ok(());
        }
        if !pc.is_multiple_of(size_of::<u32>() as u64) {
            return Err(Exit::AlignmentFault { address: pc });
        }
        let refused = Exit::MemoryFault {
            address: pc,
            access: Access::Execute,
        };
        if !m.allows(page_of(pc)).contains(Permissions::EXECUTE) {
            return Err(refused);
        }
        // The rest of the page, as far as one link carries: a piece never runs on into another page, whose
        // permissions may differ.
        let rest = (page_of(pc) + PAGE_SIZE - pc) as usize;
        let mut code = vec![0; rest.min(LINK_BYTES)];
        m.read(&self.shared, pc, &mut code);
        let piece = floor::ask_link(
            &Translate {
                address: pc,
                exact: u64::from(self.exact_replay),
            },
            &code,
            translation::piece,
        )
        .ok_or(Exit::IllegalInstruction)?;
        self.slow(NOT_YET_TRANSLATED, 1, pc);
        let base = match self.place(&piece.code) {
            Ok(base) => base,
            Err(Full) => {
                self.empty_code_area();
                // Guest-physical memory has no room for one piece: the process can only be killed.
                self.place(&piece.code)
                    .map_err(|Full| Exit::MemoryFaultWithoutRegisters)?
            }
        };
        for reason in NativeReason::ALL {
            let sites = piece.sites[reason.index()];
            if sites != 0 {
                self.slow(reason.name(), sites, pc);
            }
        }
        // Code placed out of line comes after the run's last instruction, so the run ends at its highest guest.
        let covered = piece
            .spans
            .iter()
            .map(|span| span.guest + size_of::<u32>() as u64 - pc)
            .max()
            .unwrap_or(0);
        code.truncate(covered as usize);
        let site = |start: u32, retarget: Option<u32>| {
            let from = start as usize;
            let word = &piece.code[from..from + size_of::<u32>()];
            Site {
                at: base + u64::from(start),
                word: u32::from_le_bytes(word.try_into().expect("a word")),
                retarget,
            }
        };
        let mut sites: Vec<(u64, Site)> = piece
            .exits
            .iter()
            .map(|exit| (exit.target, site(exit.start, exit.retarget)))
            .collect();
        // Each branch of a known jump's table is an exit to its target, linked as a conditional branch's is; one no
        // piece can be at stays on the jump through the slot.
        for table in &piece.tables {
            for k in 0..table.entries {
                let target = table.target(k);
                if target >> REACH_BITS == 0 && target.is_multiple_of(size_of::<u32>() as u64) {
                    let start = table.start + k * size_of::<u32>() as u32;
                    sites.push((target, site(start, Some(table.retarget))));
                }
            }
        }
        let m = self.memories.get_mut(&memory).expect("the memory is there");
        m.pieces.insert(
            pc,
            Loaded {
                base,
                length: piece.code.len(),
                spans: piece.spans,
                source: code,
                exits: sites
                    .iter()
                    .map(|(target, site)| (*target, site.at))
                    .collect(),
            },
        );
        m.hosts.insert(base, pc);
        for &(target, site) in &sites {
            m.exits_to.entry(target).or_default().push(site);
        }
        // No frame for a page of the mirror: the process can only be killed.
        self.fill_slot(memory, pc, base)
            .map_err(|Full| Exit::MemoryFaultWithoutRegisters)?;
        let m = &self.memories[&memory];
        let into = m
            .exits_to
            .get(&pc)
            .into_iter()
            .flatten()
            .map(|site| (*site, base));
        let out = sites
            .iter()
            .filter(|(target, _)| *target != pc)
            .filter_map(|(target, site)| Some((*site, m.pieces.get(target)?.base)));
        for (site, to) in into.chain(out).collect::<Vec<_>>() {
            if let Some(word) = self.link_word(&site, to) {
                write_code(site.at, &[word]);
            }
        }
        Ok(())
    }

    /// The word that links `site` to the piece at `to`, when it reaches: a direct branch over it, or its own
    /// branch pointed there.
    fn link_word(&self, site: &Site, to: u64) -> Option<u32> {
        let offset = (to.wrapping_sub(site.at) as i64) / size_of::<u32>() as i64;
        let branch = match site.retarget {
            Some(mask) => Field {
                clear: site.word & !mask,
                mask,
            },
            None => self.words.branch,
        };
        branch.fits(offset).then(|| branch.with(offset as u64))
    }

    /// Makes guest `target` the last target of the return or call whose call into the inside is at host address `call`:
    /// its moves and test are written, and its direct branch is an exit to `target`, linked while a piece is
    /// loaded there. A target no piece can be at is not made one, so the call comes again at the next miss.
    fn make_last_target(&mut self, memory: MemoryId, call: u64, target: u64) {
        if target >> REACH_BITS != 0 || !target.is_multiple_of(size_of::<u32>() as u64) {
            return;
        }
        let word = |index: usize| call - ((LAST_TARGET_CALL - index) * size_of::<u32>()) as u64;
        let site = Site {
            at: word(LAST_TARGET_BRANCH),
            word: self.words.last_target_branch,
            retarget: None,
        };
        let m = &self.memories[&memory];
        let link = m
            .pieces
            .get(&target)
            .and_then(|piece| self.link_word(&site, piece.base));
        let holder = m
            .hosts
            .range(..=call)
            .next_back()
            .map(|(_, &guest)| guest)
            .expect("a call into the inside is in a loaded piece");
        let mut moves = [0; SLOT_MOVES];
        let fields = core::iter::once(&self.words.last_target_first).chain(&self.words.moves[1..]);
        for (k, (move_word, field)) in moves.iter_mut().zip(fields).enumerate() {
            *move_word = field.with(target >> (k as u32 * SLOT_MOVE_BITS));
        }
        write_code(word(0), &moves);
        write_code(word(LAST_TARGET_TEST), &[self.words.last_target_test]);
        if let Some(linked) = link {
            write_code(site.at, &[linked]);
        }
        let m = self.memories.get_mut(&memory).expect("the memory is there");
        m.pieces
            .get_mut(&holder)
            .expect("the holder is loaded")
            .exits
            .push((target, site.at));
        m.exits_to.entry(target).or_default().push(site);
    }

    /// Fills the entry slot of guest address `guest` in `memory` with a jump to the piece at `base`, mapping the
    /// slot's page at its first use; leaves `memory`'s address space switched to.
    fn fill_slot(&mut self, memory: MemoryId, guest: u64, base: u64) -> Result<(), Full> {
        let at = mirror::slot(guest);
        let page = page_of(at);
        let m = self.memories.get_mut(&memory).expect("the memory is there");
        if !m.slots.contains_key(&page) {
            let inside = m.space.inside;
            let frame = with_frames(|frames| {
                let frame = frames.take()?;
                inside.map(frames, page, frame, process_code())?;
                Ok(frame)
            })?;
            m.slots.insert(page, frame);
        }
        let mut words = [self.words.jump; SLOT_MOVES + 1];
        for (k, (word, field)) in words.iter_mut().zip(&self.words.moves).enumerate() {
            *word = field.with(base >> (k as u32 * SLOT_MOVE_BITS));
        }
        // Written where the guest runs it, in its process's address space, which dropping another process's
        // pieces may have switched away from.
        m.space.switch();
        write_code(at, &words);
        Ok(())
    }

    /// Runs `thread` from `resume` to its next exit, or to the first instruction boundary where its count has
    /// reached `until`, which is [`Exit::Stop`]. `clock` reads the monotonic clock for the guest's counter reads.
    /// An interrupt is [`Exit::Stop`], left pending for the caller to take.
    pub fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        clock: &mut dyn FnMut() -> u64,
        until: Option<u64>,
    ) -> Exit {
        let mut t = self.threads.remove(&thread).expect("a thread of this side");
        if let Resume::Return(value) = resume {
            t.state.x[0] = value;
        }
        self.take_hardware(thread, &t.vectors);
        let mut not_kept = 0;
        let exit = self.run_taken(&mut t, clock, until, &mut not_kept);
        // The guest's level writes its thread pointer itself.
        t.tpidr = TPIDR_EL0.get();
        if not_kept != 0 {
            self.slow(EXACT_FLOATING_POINT_REQUESTED, not_kept, t.state.pc);
        }
        self.stops += 1;
        if !matches!(exit, Exit::MemoryFaultWithoutRegisters) {
            self.positions.write_u64(t.state.count);
            self.positions.write_u64(t.state.pc);
            self.last_count = t.state.count;
        }
        self.threads.insert(thread, t);
        exit
    }

    /// How many times a thread stopped, and a hash of each stop's position (its count and pc) in order: the
    /// same under every look-ahead, since the count is defined on the guest's instructions alone. A stop where
    /// guest-physical memory ran out has no position in it: where that happens depends on what the inside holds
    /// for itself, which the look-ahead changes.
    pub fn positions(&self) -> (u64, u64) {
        (self.stops, self.positions.finish())
    }

    /// The count of the thread that stopped last, at that stop: a one-thread program's count at its end.
    pub fn last_count(&self) -> u64 {
        self.last_count
    }

    /// Hands `warn` every slow path taken since it was last called.
    pub fn send_warnings(&mut self, warn: &mut dyn FnMut(Warning<'_>)) {
        let (asked, request) = floor::asked();
        let floor_counts = [
            (ASKED_THE_HOST, asked, request, &mut self.asked_reported),
            (
                GUEST_PHYSICAL_SPACE_FULL,
                floor::space_full(),
                0,
                &mut self.full_reported,
            ),
            (
                LARGE_ENTRY_SPLIT,
                floor::tables::large_entry_splits(),
                0,
                &mut self.splits_reported,
            ),
        ];
        for (reason, count, detail, reported) in floor_counts {
            if count != *reported {
                *reported = count;
                self.counters.insert(reason, count);
                self.unsent.insert(reason, (count, detail));
            }
        }
        for (reason, (count, detail)) in core::mem::take(&mut self.unsent) {
            warn(Warning {
                reason,
                count,
                detail,
            });
        }
    }

    /// The progress count of `thread` so far.
    fn count(&self, thread: ThreadId) -> u64 {
        self.threads[&thread].state.count
    }

    /// Runs `t` to its next exit or to `until`, adding to `not_kept` each read of the float status while its
    /// flags are not kept. A turn that ends at a count runs one host instruction at a time, so that it ends at
    /// the first instruction boundary where the count has reached it, however pieces are linked.
    fn run_taken(
        &mut self,
        t: &mut Thread,
        clock: &mut dyn FnMut() -> u64,
        until: Option<u64>,
        not_kept: &mut u64,
    ) -> Exit {
        let memory = t.memory;
        self.memories[&memory].space.switch();
        TPIDR_EL0.set(t.tpidr);
        // MDSCR_EL1 is the hypervisor's to answer, so it is written only for a stepped turn.
        if until.is_some() {
            self.slow(TURN_LIMIT_STEPPED, 1, t.state.pc);
            MDSCR_EL1.modify(MDSCR_EL1::SS::SoftwareStepEnabled);
        }
        let exit = self.run_pieces(t, clock, until, not_kept);
        if until.is_some() {
            MDSCR_EL1.modify(MDSCR_EL1::SS::SoftwareStepDisabled);
        }
        exit
    }

    fn run_pieces(
        &mut self,
        t: &mut Thread,
        clock: &mut dyn FnMut() -> u64,
        until: Option<u64>,
        not_kept: &mut u64,
    ) -> Exit {
        let memory = t.memory;
        let step = if until.is_some() {
            SPSR_EL1::SS::SET.value
        } else {
            SPSR_EL1::SS::CLEAR.value
        };
        let reached = |state: &GuestState| until.is_some_and(|until| state.count >= until);
        // Where the guest's level goes on in host code, when the inside answered a call without a stop.
        let mut carried: Option<(Interrupted, RegisterPage)> = None;
        loop {
            let pc = t.state.pc;
            let resumed = carried.take();
            if resumed.is_none()
                && let Err(exit) = self.load(memory, pc)
            {
                return exit;
            }
            let m = &self.memories[&memory];
            let blocks = &self.shared;
            let (mut hardware, mut page) = resumed.unwrap_or_else(|| {
                let (mut hardware, page) =
                    stop::rejoin(&t.state, t.page.clone(), &t.float, t.register_page);
                hardware.pc = m.pieces[&pc].base;
                (hardware, page)
            });
            let ended = loop {
                write_page(t.page_frame, &page);
                let mut user = UserRegisters {
                    x: hardware.x,
                    sp: hardware.sp,
                    pc: hardware.pc,
                    pstate: hardware.state | step,
                };
                (self.write_float_control)(u64::from(hardware.fpsr), u64::from(hardware.fpcr));
                let taken = enter_user(&mut user);
                let control = (self.read_float_control)();
                let h = Interrupted {
                    x: user.x,
                    sp: user.sp,
                    pc: user.pc,
                    state: user.pstate,
                    fpsr: control.fpsr as u32,
                    fpcr: control.fpcr as u32,
                };
                let held = read_page(t.page_frame);
                let mut memory_of = |at: u64, into: &mut [u8]| m.read(blocks, at, into);
                let piece = m.piece_at(h.pc);
                let syndrome = ESR_EL1.get();
                let stepped =
                    ESR_EL1::EC.read(syndrome) == ESR_EL1::EC::Value::SoftwareStepLowerEL as u64;
                match taken {
                    Taken::Synchronous if stepped => {
                        let mut float = t.float;
                        let state =
                            stop::interrupted(&piece, &h, &held, &mut float, &mut memory_of)
                                .expect("a step lands in a span");
                        if reached(&state) {
                            t.state = state;
                            t.float = float;
                            t.page = held;
                            break Ended::TurnOver;
                        }
                        hardware = h;
                        page = held;
                    }
                    Taken::Synchronous => {
                        let exception = Exception {
                            syndrome,
                            fault: FAR_EL1.get(),
                            hardware: &h,
                            page: &held,
                        };
                        match stop::resolve(&piece, &exception, &mut t.float, &mut memory_of) {
                            Ok(Resolved::Again(next, next_page)) => {
                                hardware = next;
                                page = next_page;
                            }
                            Ok(Resolved::FloatStatusNotKept(next, next_page)) => {
                                hardware = next;
                                page = next_page;
                                *not_kept += 1;
                            }
                            Ok(Resolved::Clock {
                                into,
                                then,
                                page: then_page,
                            }) => {
                                hardware = then;
                                page = then_page;
                                if let Some(register) = into {
                                    stop::set_register(register, clock(), &mut hardware, &mut page);
                                }
                            }
                            Ok(Resolved::CodeChanged {
                                address,
                                at,
                                then,
                                page: then_page,
                            }) => {
                                if !self.readable(memory, address) {
                                    t.state = *at;
                                    t.page = held;
                                    break Ended::Stopped(Stop::MemoryFault {
                                        address,
                                        write: false,
                                    });
                                }
                                // The guest goes on after the announcement, in a piece translated again.
                                t.state = stop::interrupted(
                                    &m.piece_at(then.pc),
                                    &then,
                                    &then_page,
                                    &mut t.float,
                                    &mut memory_of,
                                )
                                .expect("an announcement is followed by a span");
                                t.page = then_page;
                                break Ended::CodeChanged(address);
                            }
                            Ok(Resolved::LastTarget {
                                call,
                                target,
                                then,
                                page: then_page,
                            }) => {
                                carried = Some((then, then_page));
                                break Ended::LastTarget { call, target };
                            }
                            Ok(Resolved::Stop(stop, state)) => {
                                t.state = state;
                                t.page = held;
                                break Ended::Stopped(stop);
                            }
                            Err(unexpected) => {
                                panic!("an exception translated code never raises: {unexpected:?}")
                            }
                        }
                    }
                    Taken::Interrupt => {
                        t.state =
                            stop::interrupted(&piece, &h, &held, &mut t.float, &mut memory_of)
                                .expect("an interrupt lands in a span");
                        t.page = held;
                        break Ended::TurnOver;
                    }
                    Taken::Other => {
                        panic!("a fast interrupt or a system error from the guest's level")
                    }
                }
            };
            let stop = match ended {
                Ended::TurnOver => return Exit::Stop,
                Ended::CodeChanged(address) => {
                    self.code_line_changed(memory, address);
                    // Dropping another process's pieces switched to its address space.
                    self.memories[&memory].space.switch();
                    if reached(&t.state) {
                        return Exit::Stop;
                    }
                    continue;
                }
                Ended::LastTarget { call, target } => {
                    self.make_last_target(memory, call, target);
                    continue;
                }
                Ended::Stopped(stop) => stop,
            };
            match stop {
                Stop::Landed(_) if reached(&t.state) => return Exit::Stop,
                Stop::Landed(_) => {}
                Stop::Syscall => {
                    let mut call = Syscall {
                        number: t.state.x[SYSCALL_NUMBER],
                        arguments: Default::default(),
                    };
                    let arguments = call.arguments.len();
                    call.arguments.copy_from_slice(&t.state.x[..arguments]);
                    return Exit::Syscall(call);
                }
                Stop::Breakpoint(comment) => return Exit::Breakpoint { comment },
                Stop::Illegal => return Exit::IllegalInstruction,
                Stop::Alignment(address) => return Exit::AlignmentFault { address },
                Stop::MemoryFault { address, write } => {
                    match self.first_touch(memory, address, write) {
                        // The same instruction runs again, over the page's new frame.
                        Ok(true) => {}
                        Ok(false) => {
                            self.slow(MEMORY_FAULT, 1, address);
                            return Exit::MemoryFault {
                                address,
                                access: if write { Access::Write } else { Access::Read },
                            };
                        }
                        // No frame for a page the guest may touch: the process can only be killed.
                        Err(_) => return Exit::MemoryFaultWithoutRegisters,
                    }
                }
            }
        }
    }
}

/// How a run of translated code came back to the inside.
enum Ended {
    /// An interrupt, or a step that reached the turn's count: the turn ends here.
    TurnOver,
    /// The guest announced that the code at this address changed.
    CodeChanged(u64),
    /// A jump's first miss: its last target is to be made, and the guest's level goes on where it was carried.
    LastTarget {
        call: u64,
        target: u64,
    },
    Stopped(Stop),
}

/// Drops the piece at guest `at` of `m`: its entry slot emptied (its first word made `empty`), the exits linked to it unlinked, its own exits
/// forgotten. Only between runs of translated code.
fn drop_piece(m: &mut Memory, at: u64, empty: u32) {
    let Some(loaded) = m.pieces.remove(&at) else {
        return;
    };
    m.hosts.remove(&loaded.base);
    let slot = mirror::slot(at);
    if m.slots.contains_key(&page_of(slot)) {
        // The slot is written where the guest runs it, in its process's address space; every run switches to
        // its own.
        m.space.switch();
        write_code(slot, &[empty]);
    }
    for site in m.exits_to.get(&at).into_iter().flatten() {
        write_code(site.at, &[site.word]);
    }
    for (target, site) in loaded.exits {
        if let Some(sites) = m.exits_to.get_mut(&target) {
            sites.retain(|s| s.at != site);
            if sites.is_empty() {
                m.exits_to.remove(&target);
            }
        }
    }
}

fn write_page(frame: u64, page: &RegisterPage) {
    // SAFETY: the register page's frame is the thread's; `RegisterPage` is `#[repr(C)]` words.
    unsafe { ptr::write_volatile(linear(frame) as *mut RegisterPage, page.clone()) };
}

fn read_page(frame: u64) -> RegisterPage {
    // SAFETY: as in `write_page`.
    unsafe { ptr::read_volatile(linear(frame) as *const RegisterPage) }
}

impl Cpu for NativeCpu {
    fn address_space(&self) -> Addresses {
        Addresses {
            end: 1 << REACH_BITS,
        }
    }

    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        let asid = match self.next_asid {
            Some(asid) => asid,
            None => self.new_generation().ok_or(CpuError::OutOfMemory)?,
        };
        self.next_asid = asid.checked_add(1);
        let global = self.global;
        let space = with_frames(|frames| AddressSpace::new(frames, &global, asid))
            .map_err(out_of_memory)?;
        let id = MemoryId(self.id());
        self.memories.insert(id, Memory::new(space));
        Ok(id)
    }

    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        let copy = self.memory_new()?;
        let copied = self.copy_memory(source, copy);
        match copied {
            Ok(copied) => {
                self.slow(MEMORY_COPIED_AT_FORK, 1, copied);
                Ok(copy)
            }
            Err(error) => {
                self.memory_drop(copy);
                Err(error)
            }
        }
    }

    fn memory_drop(&mut self, memory: MemoryId) {
        let Some(m) = self.memories.remove(&memory) else {
            return;
        };
        // Its tables may be the ones switched to; its identifier is not used again before a new generation.
        self.idle.switch();
        let global = self.global;
        self.runs.extend(m.reserved.values().map(|r| r.run));
        with_frames(|frames| {
            let reserved = |at: u64, frame: u64| {
                let base = block_of(at);
                m.reserved
                    .get(&base)
                    .is_some_and(|r| r.holds(at - base, frame))
            };
            for (&at, page) in m.pages.iter().filter(|(_, p)| p.shared.is_none()) {
                if !reserved(at, page.frame) {
                    frames.give(page.frame);
                }
            }
            for &frame in m.slots.values() {
                frames.give(frame);
            }
            m.space.user.free(frames, |_| true);
            m.space.inside.free(frames, |top| {
                global.top_entry(top) != m.space.inside.top_entry(top)
            });
        });
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        self.check(address, into.len() as u64)?;
        self.memories
            .get(&memory)
            .ok_or(CpuError::BadAddress)?
            .read(&self.shared, address, into);
        Ok(())
    }

    fn write(&mut self, memory: MemoryId, mut address: u64, from: &[u8]) -> Result<(), CpuError> {
        self.check(address, from.len() as u64)?;
        let mut done = 0;
        while done < from.len() {
            let offset = (address - page_of(address)) as usize;
            let take = (PAGE_SIZE as usize - offset).min(from.len() - done);
            let frame = self.frame_for(memory, page_of(address))?;
            frame_bytes(frame)[offset..offset + take].copy_from_slice(&from[done..done + take]);
            done += take;
            address += take as u64;
        }
        Ok(())
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        self.check(address, length)?;
        let end = address + length;
        let whole = |page: u64| address <= page && page + PAGE_SIZE <= end;
        let touched = page_of(address)..end.next_multiple_of(PAGE_SIZE);
        self.unshare(memory, touched.clone(), whole)?;
        self.unback_blocks(memory, &(address..end))?;
        let backed: Vec<(u64, u64)> = self
            .memory(memory)?
            .pages
            .range(touched)
            .map(|(&at, p)| (at, p.frame))
            .collect();
        for (at, frame) in backed {
            if whole(at) {
                self.unback(memory, at)?;
            } else {
                let (from, to) = (address.max(at) - at, end.min(at + PAGE_SIZE) - at);
                frame_bytes(frame)[from as usize..to as usize].fill(0);
            }
        }
        Ok(())
    }

    fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        self.check(address, length)?;
        if length == 0 {
            return Ok(());
        }
        if !permissions.contains(Permissions::EXECUTE) {
            self.forget_changed_code(memory, address, length);
        }
        let range = address..address + length;
        let m = self.memory(memory)?;
        let (table, now) = (m.space.user, attributes(permissions));
        let blocks: Vec<u64> = whole_blocks(&range)
            .filter(|&base| table.is_block(base))
            .collect();
        let was: Vec<(u64, El1Attributes)> = m
            .pages
            .range(range.clone())
            .filter(|&(&at, _)| blocks.binary_search(&block_of(at)).is_err())
            .map(|(&at, _)| (at, attributes(m.allows(at))))
            .collect();
        let reserved: Vec<u64> = m
            .reserved
            .range(block_of(range.start)..range.end)
            .map(|(&base, _)| base)
            .collect();
        if permissions.is_empty() {
            m.allowed.remove(range.clone());
        } else {
            m.allowed.insert(range.clone(), permissions);
        }
        for base in blocks {
            table.protect_block(base, now);
        }
        for (at, before) in was {
            if before != now {
                with_frames(|frames| table.protect(frames, at, now)).map_err(out_of_memory)?;
            }
        }
        for base in reserved {
            self.promote(memory, base)?;
        }
        Ok(())
    }

    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, _starts: &[u64]) {
        self.forget_changed_code(memory, address, length);
    }

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        let id = SharedId(self.id());
        self.shared
            .insert(id, vec![None; length.div_ceil(PAGE_SIZE) as usize]);
        Ok(id)
    }

    fn shared_map(
        &mut self,
        shared: SharedId,
        offset: u64,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.check(address, length)?;
        let block = self.shared.get(&shared).ok_or(CpuError::BadAddress)?;
        let first = (offset / PAGE_SIZE) as usize;
        let frames: Vec<Option<u64>> = block
            .get(first..first + (length / PAGE_SIZE) as usize)
            .ok_or(CpuError::BadAddress)?
            .to_vec();
        if length == 0 {
            return Ok(());
        }
        for (at, frame) in pages(address, length).zip(frames) {
            let own = self.memory(memory)?.pages.get(&at).copied();
            match frame {
                Some(frame) => {
                    self.replace(
                        memory,
                        at,
                        Page {
                            frame,
                            shared: Some(shared),
                        },
                    )?;
                    if let Some(Page {
                        frame,
                        shared: None,
                    }) = own
                    {
                        self.release(memory, at, frame);
                    }
                }
                None => self.unback(memory, at)?,
            }
        }
        self.memory(memory)?.shown.insert(
            address..address + length,
            Shown {
                block: shared,
                origin: offset.wrapping_sub(address),
            },
        );
        self.forget_changed_code(memory, address, length);
        Ok(())
    }

    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.check(address, length)?;
        self.unshare(memory, address..address + length, |_| false)
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        let block = self.shared.get(&shared).ok_or(CpuError::BadAddress)?;
        let mut done = 0;
        while done < into.len() {
            let at = offset + done as u64;
            let within = (at % PAGE_SIZE) as usize;
            let take = (PAGE_SIZE as usize - within).min(into.len() - done);
            let part = &mut into[done..done + take];
            match *block
                .get((at / PAGE_SIZE) as usize)
                .ok_or(CpuError::BadAddress)?
            {
                Some(frame) => part.copy_from_slice(&frame_bytes(frame)[within..within + take]),
                None => part.fill(0),
            }
            done += take;
        }
        Ok(())
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        let mut done = 0;
        while done < from.len() {
            let at = offset + done as u64;
            let within = (at % PAGE_SIZE) as usize;
            let take = (PAGE_SIZE as usize - within).min(from.len() - done);
            let part = &from[done..done + take];
            done += take;
            // A page with no frame reads as zeros already: zeros written to it take no frame.
            let absent = self
                .shared
                .get(&shared)
                .and_then(|block| block.get((at / PAGE_SIZE) as usize))
                .is_some_and(Option::is_none);
            if absent && part.iter().all(|&byte| byte == 0) {
                continue;
            }
            let frame = self.block_frame(shared, at)?;
            frame_bytes(frame)[within..within + take].copy_from_slice(part);
        }
        Ok(())
    }

    fn shared_drop(&mut self, shared: SharedId) {
        if let Some(block) = self.shared.remove(&shared) {
            with_frames(|frames| {
                block
                    .into_iter()
                    .flatten()
                    .for_each(|frame| frames.give(frame))
            });
        }
    }

    fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        let id = ThreadId(self.id());
        let register_page = Region::Threads.base() + u64::from(id.0) * PAGE_SIZE;
        let inside = self.memory(memory)?.space.inside;
        let page_frame = with_frames(|frames| {
            let frame = frames.take()?;
            inside.map(frames, register_page, frame, floor::register_page())?;
            Ok(frame)
        })
        .map_err(out_of_memory)?;
        self.threads.insert(
            id,
            Box::new(Thread {
                memory,
                state: GuestState {
                    x: [0; GENERAL],
                    sp: 0,
                    pc: 0,
                    flags: 0,
                    count: 0,
                    exclusive: Default::default(),
                },
                float: FloatStatus::default(),
                page: RegisterPage::default(),
                register_page,
                page_frame,
                vectors: Default::default(),
                tpidr: 0,
            }),
        );
        Ok(id)
    }

    fn thread_drop(&mut self, thread: ThreadId) {
        let Some(t) = self.threads.remove(&thread) else {
            return;
        };
        if self.live == Some(thread) {
            self.live = None;
        }
        if let Some(m) = self.memories.get(&t.memory) {
            let inside = m.space.inside;
            inside.unmap(t.register_page);
        }
        with_frames(|frames| frames.give(t.page_frame));
    }

    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        if self.live == Some(thread) {
            self.save_vectors(thread);
        }
        let t = &self.threads[&thread];
        into.x = t.state.x;
        into.sp = t.state.sp;
        into.pc = t.state.pc;
        into.pstate = t.state.flags;
        into.v = t.vectors;
        into.fpsr = t.float.fpsr;
        into.fpcr = t.float.fpcr;
        into.tpidr_el0 = t.tpidr;
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        let t = self
            .threads
            .get_mut(&thread)
            .expect("a thread of this side");
        t.state.x = from.x;
        t.state.sp = from.sp;
        t.state.pc = from.pc;
        t.state.flags = from.pstate;
        t.vectors = from.v;
        t.float.fpsr = from.fpsr;
        t.float.fpcr = from.fpcr & FPCR_BITS;
        t.tpidr = from.tpidr_el0;
        if self.live == Some(thread) {
            (self.load_vectors)(&from.v);
        }
    }
}

/// What the native back end refuses to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// A turn that ends after a number of units of progress, for a process whose pieces carry no count.
    TurnLimitWithoutCount,
}

impl NativeCpu {
    /// The count at which `turn`, starting on `thread` now, ends.
    fn turn_end(&self, thread: ThreadId, turn: Turn) -> Result<Option<u64>, Refused> {
        match turn {
            Turn::Counted { limit: None } | Turn::Free => Ok(None),
            Turn::Counted { limit: Some(_) } if !self.exact_replay => {
                Err(Refused::TurnLimitWithoutCount)
            }
            Turn::Counted { limit: Some(limit) } => Ok(Some(self.count(thread) + limit)),
        }
    }
}

impl Execution for NativeCpu {
    type Failure = Refused;

    /// Counter reads are told no progress; the progress made before one is handed with the exit that follows.
    fn run(
        &mut self,
        thread: ThreadId,
        mut resume: Resume,
        turn: Turn,
        answer: &mut dyn Answer,
    ) -> Result<(), Refused> {
        let mut until = self.turn_end(thread, turn)?;
        loop {
            let before = self.count(thread);
            let exit = NativeCpu::run(self, thread, resume, &mut || answer.clock(0), until);
            self.send_warnings(&mut |warning| answer.warn(warning));
            let progress = self.count(thread) - before;
            match answer.exit(self, exit, progress) {
                Carry::On {
                    resume: next,
                    turn: next_turn,
                } => {
                    until = self.turn_end(thread, next_turn)?;
                    resume = next;
                }
                Carry::Leave(_) => return Ok(()),
            }
        }
    }

    fn restored(&self) -> Self {
        unimplemented!("a save of the native back end is N7's")
    }
}
