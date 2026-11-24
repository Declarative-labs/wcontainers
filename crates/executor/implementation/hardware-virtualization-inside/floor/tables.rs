//! Frames of guest-physical memory and the translation tables written over them.

use alloc::vec::Vec;
use core::ops::Range;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use aarch64_cpu::asm::barrier::{self, ISH, ISHST, SY};

use aarch64_paging::descriptor::{El1Attributes, VirtualAddress};
use aarch64_paging::paging::{El1And0, TranslationRegime};
use wcontainers_abi::PAGE_SIZE;

use crate::layout::{
    BLOCK_LEVEL, BLOCK_SIZE, ENTRIES, ENTRY_BYTES, LEAF_LEVEL, TOP_LEVEL, level_index, linear,
};

/// Width of the output address field of a descriptor.
const OUTPUT_ADDRESS_BITS: u32 = 48;

/// The output address bits of a descriptor.
const OUTPUT_ADDRESS: u64 = ((1 << OUTPUT_ADDRESS_BITS) - 1) & !(PAGE_SIZE - 1);

/// Blocks broken into pages because one page of them changed: the slow path "large entry split".
static LARGE_ENTRY_SPLITS: AtomicU64 = AtomicU64::new(0);

/// How many times a block was broken into pages.
pub fn large_entry_splits() -> u64 {
    LARGE_ENTRY_SPLITS.load(Ordering::Relaxed)
}

/// Requests for a frame refused because guest-physical memory is used up: the slow path "guest-physical
/// space full".
static SPACE_FULL: AtomicU64 = AtomicU64::new(0);

/// How many requests for a frame were refused.
pub fn space_full() -> u64 {
    SPACE_FULL.load(Ordering::Relaxed)
}

/// Guest-physical memory is used up: the request for a frame fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Full;

/// Counts a refused request.
fn full() -> Full {
    SPACE_FULL.fetch_add(1, Ordering::Relaxed);
    Full
}

/// The unit of Linux's free-memory watermark formula, a kibibyte.
const WATERMARK_UNIT: u64 = 1 << 10;

/// Linux keeps free the square root of this many times its memory in kibibytes, in kibibytes
/// (mm/page_alloc.c, `calculate_min_free_kbytes`).
const WATERMARK_SCALE: u64 = 16;

/// The bytes at the end of `memory` bytes of guest-physical memory that only the inside may take, so its heap
/// can still grow when the guest's memory is full: Linux's minimum free-memory watermark, in whole blocks.
const fn kept_for_inside(memory: u64) -> u64 {
    ((memory / WATERMARK_UNIT * WATERMARK_SCALE).isqrt() * WATERMARK_UNIT)
        .next_multiple_of(BLOCK_SIZE)
}

/// Frames of guest-physical memory, given out one page (or one aligned block) at a time.
pub struct Frames {
    next: u64,
    end: u64,
    /// Frames given back, taken before those never used. Kept here, not in the frames, so that a frame
    /// someone still writes cannot choose the next frame given out. Its room for every frame is made by
    /// [`Frames::keep_free_list_in`] before any is given back: it never grows under the frames' lock, which the
    /// heap takes to grow.
    free: Vec<u64>,
    /// Frames given back before the free list is kept (the heap's first growth, which makes the list's room,
    /// gives back the frames below its block): one run, taken last, once no other frame is left, so that the
    /// frames after it still come in a row.
    before_list: Range<u64>,
    /// The bytes before `end` the guest's memory may not take.
    kept: u64,
}

impl Frames {
    /// Frames from `start` (aligned up to a page) to `end`.
    pub const fn new(start: u64, end: u64) -> Frames {
        let next = start.next_multiple_of(PAGE_SIZE);
        Frames {
            next,
            end,
            free: Vec::new(),
            before_list: 0..0,
            kept: kept_for_inside(end.saturating_sub(next)),
        }
    }

    /// How many frames there are at most: the room the free list needs.
    pub fn most(&self) -> usize {
        (self.end.saturating_sub(self.next) / PAGE_SIZE) as usize
    }

