//! Reading the memory a list of changes leaves, and reporting the changes between two copies of it.

use alloc::borrow::Cow;

use vstd::prelude::*;
use wcontainers_processor_architecture::{MemoryDelta, MemorySink, ProcessorArchitecture};
#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::{apply, assignments, memory_of, zeroed};

use crate::PagePermission;

// VERUS: no specification of `core::array::from_fn`, so this reader of a whole array, which only the fetch calls, stays
// outside `verus!`; each byte it reads is `read_byte`'s.
/// Reads bytes as the memory holds them, whatever its permissions.
pub fn read_bytes<Architecture: ProcessorArchitecture, const LENGTH: usize>(
    memory: &[MemoryDelta<'_, Architecture>],
    start: u64,
) -> [u8; LENGTH] {
    core::array::from_fn(|offset| read_byte(memory, start.wrapping_add(offset as u64)))
}

verus! {

/// The memory a list of changes leaves is its last change applied to what the others leave.
proof fn lemma_memory_of_last<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
)
    requires
        mutations.len() > 0,
    ensures
        memory_of(mutations) == apply(memory_of(mutations.drop_last()), mutations.last()@),
{
    assert(assignments(mutations).drop_last() =~= assignments(mutations.drop_last()));
}

/// The memory no change has reached.
proof fn lemma_memory_of_none<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
)
    requires
        mutations.len() == 0,
    ensures
        memory_of(mutations) == zeroed::<Architecture>(),
{
    assert(assignments(mutations) =~= Seq::empty());
}

/// Reads bytes as the memory holds them into the slice, whatever its permissions.
pub fn read_bytes_into<Architecture: ProcessorArchitecture>(
    memory: &[MemoryDelta<'_, Architecture>],
    start: u64,
    bytes: &mut [u8],
)
    ensures
        final(bytes)@.len() == old(bytes)@.len(),
        forall|offset: int| 0 <= offset < final(bytes)@.len()
            ==> #[trigger] final(bytes)@[offset] == memory_of(memory@)(start.wrapping_add(offset as u64)).0,
{
    // VERUS: no specification of `iter_mut` or `enumerate`, so the bytes are read by index.
    let mut offset: usize = 0;
    while offset < bytes.len()
        invariant
            bytes@.len() == old(bytes)@.len(),
            offset <= bytes@.len(),
            forall|at: int| 0 <= at < offset
                ==> #[trigger] bytes@[at] == memory_of(memory@)(start.wrapping_add(at as u64)).0,
        decreases bytes@.len() - offset,
    {
        bytes[offset] = read_byte(memory, start.wrapping_add(offset as u64));
        offset += 1;
    }
}

/// The byte the memory holds at the address, whatever its permissions.
fn read_byte<Architecture: ProcessorArchitecture>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
) -> (byte: u8)
    ensures
        byte == memory_of(memory@)(address).0,
{
    // VERUS: no specification of `Iterator::rev` or `find_map`, so the changes are walked back by index.
    let mut index = memory.len();
    proof {
        assert(memory@.take(index as int) =~= memory@);
    }
    while index > 0
        invariant
            index <= memory@.len(),
            memory_of(memory@)(address).0 == memory_of(memory@.take(index as int))(address).0,
        decreases index,
    {
        proof {
            lemma_memory_of_last(memory@.take(index as int));
            assert(memory@.take(index as int).drop_last() =~= memory@.take(index - 1));
            assert(memory@.take(index as int).last() == memory@[index - 1]);
        }
        match &memory[index - 1] {
            MemoryDelta::ValueAssignment { start, bytes } => {
                // VERUS: no deref of a `Cow`, so its two cases are matched.
                let bytes: &[u8] = match bytes {
                    Cow::Borrowed(borrowed) => borrowed,
                    Cow::Owned(owned) => owned.as_slice(),
                };
                let offset = address.wrapping_sub(*start);
                if offset < bytes.len() as u64 {
                    return bytes[offset as usize];
                }
            },
            MemoryDelta::PermissionAssignment { .. } => {},
        }
        index -= 1;
    }
    proof {
        lemma_memory_of_none(memory@.take(0));
    }
    0
}

/// What the memory lets a program do at the address.
pub fn read_permission<Architecture>(
    memory: &[MemoryDelta<'_, Architecture>],
    address: u64,
) -> (permission: PagePermission)
    where
        Architecture: ProcessorArchitecture<PagePermission = PagePermission>,
    ensures
        permission == memory_of(memory@)(address).1,
{
    // VERUS: no specification of `Iterator::rev` or `find_map`, so the changes are walked back by index.
    let mut index = memory.len();
    proof {
        assert(memory@.take(index as int) =~= memory@);
    }
    while index > 0
        invariant
            index <= memory@.len(),
            memory_of(memory@)(address).1 == memory_of(memory@.take(index as int))(address).1,
        decreases index,
    {
        proof {
            lemma_memory_of_last(memory@.take(index as int));
            assert(memory@.take(index as int).drop_last() =~= memory@.take(index - 1));
            assert(memory@.take(index as int).last() == memory@[index - 1]);
        }
        match &memory[index - 1] {
            MemoryDelta::PermissionAssignment { addresses, permission } => {
                // VERUS: no specification of `Range::contains`, so the bounds are compared.
                if addresses.start <= address && address < addresses.end {
                    return *permission;
                }
            },
            MemoryDelta::ValueAssignment { .. } => {},
        }
        index -= 1;
    }
    proof {
        lemma_memory_of_none(memory@.take(0));
    }
    Architecture::DEFAULT_PERMISSION
}

} // verus!

/// Reports each maximal run of bytes that differs between two copies of memory starting at `start`.
pub fn report_changed_bytes<Architecture: ProcessorArchitecture>(
    start: u64,
    before: &[u8],
    after: &[u8],
    mutation_hook: &mut impl MemorySink<Architecture>,
) {
    /// Bytes compared at once before looking at single ones: on a Neoverse V2 this took a warm batch of the single
    /// step machine from 4.9 to 4.3 microseconds a state, comparing two pages for each.
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
            mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                start: start + first as u64,
                bytes: Cow::Borrowed(&after[first..offset]),
            });
        }
    }
}
