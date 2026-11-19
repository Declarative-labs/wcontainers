//! Frames of guest-physical memory and the translation tables written over them.

use core::arch::asm;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use aarch64_cpu::asm::barrier::{self, ISH, ISHST, SY};

use aarch64_paging::descriptor::El1Attributes;
use wcontainers_abi::PAGE_SIZE;

use crate::layout::{
    BLOCK_LEVEL, BLOCK_SIZE, ENTRIES, ENTRY_BYTES, LEAF_LEVEL, level_index, linear,
};

/// Width of the output address field of a descriptor.
const OUTPUT_ADDRESS_BITS: u32 = 48;

/// The output address bits of a descriptor.
const OUTPUT_ADDRESS: u64 = ((1 << OUTPUT_ADDRESS_BITS) - 1) & !(PAGE_SIZE - 1);

/// Where a TLB invalidation by address takes the address-space identifier.
const TLBI_ASID_SHIFT: u32 = 48;

/// TLB invalidation by address takes the address shifted by this, whatever the granule.
const TLBI_ADDRESS_SHIFT: u32 = 12;

/// Width of the address field of a TLB invalidation by address (VA[55:12]); the bits above it up to the
/// identifier are the level hint, left as 0 (no hint).
const TLBI_ADDRESS_BITS: u32 = 44;

/// Blocks broken into pages because one page of them changed: the slow path "large entry split".
static LARGE_ENTRY_SPLITS: AtomicU64 = AtomicU64::new(0);

/// How many times a block was broken into pages.
pub fn large_entry_splits() -> u64 {
    LARGE_ENTRY_SPLITS.load(Ordering::Relaxed)
}

/// Frames of guest-physical memory, given out one page (or one aligned block) at a time.
pub struct Frames {
    next: u64,
    end: u64,
    free: u64,
}

impl Frames {
    /// Frames from `start` (aligned up to a page) to `end`.
    pub const fn new(start: u64, end: u64) -> Frames {
        Frames {
            next: start.next_multiple_of(PAGE_SIZE),
            end,
            free: 0,
        }
    }

    /// A zeroed frame.
    pub fn take(&mut self) -> u64 {
        let frame = if self.free != 0 {
            let frame = self.free;
            // SAFETY: a freed frame holds the next free frame's address in its first word.
            self.free = unsafe { ptr::read(linear(frame) as *const u64) };
            frame
        } else {
            let frame = self.next;
            assert!(
                frame + PAGE_SIZE <= self.end,
                "guest-physical memory is used up"
            );
            self.next += PAGE_SIZE;
            frame
        };
        // SAFETY: the frame is ours and reached through the linear map.
        unsafe { ptr::write_bytes(linear(frame) as *mut u8, 0, PAGE_SIZE as usize) };
        frame
    }

    /// A run of frames that a block entry can map, never used since the host gave it, so it reads as zeros.
    pub fn take_block(&mut self) -> u64 {
        let block = self.next.next_multiple_of(BLOCK_SIZE);
        assert!(
            block + BLOCK_SIZE <= self.end,
            "guest-physical memory is used up"
        );
        let mut gap = self.next;
        while gap < block {
            self.give(gap);
            gap += PAGE_SIZE;
        }
        self.next = block + BLOCK_SIZE;
        block
    }

    /// Takes back one frame.
    pub fn give(&mut self, frame: u64) {
        // SAFETY: the frame is no longer used by anyone and is reached through the linear map.
        unsafe { ptr::write(linear(frame) as *mut u64, self.free) };
        self.free = frame;
    }
}

