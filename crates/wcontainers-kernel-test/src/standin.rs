//! The stand-in CPU: the unicorn-engine ARM64 emulator behind the abi's [`Cpu`] trait.
//!
//! Each memory is one unicorn instance, because a unicorn instance is one address space. The bytes live
//! in a host mapping the size of the whole address space, which the kernel reads and writes directly.
//! Unicorn sees only the pages that allow something, mapped onto that same host memory, so a page that
//! allows nothing faults by being absent and one that allows less faults on its protection. A range that
//! shows a block of shared pages is mapped in unicorn onto the block's host memory instead, so every
//! memory that shows the block sees the same bytes at once.

use std::collections::{BTreeMap, HashMap};

use memmap2::MmapMut;
use unicorn_engine::{
    Arch, Arm64CpuModel, HookType, MemType, Mode, Prot, RegisterARM64, Unicorn, uc_error,
};
use wcontainers_abi::{
    Access, AddressSpace, Cpu, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock,
    Resume, SharedId, Syscall, ThreadId,
};

/// Every process gets 4 GiB, as on a host with 32 bit memories.
const ADDRESS_SPACE_END: u64 = 1 << 32;

/// QEMU's exception numbers, as unicorn's interrupt hook reports them.
const EXCEPTION_UNDEFINED: u32 = 1;
const EXCEPTION_SYSCALL: u32 = 2;
const EXCEPTION_DATA_ABORT: u32 = 4;
const EXCEPTION_BREAKPOINT: u32 = 7;

/// Something the stand-in cannot run as the reference's CPU does. It is reported instead of an [`Exit`],
/// because no exit would be true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsupported {
    /// The emulator requires this access to be aligned and real ARM64 hardware does not, as for an
    /// exclusive load or store to an unaligned address. `pc` is the instruction.
    AlignmentFault { pc: u64 },
    /// The emulator stopped for a reason the stand-in does not know.
    UnknownStop {
        pc: u64,
        result: Result<(), uc_error>,
        exception: Option<u32>,
    },
}

/// What stopped the emulator, recorded by its hooks.
#[derive(Default)]
struct Stopped {
    exception: Option<u32>,
    fault: Option<(u64, Access)>,
}

struct Memory {
    unicorn: Unicorn<'static, Stopped>,
    bytes: MmapMut,
    /// What each page allows, as the kernel last said.
    pages: Vec<Permissions>,
    /// The ranges currently mapped in unicorn: start to (end, permissions).
    mapped: BTreeMap<u64, (u64, Permissions)>,
    /// Ranges that show a block: start to (end, the block's host memory for the start).
    shared: BTreeMap<u64, (u64, *mut u8)>,
    /// Pages that may hold something other than zeros, one bit each, so a copy copies only those.
    touched: Vec<u64>,
}

/// A block of shared pages, in host memory that lives until the block is dropped.
struct Block {
    bytes: MmapMut,
}

struct Thread {
    memory: MemoryId,
    registers: RegisterBlock,
}

/// The stand-in CPU side.
#[derive(Default)]
pub struct StandIn {
    memories: HashMap<MemoryId, Memory>,
    threads: HashMap<ThreadId, Thread>,
    blocks: HashMap<SharedId, Block>,
    last_memory: u32,
    last_thread: u32,
    last_block: u32,
}

fn prot(permissions: Permissions) -> Prot {
    let mut prot = Prot::NONE.0;
    if permissions.contains(Permissions::READ) {
        prot |= Prot::READ.0;
    }
    if permissions.contains(Permissions::WRITE) {
        prot |= Prot::WRITE.0;
    }
    if permissions.contains(Permissions::EXECUTE) {
        prot |= Prot::EXEC.0;
    }
    Prot(prot)
}

fn x_register(index: usize) -> RegisterARM64 {
    match index {
        29 => RegisterARM64::X29,
        30 => RegisterARM64::X30,
        _ => register_at(RegisterARM64::X0 as i32 + index as i32),
    }
}

