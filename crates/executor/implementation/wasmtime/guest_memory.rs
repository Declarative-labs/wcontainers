//! The executor interface's guest memory, held as a memory of the executor's [`CpuSide`].
//!
//! Every space one executor makes shares its `CpuSide`, and a mapping between spaces is a block of shared pages.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

use wcontainers_abi::{Cpu, CpuError, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, SharedId};
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, VolatileBytes,
};
use wcontainers_guest_cpu::Guest;

use crate::{CpuSide, Shown};

/// The executor's memories, translations and blocks of shared pages, which every space it makes holds.
pub(crate) type World<G> = Rc<RefCell<CpuSide<G>>>;

/// One guest address space of the WebAssembly executor.
pub struct GuestMemory<G: Guest<Block = RegisterBlock>> {
    pub(crate) world: World<G>,
    pub(crate) memory: MemoryId,
}

/// A run of a memory's addresses that shows a block of shared pages, from the given offset in the block.
struct ShownRun {
    addresses: Range<u64>,
    block: SharedId,
    offset: u64,
    /// Whether the run is the memory's own pages that other memories map.
    lent: bool,
}

/// Addresses of a [`GuestMemory`], read and written one copy at a time.
pub struct GuestBytes<'space, G: Guest<Block = RegisterBlock>> {
    space: &'space GuestMemory<G>,
    addresses: Range<u64>,
}

impl<G: Guest<Block = RegisterBlock>> GuestMemory<G> {
    /// Returns a new space of the world, all zeros, allowing nothing.
    pub(crate) fn new(world: World<G>) -> Result<Self, CpuError> {
        let memory = world.borrow_mut().memory_new()?;
        Ok(Self { world, memory })
    }

    /// Returns the length of the addresses after checking they lie in the space.
    fn checked(&self, addresses: &Range<u64>) -> Result<u64, InvalidAddressError> {
        let length = addresses
            .end
            .checked_sub(addresses.start)
            .ok_or(InvalidAddressError)?;
        let side = self.world.borrow();
        side.check(addresses.start, length)
            .map_err(|_| InvalidAddressError)?;
        Ok(length)
    }

    /// Returns the length of the addresses after checking both ranges lie on pages of their spaces in one world.
    fn checked_pair(
        &self,
        addresses: &Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<u64, InvalidAddressError> {
        let length = self.checked(addresses)?;
        let other_end = other_start.checked_add(length).ok_or(InvalidAddressError)?;
        other.checked(&(other_start..other_end))?;
        let on_pages = [addresses.start, addresses.end, other_start]
            .iter()
            .all(|address| address.is_multiple_of(PAGE_SIZE));
        if !on_pages || !Rc::ptr_eq(&self.world, &other.world) {
            return Err(InvalidAddressError);
        }
        Ok(length)
    }
}

impl<G: Guest<Block = RegisterBlock>> Drop for GuestMemory<G> {
    fn drop(&mut self) {
        let mut side = self.world.borrow_mut();
        side.memory_drop(self.memory);
        side.drop_unshown_blocks();
    }
}

impl<G: Guest<Block = RegisterBlock>> MemorySpace for GuestMemory<G> {
    type Bytes<'space>
        = GuestBytes<'space, G>
    where
        Self: 'space;

    fn borrow(&self, addresses: Range<u64>) -> Result<GuestBytes<'_, G>, InvalidAddressError> {
        self.checked(&addresses)?;
        Ok(GuestBytes {
            space: self,
            addresses,
        })
    }

    fn zero(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        self.checked(&addresses)?;
        self.world
            .borrow_mut()
            .zero_keeping_mappings(self.memory, addresses)
            .map_err(|_| InvalidAddressError)
    }

    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError> {
        let length = self.checked(&addresses)?;
        let mut side = self.world.borrow_mut();
        let pages = addresses.start / PAGE_SIZE..addresses.end.div_ceil(PAGE_SIZE);
        let no_longer_run = pages.clone().any(|page| {
            let allows = side.process(self.memory).pages.get(page).allows;
            allows.contains(Permissions::EXECUTE) && !allowed.contains(Permissions::EXECUTE)
        });
        side.set_permissions(self.memory, addresses.start, length, allowed)
            .map_err(|_| InvalidAddressError)?;
        if no_longer_run {
            side.code_touched(self.memory, addresses);
        }
        Ok(())
    }

    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError> {
        self.checked_pair(&addresses, other, other_start)
            .map_err(MapMemoryError::InvalidAddress)?;
        self.world
            .borrow_mut()
            .map_pages(self.memory, addresses, other.memory, other_start)
            .map_err(|error| match error {
                CpuError::OutOfMemory => MapMemoryError::OutOfMemory,
                CpuError::BadAddress | CpuError::NotSupported => {
                    MapMemoryError::InvalidAddress(InvalidAddressError)
                }
            })
    }

