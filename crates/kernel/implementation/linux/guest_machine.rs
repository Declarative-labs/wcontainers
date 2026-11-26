//! The guests' memories and registers, which the kernel keeps itself on the executor's memory
//! spaces, and the calls the rest of the kernel makes on them.

use alloc::collections::BTreeMap;
use alloc::vec;

use wcontainers_abi::{
    AddressSpace, Cpu, CpuError, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, SharedId,
    ThreadId,
};
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_executor::{Executor, PageTableOf};
use wcontainers_volatile_bytes::VolatileBytes;

/// How many bytes ending a range's sharing moves at a time, so a large range needs no buffer of
/// its own size.
const UNSHARED_PIECE_BYTES: u64 = 16 * PAGE_SIZE;

/// An executor with the memory spaces and registers of the guest threads it runs.
pub struct GuestMachine<C: Executor> {
    executor: C,
    /// The addresses each of the executor's spaces holds.
    address_space: AddressSpace,
    memories: BTreeMap<MemoryId, Space<C>>,
    /// Each block of shared pages is a space of its own, its pages from address 0.
    blocks: BTreeMap<SharedId, Space<C>>,
    threads: BTreeMap<ThreadId, GuestThread>,
    last_memory: u32,
    last_block: u32,
    last_thread: u32,
}

/// A memory space and its pages, which the machine alone holds and changes.
struct Space<C: Executor> {
    memory: C::GuestMemory,
    pages: PageTableOf<C>,
}

/// One guest thread's memory and registers while it is not running.
struct GuestThread {
    memory: MemoryId,
    registers: RegisterBlock,
}

impl<C: Executor + Cpu> GuestMachine<C> {
    /// The executor, with no memory or thread yet.
    ///
    /// The size of its spaces is the one thing the kernel still asks of the abi's `Cpu`: no
    /// interface tells it.
    pub(crate) fn new(executor: C) -> Self {
        let address_space = executor.address_space();
        Self {
            executor,
            address_space,
            memories: BTreeMap::new(),
            blocks: BTreeMap::new(),
            threads: BTreeMap::new(),
            last_memory: 0,
            last_block: 0,
            last_thread: 0,
        }
    }
}

impl<C: Executor> GuestMachine<C> {
    /// Returns the executor, and the registers and memory space of the thread, for a driver that
    /// runs the thread itself.
    ///
    /// # Panics
    ///
    /// If the kernel has no such thread.
    pub fn thread_parts(
        &mut self,
        thread: ThreadId,
    ) -> (&mut C, &mut RegisterBlock, &C::GuestMemory) {
        let GuestThread { memory, registers } = self
            .threads
            .get_mut(&thread)
            .expect("the kernel runs a thread it made");
        let memory = self
            .memories
            .get(memory)
            .expect("a thread's memory lives as long as the thread");
        (&mut self.executor, registers, &memory.memory)
    }

    fn memory(&self, memory: MemoryId) -> Result<&C::GuestMemory, CpuError> {
        self.space(memory).map(|space| &space.memory)
    }

    fn space(&self, memory: MemoryId) -> Result<&Space<C>, CpuError> {
        self.memories.get(&memory).ok_or(CpuError::BadAddress)
    }

    fn space_mut(&mut self, memory: MemoryId) -> Result<&mut Space<C>, CpuError> {
        self.memories.get_mut(&memory).ok_or(CpuError::BadAddress)
    }

    fn block(&self, shared: SharedId) -> Result<&Space<C>, CpuError> {
        self.blocks.get(&shared).ok_or(CpuError::BadAddress)
    }

    fn create(&self) -> Result<Space<C>, CpuError> {
        let (memory, pages) = self
            .executor
            .create_guest_memory()
            .map_err(|_| CpuError::OutOfMemory)?;
        Ok(Space { memory, pages })
    }

    /// The addresses of a whole space.
    fn whole_space(&self) -> core::ops::Range<u64> {
        0..self.address_space.end
    }
}

/// Returns the addresses from `address` that hold `length` bytes.
fn span(address: u64, length: u64) -> Result<core::ops::Range<u64>, CpuError> {
    let end = address.checked_add(length).ok_or(CpuError::BadAddress)?;
    Ok(address..end)
}

fn length_of(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).expect("a buffer's length fits in an address")
}

/// Reads all of `into` from the addresses of `space` starting at `address`.
fn read_space<M: MemorySpace>(space: &M, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
    let bytes = space
        .borrow(span(address, length_of(into))?)
        .map_err(invalid)?;
    if bytes.read(into) == into.len() {
        Ok(())
    } else {
        Err(CpuError::BadAddress)
    }
}

/// Writes all of `from` to the addresses of `space` starting at `address`.
fn write_space<M: MemorySpace>(space: &M, address: u64, from: &[u8]) -> Result<(), CpuError> {
    let bytes = space
        .borrow(span(address, length_of(from))?)
        .map_err(invalid)?;
    if bytes.write(from) == from.len() {
        Ok(())
    } else {
        Err(CpuError::BadAddress)
    }
}

