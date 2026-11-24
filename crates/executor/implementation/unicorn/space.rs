//! The stand-in's guest memory: each space is one unicorn instance, and its bytes are host pages that
//! spaces share.
//!
//! Every address of a space shows a run of host pages through a view. A new space shows host pages of its
//! own; a mapping shows the other space's host pages, so a write on either side, by the kernel or by a guest,
//! lands in the same bytes; a copy shows new host pages holding the other's bytes, and the other's mappings
//! as they are. Unicorn maps the pages that allow something onto the host pages their view shows, and a page
//! that allows nothing as well, without access, once the kernel reads or writes it, so that unicorn's own reads
//! and writes reach every byte while its guest stays bound by the permissions.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use memmap2::MmapMut;
use wcontainers_abi::{CpuError, Exit, PAGE_SIZE, Permissions, RegisterBlock};
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, VolatileBytes,
};

use crate::standin::{Memory, Unsupported};

/// The pages one bit of a touched-page word stands for.
const PAGES_PER_WORD: u64 = u64::BITS as u64;

/// An address space of the stand-in, held with full authority.
pub struct UnicornSpace {
    space: RefCell<Space>,
}

struct Space {
    /// Dropped before the views, so unicorn stops showing host pages before they can go.
    memory: Memory,
    /// What each address shows: start to the view of the addresses from it.
    views: BTreeMap<u64, View>,
}

#[derive(Clone)]
struct View {
    end: u64,
    pages: Rc<HostPages>,
    /// Where in the host pages the view's start lies.
    offset: u64,
    /// Whether the view came from another space's pages, so a copy shows them too instead of their bytes.
    mapping: bool,
}

/// Host pages that live while any view shows them.
struct HostPages {
    bytes: MmapMut,
    /// Pages that may hold something other than zeros, one bit each, so a copy copies only those.
    touched: RefCell<Vec<u64>>,
}

/// Addresses of a space, borrowed.
pub struct UnicornBytes<'space> {
    space: &'space UnicornSpace,
    addresses: Range<u64>,
}

impl HostPages {
    /// Returns new zeroed host pages for the number of bytes.
    fn new(length: u64) -> Result<Self, CpuError> {
        let bytes = MmapMut::map_anon(usize::try_from(length).map_err(|_| CpuError::OutOfMemory)?)
            .map_err(|_| CpuError::OutOfMemory)?;
        let words = length.div_ceil(PAGE_SIZE).div_ceil(PAGES_PER_WORD);
        Ok(Self {
            bytes,
            touched: RefCell::new(vec![u64::MIN; usize::try_from(words).unwrap_or(usize::MAX)]),
        })
    }

    /// Returns the host address of the byte at the offset, for unicorn to map.
    fn host_address(&self, offset: u64) -> *mut u8 {
        let offset = usize::try_from(offset).expect("an offset inside host pages fits in host memory");
        self.bytes.as_ptr().cast_mut().wrapping_add(offset)
    }

    /// Notes that the pages over the offsets may hold bytes.
    fn touch(&self, offsets: Range<u64>) {
        let mut touched = self.touched.borrow_mut();
        for page in offsets.start / PAGE_SIZE..offsets.end.div_ceil(PAGE_SIZE) {
            touched[to_index(page / PAGES_PER_WORD)] |= 1 << (page % PAGES_PER_WORD);
        }
    }

    /// Returns the pages among the offsets that may hold bytes, as offsets of their starts.
    fn list_touched(&self, offsets: Range<u64>) -> Vec<u64> {
        let touched = self.touched.borrow();
        (offsets.start / PAGE_SIZE..offsets.end / PAGE_SIZE)
            .filter(|page| touched[to_index(page / PAGES_PER_WORD)] & (1 << (page % PAGES_PER_WORD)) != 0)
            .map(|page| page * PAGE_SIZE)
            .collect()
    }
}

impl Space {
    /// Returns the views over the addresses, each cut to them.
    fn list_views(&self, addresses: Range<u64>) -> Vec<(u64, View)> {
        let first = self
            .views
            .range(..=addresses.start)
            .next_back()
            .map_or(addresses.start, |(start, _)| *start);
        self.views
            .range(first..addresses.end)
            .filter(|(_, view)| view.end > addresses.start)
            .map(|(start, view)| {
                let cut_start = (*start).max(addresses.start);
                let cut = View {
                    end: view.end.min(addresses.end),
                    offset: view.offset + (cut_start - start),
                    ..view.clone()
                };
                (cut_start, cut)
            })
            .collect()
    }

