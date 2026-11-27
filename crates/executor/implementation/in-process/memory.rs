//! Form I's guest memories: each one a slot of this process's pool, its window showing a memory file of its own with
//! the guest's permissions (docs/native-without-virtual-machine.md section 5.2). Pages mapped from another memory show
//! a shared block, a memory file of their own that every memory mapping them shows, so a memory's own file is never
//! shown by another.

use std::cell::{RefCell, RefMut};
use std::collections::BTreeSet;
use std::ops::Range;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use rangemap::RangeMap;
use rustix::fs::{FallocateFlags, MemfdFlags, SealFlags, SeekFrom};
use rustix::io::Errno;
use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_arm64::way::{IN_PROCESS_SLOTS, OUTSIDE, SLOT_BITS};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_linux_process::{LinuxProcess, ProtFlags};
use wcontainers_linux_process_direct::{DirectFixedMapping, DirectLinuxProcess};
use wcontainers_volatile_bytes::VolatileBytes;

/// The guest addresses of every memory: its window, below the never mapped bytes.
pub const WINDOW: Range<u64> = 0..OUTSIDE;
/// The bytes a slot reserves: its window, then as many never mapped.
const SLOT_BYTES: u64 = 1 << SLOT_BITS;
/// The first slot a memory takes: slot 0's window, at host address 0, is no guest's.
const FIRST_SLOT: u16 = 1;
/// The most bytes copied between memory files in one call.
const COPY_BYTES: u64 = 1 << 30;

/// Why no memory was made.
#[derive(Debug, thiserror::Error)]
pub enum CreateMemoryError {
    #[error("the host's pages are {0} bytes, not the guest's")]
    HostPageSize(usize),
    #[error("every slot of the pool is taken")]
    NoSlot,
    #[error("the host refused the memory: {0}")]
    Host(Errno),
}

/// The slots of this process's pool, each one memory's window and the never mapped bytes after it.
#[derive(Debug, Default)]
pub struct InProcessPool {
    linux: DirectLinuxProcess,
    /// The slots a memory holds.
    held: Mutex<BTreeSet<u16>>,
}

impl InProcessPool {
    /// A pool with every slot free; refused on a host whose pages are not the guest's.
    pub fn new() -> Result<Arc<Self>, CreateMemoryError> {
        let page = rustix::param::page_size();
        if page as u64 != PAGE_SIZE {
            return Err(CreateMemoryError::HostPageSize(page));
        }
        Ok(Arc::new(Self::default()))
    }

    /// A new memory in the first free slot whose addresses nothing else of this process maps: zeros, allowing
    /// nothing.
    pub fn create_memory(
        self: &Arc<Self>,
    ) -> Result<(InProcessMemory, InProcessPageTable), CreateMemoryError> {
        let (slot, mut reserved) = self.reserve_slot()?;
        let own = rustix::fs::memfd_create(
            "wcontainers in-process memory",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .map_err(CreateMemoryError::Host)?;
        rustix::fs::ftruncate(&own, OUTSIDE).map_err(CreateMemoryError::Host)?;
        // A file that cannot change size: no access inside the window is ever past its end.
        rustix::fs::fcntl_add_seals(&own, SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL)
            .map_err(CreateMemoryError::Host)?;
        self.linux
            .map_file_within(
                &mut reserved,
                0,
                OUTSIDE as usize,
                own.as_fd(),
                0,
                ProtFlags::empty(),
            )
            .map_err(CreateMemoryError::Host)?;
        let window = Rc::new(RefCell::new(Window {
            reserved,
            own,
            shown: RangeMap::new(),
            permissions: RangeMap::new(),
            slot: SlotHeld {
                pool: Arc::clone(self),
                slot,
            },
        }));
        Ok((
            InProcessMemory {
                window: Rc::clone(&window),
            },
            InProcessPageTable { window },
        ))
    }

    /// Reserves the first free slot nothing else of this process maps, and holds it.
    fn reserve_slot(&self) -> Result<(u16, DirectFixedMapping), CreateMemoryError> {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        for slot in FIRST_SLOT..=IN_PROCESS_SLOTS {
            if held.contains(&slot) {
                continue;
            }
            let base = u64::from(slot) << SLOT_BITS;
            match self
                .linux
                .mmap_fixed_noreplace(base as usize, SLOT_BYTES as usize)
            {
                Ok(reserved) => {
                    held.insert(slot);
                    return Ok((slot, reserved));
                }
                // Something else of this process maps part of the slot.
                Err(Errno::EXIST) => {}
                Err(other) => return Err(CreateMemoryError::Host(other)),
            }
        }
        Err(CreateMemoryError::NoSlot)
    }
}

/// A slot a memory holds, given back to its pool on drop.
#[derive(Debug)]
struct SlotHeld {
    pool: Arc<InProcessPool>,
    slot: u16,
}

impl Drop for SlotHeld {
    fn drop(&mut self) {
        let mut held = self
            .pool
            .held
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        held.remove(&self.slot);
    }
}

/// A memory file that every memory mapping its pages shows.
#[derive(Clone, Debug)]
struct Block(Rc<OwnedFd>);

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Block {}

/// Pages that show a block: the block and its offset less the address.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Shown {
    block: Block,
    offset_less_address: u64,
}