fn invalid(_: InvalidAddressError) -> CpuError {
    CpuError::BadAddress
}

fn not_mapped(error: MapMemoryError) -> CpuError {
    match error {
        MapMemoryError::InvalidAddress(_) => CpuError::BadAddress,
        MapMemoryError::OutOfMemory => CpuError::OutOfMemory,
    }
}

fn not_copied(error: CopyMemoryError) -> CpuError {
    match error {
        CopyMemoryError::InvalidAddress(_) => CpuError::BadAddress,
        CopyMemoryError::OutOfMemory => CpuError::OutOfMemory,
    }
}

impl<C: Executor> GuestMachine<C> {
    /// Returns the addresses each space holds.
    pub(crate) fn address_space(&self) -> AddressSpace {
        self.address_space
    }

    pub(crate) fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        let space = self.create()?;
        self.last_memory += 1;
        let id = MemoryId(self.last_memory);
        self.memories.insert(id, space);
        Ok(id)
    }

    /// The copy is made by the space, which may share pages until they are written.
    pub(crate) fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        let mut copy = self.create()?;
        let whole = self.whole_space();
        let start = whole.start;
        copy.pages
            .copy_from(whole, &self.space(source)?.pages, start)
            .map_err(not_copied)?;
        self.last_memory += 1;
        let id = MemoryId(self.last_memory);
        self.memories.insert(id, copy);
        Ok(id)
    }

    pub(crate) fn memory_drop(&mut self, memory: MemoryId) {
        self.memories.remove(&memory);
    }

    pub(crate) fn read(
        &mut self,
        memory: MemoryId,
        address: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        read_space(self.memory(memory)?, address, into)
    }

    pub(crate) fn write(
        &mut self,
        memory: MemoryId,
        address: u64,
        from: &[u8],
    ) -> Result<(), CpuError> {
        write_space(self.memory(memory)?, address, from)
    }

    pub(crate) fn zero(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.memory(memory)?
            .zero(span(address, length)?)
            .map_err(invalid)
    }

    pub(crate) fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        self.space_mut(memory)?
            .pages
            .set_permissions(span(address, length)?, permissions)
            .map_err(invalid)
    }

    /// Nothing to do: a memory space's executor sees every write made through the space, the
    /// kernel's too.
    pub(crate) fn code_changed(
        &mut self,
        _memory: MemoryId,
        _address: u64,
        _length: u64,
        _starts: &[u64],
    ) {
    }

    pub(crate) fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        if length > self.whole_space().end {
            return Err(CpuError::OutOfMemory);
        }
        let block = self.create()?;
        self.last_block += 1;
        let id = SharedId(self.last_block);
        self.blocks.insert(id, block);
        Ok(id)
    }

    pub(crate) fn shared_map(
        &mut self,
        shared: SharedId,
        offset: u64,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        let block = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
        let space = self.memories.get_mut(&memory).ok_or(CpuError::BadAddress)?;
        space
            .pages
            .map_from(span(address, length)?, &block.pages, offset)
            .map_err(not_mapped)
    }

    /// The space zeroes what it unmaps, so the bytes are copied out and written back in pieces.
    pub(crate) fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        let Space { memory, pages } = self.space_mut(memory)?;
        let range = span(address, length)?;
        let mut piece = vec![0; usize::try_from(UNSHARED_PIECE_BYTES.min(length)).unwrap_or(0)];
        let mut at = range.start;
        while at < range.end {
            let end = range.end.min(at.saturating_add(UNSHARED_PIECE_BYTES));
            let bytes = &mut piece[..usize::try_from(end - at).expect("a piece fits in memory")];
            read_space(&*memory, at, bytes)?;
            pages.unmap_from(at..end).map_err(invalid)?;
            write_space(&*memory, at, bytes)?;
            at = end;
        }
        Ok(())
    }

    pub(crate) fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        read_space(&self.block(shared)?.memory, offset, into)
    }

    pub(crate) fn shared_write(
        &mut self,
        shared: SharedId,
        offset: u64,
        from: &[u8],
    ) -> Result<(), CpuError> {
        write_space(&self.block(shared)?.memory, offset, from)
    }

    pub(crate) fn shared_drop(&mut self, shared: SharedId) {
        self.blocks.remove(&shared);
    }

    pub(crate) fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        self.memory(memory)?;
        self.last_thread += 1;
        let id = ThreadId(self.last_thread);
        self.threads.insert(
            id,
            GuestThread {
                memory,
                registers: RegisterBlock::ZERO,
            },
        );
        Ok(id)
    }

    pub(crate) fn thread_drop(&mut self, thread: ThreadId) {
        self.threads.remove(&thread);
    }

    pub(crate) fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        *into = self.threads[&thread].registers;
    }

    pub(crate) fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        if let Some(state) = self.threads.get_mut(&thread) {
            state.registers = *from;
        }
    }
}