    /// Keeps the free list in `list`, empty, with room for [`Frames::most`] frames, made outside the frames' lock.
    pub fn keep_free_list_in(&mut self, list: Vec<u64>) {
        assert!(list.is_empty() && list.capacity() >= self.most(), "room for every frame");
        assert!(self.free.capacity() == 0, "the free list is kept once");
        self.free = list;
    }

    /// Whether a frame for the guest's memory can be taken without the frames kept for the inside.
    pub fn has_room_for_guest(&self) -> bool {
        !self.free.is_empty() || self.next + PAGE_SIZE + self.kept <= self.end
    }

    /// A zeroed frame for the guest's memory, which leaves the frames kept for the inside alone.
    pub fn take_for_guest(&mut self) -> Result<u64, Full> {
        if !self.has_room_for_guest() {
            return Err(full());
        }
        self.take()
    }

    /// A run of frames a block entry can map, for the guest's memory and never used since the host gave it, if
    /// one is left before the frames kept for the inside. None is not a refused request: pages then take frames
    /// one at a time.
    pub fn take_run_for_guest(&mut self) -> Option<u64> {
        let block = self.next.next_multiple_of(BLOCK_SIZE);
        (block + BLOCK_SIZE + self.kept <= self.end)
            .then(|| self.take_blocks(1).expect("checked above"))
    }

    /// A zeroed frame.
    pub fn take(&mut self) -> Result<u64, Full> {
        let frame = if let Some(frame) = self.free.pop() {
            frame
        } else if self.next + PAGE_SIZE <= self.end {
            self.next += PAGE_SIZE;
            self.next - PAGE_SIZE
        } else if !self.before_list.is_empty() {
            self.before_list.start += PAGE_SIZE;
            self.before_list.start - PAGE_SIZE
        } else {
            return Err(full());
        };
        clear(frame);
        Ok(frame)
    }

    /// A run of frames that a block entry can map, never used since the host gave it, so it reads as zeros.
    pub fn take_block(&mut self) -> Result<u64, Full> {
        self.take_blocks(1)
    }

    /// As [`Frames::take_block`], for `count` blocks one after the other.
    pub fn take_blocks(&mut self, count: u64) -> Result<u64, Full> {
        let block = self.next.next_multiple_of(BLOCK_SIZE);
        if block + count * BLOCK_SIZE > self.end {
            return Err(full());
        }
        let mut gap = self.next;
        while gap < block {
            self.give(gap);
            gap += PAGE_SIZE;
        }
        self.next = block + count * BLOCK_SIZE;
        Ok(block)
    }

    /// Takes back one frame. Nothing is written into it: the next `take` zeroes it.
    pub fn give(&mut self, frame: u64) {
        let frame = checked_frame(frame);
        if self.free.capacity() == 0 {
            assert!(
                self.before_list.is_empty() || self.before_list.end == frame,
                "frames given back before the free list is kept are one run"
            );
            self.before_list = if self.before_list.is_empty() {
                frame..frame + PAGE_SIZE
            } else {
                self.before_list.start..frame + PAGE_SIZE
            };
            return;
        }
        assert!(
            self.free.len() < self.free.capacity(),
            "the free list has room for every frame: more were given back than there are"
        );
        self.free.push(frame);
    }
}

/// Zeroes a frame that is ours.
pub fn clear(frame: u64) {
    // SAFETY: the frame is ours and reached through the linear map.
    #[expect(
        unsafe_code,
        reason = "UNSOUND: zeroes whatever frame number this safe pub fn is given, through the linear map; Words over the linear map replaces it"
    )]
    unsafe { ptr::write_bytes(linear(frame) as *mut u8, 0, PAGE_SIZE as usize) };
}

/// A frame number an entry may hold: page aligned and within the output address field, so that no bit of it
/// lands in an attribute.
fn checked_frame(frame: u64) -> u64 {
    assert_eq!(
        frame & !OUTPUT_ADDRESS,
        0,
        "a frame is page aligned and within the output address"
    );
    frame
}

/// The attributes an entry may hold: none in the output address field, where they would change the frame.
fn attributes_only(attributes: El1Attributes) -> El1Attributes {
    El1Attributes::from_bits_retain(attributes.bits() & !(OUTPUT_ADDRESS as usize))
}

/// One set of translation tables for one half, from its top-level table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Table {
    /// The top-level table's guest-physical address: a frame [`Table::new`] took, or the boot map's root.
    root: u64,
}

