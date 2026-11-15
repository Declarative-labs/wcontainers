//! The stand-in CPU: the unicorn-engine ARM64 emulator behind the abi's [`Cpu`] trait.
//!
//! Each memory is one unicorn instance, because a unicorn instance is one address space. The bytes live
//! in a host mapping the size of the whole address space, which the kernel reads and writes directly.
//! Unicorn sees only the pages that allow something, mapped onto that same host memory, so a page that
//! allows nothing faults by being absent and one that allows less faults on its protection.

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
const EXCEPTION_BREAKPOINT: u32 = 7;

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
    last_memory: u32,
    last_thread: u32,
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
        })
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
            while page < last && self.pages[page as usize] == permissions {
                page += 1;
            }
            if permissions.is_empty() {
                continue;
            }
            let address = run_start * PAGE_SIZE;
            let length = (page - run_start) * PAGE_SIZE;
            // SAFETY: the pointer is inside `bytes`, which lives as long as the unicorn instance, and
            // unicorn only touches it while the stand-in runs a thread or is asked to.
            unsafe {
                self.unicorn
                    .mem_map_ptr(
                        address,
                        length,
                        prot(permissions),
                        self.bytes.as_mut_ptr().add(address as usize).cast(),
                    )
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

    /// Runs a thread until it comes back, as a driver does after [`wcontainers_abi::Next::Run`]. With a
    /// slice, the stop flag is raised once that much time has passed.
    pub fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        slice_nanoseconds: Option<u64>,
    ) -> Exit {
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
        // Unicorn takes the time limit in microseconds; zero means none.
        let timeout = slice_nanoseconds.map_or(0, |nanoseconds| (nanoseconds / 1000).max(1));
        let result = memory
            .unicorn
            .emu_start(state.registers.pc, u64::MAX, timeout, 0);
        memory
            .save_registers(&mut state.registers)
            .expect("reading registers from unicorn");
        let stopped = std::mem::take(memory.unicorn.get_data_mut());
        if let Some((address, access)) = stopped.fault {
            return Exit::MemoryFault { address, access };
        }
        match (result, stopped.exception) {
            (_, Some(EXCEPTION_SYSCALL)) => {
                let x = &state.registers.x;
                Exit::Syscall(Syscall {
                    number: x[8],
                    arguments: [x[0], x[1], x[2], x[3], x[4], x[5]],
                })
            }
            (_, Some(EXCEPTION_BREAKPOINT)) => {
                let mut instruction = [0u8; 4];
                let pc = state.registers.pc as usize;
                instruction.copy_from_slice(&memory.bytes[pc..pc + 4]);
                let comment = (u32::from_le_bytes(instruction) >> 5) as u16;
                Exit::Breakpoint { comment }
            }
            (_, Some(EXCEPTION_UNDEFINED)) | (Err(uc_error::INSN_INVALID), None) => {
                Exit::IllegalInstruction
            }
            (Ok(()), None) => Exit::Stop,
            (result, exception) => {
                panic!(
                    "the stand-in stopped for an unknown reason: {result:?}, exception {exception:?}"
                )
            }
        }
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

    fn memory_duplicate(&mut self, _source: MemoryId) -> Result<MemoryId, CpuError> {
        Err(CpuError::NotSupported)
    }

    fn memory_drop(&mut self, memory: MemoryId) {
        self.memories.remove(&memory);
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        let memory = &self.memories[&memory];
        let range = memory.range(address, into.len() as u64)?;
        into.copy_from_slice(&memory.bytes[range]);
        Ok(())
    }

    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        let range = memory.range(address, from.len() as u64)?;
        memory.bytes[range].copy_from_slice(from);
        Ok(())
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        let range = memory.range(address, length)?;
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

    fn shared_new(&mut self, _length: u64) -> Result<SharedId, CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_map(
        &mut self,
        _shared: SharedId,
        _offset: u64,
        _memory: MemoryId,
        _address: u64,
        _length: u64,
    ) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_unmap(
        &mut self,
        _memory: MemoryId,
        _address: u64,
        _length: u64,
    ) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_read(
        &mut self,
        _shared: SharedId,
        _offset: u64,
        _into: &mut [u8],
    ) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_write(
        &mut self,
        _shared: SharedId,
        _offset: u64,
        _from: &[u8],
    ) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }

    fn shared_drop(&mut self, _shared: SharedId) {}

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