fn q_register(index: usize) -> RegisterARM64 {
    register_at(RegisterARM64::Q0 as i32 + index as i32)
}

fn register_at(id: i32) -> RegisterARM64 {
    // SAFETY: the callers stay inside the contiguous runs X0..X28 and Q0..Q31 of the enum.
    unsafe { std::mem::transmute::<i32, RegisterARM64>(id) }
}

impl Memory {
    fn new() -> Result<Self, CpuError> {
        let mut unicorn = Unicorn::new_with_data(Arch::ARM64, Mode::ARM, Stopped::default())
            .map_err(|_| CpuError::NotSupported)?;
        unicorn
            .ctl_set_cpu_model(Arm64CpuModel::MAX as i32)
            .expect("unicorn has the max ARM64 model");
        unicorn
            .add_intr_hook(|unicorn, exception| {
                unicorn.get_data_mut().exception = Some(exception);
                unicorn.emu_stop().expect("stopping unicorn");
            })
            .expect("adding the interrupt hook");
        let invalid = HookType(
            HookType::MEM_READ_UNMAPPED.0
                | HookType::MEM_WRITE_UNMAPPED.0
                | HookType::MEM_FETCH_UNMAPPED.0
                | HookType::MEM_READ_PROT.0
                | HookType::MEM_WRITE_PROT.0
                | HookType::MEM_FETCH_PROT.0,
        );
        unicorn
            .add_mem_hook(invalid, 1, 0, |unicorn, kind, address, _, _| {
                let access = match kind {
                    MemType::WRITE_UNMAPPED | MemType::WRITE_PROT => Access::Write,
                    MemType::FETCH_UNMAPPED | MemType::FETCH_PROT => Access::Execute,
                    _ => Access::Read,
                };
                unicorn.get_data_mut().fault = Some((address, access));
                false
            })
            .expect("adding the fault hook");
        let bytes =
            MmapMut::map_anon(ADDRESS_SPACE_END as usize).map_err(|_| CpuError::OutOfMemory)?;
        Ok(Self {
            unicorn,
            bytes,
            pages: vec![Permissions::empty(); (ADDRESS_SPACE_END / PAGE_SIZE) as usize],
            mapped: BTreeMap::new(),
            shared: BTreeMap::new(),
            touched: vec![0; (ADDRESS_SPACE_END / PAGE_SIZE / 64) as usize],
        })
    }

    /// Notes that the pages over `[start, end)` may hold bytes.
    fn touch(&mut self, start: u64, end: u64) {
        for page in start / PAGE_SIZE..end.div_ceil(PAGE_SIZE) {
            self.touched[(page / 64) as usize] |= 1 << (page % 64);
        }
    }

    /// A copy with the same bytes, permissions and shared ranges, on a new unicorn instance.
    fn duplicate(&self) -> Result<Self, CpuError> {
        let mut copy = Self::new()?;
        for (word, bits) in self.touched.iter().enumerate() {
            if *bits == 0 {
                continue;
            }
            let start = word as u64 * 64 * PAGE_SIZE;
            let range = start as usize..(start + 64 * PAGE_SIZE) as usize;
            copy.bytes[range.clone()].copy_from_slice(&self.bytes[range]);
        }
        copy.touched.clone_from(&self.touched);
        copy.pages.clone_from(&self.pages);
        copy.shared.clone_from(&self.shared);
        copy.remap(0, ADDRESS_SPACE_END);
        Ok(copy)
    }

