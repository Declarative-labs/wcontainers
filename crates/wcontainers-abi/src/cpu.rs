//! What the kernel asks of the CPU side.

use crate::{AddressSpace, Permissions, RegisterBlock};

/// One guest memory: the whole address space of one process. Threads of a process share it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MemoryId(pub u32);

/// One guest thread on the CPU side. The kernel keeps its own process and thread numbers; this is only a
/// handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThreadId(pub u32);

/// A block of pages that several memories can map and then see each other's writes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SharedId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuError {
    /// The host has no more memory to give.
    OutOfMemory,
    /// The range is not inside the address space, or not inside the shared block.
    BadAddress,
    /// This host cannot do this at all.
    NotSupported,
}

/// What the kernel asks of the CPU side. Addresses and lengths of page operations are multiples of
/// [`crate::PAGE_SIZE`].
///
/// Reading and writing through this trait ignores page permissions: the kernel keeps its own record of
/// what the guest asked for and checks that itself.
pub trait Cpu {
    /// The addresses every process may use on this host.
    fn address_space(&self) -> AddressSpace;

    /// A fresh memory, all zeros, where every page allows nothing.
    fn memory_new(&mut self) -> Result<MemoryId, CpuError>;
    /// A copy of a memory with the same contents, permissions and shared mappings. Used by fork. A host
    /// may copy at once or only as pages are written.
    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError>;
    /// The memory is no longer needed. Its threads have already been dropped.
    fn memory_drop(&mut self, memory: MemoryId);

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError>;
    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError>;
    /// Fill a range with zeros. A range that has never been used is already zero and needs no call.
    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError>;

    /// Say what a range allows from now on. An access that is not allowed comes back as
    /// [`crate::Exit::MemoryFault`].
    fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError>;

    /// A notice that the bytes in a range changed and may be run, or may no longer be run. `starts` are
    /// addresses in the range where the kernel knows code begins; it may be empty. Nothing comes back. The
    /// CPU side may translate now, later or on first use, but must drop anything it had translated from the
    /// range.
    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, starts: &[u64]);

    /// A new block of shared pages, all zeros.
    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError>;
    /// Make `length` bytes of a memory, at `address`, show the block's pages starting at `offset`. Permissions
    /// are set separately. Mapping anything else over the range, or zeroing it, ends the sharing there.
    fn shared_map(
        &mut self,
        shared: SharedId,
        offset: u64,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError>;
    /// End the sharing in a range of a memory. The memory keeps a private copy of what the pages held.
    fn shared_unmap(&mut self, memory: MemoryId, address: u64, length: u64)
    -> Result<(), CpuError>;
    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError>;
    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError>;
    /// The block is no longer needed. Nothing maps it any more.
    fn shared_drop(&mut self, shared: SharedId);

    /// A new thread on a memory, with all registers zero. It does not run until the kernel says so.
    fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError>;
    fn thread_drop(&mut self, thread: ThreadId);
    /// Read a thread's registers. Only valid while the thread is not running.
    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock);
    /// Replace a thread's registers. The thread must next be run with [`crate::Resume::FromRegisters`].
    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock);
}