    /// Makes the addresses show the views given, which cover them.
    fn show(&mut self, addresses: Range<u64>, views: Vec<(u64, View)>) {
        let overlapping: Vec<u64> = self
            .list_views(addresses.clone())
            .into_iter()
            .map(|(start, _)| start)
            .collect();
        // The views replaced keep their host pages until unicorn has stopped showing them, at the end.
        let mut replaced = Vec::new();
        for cut_start in overlapping {
            let (start, view) = self
                .views
                .range(..=cut_start)
                .next_back()
                .map(|(start, view)| (*start, view.clone()))
                .expect("every address shows a view");
            self.views.remove(&start);
            if start < addresses.start {
                self.views.insert(
                    start,
                    View {
                        end: addresses.start,
                        ..view.clone()
                    },
                );
            }
            if view.end > addresses.end {
                self.views.insert(
                    addresses.end,
                    View {
                        offset: view.offset + (addresses.end - start),
                        ..view.clone()
                    },
                );
            }
            replaced.push(view);
        }
        self.memory.unshare(addresses.start, addresses.end);
        for (start, view) in views {
            self.memory
                .shared
                .insert(start, (view.end, view.pages.host_address(view.offset)));
            self.views.insert(start, view);
        }
        self.memory.remap(addresses.start, addresses.end);
        drop(replaced);
    }

    /// Notes that the host pages the addresses show may hold bytes.
    fn touch(&self, addresses: Range<u64>) {
        for (start, view) in self.list_views(addresses) {
            view.pages.touch(view.offset..view.offset + (view.end - start));
        }
    }
}

impl UnicornSpace {
    /// Returns a space of all zeros allowing nothing, whose addresses end at the end given.
    pub(crate) fn new(end: u64) -> Result<Self, CpuError> {
        let mut memory = Memory::new(end)?;
        let words = end.div_ceil(PAGE_SIZE).div_ceil(PAGES_PER_WORD);
        memory.reachable = Some(vec![u64::MIN; to_index(words)]);
        let pages = Rc::new(HostPages::new(end)?);
        let mut space = Space {
            memory,
            views: BTreeMap::new(),
        };
        let own = View {
            end,
            pages,
            offset: u64::MIN,
            mapping: false,
        };
        space.show(u64::MIN..end, vec![(u64::MIN, own)]);
        Ok(Self {
            space: RefCell::new(space),
        })
    }

    /// Runs the registers on this space as [`Memory::run_turn`] does.
    pub(crate) fn run_turn(
        &self,
        registers: &mut RegisterBlock,
        slice_nanoseconds: Option<u64>,
        interrupt: Option<Arc<AtomicBool>>,
    ) -> (Result<Exit, Unsupported>, u64) {
        self.space.borrow_mut().memory.run_turn(
            registers,
            slice_nanoseconds,
            None,
            interrupt,
        )
    }

    /// Returns what the page at the address allows.
    pub(crate) fn read_permissions(&self, address: u64) -> Permissions {
        self.space.borrow().memory.pages[to_index(address / PAGE_SIZE)]
    }

    /// Returns the addresses if they lie inside the space, and whole pages where `whole_pages` asks it.
    fn check(&self, addresses: &Range<u64>, whole_pages: bool) -> Result<(), InvalidAddressError> {
        let length = addresses
            .end
            .checked_sub(addresses.start)
            .ok_or(InvalidAddressError)?;
        self.space
            .borrow()
            .memory
            .range(addresses.start, length)
            .map_err(|_| InvalidAddressError)?;
        if whole_pages && !(addresses.start.is_multiple_of(PAGE_SIZE) && addresses.end.is_multiple_of(PAGE_SIZE)) {
            return Err(InvalidAddressError);
        }
        Ok(())
    }

    /// Returns the other space's addresses that line up with these, if they lie inside it.
    fn line_up(
        &self,
        addresses: &Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<Range<u64>, InvalidAddressError> {
        self.check(addresses, true)?;
        let other_end = other_start
            .checked_add(addresses.end - addresses.start)
            .ok_or(InvalidAddressError)?;
        let lined_up = other_start..other_end;
        other.check(&lined_up, true)?;
        Ok(lined_up)
    }
}

impl MemorySpace for UnicornSpace {
    type Bytes<'space> = UnicornBytes<'space>;