/// One memory's slot and what its window shows.
#[derive(Debug)]
struct Window {
    /// The slot's addresses, whose window shows the memory. Dropped before `slot`, so the slot is given back unmapped.
    reserved: DirectFixedMapping,
    /// The memory's own bytes, at their addresses; zero wherever a block is shown.
    own: OwnedFd,
    /// The pages that show a block; every other page shows the own file.
    shown: RangeMap<u64, Shown>,
    /// The permissions of every page that allows something.
    permissions: RangeMap<u64, Permissions>,
    slot: SlotHeld,
}

/// A part of a window and where its bytes are: the file and the file's offset of its first byte.
struct Piece<'window> {
    addresses: Range<u64>,
    file: BorrowedFd<'window>,
    offset: u64,
    block: Option<&'window Shown>,
}

impl Window {
    /// The host address of guest address 0.
    fn base(&self) -> u64 {
        u64::from(self.slot.slot) << SLOT_BITS
    }

    /// Where the bytes of the addresses are, in order.
    fn pieces(&self, addresses: Range<u64>) -> Vec<Piece<'_>> {
        let mut pieces = Vec::new();
        if addresses.is_empty() {
            return pieces;
        }
        let mut at = addresses.start;
        let own = |range: Range<u64>| Piece {
            offset: range.start,
            addresses: range,
            file: self.own.as_fd(),
            block: None,
        };
        for (range, shown) in self.shown.overlapping(addresses.clone()) {
            let start = range.start.max(addresses.start);
            let end = range.end.min(addresses.end);
            if at < start {
                pieces.push(own(at..start));
            }
            pieces.push(Piece {
                offset: start.wrapping_add(shown.offset_less_address),
                addresses: start..end,
                file: shown.block.0.as_fd(),
                block: Some(shown),
            });
            at = end;
        }
        if at < addresses.end {
            pieces.push(own(at..addresses.end));
        }
        pieces
    }

    /// Reads the bytes at `start` into `into`, returning how many were read before a host refusal.
    fn read(&self, start: u64, into: &mut [u8]) -> usize {
        let mut done = 0;
        for piece in self.pieces(start..start + into.len() as u64) {
            let part = &mut into[done..done + length(&piece.addresses)];
            let read = read_exactly(piece.file, part, piece.offset);
            done += read;
            if read < part.len() {
                break;
            }
        }
        done
    }

    /// Writes `bytes` at `start`, returning how many were written before a host refusal.
    fn write(&self, start: u64, bytes: &[u8]) -> usize {
        let mut done = 0;
        for piece in self.pieces(start..start + bytes.len() as u64) {
            let part = &bytes[done..done + length(&piece.addresses)];
            let written = write_exactly(piece.file, part, piece.offset);
            done += written;
            if written < part.len() {
                break;
            }
        }
        done
    }

    /// Shows the own file at the window's addresses.
    fn show_own(&mut self, addresses: &Range<u64>) {
        let own = self.own.as_fd();
        show(
            &mut self.reserved,
            &self.permissions,
            addresses,
            own,
            addresses.start,
        );
    }

    /// Gives the host's pages of the addresses their permissions.
    fn protect(&mut self, addresses: &Range<u64>) {
        for (part, allowed) in permission_parts(&self.permissions, addresses) {
            let protected = DirectLinuxProcess.protect_within(
                &mut self.reserved,
                part.start as usize,
                length(&part),
                host_protection(allowed),
            );
            host_change(protected);
        }
    }

    /// Moves the pages of the addresses that show the own file into blocks of their own, so another memory may show
    /// them too.
    fn share(&mut self, addresses: &Range<u64>) -> Result<(), Errno> {
        let owned: Vec<Range<u64>> = (self.pieces(addresses.clone()).into_iter())
            .filter(|piece| piece.block.is_none())
            .map(|piece| piece.addresses)
            .collect();
        for range in owned {
            let block = rustix::fs::memfd_create(
                "wcontainers in-process shared block",
                MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
            )?;
            rustix::fs::ftruncate(&block, range.end - range.start)?;
            rustix::fs::fcntl_add_seals(
                &block,
                SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL,
            )?;
            copy_data(self.own.as_fd(), range.clone(), block.as_fd(), 0)?;
            let block = Block(Rc::new(block));
            self.shown.insert(
                range.clone(),
                Shown {
                    block: block.clone(),
                    offset_less_address: 0_u64.wrapping_sub(range.start),
                },
            );
            show(
                &mut self.reserved,
                &self.permissions,
                &range,
                block.0.as_fd(),
                0,
            );
            punch(self.own.as_fd(), &range)?;
        }
        Ok(())
    }
}

