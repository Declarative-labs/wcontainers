//! Guest memory as owned pages, shared between spaces only where a mapping shares them.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use wcontainers_abi::{PAGE_MASK, PAGE_SIZE, Permissions};
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, VolatileBytes,
};
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, InstructionFetch, PagePermission,
};

const PAGE_BYTES: usize = PAGE_SIZE as usize;

/// The bytes of one page, held by every space that maps it.
type Frame = Arc<Mutex<Box<[u8]>>>;

/// One page of a space; a page with no entry reads as zeros and allows nothing.
#[derive(Clone)]
struct Page {
    /// None while the page reads as zeros and no space shares it.
    frame: Option<Frame>,
    /// Whether the frame is shared through a mapping, so that a copy of the page shares it too.
    mapped: bool,
    permissions: Permissions,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            frame: None,
            mapped: false,
            permissions: Permissions::empty(),
        }
    }
}

/// A guest's address space, every page of it owned or shared in host memory.
///
/// The page methods (`set_permissions`, `map_from`, `unmap_from` and `copy_from`) take whole pages: addresses that
/// do not start and end at a page boundary are refused. A copy copies the bytes at once rather than on a write.
#[derive(Default)]
pub struct SimulatedMemory {
    /// The pages that have bytes, a mapping or permissions, by their first address.
    pages: Mutex<BTreeMap<u64, Page>>,
}

/// Addresses of a [`SimulatedMemory`], which another thread's guest may be changing while they are held.
pub struct SimulatedBytes<'space> {
    space: &'space SimulatedMemory,
    addresses: Range<u64>,
}

impl SimulatedMemory {
    /// Returns every page's permission and bytes as the architecture's memory changes, from a space of zeros.
    pub(crate) fn to_memory_deltas(&self) -> Vec<MemoryDelta<'static, Arm64>> {
        let pages = self.lock_pages();
        let mut deltas = Vec::new();
        for (&start, page) in pages.iter() {
            if let Some(permission) = to_page_permission(page.permissions) {
                deltas.push(MemoryDelta::PermissionAssignment {
                    addresses: start..start + PAGE_SIZE,
                    permission,
                });
            }
            if let Some(frame) = &page.frame {
                deltas.push(MemoryDelta::ValueAssignment {
                    start,
                    bytes: Cow::Owned(lock_frame(frame).to_vec()),
                });
            }
        }
        deltas
    }

    /// Applies a change an instruction made, whatever the permissions.
    pub(crate) fn apply(&self, delta: MemoryDelta<'_, Arm64>) {
        match delta {
            MemoryDelta::ValueAssignment { start, bytes } => {
                self.write_at(start, &bytes);
            }
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => {
                let mut pages = self.lock_pages();
                for start in page_starts(&addresses) {
                    pages.entry(start).or_default().permissions = to_permissions(permission);
                }
            }
        }
    }

    fn lock_pages(&self) -> MutexGuard<'_, BTreeMap<u64, Page>> {
        self.pages.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn find_frame(&self, page: u64) -> Option<Frame> {
        self.lock_pages()
            .get(&page)
            .and_then(|found| found.frame.clone())
    }

    /// Returns the frame of the page, given one of zeros if it had none.
    fn make_frame(&self, page: u64) -> Frame {
        self.lock_pages()
            .entry(page)
            .or_default()
            .frame
            .get_or_insert_with(new_frame)
            .clone()
    }

    /// Returns the frame of the page for a mapping, which shares it from now on.
    fn share_frame(&self, page: u64) -> Frame {
        let mut pages = self.lock_pages();
        let shared = pages.entry(page).or_default();
        shared.mapped = true;
        shared.frame.get_or_insert_with(new_frame).clone()
    }

    /// Returns the number of bytes read, starting at the address.
    fn read_at(&self, start: u64, into: &mut [u8]) -> usize {
        for (page, within, buffer) in split_at_pages(start, into.len()) {
            match self.find_frame(page) {
                Some(frame) => into[buffer].copy_from_slice(&lock_frame(&frame)[within]),
                None => into[buffer].fill(u8::MIN),
            }
        }
        into.len()
    }

    /// Returns the number of bytes written, starting at the address.
    fn write_at(&self, start: u64, bytes: &[u8]) -> usize {
        for (page, within, buffer) in split_at_pages(start, bytes.len()) {
            lock_frame(&self.make_frame(page))[within].copy_from_slice(&bytes[buffer]);
        }
        bytes.len()
    }
}

impl MemorySpace for SimulatedMemory {
    type Bytes<'space> = SimulatedBytes<'space>;