    fn borrow(&self, addresses: Range<u64>) -> Result<UnicornBytes<'_>, InvalidAddressError> {
        self.check(&addresses, false)?;
        Ok(UnicornBytes {
            space: self,
            addresses,
        })
    }

    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError> {
        self.check(&addresses, true)?;
        let mut space = self.space.borrow_mut();
        let pages = to_index(addresses.start / PAGE_SIZE)..to_index(addresses.end / PAGE_SIZE);
        space.memory.pages[pages].fill(allowed);
        // Its guest may write what it allows.
        if !allowed.is_empty() {
            space.touch(addresses.clone());
        }
        space.memory.remap(addresses.start, addresses.end);
        Ok(())
    }

    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError> {
        let lined_up = self
            .line_up(&addresses, other, other_start)
            .map_err(MapMemoryError::InvalidAddress)?;
        if addresses.is_empty() {
            return Ok(());
        }
        let views: Vec<(u64, View)> = other
            .space
            .borrow()
            .list_views(lined_up)
            .into_iter()
            .map(|(start, view)| {
                let moved = addresses.start + (start - other_start);
                let view = View {
                    end: addresses.start + (view.end - other_start),
                    mapping: true,
                    ..view
                };
                (moved, view)
            })
            .collect();
        self.space.borrow_mut().show(addresses, views);
        Ok(())
    }

    fn unmap_from(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        self.check(&addresses, true)?;
        if addresses.is_empty() {
            return Ok(());
        }
        // The interface gives this call no error for want of host memory.
        let pages = HostPages::new(addresses.end - addresses.start)
            .unwrap_or_else(|error| panic!("no host memory for zeroed pages: {error:?}"));
        let zeroed = View {
            end: addresses.end,
            pages: Rc::new(pages),
            offset: u64::MIN,
            mapping: false,
        };
        self.space
            .borrow_mut()
            .show(addresses.clone(), vec![(addresses.start, zeroed)]);
        Ok(())
    }

    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        let lined_up = self
            .line_up(&addresses, other, other_start)
            .map_err(CopyMemoryError::InvalidAddress)?;
        if addresses.is_empty() {
            return Ok(());
        }
        let mut copied =
            HostPages::new(addresses.end - addresses.start).map_err(|_| CopyMemoryError::OutOfMemory)?;
        let mut touched = Vec::new();
        let (views, permissions) = {
            let mut source = other.space.borrow_mut();
            let views = source.list_views(lined_up.clone());
            for (start, view) in &views {
                if view.mapping {
                    continue;
                }
                let at = view.offset;
                for page_offset in view.pages.list_touched(at..at + (view.end - start)) {
                    let address = start + (page_offset - at);
                    let into = to_index(address - other_start);
                    source.memory.reach(address, address + PAGE_SIZE);
                    source
                        .memory
                        .unicorn
                        .mem_read(address, &mut copied.bytes[into..into + to_index(PAGE_SIZE)])
                        .expect("the pages reached are mapped in unicorn");
                    touched.push(address - other_start);
                }
            }
            let pages = to_index(lined_up.start / PAGE_SIZE)..to_index(lined_up.end / PAGE_SIZE);
            (views, source.memory.pages[pages].to_vec())
        };
        for offset in touched {
            copied.touch(offset..offset + PAGE_SIZE);
        }
        let copied = Rc::new(copied);
        let views = views
            .into_iter()
            .map(|(start, view)| {
                let moved = addresses.start + (start - other_start);
                let end = addresses.start + (view.end - other_start);
                let view = if view.mapping {
                    View { end, ..view }
                } else {
                    View {
                        end,
                        pages: Rc::clone(&copied),
                        offset: start - other_start,
                        mapping: false,
                    }
                };
                (moved, view)
            })
            .collect();
        let mut space = self.space.borrow_mut();
        let pages = to_index(addresses.start / PAGE_SIZE)..to_index(addresses.end / PAGE_SIZE);
        space.memory.pages[pages].copy_from_slice(&permissions);
        space.show(addresses, views);
        Ok(())
    }
}

impl VolatileBytes for UnicornBytes<'_> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = to_index(self.len()).min(into.len());
        if count > usize::MIN {
            let mut space = self.space.space.borrow_mut();
            let end = self.addresses.start + u64::try_from(count).expect("a count of bytes fits in an address");
            space.memory.reach(self.addresses.start, end);
            space
                .memory
                .unicorn
                .mem_read(self.addresses.start, &mut into[..count])
                .expect("the pages reached are mapped in unicorn");
        }
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = to_index(self.len()).min(bytes.len());
        if count > usize::MIN {
            let mut space = self.space.space.borrow_mut();
            let end = self.addresses.start + u64::try_from(count).expect("a count of bytes fits in an address");
            space.memory.reach(self.addresses.start, end);
            space
                .memory
                .unicorn
                .mem_write(self.addresses.start, &bytes[..count])
                .expect("the pages reached are mapped in unicorn");
            space.touch(self.addresses.start..end);
        }
        count
    }
}

/// Returns the number as an index into host memory.
fn to_index(number: u64) -> usize {
    usize::try_from(number).expect("an address of a space fits in host memory")
}