/// A guest memory of form I: a window of the pool, shared with its page table.
#[derive(Debug)]
pub struct InProcessMemory {
    window: Rc<RefCell<Window>>,
}

impl InProcessMemory {
    /// The host address of guest address 0: the slot's base.
    pub fn window_base(&self) -> u64 {
        self.window.borrow().base()
    }

    /// The slot of the pool the memory is in.
    pub fn slot(&self) -> u16 {
        self.window.borrow().slot.slot
    }
}

/// The one holder of a form I memory's pages.
#[derive(Debug)]
pub struct InProcessPageTable {
    window: Rc<RefCell<Window>>,
}

impl InProcessPageTable {
    fn window(&self) -> RefMut<'_, Window> {
        self.window.borrow_mut()
    }
}

/// Bytes of a form I memory, read and written by position in the files its window shows.
pub struct InProcessBytes<'space> {
    window: &'space RefCell<Window>,
    addresses: Range<u64>,
}

impl VolatileBytes for InProcessBytes<'_> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = into.len().min(self.len() as usize);
        self.window
            .borrow()
            .read(self.addresses.start, &mut into[..count])
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = bytes.len().min(self.len() as usize);
        self.window
            .borrow()
            .write(self.addresses.start, &bytes[..count])
    }

    /// The bytes are copied out first, so the sink is given them in one write.
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut buffer = vec![u8::MIN; self.len() as usize];
        let count = self.read(&mut buffer);
        sink.write(&buffer[..count])
    }

    /// The source fills a buffer in one read, which is then copied in.
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut buffer = vec![u8::MIN; self.len() as usize];
        let count = source.read(&mut buffer)?;
        Ok(self.write(&buffer[..count]))
    }
}

impl MemorySpace for InProcessMemory {
    type Bytes<'space> = InProcessBytes<'space>;
    type PageTable = InProcessPageTable;

    fn borrow(&self, addresses: Range<u64>) -> Result<InProcessBytes<'_>, InvalidAddressError> {
        inside(&addresses)?;
        Ok(InProcessBytes {
            window: &self.window,
            addresses,
        })
    }

    /// The files' bytes are freed, which then read as zeros.
    fn zero(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        inside(&addresses)?;
        let window = self.window.borrow();
        for piece in window.pieces(addresses) {
            let range = piece.offset..piece.offset + length(&piece.addresses) as u64;
            host_change(punch(piece.file, &range));
        }
        Ok(())
    }
}