    /// Where the byte at `address` lives in host memory, and how many bytes from it are contiguous
    /// there (to the end of its shared range or of the memory).
    fn host(&mut self, address: u64) -> (*mut u8, u64) {
        if let Some((start, (end, base))) = self.shared.range(..=address).next_back()
            && address < *end
        {
            // SAFETY: the offset is inside the block, which outlives every range that shows it.
            return (
                unsafe { base.add((address - start) as usize) },
                end - address,
            );
        }
        let next_shared = self
            .shared
            .range(address..)
            .next()
            .map_or(ADDRESS_SPACE_END, |(start, _)| *start);
        // SAFETY: the address is inside the address space, which `bytes` covers.
        (
            unsafe { self.bytes.as_mut_ptr().add(address as usize) },
            next_shared - address,
        )
    }

    /// Calls `f` with each contiguous piece of host memory for `[address, address + length)` and its
    /// offset in the range.
    fn pieces(
        &mut self,
        address: u64,
        length: u64,
        mut f: impl FnMut(&mut [u8], usize),
    ) -> Result<(), CpuError> {
        self.range(address, length)?;
        let mut done = 0;
        while done < length {
            let (pointer, contiguous) = self.host(address + done);
            let chunk = contiguous.min(length - done);
            // SAFETY: `host` says this many bytes from the pointer are one allocation, and nothing else
            // touches them while the stand-in is not running a thread.
            f(
                unsafe { std::slice::from_raw_parts_mut(pointer, chunk as usize) },
                done as usize,
            );
            done += chunk;
        }
        Ok(())
    }

    /// Takes `[start, end)` out of the shared ranges, splitting those at its ends, and returns the parts
    /// taken with their host memory.
    fn unshare(&mut self, start: u64, end: u64) -> Vec<(u64, u64, *mut u8)> {
        let overlapping: Vec<u64> = self
            .shared
            .range(..end)
            .filter(|(_, (range_end, _))| *range_end > start)
            .map(|(range_start, _)| *range_start)
            .collect();
        let mut taken = Vec::new();
        for range_start in overlapping {
            let (range_end, base) = self.shared.remove(&range_start).expect("range");
            // SAFETY: offsets inside the block.
            let at = |address: u64| unsafe { base.add((address - range_start) as usize) };
            if range_start < start {
                self.shared.insert(range_start, (start, base));
            }
            if range_end > end {
                self.shared.insert(end, (range_end, at(end)));
            }
            let from = range_start.max(start);
            taken.push((from, range_end.min(end), at(from)));
        }
        taken
    }

    fn range(&self, address: u64, length: u64) -> Result<std::ops::Range<usize>, CpuError> {
        let end = address.checked_add(length).ok_or(CpuError::BadAddress)?;
        if end > ADDRESS_SPACE_END {
            return Err(CpuError::BadAddress);
        }
        Ok(address as usize..end as usize)
    }

    /// Makes unicorn's mappings over `[start, end)` match the page table again.
    fn remap(&mut self, start: u64, end: u64) {
        let mut low = start;
        let mut high = end;
        let overlapping: Vec<u64> = self
            .mapped
            .range(..end)
            .filter(|(_, (region_end, _))| *region_end > start)
            .map(|(region_start, _)| *region_start)
            .collect();
        for region_start in overlapping {
            let (region_end, _) = self.mapped.remove(&region_start).expect("region");
            self.unicorn
                .mem_unmap(region_start, region_end - region_start)
                .expect("unmapping a region unicorn has");
            low = low.min(region_start);
            high = high.max(region_end);
        }
        let mut page = low / PAGE_SIZE;
        let last = high / PAGE_SIZE;
        while page < last {
            let permissions = self.pages[page as usize];
            let run_start = page;
            let (pointer, contiguous) = self.host(page * PAGE_SIZE);
            let run_last = last.min(page + contiguous / PAGE_SIZE);
            while page < run_last && self.pages[page as usize] == permissions {
                page += 1;
            }
            if permissions.is_empty() {
                continue;
            }
            let address = run_start * PAGE_SIZE;
            let length = (page - run_start) * PAGE_SIZE;
            // SAFETY: the pointer is inside `bytes` or a block, which live as long as unicorn maps them,
            // and unicorn only touches them while the stand-in runs a thread or is asked to.
            unsafe {
                self.unicorn
                    .mem_map_ptr(address, length, prot(permissions), pointer.cast())
                    .expect("mapping host memory into unicorn");
            }
            self.mapped.insert(address, (address + length, permissions));
        }
    }

