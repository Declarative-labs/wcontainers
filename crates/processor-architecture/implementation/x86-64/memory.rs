//! Reading the memory a list of changes leaves, and reporting the changes between two copies of it.

use alloc::borrow::Cow;

use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture};

use crate::PagePermission;

pub const PAGE_BYTES: u64 = 1 << 12;

/// Reads bytes as the memory holds them, whatever its permissions.
pub fn read_bytes<Architecture: ProcessorArchitecture, const LENGTH: usize>(
    memory: &[MemoryDelta<'_, Architecture>],
    start: u64,
) -> [u8; LENGTH] {
    core::array::from_fn(|offset| read_byte(memory, start.wrapping_add(offset as u64)))
}

fn read_byte<Architecture: ProcessorArchitecture>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
) -> u8 {
    memory
        .iter()
        .rev()
        .find_map(|delta| match delta {
            MemoryDelta::ValueAssignment { start, bytes } => {
                usize::try_from(address.wrapping_sub(*start))
                    .ok()
                    .and_then(|offset| bytes.get(offset))
                    .copied()
            }
            MemoryDelta::PermissionAssignment { .. } => None,
        })
        .unwrap_or_default()
}

pub fn read_permission<Architecture>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
) -> PagePermission
where
    Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
{
    memory
        .iter()
        .rev()
        .find_map(|delta| match delta {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => addresses.contains(&address).then_some(*permission),
            MemoryDelta::ValueAssignment { .. } => None,
        })
        .unwrap_or(PagePermission::Unmapped)
}

/// Reports each maximal run of bytes that differs between two copies of memory starting at `start`.
pub fn report_changed_bytes<Architecture: ProcessorArchitecture>(
    start: u64,
    before: &[u8],
    after: &[u8],
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, Architecture>),
) {
    const CHUNK_BYTES: usize = 64;
    let mut offset = 0;
    while offset < after.len() {
        let chunk_end = (offset + CHUNK_BYTES).min(after.len());
        if before[offset..chunk_end] == after[offset..chunk_end] {
            offset = chunk_end;
            continue;
        }
        while offset < chunk_end {
            if after[offset] == before[offset] {
                offset += 1;
                continue;
            }
            let first = offset;
            while offset < after.len() && after[offset] != before[offset] {
                offset += 1;
            }
            on_memory_mutated(MemoryDelta::ValueAssignment {
                start: start + first as u64,
                bytes: Cow::Borrowed(&after[first..offset]),
            });
        }
    }
}
