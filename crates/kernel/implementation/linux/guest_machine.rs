//! The guests' memories and registers, which the kernel keeps on the executor's memory spaces.

use alloc::collections::BTreeMap;
use alloc::vec;

use wcontainers_abi::{
    AddressSpace, CpuError, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, SharedId, ThreadId,
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
pub struct GuestMachine<C: Executor + 'static> {
    executor: C,
    /// The addresses each of the executor's spaces holds: the whole address space until the
    /// container's start bounds it to the ceiling its embedder gives.
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
struct Space<C: Executor + 'static> {
    memory: C::GuestMemory,
    pages: PageTableOf<C>,
}

/// One guest thread's memory and registers while it is not running.
struct GuestThread {
    memory: MemoryId,
    registers: RegisterBlock,
    /// The predicate registers and FFR, which the abi's registers do not hold.
    predicates: Predicates,
}

/// A thread's SVE predicate registers and first-fault register.
///
/// Linux discards them at every system call (<https://docs.kernel.org/arch/arm64/sve.html#:~:text=all%20bits%20of%20the%20predicate%20registers>),
/// so they are kept only across the other ends of a turn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Predicates {
    /// P0 to P15.
    pub(crate) registers: [u16; wcontainers_processor_architecture_arm64::PREDICATE_REGISTERS],
    /// FFR.
    pub(crate) first_fault: u16,
}

impl<C: Executor> GuestMachine<C> {
    /// The executor, with no memory or thread yet.
    pub(crate) fn new(executor: C) -> Self {
        Self {
            executor,
            address_space: AddressSpace {
                end: crate::ARM64_ADDRESS_SPACE_BYTES,
            },
            memories: BTreeMap::new(),
            blocks: BTreeMap::new(),
            threads: BTreeMap::new(),
            last_memory: 0,
            last_block: 0,
            last_thread: 0,
        }
    }
}

impl<C: Executor + 'static> GuestMachine<C> {
    /// Returns the executor, and the registers, predicates and memory space of the thread, for
    /// the runner that runs the thread itself.
    ///
    /// # Panics
    ///
    /// If the kernel has no such thread.
    pub(crate) fn thread_parts(
        &mut self,
        thread: ThreadId,
    ) -> (&mut C, &mut RegisterBlock, &mut Predicates, &C::GuestMemory) {
        let GuestThread {
            memory,
            registers,
            predicates,
        } = self
            .threads
            .get_mut(&thread)
            .expect("the kernel runs a thread it made");
        let memory = self
            .memories
            .get(memory)
            .expect("a thread's memory lives as long as the thread");
        (&mut self.executor, registers, predicates, &memory.memory)
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

impl<C: Executor + 'static> GuestMachine<C> {
    /// Bounds each space to the addresses below `end`, the highest the executor can give, as the
    /// container's start is told it.
    pub(crate) fn bound_spaces(&mut self, end: u64) {
        self.address_space = AddressSpace { end };
    }

    pub(crate) fn make_memory(&mut self) -> Result<MemoryId, CpuError> {
        let space = self.create()?;
        self.last_memory += 1;
        let id = MemoryId(self.last_memory);
        self.memories.insert(id, space);
        Ok(id)
    }

    /// The copy is made by the space, which may share pages until they are written.
    pub(crate) fn duplicate_memory(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
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

    /// Copies every memory into a new space of the executor under the same handle, as a host that
    /// saved them brings them back: the executor keeps nothing of the old spaces' code. The blocks
    /// of shared pages stay, and each copy maps them as its memory did.
    pub(crate) fn renew_memories(&mut self) -> Result<(), CpuError> {
        let whole = self.whole_space();
        let ids: alloc::vec::Vec<MemoryId> = self.memories.keys().copied().collect();
        for id in ids {
            let mut renewed = self.create()?;
            renewed
                .pages
                .copy_from(whole.clone(), &self.space(id)?.pages, whole.start)
                .map_err(not_copied)?;
            self.memories.insert(id, renewed);
        }
        Ok(())
    }

    pub(crate) fn drop_memory(&mut self, memory: MemoryId) {
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
    pub(crate) fn note_changed_code(
        &mut self,
        _memory: MemoryId,
        _address: u64,
        _length: u64,
        _starts: &[u64],
    ) {
    }

    pub(crate) fn make_shared(&mut self, length: u64) -> Result<SharedId, CpuError> {
        if length > self.whole_space().end {
            return Err(CpuError::OutOfMemory);
        }
        let block = self.create()?;
        self.last_block += 1;
        let id = SharedId(self.last_block);
        self.blocks.insert(id, block);
        Ok(id)
    }

    pub(crate) fn map_shared(
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
    pub(crate) fn unmap_shared(
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

    pub(crate) fn read_shared(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        read_space(&self.block(shared)?.memory, offset, into)
    }

    pub(crate) fn write_shared(
        &mut self,
        shared: SharedId,
        offset: u64,
        from: &[u8],
    ) -> Result<(), CpuError> {
        write_space(&self.block(shared)?.memory, offset, from)
    }

    pub(crate) fn drop_shared(&mut self, shared: SharedId) {
        self.blocks.remove(&shared);
    }

    pub(crate) fn make_thread(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        self.memory(memory)?;
        self.last_thread += 1;
        let id = ThreadId(self.last_thread);
        self.threads.insert(
            id,
            GuestThread {
                memory,
                registers: RegisterBlock::ZERO,
                predicates: Predicates::default(),
            },
        );
        Ok(id)
    }

    pub(crate) fn drop_thread(&mut self, thread: ThreadId) {
        self.threads.remove(&thread);
    }

    pub(crate) fn read_registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        *into = self.threads[&thread].registers;
    }

    pub(crate) fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        if let Some(state) = self.threads.get_mut(&thread) {
            state.registers = *from;
        }
    }
}
