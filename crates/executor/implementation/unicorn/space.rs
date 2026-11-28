//! The stand-in's guest memory: one unicorn instance for each space, and its page table.
//!
//! A mapping from another space makes that space's own pages shared pages both show; a copy copies a space's own bytes
//! and shows what it mapped. The unicorn instances live on their stand-in's engine thread ([`crate::engine`]), so a
//! space and its table are handles of that engine and only spaces of one engine map or copy from each other.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use wcontainers_abi::{CpuError, Exit, PAGE_SIZE, Permissions, RegisterBlock};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::engine::{Engine, EngineHandle};
use crate::memory::{Memory, Part};
use crate::standin::Unsupported;

/// An address space of the stand-in, held with full authority.
pub struct UnicornSpace {
    engine: EngineHandle,
    key: u64,
}

/// The one page table of a [`UnicornSpace`], made with it, which refuses every change once the space is gone.
pub struct UnicornPageTable {
    engine: EngineHandle,
    key: u64,
}

/// Addresses of a space, borrowed.
pub struct UnicornBytes<'space> {
    space: &'space UnicornSpace,
    addresses: Range<u64>,
}

impl UnicornSpace {
    /// Returns a space of all zeros allowing nothing on the engine, whose addresses end at the end given, and its page
    /// table.
    pub(crate) fn new(
        engine: &EngineHandle,
        end: u64,
    ) -> Result<(Self, UnicornPageTable), CpuError> {
        let key = engine.call(move |engine| {
            let memory = Memory::new(end)?;
            let key = engine.take_key();
            engine.spaces.insert(key, Rc::new(RefCell::new(memory)));
            Ok(key)
        })?;
        let table = UnicornPageTable {
            engine: engine.clone(),
            key,
        };
        Ok((
            Self {
                engine: engine.clone(),
                key,
            },
            table,
        ))
    }

    /// Returns whether the space lives on the engine.
    pub(crate) fn is_on(&self, engine: &EngineHandle) -> bool {
        self.engine.is_same(engine)
    }

    /// Runs the call on the space's memory, on its engine's thread.
    pub(crate) fn with_memory<Answer: Send + 'static>(
        &self,
        call: impl FnOnce(&mut Memory) -> Answer + Send + 'static,
    ) -> Answer {
        let key = self.key;
        self.engine.call(move |engine| {
            let memory = engine
                .space(key)
                .expect("a space's memory lives as long as the space");
            call(&mut memory.borrow_mut())
        })
    }

    /// Runs the registers on this space as [`Memory::run_turn`] does.
    pub(crate) fn run_turn(
        &self,
        registers: &mut RegisterBlock,
        slice_nanoseconds: Option<u64>,
        deadline: Option<coarsetime::Instant>,
        interrupt: Option<Arc<AtomicBool>>,
    ) -> (Result<Exit, Unsupported>, u64) {
        let mut moved = *registers;
        let (outcome, moved) = self.with_memory(move |memory| {
            let outcome = memory.run_turn(&mut moved, slice_nanoseconds, deadline, interrupt);
            (outcome, moved)
        });
        *registers = moved;
        outcome
    }

    /// Returns what the page at the address allows, nothing past the end of the space.
    pub(crate) fn read_permissions(&self, address: u64) -> Permissions {
        self.with_memory(move |memory| memory.read_permissions(address))
    }
}

/// The space's memory goes with it, and its table refuses every change after.
impl Drop for UnicornSpace {
    fn drop(&mut self) {
        let key = self.key;
        self.engine.call(move |engine| {
            engine.spaces.remove(&key);
        });
    }
}