    fn load_registers(&mut self, registers: &RegisterBlock) -> Result<(), uc_error> {
        let unicorn = &mut self.unicorn;
        for (index, value) in registers.x.iter().enumerate() {
            unicorn.reg_write(x_register(index), *value)?;
        }
        unicorn.reg_write(RegisterARM64::SP, registers.sp)?;
        unicorn.reg_write(RegisterARM64::PC, registers.pc)?;
        unicorn.reg_write(RegisterARM64::NZCV, registers.pstate)?;
        for (index, value) in registers.v.iter().enumerate() {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&value[0].to_le_bytes());
            bytes[8..].copy_from_slice(&value[1].to_le_bytes());
            unicorn.reg_write_long(q_register(index), &bytes)?;
        }
        unicorn.reg_write(RegisterARM64::FPSR, u64::from(registers.fpsr))?;
        unicorn.reg_write(RegisterARM64::FPCR, u64::from(registers.fpcr))?;
        unicorn.reg_write(RegisterARM64::TPIDR_EL0, registers.tpidr_el0)?;
        Ok(())
    }

    fn save_registers(&self, registers: &mut RegisterBlock) -> Result<(), uc_error> {
        let unicorn = &self.unicorn;
        for (index, value) in registers.x.iter_mut().enumerate() {
            *value = unicorn.reg_read(x_register(index))?;
        }
        registers.sp = unicorn.reg_read(RegisterARM64::SP)?;
        registers.pc = unicorn.reg_read(RegisterARM64::PC)?;
        registers.pstate = unicorn.reg_read(RegisterARM64::NZCV)? & wcontainers_abi::pstate::NZCV;
        for (index, value) in registers.v.iter_mut().enumerate() {
            let bytes = unicorn.reg_read_long(q_register(index))?;
            value[0] = u64::from_le_bytes(bytes[..8].try_into().expect("16 bytes"));
            value[1] = u64::from_le_bytes(bytes[8..16].try_into().expect("16 bytes"));
        }
        registers.fpsr = unicorn.reg_read(RegisterARM64::FPSR)? as u32;
        registers.fpcr = unicorn.reg_read(RegisterARM64::FPCR)? as u32;
        registers.tpidr_el0 = unicorn.reg_read(RegisterARM64::TPIDR_EL0)?;
        Ok(())
    }
}

impl StandIn {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new CPU side holding what this one holds, as bringing back a saved container makes it: every
    /// memory, block of shared pages and thread's registers copied under the same handles, on new
    /// emulator instances with nothing translated or cached.
    pub fn restored(&self) -> Self {
        let mut blocks = HashMap::new();
        let mut moved = Vec::new();
        for (id, block) in &self.blocks {
            let mut bytes = MmapMut::map_anon(block.bytes.len()).expect("host memory for a block");
            bytes.copy_from_slice(&block.bytes);
            moved.push((
                block.bytes.as_ptr() as usize,
                block.bytes.len(),
                bytes.as_mut_ptr(),
            ));
            blocks.insert(*id, Block { bytes });
        }
        let translate = |pointer: *mut u8| {
            let address = pointer as usize;
            let (old, _, new) = moved
                .iter()
                .find(|(old, length, _)| (*old..old + length).contains(&address))
                .expect("a shared range shows a block");
            // SAFETY: the same offset inside the copy, which is as long as the original.
            unsafe { new.add(address - old) }
        };
        let memories = self
            .memories
            .iter()
            .map(|(id, memory)| {
                let mut copy = memory.duplicate().expect("a new emulator instance");
                copy.shared = memory
                    .shared
                    .iter()
                    .map(|(start, (end, base))| (*start, (*end, translate(*base))))
                    .collect();
                copy.remap(0, ADDRESS_SPACE_END);
                (*id, copy)
            })
            .collect();
        let threads = self
            .threads
            .iter()
            .map(|(id, thread)| {
                (
                    *id,
                    Thread {
                        memory: thread.memory,
                        registers: thread.registers,
                    },
                )
            })
            .collect();
        Self {
            memories,
            threads,
            blocks,
            last_memory: self.last_memory,
            last_thread: self.last_thread,
            last_block: self.last_block,
        }
    }