/// The entry kinds below a table: one that points to the next table, and a page at the leaf level.
fn table_or_page() -> u64 {
    (El1Attributes::VALID | El1Attributes::TABLE_OR_PAGE).bits() as u64
}

fn entry_at(table: u64, va: u64, level: u32) -> *mut u64 {
    (linear(table) + level_index(va, level) * ENTRY_BYTES) as *mut u64
}

fn valid(entry: u64) -> bool {
    entry & El1Attributes::VALID.bits() as u64 != 0
}

impl Table {
    /// The boot map's tables, which the monitor wrote at `root` (crate::boot::write_boot_map). Only the floor's
    /// boot names them.
    pub(crate) fn boot_map(root: u64) -> Table {
        Table {
            root: checked_frame(root),
        }
    }

    /// The top-level table's guest-physical address, for the translation table base registers.
    pub fn root(&self) -> u64 {
        self.root
    }

    /// An empty table.
    pub fn new(frames: &mut Frames) -> Result<Table, Full> {
        Ok(Table {
            root: frames.take()?,
        })
    }

    /// The entry for `va` at `level`, making the tables above it as needed.
    fn slot(&self, frames: &mut Frames, va: u64, level: u32) -> Result<*mut u64, Full> {
        let mut table = self.root;
        for above in 0..level {
            let entry = entry_at(table, va, above);
            // SAFETY: table entries are reached through the linear map.
            #[expect(
                unsafe_code,
                reason = "UNSOUND: reads an entry while walking down in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
            )]
            let value = unsafe { ptr::read_volatile(entry) };
            table = if valid(value) {
                value & OUTPUT_ADDRESS
            } else {
                let next = frames.take()?;
                // SAFETY: as above.
                #[expect(
                    unsafe_code,
                    reason = "UNSOUND: writes an entry pointing at a fresh table frame in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
                )]
                unsafe { ptr::write_volatile(entry, next | table_or_page()) };
                next
            };
        }
        Ok(entry_at(table, va, level))
    }

    /// The entry mapping `va` and its level, if any.
    pub fn find(&self, va: u64) -> Option<(u64, u32)> {
        let mut table = self.root;
        for level in 0..=LEAF_LEVEL {
            // SAFETY: table entries are reached through the linear map.
            #[expect(
                unsafe_code,
                reason = "UNSOUND: reads an entry while finding a mapping in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
            )]
            let value = unsafe { ptr::read_volatile(entry_at(table, va, level)) };
            if !valid(value) {
                return None;
            }
            if level == LEAF_LEVEL || value & El1Attributes::TABLE_OR_PAGE.bits() as u64 == 0 {
                return Some((value, level));
            }
            table = value & OUTPUT_ADDRESS;
        }
        None
    }

    /// The frame holding `va`.
    pub fn frame(&self, va: u64) -> Option<u64> {
        let (entry, level) = self.find(va)?;
        let span_mask = crate::layout::level_span(level) - 1;
        Some((entry & OUTPUT_ADDRESS & !span_mask) + (va & span_mask & !(PAGE_SIZE - 1)))
    }

    /// Maps the page at `va`, which nothing maps, to `frame`. (A mapped page changes through `protect`, which
    /// invalidates.)
    pub fn map(
        &self,
        frames: &mut Frames,
        va: u64,
        frame: u64,
        attributes: El1Attributes,
    ) -> Result<(), Full> {
        let attributes = attributes_only(attributes);
        let entry = checked_frame(frame) | attributes.bits() as u64 | table_or_page();
        let slot = self.slot(frames, va, LEAF_LEVEL)?;
        // SAFETY: the entry is ours, reached through the linear map.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: reads a leaf entry to check it is empty in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        let present = unsafe { ptr::read_volatile(slot) };
        assert!(!valid(present), "mapping a mapped page");
        // SAFETY: as above.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: writes a page entry in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, entry) };
        barrier();
        Ok(())
    }

    /// Maps the block at `va`, which nothing maps, to the block of frames at `block`.
    pub fn map_block(
        &self,
        frames: &mut Frames,
        va: u64,
        block: u64,
        attributes: El1Attributes,
    ) -> Result<(), Full> {
        assert!(block.is_multiple_of(BLOCK_SIZE), "a block entry maps an aligned run");
        let attributes = attributes_only(attributes);
        let entry = (checked_frame(block)
            | attributes.bits() as u64
            | El1Attributes::VALID.bits() as u64)
            & !(El1Attributes::TABLE_OR_PAGE.bits() as u64);
        let slot = self.slot(frames, va, BLOCK_LEVEL)?;
        // SAFETY: as in `map`.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: reads a block entry to check it is empty in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        let present = unsafe { ptr::read_volatile(slot) };
        assert!(!valid(present), "mapping a mapped block");
        // SAFETY: as in `map`.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: writes a block entry in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, entry) };
        barrier();
        Ok(())
    }

    /// Maps `length` bytes at `va`, which nothing maps, to the frames from `frame` on: a block entry wherever a
    /// whole block of the range is aligned at both ends of the mapping, page entries elsewhere.
    pub fn map_range(
        &self,
        frames: &mut Frames,
        va: u64,
        frame: u64,
        length: u64,
        attributes: El1Attributes,
    ) -> Result<(), Full> {
        let end = va + length;
        let mut at = va;
        while at < end {
            let to = frame + (at - va);
            if at.is_multiple_of(BLOCK_SIZE)
                && to.is_multiple_of(BLOCK_SIZE)
                && end - at >= BLOCK_SIZE
            {
                self.map_block(frames, at, to, attributes)?;
                at += BLOCK_SIZE;
            } else {
                self.map(frames, at, to, attributes)?;
                at += PAGE_SIZE;
            }
        }
        Ok(())
    }

    /// Gives the page at `va` new attributes, keeping its frame, and invalidates it for every identifier, so
    /// every table root sharing this table sees the change. A block holding it is first split into pages with the
    /// block's attributes, break before make.
    pub fn protect(
        &self,
        frames: &mut Frames,
        va: u64,
        attributes: El1Attributes,
    ) -> Result<(), Full> {
        let (entry, level) = self.find(va).expect("protecting an unmapped page");
        if level == BLOCK_LEVEL {
            self.split(frames, va, entry)?;
        }
        let frame = self.find(va).expect("split").0 & OUTPUT_ADDRESS;
        let attributes = attributes_only(attributes);
        let slot = self.slot(frames, va, LEAF_LEVEL)?;
        // SAFETY: the entry is ours, reached through the linear map.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: rewrites a page entry with new attributes in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, frame | attributes.bits() as u64 | table_or_page()) };
        invalidate(va);
        Ok(())
    }

    /// Points the mapped page at `va` to another frame with `attributes`, and invalidates it as `protect` does.
    pub fn replace(
        &self,
        frames: &mut Frames,
        va: u64,
        frame: u64,
        attributes: El1Attributes,
    ) -> Result<(), Full> {
        let frame = checked_frame(frame);
        let attributes = attributes_only(attributes);
        let (entry, level) = self.find(va).expect("replacing an unmapped page");
        if level == BLOCK_LEVEL {
            self.split(frames, va, entry)?;
        }
        let slot = self.slot(frames, va, LEAF_LEVEL)?;
        // SAFETY: the entry is ours, reached through the linear map: made invalid, invalidated, then written.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: clears a page entry, the break step of replace, in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, 0) };
        invalidate(va);
        // SAFETY: as above.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: writes the replacing page entry in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, frame | attributes.bits() as u64 | table_or_page()) };
        barrier();
        Ok(())
    }

    /// Unmaps the page at `va`, mapped by a page entry, and invalidates it as `protect` does.
    pub fn unmap(&self, va: u64) {
        let (_, level) = self.find(va).expect("unmapping an unmapped page");
        assert_eq!(level, LEAF_LEVEL, "unmapping a page of a block");
        let mut table = self.root;
        for above in 0..LEAF_LEVEL {
            // SAFETY: table entries are reached through the linear map; `find` saw each level valid.
            #[expect(
                unsafe_code,
                reason = "UNSOUND: reads an entry while walking down to unmap in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
            )]
            let entry = unsafe { ptr::read_volatile(entry_at(table, va, above)) };
            table = entry & OUTPUT_ADDRESS;
        }
        // SAFETY: as above.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: clears a page entry in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(entry_at(table, va, LEAF_LEVEL), 0) };
        invalidate(va);
    }

    /// Breaks the block holding `va`, if one does, into pages as `protect` does.
    pub fn split_block(&self, frames: &mut Frames, va: u64) -> Result<(), Full> {
        match self.find(va) {
            Some((entry, BLOCK_LEVEL)) => self.split(frames, va, entry),
            _ => Ok(()),
        }
    }

    /// Whether a block entry maps `va`.
    pub fn is_block(&self, va: u64) -> bool {
        self.find(va).is_some_and(|(_, level)| level == BLOCK_LEVEL)
    }

    /// Gives the block entry at `base` new attributes, keeping its frames, and invalidates it as `protect` does.
    pub fn protect_block(&self, base: u64, attributes: El1Attributes) {
        let attributes = attributes_only(attributes);
        let (entry, level) = self.find(base).expect("protecting an unmapped block");
        assert_eq!(level, BLOCK_LEVEL, "protecting a block that is not one");
        let slot = self.entry(base, BLOCK_LEVEL);
        let block = (entry & OUTPUT_ADDRESS)
            | ((attributes.bits() as u64 | El1Attributes::VALID.bits() as u64)
                & !(El1Attributes::TABLE_OR_PAGE.bits() as u64));
        // SAFETY: the entry is ours, reached through the linear map.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: rewrites a block entry with new attributes in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, block) };
        invalidate(base);
    }

    /// Unmaps the block entry at `base` and invalidates it as `protect` does.
    pub fn unmap_block(&self, base: u64) {
        assert!(self.is_block(base), "unmapping a block that is not one");
        // SAFETY: the entry is ours, reached through the linear map.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: clears a block entry in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(self.entry(base, BLOCK_LEVEL), 0) };
        invalidate(base);
    }

    /// Replaces the table of page entries at `base`, which map the run of frames at `run` in order, by one block
    /// entry over the run with `attributes`, break before make, and gives the table's frame back.
    pub fn promote(&self, frames: &mut Frames, base: u64, run: u64, attributes: El1Attributes) {
        assert!(run.is_multiple_of(BLOCK_SIZE), "a block entry maps an aligned run");
        let run = checked_frame(run);
        let attributes = attributes_only(attributes);
        let slot = self.entry(base, BLOCK_LEVEL);
        // SAFETY: the entry is ours, reached through the linear map.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: reads the entry a promotion replaces in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        let table = unsafe { ptr::read_volatile(slot) };
        assert!(
            valid(table) && table & El1Attributes::TABLE_OR_PAGE.bits() as u64 != 0,
            "promoting a range not mapped by a table"
        );
        // SAFETY: as above: made invalid, every page's entry invalidated, then the block written.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: clears that entry, the break step of a promotion, in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, 0) };
        barrier::dsb(ISHST);
        for page in 0..ENTRIES {
            El1And0::invalidate_va(VirtualAddress((base + page * PAGE_SIZE) as usize));
        }
        barrier::dsb(ISH);
        barrier::isb(SY);
        let block = (run | attributes.bits() as u64 | El1Attributes::VALID.bits() as u64)
            & !(El1Attributes::TABLE_OR_PAGE.bits() as u64);
        // SAFETY: as above.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: writes the promoted block entry in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, block) };
        barrier();
        frames.give(table & OUTPUT_ADDRESS);
    }

    /// The entry for `va` at `level`, whose tables above exist.
    fn entry(&self, va: u64, level: u32) -> *mut u64 {
        let mut table = self.root;
        for above in 0..level {
            // SAFETY: table entries are reached through the linear map.
            #[expect(
                unsafe_code,
                reason = "UNSOUND: reads an entry while walking down to one whose tables exist in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
            )]
            let value = unsafe { ptr::read_volatile(entry_at(table, va, above)) };
            assert!(valid(value), "a table above the entry is missing");
            table = value & OUTPUT_ADDRESS;
        }
        entry_at(table, va, level)
    }

    /// Breaks the block holding `va` into pages over the same frames with the same attributes.
    fn split(&self, frames: &mut Frames, va: u64, block: u64) -> Result<(), Full> {
        let base = va & !(BLOCK_SIZE - 1);
        let pages = frames.take()?;
        let attributes = block & !OUTPUT_ADDRESS;
        let first = block & OUTPUT_ADDRESS;
        for page in 0..ENTRIES {
            let entry = (first + page * PAGE_SIZE) | attributes | table_or_page();
            // SAFETY: the new table is ours.
            #[expect(
                unsafe_code,
                reason = "UNSOUND: fills a fresh table frame with the page entries of a split block, a frame from take, which hands out frames give takes back without checking that nothing uses them; Words over the linear map, or aarch64-paging, replaces it"
            )]
            unsafe { ptr::write_volatile((linear(pages) + page * ENTRY_BYTES) as *mut u64, entry) };
        }
        let slot = self.slot(frames, base, BLOCK_LEVEL)?;
        // SAFETY: the block entry is ours: made invalid, invalidated (one entry covers the whole block), then
        // the table written.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: clears a block entry, the break step of a split, in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, 0) };
        invalidate(base);
        // SAFETY: as above.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: writes the table entry of a split block in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe { ptr::write_volatile(slot, pages | table_or_page()) };
        barrier();
        LARGE_ENTRY_SPLITS.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Makes `other`'s top-level entry for `va` this table's too.
    pub fn share_top(&self, other: &Table, va: u64) {
        // SAFETY: both top-level tables are reached through the linear map.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: copies another root's top-level entry into this root, both reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        unsafe {
            ptr::write_volatile(
                entry_at(self.root, va, TOP_LEVEL),
                ptr::read_volatile(entry_at(other.root, va, TOP_LEVEL)),
            )
        };
        barrier();
    }

    /// Makes the tables under `other`'s top-level entry for `va` exist, so it can be shared.
    pub fn reserve_top(&self, frames: &mut Frames, va: u64) -> Result<(), Full> {
        self.slot(frames, va, TOP_LEVEL + 1).map(|_| ())
    }

    /// The top-level entry at `index`.
    #[expect(
        unsafe_code,
        reason = "UNSOUND: reads a top-level entry by a checked index in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
    )]
    pub fn top_entry(&self, index: u64) -> u64 {
        assert!(index < ENTRIES, "an entry of the table");
        // SAFETY: the top-level table is reached through the linear map.
        unsafe { ptr::read_volatile((linear(self.root) + index * ENTRY_BYTES) as *const u64) }
    }

    /// Gives back this table and the tables below the top-level entries `owned` names by index, never the frames
    /// pages and blocks map. Nothing may walk it afterwards.
    pub fn free(&self, frames: &mut Frames, owned: impl Fn(u64) -> bool) {
        for index in (0..ENTRIES).filter(|&index| owned(index)) {
            free_below(frames, self.top_entry(index), TOP_LEVEL);
        }
        frames.give(self.root);
    }
}

