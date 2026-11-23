use thiserror::Error;

use crate::{AddressType, Limits};

/// The part of an engine's [store](https://webassembly.github.io/spec/core/exec/runtime.html#store) that holds its [memory instances](https://webassembly.github.io/spec/core/exec/runtime.html#memory-instances).
pub trait MemoryStore {
    /// The [address](https://webassembly.github.io/spec/core/exec/runtime.html#addresses) of a [memory instance](https://webassembly.github.io/spec/core/exec/runtime.html#memory-instances).
    type MemoryAddress: Clone;

    type AllocateMemoryError;
    /// Allocates a [memory instance](https://webassembly.github.io/spec/core/exec/runtime.html#memory-instances) of the given type with every byte zero, as [`mem_alloc`](https://webassembly.github.io/spec/core/appendix/embedding.html#embed-mem-alloc) does.
    fn allocate_memory(
        &mut self,
        memory_type: MemoryType,
    ) -> Result<Self::MemoryAddress, Self::AllocateMemoryError>;

    /// Returns the size of the memory in [pages](https://webassembly.github.io/spec/core/exec/runtime.html#page-size).
    fn count_memory_pages(&self, memory: &Self::MemoryAddress) -> u64;

    /// Grows the memory by the given number of pages, as [`mem_grow`](https://webassembly.github.io/spec/core/appendix/embedding.html#embed-mem-grow) does.
    fn grow_memory(
        &mut self,
        memory: &Self::MemoryAddress,
        pages: u64,
    ) -> Result<(), GrowMemoryError>;

    /// Copies the bytes of the memory that start at the offset into the given bytes.
    fn read_memory(
        &self,
        memory: &Self::MemoryAddress,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), ReadMemoryError>;

    /// Copies the given bytes into the memory, starting at the offset.
    fn write_memory(
        &mut self,
        memory: &Self::MemoryAddress,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), WriteMemoryError>;

    /// A shared memory that belongs to no engine, so that it can be sent to another thread.
    type SharedMemory: Clone + Send + Sync;

    /// Returns the memory in the form another engine can add, or nothing when the memory is unshared.
    fn to_shared_memory(&self, memory: &Self::MemoryAddress) -> Option<Self::SharedMemory>;

    fn add_shared_memory(&mut self, memory: &Self::SharedMemory) -> Self::MemoryAddress;
}

/// The [page size](https://webassembly.github.io/spec/core/exec/runtime.html#page-size) of a memory, in bytes.
pub const PAGE_SIZE: u64 = 65536;

/// A [memory type](https://webassembly.github.io/spec/core/syntax/types.html#memory-types).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryType {
    pub address_type: AddressType,
    pub limits: Limits,
    pub sharing: Sharing,
}

/// The [sharing](https://webassembly.github.io/threads/core/syntax/types.html#memory-types) of a memory between threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sharing {
    Unshared,
    Shared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the memory could not be grown by the given number of pages")]
pub struct GrowMemoryError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the bytes to read do not all lie inside the memory")]
pub struct ReadMemoryError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("the bytes to write do not all fit inside the memory")]
pub struct WriteMemoryError;