    fn borrow(&self, addresses: Range<u64>) -> Result<SimulatedBytes<'_>, InvalidAddressError> {
        check_inside(&addresses)?;
        Ok(SimulatedBytes {
            space: self,
            addresses,
        })
    }

    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError> {
        check_pages(&addresses)?;
        let mut pages = self.lock_pages();
        for start in page_starts(&addresses) {
            pages.entry(start).or_default().permissions = allowed;
        }
        Ok(())
    }

    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError> {
        check_pages(&addresses).map_err(MapMemoryError::InvalidAddress)?;
        let other_addresses =
            shift(&addresses, other_start).map_err(MapMemoryError::InvalidAddress)?;
        let frames: Vec<Frame> = page_starts(&other_addresses)
            .map(|page| other.share_frame(page))
            .collect();
        let mut pages = self.lock_pages();
        for (start, frame) in page_starts(&addresses).zip(frames) {
            let page = pages.entry(start).or_default();
            page.frame = Some(frame);
            page.mapped = true;
        }
        Ok(())
    }

    fn unmap_from(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        check_pages(&addresses)?;
        let mut pages = self.lock_pages();
        for (_, page) in pages.range_mut(addresses) {
            if page.mapped {
                page.frame = None;
                page.mapped = false;
            }
        }
        Ok(())
    }

    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        check_pages(&addresses).map_err(CopyMemoryError::InvalidAddress)?;
        let other_addresses =
            shift(&addresses, other_start).map_err(CopyMemoryError::InvalidAddress)?;
        let copied: Vec<Option<Page>> = {
            let other_pages = other.lock_pages();
            page_starts(&other_addresses)
                .map(|start| other_pages.get(&start).map(copy_page))
                .collect()
        };
        let mut pages = self.lock_pages();
        for (start, page) in page_starts(&addresses).zip(copied) {
            match page {
                Some(page) => pages.insert(start, page),
                None => pages.remove(&start),
            };
        }
        Ok(())
    }
}

impl VolatileBytes for SimulatedBytes<'_> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = into.len().min(to_count(self.len()));
        self.space.read_at(self.addresses.start, &mut into[..count])
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = bytes.len().min(to_count(self.len()));
        self.space.write_at(self.addresses.start, &bytes[..count])
    }
}

fn new_frame() -> Frame {
    Arc::new(Mutex::new(vec![u8::MIN; PAGE_BYTES].into_boxed_slice()))
}

fn lock_frame(frame: &Frame) -> MutexGuard<'_, Box<[u8]>> {
    frame.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Returns the page as a copy carries it: a mapped frame shared, an owned one copied.
fn copy_page(page: &Page) -> Page {
    let frame = match &page.frame {
        Some(frame) if page.mapped => Some(Arc::clone(frame)),
        Some(frame) => Some(Arc::new(Mutex::new(lock_frame(frame).clone()))),
        None => None,
    };
    Page {
        frame,
        mapped: page.mapped,
        permissions: page.permissions,
    }
}

fn check_inside(addresses: &Range<u64>) -> Result<(), InvalidAddressError> {
    let bounds = Arm64::ADDRESSABLE.start..=Arm64::ADDRESSABLE.end;
    let inside = bounds.contains(&addresses.start)
        && bounds.contains(&addresses.end)
        && addresses.start <= addresses.end;
    if inside {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

fn check_pages(addresses: &Range<u64>) -> Result<(), InvalidAddressError> {
    check_inside(addresses)?;
    if addresses.start & PAGE_MASK == u64::MIN && addresses.end & PAGE_MASK == u64::MIN {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

/// Returns the addresses as long as the given ones that start at the start, refused unless they are whole pages.
fn shift(addresses: &Range<u64>, start: u64) -> Result<Range<u64>, InvalidAddressError> {
    let end = start
        .checked_add(addresses.end - addresses.start)
        .ok_or(InvalidAddressError)?;
    check_pages(&(start..end))?;
    Ok(start..end)
}

/// Returns the first address of each page of addresses that start a page.
fn page_starts(addresses: &Range<u64>) -> impl Iterator<Item = u64> + use<> {
    addresses.clone().step_by(PAGE_BYTES)
}

/// Returns each page the bytes from the start touch, with the bytes' place in the page and in the buffer.
fn split_at_pages(start: u64, length: usize) -> Vec<(u64, Range<usize>, Range<usize>)> {
    let mut parts = Vec::new();
    let mut done = usize::MIN;
    while done < length {
        let address = start + done as u64;
        let within = to_count(address & PAGE_MASK);
        let count = (PAGE_BYTES - within).min(length - done);
        parts.push((
            address & !PAGE_MASK,
            within..within + count,
            done..done + count,
        ));
        done += count;
    }
    parts
}

/// Returns the length as a count of host bytes, at most the most the host can count.
fn to_count(length: u64) -> usize {
    usize::try_from(length).unwrap_or(usize::MAX)
}

/// Returns what the page allows as ARM64 sees it; a page that allows nothing is held unmapped.
fn to_page_permission(permissions: Permissions) -> Option<PagePermission> {
    if permissions.is_empty() {
        return None;
    }
    // ARM64 has no page that can be written and not read; Linux gives such a page read and write.
    let data_access = if permissions.contains(Permissions::WRITE) {
        DataAccess::ReadWrite
    } else if permissions.contains(Permissions::READ) {
        DataAccess::ReadOnly
    } else {
        DataAccess::Forbidden
    };
    let instruction_fetch = if permissions.contains(Permissions::EXECUTE) {
        InstructionFetch::Allowed
    } else {
        InstructionFetch::Forbidden
    };
    Some(PagePermission::Mapped {
        data_access,
        instruction_fetch,
    })
}

fn to_permissions(permission: PagePermission) -> Permissions {
    match permission {
        PagePermission::Unmapped => Permissions::empty(),
        PagePermission::Mapped {
            data_access,
            instruction_fetch,
        } => {
            let data = match data_access {
                DataAccess::Forbidden => Permissions::empty(),
                DataAccess::ReadOnly => Permissions::READ,
                DataAccess::ReadWrite => Permissions::READ | Permissions::WRITE,
            };
            match instruction_fetch {
                InstructionFetch::Forbidden => data,
                InstructionFetch::Allowed => data | Permissions::EXECUTE,
            }
        }
    }
}