    fn unmap_from(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        self.checked(&addresses)?;
        let on_pages =
            addresses.start.is_multiple_of(PAGE_SIZE) && addresses.end.is_multiple_of(PAGE_SIZE);
        if !on_pages {
            return Err(InvalidAddressError);
        }
        self.world
            .borrow_mut()
            .unmap_pages(self.memory, addresses)
            .map_err(|_| InvalidAddressError)
    }

    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        self.checked_pair(&addresses, other, other_start)
            .map_err(CopyMemoryError::InvalidAddress)?;
        self.world
            .borrow_mut()
            .copy_pages_from(self.memory, addresses, other.memory, other_start)
            .map_err(|error| match error {
                CpuError::OutOfMemory => CopyMemoryError::OutOfMemory,
                CpuError::BadAddress | CpuError::NotSupported => {
                    CopyMemoryError::InvalidAddress(InvalidAddressError)
                }
            })
    }
}

impl<G: Guest<Block = RegisterBlock>> VolatileBytes for GuestBytes<'_, G> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = usize::try_from(self.len())
            .unwrap_or(usize::MAX)
            .min(into.len());
        let space = self.space;
        space
            .world
            .borrow_mut()
            .read(space.memory, self.addresses.start, &mut into[..count])
            .expect("borrowed addresses lie in the space");
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = usize::try_from(self.len())
            .unwrap_or(usize::MAX)
            .min(bytes.len());
        let space = self.space;
        let mut side = space.world.borrow_mut();
        side.write(space.memory, self.addresses.start, &bytes[..count])
            .expect("borrowed addresses lie in the space");
        let end = self.addresses.start + count as u64;
        side.code_touched(space.memory, self.addresses.start..end);
        count
    }
}

impl<G: Guest<Block = RegisterBlock>> CpuSide<G> {
    /// Returns the shown runs of a memory overlapping the addresses, cut to them.
    fn shown_within(&self, memory: MemoryId, addresses: &Range<u64>) -> Vec<ShownRun> {
        self.process(memory)
            .shown
            .range(..addresses.end)
            .filter(|(_, shown)| shown.end > addresses.start)
            .map(|(start, shown)| {
                let from = addresses.start.max(*start);
                ShownRun {
                    addresses: from..shown.end.min(addresses.end),
                    block: shown.block,
                    offset: shown.offset + (from - start),
                    lent: shown.lent,
                }
            })
            .collect()
    }

    /// Drops the blocks of shared pages that no memory shows any more.
    pub(crate) fn drop_unshown_blocks(&mut self) {
        let shown: HashSet<SharedId> = (self.processes.values())
            .flat_map(|process| process.shown.values().map(|shown| shown.block))
            .collect();
        self.blocks.retain(|block, _| shown.contains(block));
    }

    /// Drops the translations of the code overlapping the addresses, here and wherever their shared blocks are shown.
    ///
    /// Their bytes or permissions changed.
    pub(crate) fn code_touched(&mut self, memory: MemoryId, addresses: Range<u64>) {
        let mut places = vec![(memory, addresses.clone())];
        for ShownRun {
            addresses: run,
            block,
            offset,
            ..
        } in self.shown_within(memory, &addresses)
        {
            let block_range = offset..offset + (run.end - run.start);
            for (&other, process) in &self.processes {
                for (start, shown) in &process.shown {
                    let Shown {
                        end,
                        block: other_block,
                        offset: other_offset,
                        ..
                    } = *shown;
                    let other_range = other_offset..other_offset + (end - start);
                    let (first, last) = (
                        block_range.start.max(other_range.start),
                        block_range.end.min(other_range.end),
                    );
                    if other_block == block && first < last {
                        let at = start + (first - other_offset);
                        places.push((other, at..at + (last - first)));
                    }
                }
            }
        }
        for (memory, addresses) in places {
            let stale: Vec<Range<u64>> = (self.process(memory).ranges.iter())
                .filter(|code| {
                    code.range.start < addresses.end
                        && addresses.start < code.range.end
                        && !code.covered.is_empty()
                })
                .map(|code| code.range.clone())
                .collect();
            for range in stale {
                self.code_changed(memory, range.start, range.end - range.start, &[]);
            }
        }
    }