impl UnicornPageTable {
    /// Runs the call on the engine's thread with the table's memory, refused once the space is gone.
    fn with_memory<Answer: Send + 'static, Error: Send + 'static>(
        &self,
        gone: Error,
        call: impl FnOnce(&Engine, Rc<RefCell<Memory>>) -> Result<Answer, Error> + Send + 'static,
    ) -> Result<Answer, Error> {
        let key = self.key;
        self.engine.call(move |engine| match engine.space(key) {
            Some(memory) => call(engine, memory),
            None => Err(gone),
        })
    }
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
        let checked = addresses.clone();
        self.with_memory(move |memory| check(memory, &checked, false))?;
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
        self.with_memory(InvalidAddressError, move |_, memory| {
            let mut memory = memory.borrow_mut();
            check(&memory, &addresses, true)?;
            memory.set_permissions(addresses, allowed);
            Ok(())
        })
    }

    /// Only a table of a space on the same engine maps from another; any other is refused as an invalid address.
    fn map_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError> {
        if !self.engine.is_same(&other.engine) {
            return Err(MapMemoryError::InvalidAddress(InvalidAddressError));
        }
        let other_key = other.key;
        self.with_memory(
            MapMemoryError::InvalidAddress(InvalidAddressError),
            move |engine, memory| {
                let other = engine
                    .space(other_key)
                    .ok_or(MapMemoryError::InvalidAddress(InvalidAddressError))?;
                map_from(&memory, addresses, &other, other_start)
            },
        )
    }

    fn unmap_from(&mut self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        self.with_memory(InvalidAddressError, move |_, memory| {
            let mut memory = memory.borrow_mut();
            check(&memory, &addresses, true)?;
            memory.drop_mappings(addresses);
            Ok(())
        })
    }

    /// Only a table of a space on the same engine copies from another; any other is refused as an invalid address.
    fn copy_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        if !self.engine.is_same(&other.engine) {
            return Err(CopyMemoryError::InvalidAddress(InvalidAddressError));
        }
        let other_key = other.key;
        self.with_memory(
            CopyMemoryError::InvalidAddress(InvalidAddressError),
            move |engine, memory| {
                let other = engine
                    .space(other_key)
                    .ok_or(CopyMemoryError::InvalidAddress(InvalidAddressError))?;
                copy_from(&memory, addresses, &other, other_start)
            },
        )
    }
}

/// Makes the addresses of the memory show what the other memory holds from its start.
fn map_from(
    memory: &Rc<RefCell<Memory>>,
    addresses: Range<u64>,
    other: &Rc<RefCell<Memory>>,
    other_start: u64,
) -> Result<(), MapMemoryError> {
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

/// Copies to the addresses of the memory what the other memory holds from its start, sharing what it mapped.
fn copy_from(
    memory: &Rc<RefCell<Memory>>,
    addresses: Range<u64>,
    other: &Rc<RefCell<Memory>>,
    other_start: u64,
) -> Result<(), CopyMemoryError> {
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
            } => memory.show_unmapped(moved(range.start)..moved(range.end), pages, offset, true),
            Part::Own | Part::Shown { mapped: false, .. } => {
                memory.unshare(moved(range.start)..moved(range.end));
                for page in range.start / PAGE_SIZE..range.end / PAGE_SIZE {
                    let at = page * PAGE_SIZE;
                    let into = moved(at);
                    if source.is_touched(page) {
                        source
                            .read(at, &mut bytes)
                            .map_err(|_| CopyMemoryError::InvalidAddress(InvalidAddressError))?;
                        memory
                            .write(into, &bytes)
                            .map_err(|_| CopyMemoryError::InvalidAddress(InvalidAddressError))?;
                    } else if memory.is_touched(into / PAGE_SIZE) {
                        memory
                            .write(into, &zeros)
                            .map_err(|_| CopyMemoryError::InvalidAddress(InvalidAddressError))?;
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

impl VolatileBytes for UnicornBytes<'_> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let count = copied_count(self.len(), offset, into.len());
        if count == usize::MIN {
            return count;
        }
        let start = self.addresses.start + offset;
        let read = self.space.with_memory(move |memory| {
            let mut read = vec![u8::MIN; count];
            memory
                .read(start, &mut read)
                .expect("borrowed addresses lie inside the space");
            read
        });
        into[..count].copy_from_slice(&read);
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let count = copied_count(self.len(), offset, bytes.len());
        if count == usize::MIN {
            return count;
        }
        let start = self.addresses.start + offset;
        let written = bytes[..count].to_vec();
        self.space.with_memory(move |memory| {
            memory
                .write(start, &written)
                .expect("borrowed addresses lie inside the space");
        });
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

/// Returns how many bytes a copy at the offset moves: the shorter of the buffer and what lies from the offset to the end
/// (`wcontainers_volatile_bytes::copied`).
fn copied_count(length: u64, offset: u64, buffer: usize) -> usize {
    to_count(length.saturating_sub(offset)).min(buffer)
}

/// Returns the length as a count of host bytes, at most the most the host can count.
fn to_count(length: u64) -> usize {
    usize::try_from(length).unwrap_or(usize::MAX)
}