    /// Runs a thread until it comes back, as a driver does after [`wcontainers_abi::Next::Run`]. With a
    /// slice, the stop flag is raised after that many instructions (one a nanosecond), so where a
    /// turn ends repeats exactly.
    pub fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        slice_nanoseconds: Option<u64>,
    ) -> Result<Exit, Unsupported> {
        let state = self
            .threads
            .get_mut(&thread)
            .expect("running a thread that exists");
        if let Resume::Return(value) = resume {
            state.registers.x[0] = value;
        }
        let memory = self
            .memories
            .get_mut(&state.memory)
            .expect("thread's memory");
        memory
            .load_registers(&state.registers)
            .expect("loading registers into unicorn");
        *memory.unicorn.get_data_mut() = Stopped::default();
        // Zero means no limit.
        let count = slice_nanoseconds.map_or(0, |nanoseconds| nanoseconds.max(1) as usize);
        let result = memory
            .unicorn
            .emu_start(state.registers.pc, u64::MAX, 0, count);
        memory
            .save_registers(&mut state.registers)
            .expect("reading registers from unicorn");
        let stopped = std::mem::take(memory.unicorn.get_data_mut());
        // A jump to an address that is not a multiple of 4 faults on alignment before any fetch.
        if let Some((address, access)) = stopped.fault {
            if access == Access::Execute && !address.is_multiple_of(4) {
                return Ok(Exit::AlignmentFault { address });
            }
            return Ok(Exit::MemoryFault { address, access });
        }
        let pc = state.registers.pc;
        Ok(match (result, stopped.exception) {
            (_, Some(EXCEPTION_SYSCALL)) => {
                let x = &state.registers.x;
                Exit::Syscall(Syscall {
                    number: x[8],
                    arguments: [x[0], x[1], x[2], x[3], x[4], x[5]],
                })
            }
            (_, Some(EXCEPTION_BREAKPOINT)) => {
                let mut instruction = [0u8; 4];
                memory
                    .pieces(pc, 4, |piece, at| {
                        instruction[at..at + piece.len()].copy_from_slice(piece);
                    })
                    .expect("the instruction is in the address space");
                let comment = (u32::from_le_bytes(instruction) >> 5) as u16;
                Exit::Breakpoint { comment }
            }
            // QEMU raises a fetch from a pc that is not a multiple of 4 as undefined.
            (_, Some(EXCEPTION_UNDEFINED)) if !pc.is_multiple_of(4) => {
                Exit::AlignmentFault { address: pc }
            }
            (_, Some(EXCEPTION_UNDEFINED)) | (Err(uc_error::INSN_INVALID), None) => {
                Exit::IllegalInstruction
            }
            // Permission and mapping faults come through the fault hook, so a data abort here is QEMU's
            // alignment check.
            (_, Some(EXCEPTION_DATA_ABORT)) => return Err(Unsupported::AlignmentFault { pc }),
            (Ok(()), None) => Exit::Stop,
            (result, exception) => {
                return Err(Unsupported::UnknownStop {
                    pc,
                    result,
                    exception,
                });
            }
        })
    }
}