    /// Sets the addresses to zeros, written through where they show a shared block and fresh pages elsewhere.
    fn zero_keeping_mappings(
        &mut self,
        memory: MemoryId,
        addresses: Range<u64>,
    ) -> Result<(), CpuError> {
        let mut at = addresses.start;
        let zeros = [u8::MIN; PAGE_SIZE as usize];
        for ShownRun {
            addresses: Range {
                start: from,
                end: to,
            },
            ..
        } in self.shown_within(memory, &addresses)
        {
            if at < from {
                self.zero(memory, at, from - at)?;
            }
            let mut start = from;
            while start < to {
                let end = to.min(start + PAGE_SIZE);
                self.write(memory, start, &zeros[..(end - start) as usize])?;
                start = end;
            }
            at = to;
        }
        if at < addresses.end {
            self.zero(memory, at, addresses.end - at)?;
        }
        self.code_touched(memory, addresses);
        Ok(())
    }

    /// Makes the addresses show the other memory's pages from its start, each run as a block of shared pages.
    ///
    /// A run that shows a block already is mapped to it; any other becomes a new block the other memory shows too.
    fn map_pages(
        &mut self,
        memory: MemoryId,
        addresses: Range<u64>,
        other: MemoryId,
        other_start: u64,
    ) -> Result<(), CpuError> {
        let length = addresses.end - addresses.start;
        self.flush(other);
        self.flush(memory);
        let source = other_start..other_start + length;
        let mut pieces = Vec::new();
        let mut at = source.start;
        for run in self.shown_within(other, &source) {
            if at < run.addresses.start {
                pieces.push((at..run.addresses.start, None));
            }
            at = run.addresses.end;
            pieces.push((run.addresses, Some((run.block, run.offset))));
        }
        if at < source.end {
            pieces.push((at..source.end, None));
        }
        for (piece, shown) in pieces {
            let piece_length = piece.end - piece.start;
            let (block, offset) = match shown {
                Some(shown) => shown,
                None => {
                    let block = self.shared_new(piece_length)?;
                    let bytes = self.code_bytes(other, &piece);
                    self.blocks.insert(block, bytes);
                    self.shared_map(block, u64::MIN, other, piece.start, piece_length)?;
                    let lent = self
                        .processes
                        .get_mut(&other)
                        .expect("a memory")
                        .shown
                        .get_mut(&piece.start);
                    lent.expect("the run just shown").lent = true;
                    (block, u64::MIN)
                }
            };
            let into = addresses.start + (piece.start - source.start);
            self.shared_map(block, offset, memory, into, piece_length)?;
        }
        self.code_touched(memory, addresses);
        self.drop_unshown_blocks();
        Ok(())
    }

    /// Ends the mappings of the addresses, which then hold zeros where they mapped a block; pages they lend keep their bytes.
    fn unmap_pages(&mut self, memory: MemoryId, addresses: Range<u64>) -> Result<(), CpuError> {
        let mapped = self
            .shown_within(memory, &addresses)
            .into_iter()
            .filter(|run| !run.lent);
        for ShownRun { addresses: run, .. } in mapped {
            self.zero(memory, run.start, run.end - run.start)?;
        }
        self.code_touched(memory, addresses);
        self.drop_unshown_blocks();
        Ok(())
    }

    /// Makes the addresses hold the bytes, permissions and mappings the other memory holds from its start, with pages it lends copied.
    fn copy_pages_from(
        &mut self,
        memory: MemoryId,
        addresses: Range<u64>,
        other: MemoryId,
        other_start: u64,
    ) -> Result<(), CpuError> {
        let length = addresses.end - addresses.start;
        self.flush(other);
        self.flush(memory);
        self.zero(memory, addresses.start, length)?;
        let source = other_start..other_start + length;
        let into = |address: u64| addresses.start + (address - source.start);
        let source_pages = source.start / PAGE_SIZE..source.end / PAGE_SIZE;
        let mut page_bytes = [u8::MIN; PAGE_SIZE as usize];
        let runs = self.process(other).pages.data_runs();
        for run in runs {
            let (first, end) = (
                run.start.max(source_pages.start),
                run.end.min(source_pages.end),
            );
            for page in first..end {
                self.read(other, page * PAGE_SIZE, &mut page_bytes)?;
                self.write(memory, into(page * PAGE_SIZE), &page_bytes)?;
            }
        }
        let mut page = source_pages.start;
        while page < source_pages.end {
            let allows = self.process(other).pages.get(page).allows;
            let mut end = page + 1;
            while end < source_pages.end && self.process(other).pages.get(end).allows == allows {
                end += 1;
            }
            let (start, length) = (page * PAGE_SIZE, (end - page) * PAGE_SIZE);
            self.set_permissions(memory, into(start), length, allows)?;
            page = end;
        }
        let mapped = self
            .shown_within(other, &source)
            .into_iter()
            .filter(|run| !run.lent);
        for ShownRun {
            addresses: run,
            block,
            offset,
            ..
        } in mapped
        {
            self.shared_map(block, offset, memory, into(run.start), run.end - run.start)?;
        }
        self.code_touched(memory, addresses);
        self.drop_unshown_blocks();
        Ok(())
    }
}