/// One set of translation tables for one half, from its top-level table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Table {
    /// The top-level table's guest-physical address.
    pub root: u64,
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
    /// An empty table.
    pub fn new(frames: &mut Frames) -> Table {
        Table {
            root: frames.take(),
        }
    }

    /// The entry for `va` at `level`, making the tables above it as needed.
    fn slot(&self, frames: &mut Frames, va: u64, level: u32) -> *mut u64 {
        let mut table = self.root;
        for above in 0..level {
            let entry = entry_at(table, va, above);
            // SAFETY: table entries are reached through the linear map.
            let value = unsafe { ptr::read_volatile(entry) };
            table = if valid(value) {
                value & OUTPUT_ADDRESS
            } else {
                let next = frames.take();
                // SAFETY: as above.
                unsafe { ptr::write_volatile(entry, next | table_or_page()) };
                next
            };
        }
        entry_at(table, va, level)
    }

    /// The entry mapping `va` and its level, if any.
    pub fn find(&self, va: u64) -> Option<(u64, u32)> {
        let mut table = self.root;
        for level in 0..=LEAF_LEVEL {
            // SAFETY: table entries are reached through the linear map.
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
    pub fn map(&self, frames: &mut Frames, va: u64, frame: u64, attributes: El1Attributes) {
        let entry = frame | attributes.bits() as u64 | table_or_page();
        let slot = self.slot(frames, va, LEAF_LEVEL);
        // SAFETY: the entry is ours, reached through the linear map.
        assert!(
            !valid(unsafe { ptr::read_volatile(slot) }),
            "mapping a mapped page"
        );
        // SAFETY: as above.
        unsafe { ptr::write_volatile(slot, entry) };
        barrier();
    }

    /// Maps the block at `va`, which nothing maps, to the block of frames at `block`.
    pub fn map_block(&self, frames: &mut Frames, va: u64, block: u64, attributes: El1Attributes) {
        let entry = (block | attributes.bits() as u64 | El1Attributes::VALID.bits() as u64)
            & !(El1Attributes::TABLE_OR_PAGE.bits() as u64);
        let slot = self.slot(frames, va, BLOCK_LEVEL);
        // SAFETY: as in `map`.
        assert!(
            !valid(unsafe { ptr::read_volatile(slot) }),
            "mapping a mapped block"
        );
        // SAFETY: as in `map`.
        unsafe { ptr::write_volatile(slot, entry) };
        barrier();
    }

    /// Gives the page at `va` new attributes, keeping its frame, and invalidates it for `asid`. A block
    /// holding it is first split into pages with the block's attributes, break before make.
    pub fn protect(&self, frames: &mut Frames, va: u64, attributes: El1Attributes, asid: u16) {
        let (entry, level) = self.find(va).expect("protecting an unmapped page");
        if level == BLOCK_LEVEL {
            self.split(frames, va, entry, asid);
        }
        let frame = self.find(va).expect("split").0 & OUTPUT_ADDRESS;
        let slot = self.slot(frames, va, LEAF_LEVEL);
        // SAFETY: the entry is ours, reached through the linear map.
        unsafe { ptr::write_volatile(slot, frame | attributes.bits() as u64 | table_or_page()) };
        invalidate_page(va, asid);
    }

    /// Breaks the block holding `va` into pages over the same frames with the same attributes.
    fn split(&self, frames: &mut Frames, va: u64, block: u64, asid: u16) {
        let base = va & !(BLOCK_SIZE - 1);
        let pages = frames.take();
        let attributes = block & !OUTPUT_ADDRESS;
        let first = block & OUTPUT_ADDRESS;
        for page in 0..ENTRIES {
            let entry = (first + page * PAGE_SIZE) | attributes | table_or_page();
            // SAFETY: the new table is ours.
            unsafe { ptr::write_volatile((linear(pages) + page * ENTRY_BYTES) as *mut u64, entry) };
        }
        let slot = self.slot(frames, base, BLOCK_LEVEL);
        // SAFETY: the block entry is ours: made invalid, invalidated (one entry covers the whole block), then
        // the table written.
        unsafe { ptr::write_volatile(slot, 0) };
        invalidate(base, asid);
        // SAFETY: as above.
        unsafe { ptr::write_volatile(slot, pages | table_or_page()) };
        barrier();
        if LARGE_ENTRY_SPLITS.fetch_add(1, Ordering::Relaxed) == 0 {
            super::send(
                &crate::link::Print {},
                format_args!(
                    "warning: large entry split: a block was broken into pages because one page of it changed (counted)"
                ),
            );
        }
    }

    /// Makes `other`'s top-level entry for `va` this table's too.
    pub fn share_top(&self, other: &Table, va: u64) {
        // SAFETY: both top-level tables are reached through the linear map.
        unsafe {
            ptr::write_volatile(
                entry_at(self.root, va, 0),
                ptr::read_volatile(entry_at(other.root, va, 0)),
            )
        };
        barrier();
    }

    /// Makes the tables under `other`'s top-level entry for `va` exist, so it can be shared.
    pub fn reserve_top(&self, frames: &mut Frames, va: u64) {
        let _ = self.slot(frames, va, 1);
    }
}

/// Makes table writes visible to the table walker.
pub fn barrier() {
    barrier::dsb(ISHST);
    barrier::isb(SY);
}

/// The operand of a TLB invalidation by address and identifier.
fn tlbi_operand(va: u64, asid: u16) -> u64 {
    (u64::from(asid) << TLBI_ASID_SHIFT)
        | ((va >> TLBI_ADDRESS_SHIFT) & ((1 << TLBI_ADDRESS_BITS) - 1))
}

/// Invalidates every cached entry of the walk for `va` and `asid` (and global ones) on every virtual CPU:
/// for an entry whose level changes, as when a block becomes a table.
fn invalidate(va: u64, asid: u16) {
    let operand = tlbi_operand(va, asid);
    barrier::dsb(ISHST);
    // SAFETY: TLB maintenance only.
    unsafe { asm!("tlbi vae1is, {0}", in(reg) operand, options(nostack, preserves_flags)) };
    barrier::dsb(ISH);
    barrier::isb(SY);
}

/// Invalidates the last-level entry of the page at `va` for `asid` (and global ones) on every virtual CPU:
/// for a page whose permissions change, which leaves the tables above it as they were.
fn invalidate_page(va: u64, asid: u16) {
    let operand = tlbi_operand(va, asid);
    barrier::dsb(ISHST);
    // SAFETY: TLB maintenance only.
    unsafe { asm!("tlbi vale1is, {0}", in(reg) operand, options(nostack, preserves_flags)) };
    barrier::dsb(ISH);
    barrier::isb(SY);
}

/// The page size the tables use is at least the one TLB maintenance assumes for its addresses.
const _: () = assert!(PAGE_SIZE.trailing_zeros() >= TLBI_ADDRESS_SHIFT);

/// The address field ends below the identifier.
const _: () = assert!(TLBI_ADDRESS_BITS < TLBI_ASID_SHIFT);