impl PageTable for InProcessPageTable {
    /// Every page the addresses touch gets the permissions. Guest code never runs from its own pages, so execute
    /// gives the host's page nothing.
    fn set_permissions(
        &mut self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError> {
        inside(&addresses)?;
        let pages = page_cover(&addresses);
        if pages.is_empty() {
            return Ok(());
        }
        let mut window = self.window();
        if allowed.is_empty() {
            window.permissions.remove(pages.clone());
        } else {
            window.permissions.insert(pages.clone(), allowed);
        }
        window.protect(&pages);
        Ok(())
    }

    /// Only whole pages are mapped: addresses that do not start and end on a page are refused. The other memory's
    /// pages move into shared blocks first, which both then show.
    fn map_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError> {
        let source =
            whole_pages(&addresses, other_start).map_err(MapMemoryError::InvalidAddress)?;
        if addresses.is_empty() {
            return Ok(());
        }
        let mut other = other.window();
        other
            .share(&source)
            .map_err(|_| MapMemoryError::OutOfMemory)?;
        let shift = other_start.wrapping_sub(addresses.start);
        let mapped: Vec<(Range<u64>, Shown)> = (other.pieces(source).into_iter())
            .map(|piece| {
                let shown = piece.block.expect("every page shared above").clone();
                let range = piece.addresses.start.wrapping_sub(shift)
                    ..piece.addresses.end.wrapping_sub(shift);
                let shown = Shown {
                    offset_less_address: shown.offset_less_address.wrapping_add(shift),
                    block: shown.block,
                };
                (range, shown)
            })
            .collect();
        drop(other);
        let mut window = self.window();
        host_change(punch(window.own.as_fd(), &addresses));
        for (range, shown) in mapped {
            window.shown.insert(range.clone(), shown.clone());
            let offset = range.start.wrapping_add(shown.offset_less_address);
            let window = &mut *window;
            show(
                &mut window.reserved,
                &window.permissions,
                &range,
                shown.block.0.as_fd(),
                offset,
            );
        }
        Ok(())
    }

    /// Every page the addresses touch that shows a block shows the own file again, whose bytes there are zero.
    fn unmap_from(&mut self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        inside(&addresses)?;
        let pages = page_cover(&addresses);
        if pages.is_empty() {
            return Ok(());
        }
        let mut window = self.window();
        let mapped: Vec<Range<u64>> = (window.pieces(pages.clone()).into_iter())
            .filter(|piece| piece.block.is_some())
            .map(|piece| piece.addresses)
            .collect();
        window.shown.remove(pages);
        for range in mapped {
            window.show_own(&range);
        }
        Ok(())
    }

    /// Only whole pages are copied: addresses that do not start and end on a page are refused. Pages of the other
    /// memory's own file are copied into this one's; pages that show a block show it here too.
    fn copy_from(
        &mut self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        let source =
            whole_pages(&addresses, other_start).map_err(CopyMemoryError::InvalidAddress)?;
        if addresses.is_empty() {
            return Ok(());
        }
        let other = other.window();
        let mut window = self.window();
        let shift = other_start.wrapping_sub(addresses.start);
        let back =
            |range: &Range<u64>| range.start.wrapping_sub(shift)..range.end.wrapping_sub(shift);
        window.permissions.remove(addresses.clone());
        for (range, allowed) in other.permissions.overlapping(source.clone()) {
            let range = range.start.max(source.start)..range.end.min(source.end);
            window.permissions.insert(back(&range), *allowed);
        }
        window.shown.remove(addresses.clone());
        host_change(punch(window.own.as_fd(), &addresses));
        for piece in other.pieces(source) {
            let range = back(&piece.addresses);
            match piece.block {
                Some(shown) => {
                    let shown = Shown {
                        offset_less_address: shown.offset_less_address.wrapping_add(shift),
                        block: shown.block.clone(),
                    };
                    window.shown.insert(range.clone(), shown.clone());
                    let window = &mut *window;
                    show(
                        &mut window.reserved,
                        &window.permissions,
                        &range,
                        shown.block.0.as_fd(),
                        piece.offset,
                    );
                }
                None => {
                    copy_data(
                        piece.file,
                        piece.addresses.clone(),
                        window.own.as_fd(),
                        range.start,
                    )
                    .map_err(|_| CopyMemoryError::OutOfMemory)?;
                    window.show_own(&range);
                }
            }
        }
        Ok(())
    }
}

/// Shows the file from `offset` at the window's addresses, each page with its permissions.
fn show(
    reserved: &mut DirectFixedMapping,
    permissions: &RangeMap<u64, Permissions>,
    addresses: &Range<u64>,
    file: BorrowedFd<'_>,
    offset: u64,
) {
    for (part, allowed) in permission_parts(permissions, addresses) {
        let shown = DirectLinuxProcess.map_file_within(
            reserved,
            part.start as usize,
            length(&part),
            file,
            offset + (part.start - addresses.start),
            host_protection(allowed),
        );
        host_change(shown);
    }
}

/// The addresses split where their permissions change, each part with its permissions.
fn permission_parts(
    permissions: &RangeMap<u64, Permissions>,
    addresses: &Range<u64>,
) -> Vec<(Range<u64>, Permissions)> {
    let mut parts = Vec::new();
    if addresses.is_empty() {
        return parts;
    }
    let mut at = addresses.start;
    for (range, allowed) in permissions.overlapping(addresses.clone()) {
        let start = range.start.max(addresses.start);
        let end = range.end.min(addresses.end);
        if at < start {
            parts.push((at..start, Permissions::empty()));
        }
        parts.push((start..end, *allowed));
        at = end;
    }
    if at < addresses.end {
        parts.push((at..addresses.end, Permissions::empty()));
    }
    parts
}

/// What the host's pages allow for what the guest's allow: guest code never runs from them.
fn host_protection(allowed: Permissions) -> ProtFlags {
    let mut protection = ProtFlags::empty();
    protection.set(ProtFlags::READ, allowed.contains(Permissions::READ));
    protection.set(ProtFlags::WRITE, allowed.contains(Permissions::WRITE));
    protection
}

/// A change to the window's host mappings, which the host refuses only when out of memory or mappings.
fn host_change<T>(changed: Result<T, Errno>) -> T {
    changed.unwrap_or_else(|error| panic!("the host refused a change to a guest memory ({error})"))
}

fn length(range: &Range<u64>) -> usize {
    (range.end - range.start) as usize
}

fn inside(addresses: &Range<u64>) -> Result<(), InvalidAddressError> {
    if addresses.start <= addresses.end && addresses.end <= WINDOW.end {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

/// The other memory's addresses of a mapping or copy to `addresses` from `other_start`, both whole pages inside.
fn whole_pages(
    addresses: &Range<u64>,
    other_start: u64,
) -> Result<Range<u64>, InvalidAddressError> {
    inside(addresses)?;
    let length = addresses.end - addresses.start;
    let source = other_start..other_start.checked_add(length).ok_or(InvalidAddressError)?;
    inside(&source)?;
    let whole = |at: u64| at.is_multiple_of(PAGE_SIZE);
    if whole(addresses.start) && whole(length) && whole(other_start) {
        Ok(source)
    } else {
        Err(InvalidAddressError)
    }
}

/// The pages the addresses touch.
fn page_cover(range: &Range<u64>) -> Range<u64> {
    range.start & !(PAGE_SIZE - 1)..range.end.next_multiple_of(PAGE_SIZE)
}

/// Reads the file's bytes at `offset` into `into`, returning how many were read before a refusal or the file's end.
fn read_exactly(file: BorrowedFd<'_>, into: &mut [u8], offset: u64) -> usize {
    let mut done = 0;
    while done < into.len() {
        match rustix::io::pread(file, &mut into[done..], offset + done as u64) {
            Ok(0) | Err(_) => break,
            Ok(read) => done += read,
        }
    }
    done
}

/// Writes `bytes` to the file at `offset`, returning how many were written before a refusal.
fn write_exactly(file: BorrowedFd<'_>, bytes: &[u8], offset: u64) -> usize {
    let mut done = 0;
    while done < bytes.len() {
        match rustix::io::pwrite(file, &bytes[done..], offset + done as u64) {
            Ok(0) | Err(_) => break,
            Ok(written) => done += written,
        }
    }
    done
}

/// Frees the file's bytes in the range, which then read as zeros.
fn punch(file: BorrowedFd<'_>, range: &Range<u64>) -> Result<(), Errno> {
    if range.is_empty() {
        return Ok(());
    }
    rustix::fs::fallocate(
        file,
        FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
        range.start,
        range.end - range.start,
    )
}

/// Copies the bytes of `from`'s range that hold data to `to` from `to_offset`, whose bytes there are zero: holes
/// stay holes.
fn copy_data(
    from: BorrowedFd<'_>,
    range: Range<u64>,
    to: BorrowedFd<'_>,
    to_offset: u64,
) -> Result<(), Errno> {
    let mut at = range.start;
    while at < range.end {
        let start = match rustix::fs::seek(from, SeekFrom::Data(at)) {
            Ok(start) => start,
            // No data at or after `at`.
            Err(Errno::NXIO) => break,
            Err(other) => return Err(other),
        };
        if start >= range.end {
            break;
        }
        let end = rustix::fs::seek(from, SeekFrom::Hole(start))?.min(range.end);
        let mut from_at = start;
        let mut to_at = to_offset + (start - range.start);
        while from_at < end {
            let count = (end - from_at).min(COPY_BYTES) as usize;
            let copied =
                rustix::fs::copy_file_range(from, Some(&mut from_at), to, Some(&mut to_at), count)?;
            if copied == 0 {
                return Err(Errno::IO);
            }
        }
        at = end;
    }
    Ok(())
}
