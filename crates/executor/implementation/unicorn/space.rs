//! The stand-in's guest memory: one unicorn instance for each space, and its page table.
//!
//! A mapping from another space makes that space's own pages shared pages both show; a copy copies a space's own bytes
//! and shows what it mapped.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use wcontainers_abi::{CpuError, Exit, PAGE_SIZE, Permissions, RegisterBlock};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::memory::{Memory, Part};
use crate::standin::Unsupported;

/// An address space of the stand-in, held with full authority.
pub struct UnicornSpace {
    memory: Rc<RefCell<Memory>>,
}

/// The one page table of a [`UnicornSpace`], made with it, which refuses every change once the space is gone.
pub struct UnicornPageTable {
    memory: Weak<RefCell<Memory>>,
}

/// Addresses of a space, borrowed.
pub struct UnicornBytes<'space> {
    space: &'space UnicornSpace,
    addresses: Range<u64>,
}

impl UnicornSpace {
    /// Returns a space of all zeros allowing nothing, whose addresses end at the end given, and its page table.
    pub(crate) fn new(end: u64) -> Result<(Self, UnicornPageTable), CpuError> {
        let memory = Rc::new(RefCell::new(Memory::new(end)?));
        let table = UnicornPageTable {
            memory: Rc::downgrade(&memory),
        };
        Ok((Self { memory }, table))
    }

    /// Runs the registers on this space as [`Memory::run_turn`] does.
    pub(crate) fn run_turn(
        &self,
        registers: &mut RegisterBlock,
        slice_nanoseconds: Option<u64>,
        deadline: Option<coarsetime::Instant>,
        interrupt: Option<Arc<AtomicBool>>,
    ) -> (Result<Exit, Unsupported>, u64) {
        self.memory
            .borrow_mut()
            .run_turn(registers, slice_nanoseconds, deadline, interrupt)
    }

    /// Returns what the page at the address allows, nothing past the end of the space.
    pub(crate) fn read_permissions(&self, address: u64) -> Permissions {
        self.memory.borrow().read_permissions(address)
    }
}

/// Returns the space the table changes, refused once the space is gone.
fn reach(table: &UnicornPageTable) -> Result<Rc<RefCell<Memory>>, InvalidAddressError> {
    table.memory.upgrade().ok_or(InvalidAddressError)
}

/// Returns the addresses if they lie inside the space, and whole pages where `whole_pages` asks it.
fn check(
    memory: &Memory,
    addresses: &Range<u64>,
    whole_pages: bool,
) -> Result<(), InvalidAddressError> {
    let length = addresses
        .end
        .checked_sub(addresses.start)
        .ok_or(InvalidAddressError)?;
    memory
        .range(addresses.start, length)
        .map_err(|_| InvalidAddressError)?;
    if whole_pages
        && !(addresses.start.is_multiple_of(PAGE_SIZE) && addresses.end.is_multiple_of(PAGE_SIZE))
    {
        return Err(InvalidAddressError);
    }
    Ok(())
}

/// Returns the other space's addresses that line up with these, if both lie inside their spaces as whole pages.
fn line_up(
    memory: &Memory,
    addresses: &Range<u64>,
    other: &Memory,
    other_start: u64,
) -> Result<Range<u64>, InvalidAddressError> {
    check(memory, addresses, true)?;
    let other_end = other_start
        .checked_add(addresses.end - addresses.start)
        .ok_or(InvalidAddressError)?;
    let lined_up = other_start..other_end;
    check(other, &lined_up, true)?;
    Ok(lined_up)
}

impl MemorySpace for UnicornSpace {
    type Bytes<'space> = UnicornBytes<'space>;
    type PageTable = UnicornPageTable;

    fn borrow(&self, addresses: Range<u64>) -> Result<UnicornBytes<'_>, InvalidAddressError> {
        check(&self.memory.borrow(), &addresses, false)?;
        Ok(UnicornBytes {
            space: self,
            addresses,
        })
    }
}