impl Cpu for StandIn {
    fn address_space(&self) -> AddressSpace {
        AddressSpace {
            end: ADDRESS_SPACE_END,
        }
    }

    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        let memory = Memory::new()?;
        self.last_memory += 1;
        let id = MemoryId(self.last_memory);
        self.memories.insert(id, memory);
        Ok(id)
    }

    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        let memory = self.memories[&source].duplicate()?;
        self.last_memory += 1;
        let id = MemoryId(self.last_memory);
        self.memories.insert(id, memory);
        Ok(id)
    }

    fn memory_drop(&mut self, memory: MemoryId) {
        self.memories.remove(&memory);
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        memory.pieces(address, into.len() as u64, |piece, at| {
            into[at..at + piece.len()].copy_from_slice(piece);
        })
    }

    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        memory.pieces(address, from.len() as u64, |piece, at| {
            piece.copy_from_slice(&from[at..at + piece.len()]);
        })?;
        memory.touch(address, address + from.len() as u64);
        Ok(())
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        let range = memory.range(address, length)?;
        // Zeroing ends sharing.
        if !memory.unshare(address, address + length).is_empty() {
            memory.remap(address, address + length);
        }
        memory.bytes[range].fill(0);
        Ok(())
    }

    fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        let range = memory.range(address, length)?;
        let pages = range.start / PAGE_SIZE as usize..range.end / PAGE_SIZE as usize;
        memory.pages[pages].fill(permissions);
        if !permissions.is_empty() {
            memory.touch(address, address + length);
        }
        memory.remap(address, address + length);
        Ok(())
    }

    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, _starts: &[u64]) {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        memory
            .unicorn
            .ctl_remove_cache(address, address + length)
            .expect("dropping unicorn's translations");
    }

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        let bytes = MmapMut::map_anon(length as usize).map_err(|_| CpuError::OutOfMemory)?;
        self.last_block += 1;
        let id = SharedId(self.last_block);
        self.blocks.insert(id, Block { bytes });
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
        let block = self.blocks.get_mut(&shared).ok_or(CpuError::BadAddress)?;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > block.bytes.len() as u64)
        {
            return Err(CpuError::BadAddress);
        }
        // SAFETY: the offset is inside the block.
        let base = unsafe { block.bytes.as_mut_ptr().add(offset as usize) };
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        memory.range(address, length)?;
        memory.unshare(address, address + length);
        memory.shared.insert(address, (address + length, base));
        memory.remap(address, address + length);
        Ok(())
    }

    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        memory.range(address, length)?;
        for (start, end, base) in memory.unshare(address, address + length) {
            // SAFETY: the block holds `end - start` bytes from `base`.
            let bytes = unsafe { std::slice::from_raw_parts(base, (end - start) as usize) };
            memory.bytes[start as usize..end as usize].copy_from_slice(bytes);
            memory.touch(start, end);
        }
        memory.remap(address, address + length);
        Ok(())
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        let block = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
        let bytes = block
            .bytes
            .get(offset as usize..offset as usize + into.len())
            .ok_or(CpuError::BadAddress)?;
        into.copy_from_slice(bytes);
        Ok(())
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        let block = self.blocks.get_mut(&shared).ok_or(CpuError::BadAddress)?;
        block
            .bytes
            .get_mut(offset as usize..offset as usize + from.len())
            .ok_or(CpuError::BadAddress)?
            .copy_from_slice(from);
        Ok(())
    }

    fn shared_drop(&mut self, shared: SharedId) {
        self.blocks.remove(&shared);
    }

    fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        if !self.memories.contains_key(&memory) {
            return Err(CpuError::BadAddress);
        }
        self.last_thread += 1;
        let id = ThreadId(self.last_thread);
        self.threads.insert(
            id,
            Thread {
                memory,
                registers: RegisterBlock::ZERO,
            },
        );
        Ok(id)
    }

    fn thread_drop(&mut self, thread: ThreadId) {
        self.threads.remove(&thread);
    }

    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        *into = self.threads[&thread].registers;
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        self.threads
            .get_mut(&thread)
            .expect("thread exists")
            .registers = *from;
    }
}
