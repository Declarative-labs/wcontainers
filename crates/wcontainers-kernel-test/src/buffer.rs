//! A CPU side that runs no instructions: each memory is a plain buffer of pages. Layer 1 hands system
//! calls straight to the kernel through it.

use std::collections::{BTreeMap, HashMap};

use wcontainers_abi::{
    AddressSpace, Cpu, CpuError, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, SharedId,
    ThreadId,
};

/// The same address space the stand-in gives, so the kernel lays memory out the same way.
const ADDRESS_SPACE_END: u64 = 1 << 32;

type Page = Box<[u8; PAGE_SIZE as usize]>;

#[derive(Default)]
struct Memory {
    /// Pages that hold something other than zeros.
    pages: BTreeMap<u64, Page>,
}

#[derive(Default)]
pub struct BufferCpu {
    memories: HashMap<MemoryId, Memory>,
    threads: HashMap<ThreadId, (MemoryId, RegisterBlock)>,
    last_memory: u32,
    last_thread: u32,
}

fn check(address: u64, length: u64) -> Result<(), CpuError> {
    match address.checked_add(length) {
        Some(end) if end <= ADDRESS_SPACE_END => Ok(()),
        _ => Err(CpuError::BadAddress),
    }
}

impl BufferCpu {
    pub fn new() -> Self {
        Self::default()
    }

    fn memory(&mut self, memory: MemoryId) -> &mut Memory {
        self.memories.get_mut(&memory).expect("memory exists")
    }
}

impl Cpu for BufferCpu {
    fn address_space(&self) -> AddressSpace {
        AddressSpace {
            end: ADDRESS_SPACE_END,
        }
    }

    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        self.last_memory += 1;
        let id = MemoryId(self.last_memory);
        self.memories.insert(id, Memory::default());
        Ok(id)
    }

    fn memory_duplicate(&mut self, _source: MemoryId) -> Result<MemoryId, CpuError> {
        Err(CpuError::NotSupported)
    }

    fn memory_drop(&mut self, memory: MemoryId) {
        self.memories.remove(&memory);
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        check(address, into.len() as u64)?;
        let memory = self.memory(memory);
        for (index, byte) in into.iter_mut().enumerate() {
            let at = address + index as u64;
            *byte = memory
                .pages
                .get(&(at / PAGE_SIZE))
                .map_or(0, |page| page[(at % PAGE_SIZE) as usize]);
        }
        Ok(())
    }

    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        check(address, from.len() as u64)?;
        let memory = self.memory(memory);
        for (index, byte) in from.iter().enumerate() {
            let at = address + index as u64;
            let page = memory
                .pages
                .entry(at / PAGE_SIZE)
                .or_insert_with(|| Box::new([0; PAGE_SIZE as usize]));
            page[(at % PAGE_SIZE) as usize] = *byte;
        }
        Ok(())
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        check(address, length)?;
        let memory = self.memory(memory);
        let mut at = address;
        while at < address + length {
            let page = at / PAGE_SIZE;
            let end = ((page + 1) * PAGE_SIZE).min(address + length);
            if let Some(bytes) = memory.pages.get_mut(&page) {
                bytes[(at % PAGE_SIZE) as usize..(end - page * PAGE_SIZE) as usize].fill(0);
            }
            at = end;
        }
        Ok(())
    }

    fn set_permissions(
        &mut self,
        _: MemoryId,
        address: u64,
        length: u64,
        _: Permissions,
    ) -> Result<(), CpuError> {
        check(address, length)
    }

    fn code_changed(&mut self, _: MemoryId, _: u64, _: u64, _: &[u64]) {}

    fn shared_new(&mut self, _: u64) -> Result<SharedId, CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_map(
        &mut self,
        _: SharedId,
        _: u64,
        _: MemoryId,
        _: u64,
        _: u64,
    ) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_unmap(&mut self, _: MemoryId, _: u64, _: u64) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_read(&mut self, _: SharedId, _: u64, _: &mut [u8]) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_write(&mut self, _: SharedId, _: u64, _: &[u8]) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_drop(&mut self, _: SharedId) {}

    fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        self.last_thread += 1;
        let id = ThreadId(self.last_thread);
        self.threads.insert(id, (memory, RegisterBlock::ZERO));
        Ok(id)
    }

    fn thread_drop(&mut self, thread: ThreadId) {
        self.threads.remove(&thread);
    }

    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        *into = self.threads[&thread].1;
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        self.threads.get_mut(&thread).expect("thread exists").1 = *from;
    }
}