impl PageTable for UnicornPageTable {
    fn set_permissions(
        &mut self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError> {
        let memory = reach(self)?;
        let mut memory = memory.borrow_mut();
        check(&memory, &addresses, true)?;
        memory.set_permissions(addresses, allowed);
        Ok(())
    }

    fn map_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError> {
        let memory = reach(self).map_err(MapMemoryError::InvalidAddress)?;
        let other = reach(other).map_err(MapMemoryError::InvalidAddress)?;
        let lined_up = line_up(&memory.borrow(), &addresses, &other.borrow(), other_start)
            .map_err(MapMemoryError::InvalidAddress)?;
        if addresses.is_empty() {
            return Ok(());
        }
        let parts = other
            .borrow_mut()
            .share_out(lined_up)
            .map_err(|_| MapMemoryError::OutOfMemory)?;
        let mut memory = memory.borrow_mut();
        for (range, pages, offset) in parts {
            let moved = addresses.start + (range.start - other_start)
                ..addresses.start + (range.end - other_start);
            memory.show_unmapped(moved, pages, offset, true);
        }
        memory.remap(addresses);
        Ok(())
    }

    fn unmap_from(&mut self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        let memory = reach(self)?;
        let mut memory = memory.borrow_mut();
        check(&memory, &addresses, true)?;
        memory.drop_mappings(addresses);
        Ok(())
    }

    fn copy_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        let memory = reach(self).map_err(CopyMemoryError::InvalidAddress)?;
        let other = reach(other).map_err(CopyMemoryError::InvalidAddress)?;
        let lined_up = line_up(&memory.borrow(), &addresses, &other.borrow(), other_start)
            .map_err(CopyMemoryError::InvalidAddress)?;
        if addresses.is_empty() {
            return Ok(());
        }
        let source = other.borrow();
        let mut memory = memory.borrow_mut();
        let moved = |address: u64| addresses.start + (address - other_start);
        let zeros = [u8::MIN; PAGE_SIZE as usize];
        let mut bytes = [u8::MIN; PAGE_SIZE as usize];
        for (range, part) in source.list_parts(lined_up.clone()) {
            match part {
                Part::Shown {
                    pages,
                    offset,
                    mapped: true,
                } => {
                    memory.show_unmapped(moved(range.start)..moved(range.end), pages, offset, true)
                }
                Part::Own | Part::Shown { mapped: false, .. } => {
                    memory.unshare(moved(range.start)..moved(range.end));
                    for page in range.start / PAGE_SIZE..range.end / PAGE_SIZE {
                        let at = page * PAGE_SIZE;
                        let into = moved(at);
                        if source.is_touched(page) {
                            source.read(at, &mut bytes).map_err(|_| {
                                CopyMemoryError::InvalidAddress(InvalidAddressError)
                            })?;
                            memory.write(into, &bytes).map_err(|_| {
                                CopyMemoryError::InvalidAddress(InvalidAddressError)
                            })?;
                        } else if memory.is_touched(into / PAGE_SIZE) {
                            memory.write(into, &zeros).map_err(|_| {
                                CopyMemoryError::InvalidAddress(InvalidAddressError)
                            })?;
                        }
                    }
                }
            }
        }
        let permissions = source.list_permissions(lined_up);
        memory.copy_permissions(addresses.start, &permissions);
        memory.remap(addresses);
        Ok(())
    }
}

impl VolatileBytes for UnicornBytes<'_> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = to_count(self.len()).min(into.len());
        self.space
            .memory
            .borrow()
            .read(self.addresses.start, &mut into[..count])
            .expect("borrowed addresses lie inside the space");
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = to_count(self.len()).min(bytes.len());
        self.space
            .memory
            .borrow_mut()
            .write(self.addresses.start, &bytes[..count])
            .expect("borrowed addresses lie inside the space");
        count
    }

    /// Hands the sink a copy, since a slice of the guest's bytes may not be lent.
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut copied = vec![u8::MIN; to_count(self.len())];
        self.read(&mut copied);
        sink.write(&copied)
    }

    /// Has the source fill a buffer and writes what it gave, since a slice of the guest's bytes may not be lent.
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut buffer = vec![u8::MIN; to_count(self.len())];
        let count = source.read(&mut buffer)?;
        Ok(self.write(&buffer[..count]))
    }
}

/// Returns the length as a count of host bytes, at most the most the host can count.
fn to_count(length: u64) -> usize {
    usize::try_from(length).unwrap_or(usize::MAX)
}