/// Gives back the table `entry` at `level` points to, if it points to one, and every table below it.
fn free_below(frames: &mut Frames, entry: u64, level: u32) {
    let table_bit = El1Attributes::TABLE_OR_PAGE.bits() as u64;
    if level == LEAF_LEVEL || !valid(entry) || entry & table_bit == 0 {
        return;
    }
    let table = entry & OUTPUT_ADDRESS;
    for index in 0..ENTRIES {
        // SAFETY: table entries are reached through the linear map.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: reads the entries of a table being freed in a table reached from Table's root, which is private now but Table is Copy, and give and free take a table's frames back while a copy may still walk them, as give takes any frame number; Words over the linear map, or aarch64-paging, replaces it"
        )]
        let below =
            unsafe { ptr::read_volatile((linear(table) + index * ENTRY_BYTES) as *const u64) };
        free_below(frames, below, level + 1);
    }
    frames.give(table);
}

/// Makes table writes visible to the table walker.
pub fn barrier() {
    barrier::dsb(ISHST);
    barrier::isb(SY);
}

/// Invalidates every cached entry of the walk for `va`, of every identifier and level, on every virtual
/// CPU: one primitive for a page whose permissions change in a table several table roots share, and for a block that
/// becomes a table.
fn invalidate(va: u64) {
    barrier::dsb(ISHST);
    El1And0::invalidate_va(VirtualAddress(va as usize));
    barrier::dsb(ISH);
    barrier::isb(SY);
}
