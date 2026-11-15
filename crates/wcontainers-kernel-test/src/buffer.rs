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

/// Pages that hold something other than zeros, by page number.
type Pages = BTreeMap<u64, Page>;

#[derive(Default)]
struct Memory {
    pages: Pages,
    /// Ranges that show a block of shared pages: first page to (page after the last, block, the
    /// block's page shown first). Their bytes are the block's; `pages` holds nothing there.
    shared: BTreeMap<u64, (u64, SharedId, u64)>,
}

impl Memory {
    /// The block page a memory page shows, if it shows one.
    fn shared_page(&self, page: u64) -> Option<(SharedId, u64)> {
        let (start, (end, block, first)) = self.shared.range(..=page).next_back()?;
        (page < *end).then_some((*block, first + (page - start)))
    }

    /// Takes `[start, end)` (in pages) out of the shared ranges, splitting those at its ends, and
    /// returns the parts taken.
    fn unshare(&mut self, start: u64, end: u64) -> Vec<(u64, u64, SharedId, u64)> {
        let overlapping: Vec<u64> = self
            .shared
            .range(..end)
            .filter(|(_, (range_end, _, _))| *range_end > start)
            .map(|(range_start, _)| *range_start)
            .collect();
        let mut taken = Vec::new();
        for range_start in overlapping {
            let (range_end, block, first) = self.shared.remove(&range_start).expect("range");
            if range_start < start {
                self.shared.insert(range_start, (start, block, first));
            }
            if range_end > end {
                self.shared
                    .insert(end, (range_end, block, first + (end - range_start)));
            }
            let from = range_start.max(start);
            taken.push((
                from,
                range_end.min(end),
                block,
                first + (from - range_start),
            ));
        }
        taken
    }
}

#[derive(Default)]
pub struct BufferCpu {
    memories: HashMap<MemoryId, Memory>,
    threads: HashMap<ThreadId, (MemoryId, RegisterBlock)>,
    /// Blocks of shared pages: their length and the pages that hold something.
    blocks: HashMap<SharedId, (u64, Pages)>,
    last_memory: u32,
    last_thread: u32,
    last_block: u32,
}

fn check(address: u64, length: u64) -> Result<(), CpuError> {
    match address.checked_add(length) {
        Some(end) if end <= ADDRESS_SPACE_END => Ok(()),
        _ => Err(CpuError::BadAddress),
    }
}

/// Copies between `bytes` and the pages from byte `address` on, one page at a time.
fn each_page(address: u64, length: usize, mut f: impl FnMut(u64, std::ops::Range<usize>, usize)) {
    let mut done = 0;
    while done < length {
        let at = address + done as u64;
        let within = (at % PAGE_SIZE) as usize;
        let chunk = (PAGE_SIZE as usize - within).min(length - done);
        f(at / PAGE_SIZE, within..within + chunk, done);
        done += chunk;
    }
}

impl BufferCpu {
    pub fn new() -> Self {
        Self::default()
    }

    /// The pages a memory page lives in: the memory's own, or a block's.
    fn pages_of(&mut self, memory: MemoryId, page: u64) -> (&mut Pages, u64) {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        match memory.shared_page(page) {
            Some((block, block_page)) => (
                &mut self.blocks.get_mut(&block).expect("block exists").1,
                block_page,
            ),
            None => (&mut memory.pages, page),
        }
    }
}

fn read_pages(pages: &Pages, page: u64, range: std::ops::Range<usize>, into: &mut [u8]) {
    match pages.get(&page) {
        Some(bytes) => into.copy_from_slice(&bytes[range]),
        None => into.fill(0),
    }
}

fn write_pages(pages: &mut Pages, page: u64, range: std::ops::Range<usize>, from: &[u8]) {
    pages
        .entry(page)
        .or_insert_with(|| Box::new([0; PAGE_SIZE as usize]))[range]
        .copy_from_slice(from);
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
        each_page(address, into.len(), |page, range, done| {
            let (pages, page) = self.pages_of(memory, page);
            let length = range.len();
            read_pages(pages, page, range, &mut into[done..done + length]);
        });
        Ok(())
    }

    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        check(address, from.len() as u64)?;
        each_page(address, from.len(), |page, range, done| {
            let (pages, page) = self.pages_of(memory, page);
            let length = range.len();
            write_pages(pages, page, range, &from[done..done + length]);
        });
        Ok(())
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        check(address, length)?;
        // Zeroing ends sharing over the whole pages it covers.
        self.memories
            .get_mut(&memory)
            .expect("memory exists")
            .unshare(address.div_ceil(PAGE_SIZE), (address + length) / PAGE_SIZE);
        each_page(address, length as usize, |page, range, _| {
            let (pages, page) = self.pages_of(memory, page);
            if let Some(bytes) = pages.get_mut(&page) {
                bytes[range].fill(0);
            }
        });
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

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        self.last_block += 1;
        let id = SharedId(self.last_block);
        self.blocks.insert(id, (length, Pages::new()));
        Ok(id)
    }

    fn shared_map(
        &mut self,
        shared: SharedId,
        offset: u64,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        check(address, length)?;
        let block_length = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?.0;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > block_length)
        {
            return Err(CpuError::BadAddress);
        }
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        let (start, end) = (address / PAGE_SIZE, (address + length) / PAGE_SIZE);
        memory.unshare(start, end);
        memory.pages.retain(|page, _| *page < start || *page >= end);
        memory
            .shared
            .insert(start, (end, shared, offset / PAGE_SIZE));
        Ok(())
    }

    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        check(address, length)?;
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        for (start, end, block, first) in
            memory.unshare(address / PAGE_SIZE, (address + length) / PAGE_SIZE)
        {
            let pages = &self.blocks[&block].1;
            for page in start..end {
                if let Some(bytes) = pages.get(&(first + page - start)) {
                    memory.pages.insert(page, bytes.clone());
                }
            }
        }
        Ok(())
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        let (length, pages) = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
        if offset + into.len() as u64 > *length {
            return Err(CpuError::BadAddress);
        }
        each_page(offset, into.len(), |page, range, done| {
            let length = range.len();
            read_pages(pages, page, range, &mut into[done..done + length]);
        });
        Ok(())
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        let (length, pages) = self.blocks.get_mut(&shared).ok_or(CpuError::BadAddress)?;
        if offset + from.len() as u64 > *length {
            return Err(CpuError::BadAddress);
        }
        each_page(offset, from.len(), |page, range, done| {
            let length = range.len();
            write_pages(pages, page, range, &from[done..done + length]);
        });
        Ok(())
    }

    fn shared_drop(&mut self, shared: SharedId) {
        self.blocks.remove(&shared);
    }

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
