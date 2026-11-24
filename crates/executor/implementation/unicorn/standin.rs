//! The stand-in CPU: the unicorn-engine ARM64 emulator behind the abi's [`Cpu`] trait.
//!
//! Each memory is one unicorn instance, because a unicorn instance is one address space. The bytes live
//! in a host mapping the size of the whole address space, which the kernel reads and writes directly.
//! Unicorn sees only the pages that allow something, mapped onto that same host memory, so a page that
//! allows nothing faults by being absent and one that allows less faults on its protection. A range that
//! shows a block of shared pages is mapped in unicorn onto the block's host memory instead, so every
//! memory that shows the block sees the same bytes at once.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use memmap2::MmapMut;
use unicorn_engine::{
    Arch, Arm64CpuModel, Context, HookType, MemType, Mode, Prot, RegisterARM64, Unicorn, uc_error,
};
use wcontainers_abi::{
    Access, AddressSpace, Cpu, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock,
    Resume, SYSTEM_CALL_ARGUMENT_COUNT, SharedId, SystemCallNumber, SystemCallRequest, ThreadId,
};

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor::{
    ExecutionInterrupter, InterruptResponse, MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_processor_architecture::MemoryDelta;
use wcontainers_processor_architecture_arm64::encoding::{IMM16, RN};
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_processor_architecture_arm64::syndrome::{
    FaultStatus, WRITE_NOT_READ, encode_syndrome,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, Exception, INSTRUCTION_BYTES, InstructionFetch,
    PagePermission, Processor,
};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::replay::{DigestMemories, MemoryDigests, digest_pages};
use crate::space::UnicornSpace;

/// A process's address space on a host with 32 bit memories.
pub const SPACE_32_BIT: u64 = 1 << 32;
/// A process's address space on a host with 64 bit memories, at the smallest cap such a host
/// sets (Chrome's 16 GiB).
pub const SPACE_64_BIT: u64 = 16 << 30;

// QEMU's exception numbers, as unicorn's interrupt hook reports them.
/// [EXCP_UDEF](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_UNDEFINED: u32 = 1;
/// [EXCP_SWI](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_SYSCALL: u32 = 2;
/// [EXCP_DATA_ABORT](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_DATA_ABORT: u32 = 4;
/// [EXCP_BKPT](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_BREAKPOINT: u32 = 7;
/// [EXCP_HVC](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_HYPERVISOR_CALL: u32 = 11;
/// [EXCP_SMC](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_SECURE_MONITOR_CALL: u32 = 13;

/// The most bytes a guest space maps in unicorn as one region, a level 2 block of pages.
///
/// Unicorn flushes each page of a region it unmaps, so a change of one page costs this much at most.
const LARGEST_GUEST_REGION: u64 = PAGE_SIZE * (PAGE_SIZE / size_of::<u64>() as u64);

/// The [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D) of an alignment fault.
pub const ALIGNMENT_FAULT_STATUS: u64 = 0b10_0001;

/// The count of instructions or of microseconds that unicorn takes as no limit.
const UNICORN_NO_LIMIT: usize = 0;
const UNICORN_NO_TIMEOUT: u64 = 0;
/// The slice that runs one instruction, at one instruction a nanosecond.
const ONE_INSTRUCTION: u64 = 1;

/// The register a system call's result goes in, x0 (<https://man7.org/linux/man-pages/man2/syscall.2.html>).
const RESULT_REGISTER: usize = 0;
/// A page's length as an index into host memory.
const PAGE_BYTES: usize = PAGE_SIZE as usize;

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
pub(crate) struct Stopped {
    exception: Option<u32>,
    fault: Option<(u64, Access)>,
    /// Instructions of the blocks entered, for how long the turn took.
    instructions: u64,
    /// When a turn on the machine's clock ends, whatever it has run: on the clock at the
    /// resolution of the machine's tick, which is cheap to read at every block, as Linux looks at
    /// CPU time at its tick.
    deadline: Option<coarsetime::Instant>,
    /// Raised from another thread to end the turn at the next block.
    interrupt: Option<Arc<AtomicBool>>,
}

pub(crate) struct Memory {
    pub(crate) unicorn: Unicorn<'static, Stopped>,
    /// The CPU state as made, with the exclusive monitor clear, put back before each turn: a return to
    /// a program clears the monitor (`ClearExclusiveLocal` at exception return), and nothing of one
    /// thread's turn may reach another's.
    fresh: Context,
    bytes: MmapMut,
    /// What each page allows, as the kernel last said.
    pub(crate) pages: Vec<Permissions>,
    /// The ranges currently mapped in unicorn: start to (end, permissions).
    mapped: BTreeMap<u64, (u64, Permissions)>,
    /// Ranges that show a block: start to (end, the block's host memory for the start).
    pub(crate) shared: BTreeMap<u64, (u64, *mut u8)>,
    /// Pages that may hold something other than zeros, one bit each, so a copy copies only those.
    touched: Vec<u64>,
    /// Whether its translated blocks count instructions. Unicorn adds its counting hook without
    /// translating again, so blocks made without it would never stop at a turn's end.
    counting: bool,
    /// Where its address space ends.
    end: u64,
    /// For a guest space, the pages allowing nothing that unicorn maps without access, one bit each.
    ///
    /// They are mapped for unicorn's own reads and writes and unmapped before the guest runs, since unicorn lets
    /// an access that crosses into such a page through. A guest space's regions are cut to [`LARGEST_GUEST_REGION`].
    pub(crate) reachable: Option<Vec<u64>>,
    /// The addresses of the pages reached since the guest last ran.
    reached: Vec<(u64, u64)>,
}

/// A block of shared pages, in host memory that lives until the block is dropped.
struct Block {
    bytes: MmapMut,
}

/// A request of the kernel's that breaks the [`Cpu`] contract, which the stand-in refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// `shared_drop` of a block these memories still show, where the contract says nothing maps it any more
    /// (abi/cpu.rs:88). The block's host memory is kept until none of them shows it, since unicorn and the
    /// stand-in's own copies reach it through them.
    DroppingAShownBlock {
        block: SharedId,
        shown_in: Vec<MemoryId>,
    },
}

struct Thread {
    memory: MemoryId,
    registers: RegisterBlock,
}

impl DigestMemories for StandIn {
    fn keep_digests(&mut self) {
        self.digests.get_or_insert_default();
    }

    fn digests(&mut self) -> MemoryDigests {
        let mut digests = self.digests.clone().unwrap_or_default();
        let mut left: Vec<MemoryId> = self.memories.keys().copied().collect();
        left.sort();
        for memory in left {
            let digest = self.memories.get_mut(&memory).expect("memory").digest();
            digests.push((memory, digest));
        }
        digests
    }
}

/// The stand-in CPU side.
pub struct StandIn {
    /// Where every memory's address space ends.
    end: u64,
    memories: HashMap<MemoryId, Memory>,
    threads: HashMap<ThreadId, Thread>,
    blocks: HashMap<SharedId, Block>,
    /// Blocks the kernel dropped while a memory still showed them, kept until none does.
    kept: Vec<Block>,
    /// What the stand-in refused, in order.
    refused: Vec<Refused>,
    last_memory: u32,
    last_thread: u32,
    last_block: u32,
    /// How many instructions the last turn ran, about: whole blocks are counted.
    pub last_instructions: u64,
    /// Whether the kernel's clock is the machine's, so a slice is also a span of it.
    wall_clock: bool,
    /// What each memory held as it was dropped, once asked for.
    digests: Option<MemoryDigests>,
}

/// A kernel request the stand-in refused breaks the [`Cpu`] contract, so the test it ran in fails,
/// unless the test took the refusal ([`StandIn::take_refused`]) or is failing already.
impl Drop for StandIn {
    fn drop(&mut self) {
        if !self.refused.is_empty() && !std::thread::panicking() {
            panic!(
                "the stand-in refused the kernel's requests (abi/cpu.rs): {:?}",
                self.refused
            );
        }
    }
}

// SAFETY: a memory owns its unicorn engine (whose shared handle is cloned only inside its own
// hooks, which capture nothing, and which is trusted, not checked, to keep no state of the host
// thread between calls: nobody has read unicorn's internals) and its
// records, and its block pointers point into host mappings that live while it shows them (a
// stand-in's blocks, kept while a memory shows them, or a space's host pages). So it moves to
// another host thread whole, and it is used by one at a time.
#[expect(
    unsafe_code,
    reason = "UNEXPLAINED: promises a memory, which owns a unicorn handle, may move between threads on the claim that unicorn keeps no per-thread state, which rests on unicorn's internals nobody has read; no primitive replaces it"
)]
unsafe impl Send for Memory {}

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

/// Unicorn's number of the general register `X<index>`; X0 to X28 are numbered in a run, X29 and X30 apart.
fn x_register(index: usize) -> i32 {
    match index {
        0..=28 => RegisterARM64::X0 as i32 + index as i32,
        29 => RegisterARM64::X29 as i32,
        30 => RegisterARM64::X30 as i32,
        _ => panic!("no general register X{index}"),
    }
}

/// Unicorn's number of the vector register `Q<index>`; Q0 to Q31 are numbered in a run.
fn q_register(index: usize) -> i32 {
    assert!(index < 32, "no vector register Q{index}");
    RegisterARM64::Q0 as i32 + index as i32
}

impl Memory {
    pub(crate) fn new(end: u64) -> Result<Self, CpuError> {
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
        unicorn
            .add_block_hook(1, 0, |unicorn, _, size| {
                let stopped = unicorn.get_data_mut();
                stopped.instructions += u64::from(size / 4);
                if stopped
                    .deadline
                    .is_some_and(|deadline| coarsetime::Instant::now() >= deadline)
                    || stopped
                        .interrupt
                        .as_ref()
                        .is_some_and(|interrupt| interrupt.load(Ordering::Acquire))
                {
                    unicorn.emu_stop().expect("stopping unicorn");
                }
            })
            .expect("adding the block hook");
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
                // QEMU stores an access that crosses pages one byte at a time and goes on past the first
                // byte that faults, so the first is the instruction's fault.
                unicorn
                    .get_data_mut()
                    .fault
                    .get_or_insert((address, access));
                false
            })
            .expect("adding the fault hook");
        let fresh = unicorn
            .context_init()
            .expect("saving unicorn's fresh CPU state");
        let bytes = MmapMut::map_anon(end as usize).map_err(|_| CpuError::OutOfMemory)?;
        Ok(Self {
            unicorn,
            fresh,
            bytes,
            pages: vec![Permissions::empty(); (end / PAGE_SIZE) as usize],
            mapped: BTreeMap::new(),
            shared: BTreeMap::new(),
            touched: vec![0; (end / PAGE_SIZE / 64) as usize],
            counting: false,
            end,
            reachable: None,
            reached: Vec::new(),
        })
    }

    /// Notes that the pages over `[start, end)` may hold bytes.
    fn touch(&mut self, start: u64, end: u64) {
        for page in start / PAGE_SIZE..end.div_ceil(PAGE_SIZE) {
            self.touched[(page / 64) as usize] |= 1 << (page % 64);
        }
    }

    /// A digest of what the memory holds, its shared ranges' pages included.
    fn digest(&mut self) -> u64 {
        let mut pages: std::collections::BTreeSet<u64> = self
            .touched
            .iter()
            .enumerate()
            .flat_map(|(word, bits)| {
                (0..u64::BITS)
                    .filter(move |bit| bits & (1 << bit) != 0)
                    .map(move |bit| word as u64 * u64::from(u64::BITS) + u64::from(bit))
            })
            .collect();
        for (start, (end, _)) in &self.shared {
            pages.extend(start / PAGE_SIZE..end / PAGE_SIZE);
        }
        let mut contents = Vec::new();
        for page in pages {
            let mut bytes = vec![0; PAGE_SIZE as usize];
            self.pieces(page * PAGE_SIZE, PAGE_SIZE, |piece, at| {
                bytes[at..at + piece.len()].copy_from_slice(piece);
            })
            .expect("a page of the memory");
            contents.push((page * PAGE_SIZE, bytes));
        }
        digest_pages(
            contents
                .iter()
                .map(|(address, bytes)| (*address, bytes.as_slice())),
        )
    }

    /// A copy with the same bytes, permissions and shared ranges, on a new unicorn instance.
    fn duplicate(&self) -> Result<Self, CpuError> {
        let mut copy = Self::new(self.end)?;
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
        copy.remap(0, self.end);
        Ok(copy)
    }

    /// Where the byte at `address` lives in host memory, and how many bytes from it are contiguous
    /// there (to the end of its shared range or of the memory).
    fn host(&mut self, address: u64) -> (*mut u8, u64) {
        if let Some((start, (end, base))) = self.shared.range(..=address).next_back()
            && address < *end
        {
            return (base.wrapping_add((address - start) as usize), end - address);
        }
        let next_shared = self
            .shared
            .range(address..)
            .next()
            .map_or(self.end, |(start, _)| *start);
        (
            self.bytes.as_mut_ptr().wrapping_add(address as usize),
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
            #[expect(
                unsafe_code,
                reason = "a slice of a memory's own bytes or of a block it shows, bounded by range and host; a shown block outlives it, as shared_drop keeps it; unicorn's mem_read replaces it in step one"
            )]
            let piece = unsafe { std::slice::from_raw_parts_mut(pointer, chunk as usize) };
            f(piece, done as usize);
            done += chunk;
        }
        Ok(())
    }

    /// Takes `[start, end)` out of the shared ranges, splitting those at its ends, and returns the parts
    /// taken with their host memory.
    pub(crate) fn unshare(&mut self, start: u64, end: u64) -> Vec<(u64, u64, *mut u8)> {
        let overlapping: Vec<u64> = self
            .shared
            .range(..end)
            .filter(|(_, (range_end, _))| *range_end > start)
            .map(|(range_start, _)| *range_start)
            .collect();
        let mut taken = Vec::new();
        for range_start in overlapping {
            let (range_end, base) = self.shared.remove(&range_start).expect("range");
            let at = |address: u64| base.wrapping_add((address - range_start) as usize);
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

    pub(crate) fn range(
        &self,
        address: u64,
        length: u64,
    ) -> Result<std::ops::Range<usize>, CpuError> {
        let end = address.checked_add(length).ok_or(CpuError::BadAddress)?;
        if end > self.end {
            return Err(CpuError::BadAddress);
        }
        Ok(address as usize..end as usize)
    }

    /// Makes unicorn's mappings over `[start, end)` match the page table again.
    pub(crate) fn remap(&mut self, start: u64, end: u64) {
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
        let largest = match self.reachable {
            Some(_) => LARGEST_GUEST_REGION / PAGE_SIZE,
            None => u64::MAX,
        };
        while page < last {
            let permissions = self.pages[page as usize];
            let shown = self.is_shown(page);
            let run_start = page;
            let (pointer, contiguous) = self.host(page * PAGE_SIZE);
            let run_last = last
                .min(page + contiguous / PAGE_SIZE)
                .min(page.saturating_add(largest));
            while page < run_last
                && self.pages[page as usize] == permissions
                && self.is_shown(page) == shown
            {
                page += 1;
            }
            if !shown {
                continue;
            }
            let address = run_start * PAGE_SIZE;
            let length = (page - run_start) * PAGE_SIZE;
            // SAFETY: the pointer is inside `bytes` or a block, which live as long as unicorn maps them,
            // and unicorn only touches them while the stand-in runs a thread or is asked to.
            #[expect(
                unsafe_code,
                reason = "lends a memory's own bytes or a block's to unicorn as guest memory; both outlive the mapping, since unicorn drops before bytes and shared_drop keeps a shown block; Lend replaces it"
            )]
            unsafe {
                self.unicorn
                    .mem_map_ptr(address, length, prot(permissions), pointer.cast())
                    .expect("mapping host memory into unicorn");
            }
            self.mapped.insert(address, (address + length, permissions));
        }
    }

    /// Returns whether unicorn maps the page.
    fn is_shown(&self, page: u64) -> bool {
        !self.pages[page as usize].is_empty()
            || self.reachable.as_ref().is_some_and(|reachable| {
                reachable[(page / u64::from(u64::BITS)) as usize]
                    & (1 << (page % u64::from(u64::BITS)))
                    != 0
            })
    }

    /// Maps a guest space's pages over the addresses in unicorn, those allowing nothing without access.
    ///
    /// Unicorn's own reads and writes then reach them; a memory that is not a guest space is left as it is.
    pub(crate) fn reach(&mut self, start: u64, end: u64) {
        let Some(reachable) = &mut self.reachable else {
            return;
        };
        let mut missing = false;
        for page in start / PAGE_SIZE..end.div_ceil(PAGE_SIZE) {
            let (word, bit) = (
                (page / u64::from(u64::BITS)) as usize,
                1 << (page % u64::from(u64::BITS)),
            );
            missing |= reachable[word] & bit == 0;
            reachable[word] |= bit;
        }
        if missing {
            let pages = (
                start - start % PAGE_SIZE,
                end.div_ceil(PAGE_SIZE) * PAGE_SIZE,
            );
            self.reached.push(pages);
            self.remap(pages.0, pages.1);
        }
    }

    /// Unmaps from unicorn again the pages that allow nothing it was given to reach.
    fn unreach(&mut self) {
        let Some(reachable) = &mut self.reachable else {
            return;
        };
        reachable.fill(u64::MIN);
        for (start, end) in std::mem::take(&mut self.reached) {
            self.remap(start, end);
        }
    }

    fn load_registers(&mut self, registers: &RegisterBlock) -> Result<(), uc_error> {
        let unicorn = &mut self.unicorn;
        unicorn.context_restore(&self.fresh)?;
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
    /// A CPU side as on a host with 32 bit memories.
    pub fn new() -> Self {
        Self::with_space(SPACE_32_BIT)
    }

    /// A CPU side whose memories' address spaces end at `end`.
    pub fn with_space(end: u64) -> Self {
        Self {
            end,
            memories: HashMap::new(),
            threads: HashMap::new(),
            blocks: HashMap::new(),
            kept: Vec::new(),
            refused: Vec::new(),
            last_memory: 0,
            last_thread: 0,
            last_block: 0,
            last_instructions: 0,
            wall_clock: false,
            digests: None,
        }
    }

    /// The same CPU side for a kernel on the machine's clock: a turn also ends once its slice has
    /// passed on that clock, since an instruction then takes what it takes.
    pub fn on_wall_clock(mut self) -> Self {
        self.wall_clock = true;
        self
    }

    /// A new CPU side holding what this one holds, as bringing back a saved container makes it: every
    /// memory, block of shared pages and thread's registers copied under the same handles, on new
    /// emulator instances with nothing translated or cached.
    pub fn restored(&self) -> Self {
        let mut blocks = HashMap::new();
        let mut moved = Vec::new();
        let mut copy = |block: &Block| {
            let mut bytes = MmapMut::map_anon(block.bytes.len()).expect("host memory for a block");
            copy_touched(&block.bytes, &mut bytes);
            moved.push((
                block.bytes.as_ptr() as usize,
                block.bytes.len(),
                bytes.as_mut_ptr(),
            ));
            Block { bytes }
        };
        for (id, block) in &self.blocks {
            blocks.insert(*id, copy(block));
        }
        let kept: Vec<Block> = self.kept.iter().map(&mut copy).collect();
        let translate = |pointer: *mut u8| {
            let address = pointer as usize;
            let (old, _, new) = moved
                .iter()
                .find(|(old, length, _)| (*old..old + length).contains(&address))
                .expect("a shared range shows a block");
            new.wrapping_add(address - old)
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
                copy.remap(0, memory.end);
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
            end: self.end,
            memories,
            threads,
            blocks,
            kept,
            refused: self.refused.clone(),
            last_memory: self.last_memory,
            last_thread: self.last_thread,
            last_block: self.last_block,
            last_instructions: 0,
            wall_clock: self.wall_clock,
            digests: None,
        }
    }

    /// What the stand-in refused of the kernel's requests so far, in order, taken: a stand-in
    /// dropped with a refusal not taken fails its test.
    pub fn take_refused(&mut self) -> Vec<Refused> {
        std::mem::take(&mut self.refused)
    }

    /// Adds what the stand-in of a copy made at a save refused, which the copy reported as it
    /// ended, so they fail the test as this stand-in goes, as its own refusals do.
    #[cfg(not(feature = "runtime"))]
    pub(crate) fn refused_in_a_copy(&mut self, refused: Vec<Refused>) {
        self.refused.extend(refused);
    }

    /// The memories that show any of the block's bytes, in order.
    fn showing(&self, block: &Block) -> Vec<MemoryId> {
        let start = block.bytes.as_ptr().addr();
        let held = start..start + block.bytes.len();
        let mut shown_in: Vec<MemoryId> = self
            .memories
            .iter()
            .filter(|(_, memory)| {
                memory
                    .shared
                    .values()
                    .any(|(_, base)| held.contains(&base.addr()))
            })
            .map(|(id, _)| *id)
            .collect();
        shown_in.sort();
        shown_in
    }

    /// Frees the kept blocks no memory shows any more.
    fn let_go_of_kept(&mut self) {
        if self.kept.is_empty() {
            return;
        }
        let kept = std::mem::take(&mut self.kept);
        self.kept = kept
            .into_iter()
            .filter(|block| !self.showing(block).is_empty())
            .collect();
    }

    /// Runs a thread until it comes back, as a driver does after [`wcontainers_abi::Next::Run`]. With a
    /// slice, the stop flag is raised after that many instructions (one a nanosecond), so where a
    /// turn ends repeats exactly; on the machine's clock, also once that many nanoseconds passed.
    pub fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        slice_nanoseconds: Option<u64>,
    ) -> Result<Exit, Unsupported> {
        let deadline = self.deadline(slice_nanoseconds);
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
        let (exit, instructions) =
            memory.run_turn(&mut state.registers, slice_nanoseconds, deadline, None);
        self.last_instructions = instructions;
        exit
    }

    /// Runs a thread of a kernel that keeps its threads' registers and memories itself, as
    /// [`StandIn::run`] runs one of its own.
    pub fn run_on(
        &mut self,
        registers: &mut RegisterBlock,
        memory: &UnicornSpace,
        resume: Resume,
        slice_nanoseconds: Option<u64>,
    ) -> Result<Exit, Unsupported> {
        if let Resume::Return(value) = resume {
            registers.x[RESULT_REGISTER] = value;
        }
        let (exit, instructions) = memory.run_turn(
            registers,
            slice_nanoseconds,
            self.deadline(slice_nanoseconds),
            None,
        );
        self.last_instructions = instructions;
        exit
    }

    /// When a turn of a slice ends on the machine's clock, if the stand-in is on it.
    fn deadline(&self, slice_nanoseconds: Option<u64>) -> Option<coarsetime::Instant> {
        slice_nanoseconds
            .filter(|_| self.wall_clock)
            .map(|nanoseconds| {
                coarsetime::Instant::now()
                    + coarsetime::Duration::from(std::time::Duration::from_nanos(nanoseconds))
            })
    }
}

impl Memory {
    /// Runs the registers on this memory and returns why they came back with the number of instructions run.
    ///
    /// Instructions are counted in whole blocks. With a slice, it stops after that many instructions; with a
    /// deadline, also once the machine's clock passes it; with an interrupt, also once it is raised.
    pub(crate) fn run_turn(
        &mut self,
        registers: &mut RegisterBlock,
        slice_nanoseconds: Option<u64>,
        deadline: Option<coarsetime::Instant>,
        interrupt: Option<Arc<AtomicBool>>,
    ) -> (Result<Exit, Unsupported>, u64) {
        self.unreach();
        self.load_registers(registers)
            .expect("loading registers into unicorn");
        *self.unicorn.get_data_mut() = Stopped {
            deadline,
            interrupt,
            ..Stopped::default()
        };
        let count = slice_nanoseconds.map_or(UNICORN_NO_LIMIT, |nanoseconds| {
            usize::try_from(nanoseconds.max(ONE_INSTRUCTION)).unwrap_or(usize::MAX)
        });
        if count > UNICORN_NO_LIMIT && !self.counting {
            self.unicorn
                .ctl_flush_tb()
                .expect("flushing unicorn's translated blocks");
        }
        // Unicorn drops its blocks itself when counting stops.
        self.counting = count > UNICORN_NO_LIMIT;
        let result = self
            .unicorn
            .emu_start(registers.pc, u64::MAX, UNICORN_NO_TIMEOUT, count);
        self.save_registers(registers)
            .expect("reading registers from unicorn");
        let stopped = std::mem::take(self.unicorn.get_data_mut());
        // QEMU leaves the program counter after a call to a higher level; at EL0 the call is undefined and the
        // program counter stays on it.
        if matches!(
            stopped.exception,
            Some(EXCEPTION_HYPERVISOR_CALL | EXCEPTION_SECURE_MONITOR_CALL)
        ) {
            registers.pc -= INSTRUCTION_BYTES;
        }
        (
            self.to_exit(result, &stopped, registers),
            stopped.instructions,
        )
    }

    /// Returns what the hooks recorded and unicorn's result as the exit they are.
    fn to_exit(
        &self,
        result: Result<(), uc_error>,
        stopped: &Stopped,
        registers: &RegisterBlock,
    ) -> Result<Exit, Unsupported> {
        // A jump to an address that is not a multiple of 4 faults on alignment before any fetch.
        if let Some((address, access)) = stopped.fault {
            if access == Access::Execute && !address.is_multiple_of(INSTRUCTION_BYTES) {
                return Ok(Exit::AlignmentFault { address });
            }
            return Ok(Exit::MemoryFault { address, access });
        }
        let pc = registers.pc;
        Ok(match (result, stopped.exception) {
            (_, Some(EXCEPTION_SYSCALL)) => {
                let x = &registers.x;
                Exit::Syscall(SystemCallRequest {
                    number: SystemCallNumber(x[wcontainers_abi::SYSTEM_CALL_NUMBER_REGISTER]),
                    arguments: x[..SYSTEM_CALL_ARGUMENT_COUNT]
                        .try_into()
                        .expect("a system call's arguments are registers"),
                })
            }
            (_, Some(EXCEPTION_BREAKPOINT)) => {
                let mut instruction = [u8::MIN; INSTRUCTION_BYTES as usize];
                self.unicorn
                    .mem_read(pc, &mut instruction)
                    .expect("the breakpoint's instruction is mapped");
                let comment = u16::try_from(IMM16.read(u32::from_le_bytes(instruction)))
                    .expect("a breakpoint's comment is 16 bits");
                Exit::Breakpoint { comment }
            }
            // QEMU raises a fetch from a pc that is not a multiple of 4 as undefined.
            (_, Some(EXCEPTION_UNDEFINED)) if !pc.is_multiple_of(INSTRUCTION_BYTES) => {
                Exit::AlignmentFault { address: pc }
            }
            // Unicorn runs at EL1, where HVC and SMC may reach a higher level; at EL0 both are undefined.
            (
                _,
                Some(
                    EXCEPTION_UNDEFINED | EXCEPTION_HYPERVISOR_CALL | EXCEPTION_SECURE_MONITOR_CALL,
                ),
            )
            | (Err(uc_error::INSN_INVALID), None) => Exit::IllegalInstruction,
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

/// The stand-in behind the runtime's loop: one turn of Unicorn's for each exit, and its progress is the
/// instructions it ran.
impl wcontainers_runtime_single_kernel::Execution for StandIn {
    type Failure = Unsupported;

    fn run(
        &mut self,
        thread: ThreadId,
        mut resume: Resume,
        mut turn: wcontainers_runtime_single_kernel::Turn,
        answer: &mut dyn wcontainers_runtime_single_kernel::Answer,
    ) -> Result<(), Unsupported> {
        loop {
            let wcontainers_runtime_single_kernel::Turn::Counted { limit } = turn else {
                panic!("the stand-in counts every turn");
            };
            let exit = StandIn::run(self, thread, resume, limit)?;
            let progress = self.last_instructions;
            match answer.exit(self, exit, progress) {
                wcontainers_runtime_single_kernel::Carry::On {
                    resume: next,
                    turn: then,
                } => (resume, turn) = (next, then),
                wcontainers_runtime_single_kernel::Carry::Leave(_) => return Ok(()),
            }
        }
    }

    fn restored(&self) -> Self {
        StandIn::restored(self)
    }
}

impl Default for StandIn {
    fn default() -> Self {
        Self::new()
    }
}

/// The stand-in runs guest code as it is, with no translator to configure.
impl wcontainers_executor::Executor for StandIn {
    type Architecture = Arm64;
    type TranslatorConfig = ();
    type GuestMemory = UnicornSpace;
    type CreateGuestMemoryError = CpuError;
    type Interrupter = StandInInterrupter;

    fn create_guest_memory(&self) -> Result<UnicornSpace, CpuError> {
        UnicornSpace::new(self.end)
    }

    /// Panics where unicorn stops for a reason the stand-in cannot give as the processor's ([`Unsupported`]).
    ///
    /// The execution blocks: it is ready at its first poll.
    fn execute<'execution>(
        &'execution mut self,
        processor: Processor,
        memory: &'execution UnicornSpace,
        handler: &'execution mut impl TrapHandler,
    ) -> (
        impl Future<Output = Processor> + 'execution,
        StandInInterrupter,
    ) {
        let raised = Arc::new(AtomicBool::new(false));
        let interrupter = StandInInterrupter {
            raised: Arc::clone(&raised),
        };
        let execution = async move {
            let mut registers = to_register_block(&processor);
            loop {
                if raised.swap(false, Ordering::AcqRel) {
                    match handler.handle_interrupt() {
                        InterruptResponse::Continue => {}
                        InterruptResponse::Terminate => return to_processor(&registers, None),
                    }
                }
                let (exit, instructions) =
                    memory.run_turn(&mut registers, None, None, Some(Arc::clone(&raised)));
                self.last_instructions = instructions;
                let exit = match exit {
                    Ok(exit) => exit,
                    Err(Unsupported::AlignmentFault { pc }) => {
                        let exception =
                            to_alignment_exception(read_instruction(memory, pc), &registers);
                        return to_processor(&registers, Some(exception));
                    }
                    Err(unknown) => panic!("{unknown:?}"),
                };
                let permission_at = |address| to_page_permission(memory.read_permissions(address));
                match exit {
                    Exit::Syscall(request) => match handler.handle_system_call(request) {
                        SystemCallResponse::Return(value) => registers.x[RESULT_REGISTER] = value,
                        SystemCallResponse::Terminate => {
                            // The abi leaves the program counter after a system call; an exception leaves it on
                            // the instruction.
                            registers.pc -= INSTRUCTION_BYTES;
                            let word = read_instruction(memory, registers.pc);
                            let exception = to_exception(exit, word, registers.pc, permission_at);
                            return to_processor(&registers, exception);
                        }
                    },
                    Exit::MemoryFault { address, access } => {
                        match handler.handle_memory_fault(address, access) {
                            MemoryFaultResponse::Retry => {}
                            MemoryFaultResponse::Terminate => {
                                let exception =
                                    to_exception(exit, u32::MIN, registers.pc, permission_at);
                                return to_processor(&registers, exception);
                            }
                        }
                    }
                    // Only an interrupt stops a turn with no limit; it is answered at the top.
                    Exit::Stop => {}
                    _ => {
                        let exception = to_exception(exit, u32::MIN, registers.pc, permission_at);
                        return to_processor(&registers, exception);
                    }
                }
            }
        };
        (execution, interrupter)
    }

    /// Panics where unicorn stops for a reason the stand-in cannot give as the processor's ([`Unsupported`]).
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: &[MemoryDelta<'_, Arm64>],
        mut on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    ) {
        // The space ends after the last byte the state names, so a state costs no more than it holds.
        let named_end = memory
            .iter()
            .map(|delta| match delta {
                MemoryDelta::PermissionAssignment { addresses, .. } => addresses.end,
                MemoryDelta::ValueAssignment { start, bytes } => {
                    start + u64::try_from(bytes.len()).expect("a count of bytes fits in an address")
                }
            })
            .fold(processor.program_counter + INSTRUCTION_BYTES, u64::max);
        let end = named_end
            .div_ceil(PAGE_SIZE)
            .saturating_mul(PAGE_SIZE)
            .min(self.end);
        let space = UnicornSpace::new(end).expect("host memory for one more space");
        let mut pages: BTreeMap<u64, PagePermission> = BTreeMap::new();
        for delta in memory {
            match delta {
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => {
                    space
                        .set_permissions(addresses.clone(), to_permissions(*permission))
                        .expect("a permission is given to whole pages inside the space");
                    for page in addresses.clone().step_by(PAGE_BYTES) {
                        pages.insert(page, *permission);
                    }
                }
                MemoryDelta::ValueAssignment { start, bytes } => {
                    let end = start
                        + u64::try_from(bytes.len()).expect("a count of bytes fits in an address");
                    let written = space
                        .borrow(*start..end)
                        .expect("the bytes lie inside the space")
                        .write(bytes);
                    assert_eq!(written, bytes.len(), "the bytes written");
                }
            }
        }
        let pc = processor.program_counter;
        let word = read_instruction(&space, pc);
        let writable: Vec<u64> = pages
            .iter()
            .filter(|(_, permission)| {
                matches!(
                    permission,
                    PagePermission::Mapped {
                        data_access: DataAccess::ReadWrite,
                        ..
                    }
                )
            })
            .map(|(page, _)| *page)
            .collect();
        let read_page = |page: u64| {
            let mut bytes = vec![u8::MIN; PAGE_BYTES];
            space
                .borrow(page..page + PAGE_SIZE)
                .expect("a page the state gave a permission")
                .read(&mut bytes);
            bytes
        };
        let before: Vec<Vec<u8>> = writable.iter().map(|page| read_page(*page)).collect();
        let mut registers = to_register_block(processor);
        let (exit, instructions) =
            space.run_turn(&mut registers, Some(ONE_INSTRUCTION), None, None);
        self.last_instructions = instructions;
        let exception = match exit {
            Ok(exit) => {
                // The abi leaves the program counter after a system call; an exception leaves it on the
                // instruction.
                if let Exit::Syscall(_) = exit {
                    registers.pc = pc;
                }
                to_exception(exit, word, pc, |address| {
                    pages
                        .get(&(address - address % PAGE_SIZE))
                        .copied()
                        .unwrap_or(PagePermission::Unmapped)
                })
            }
            Err(Unsupported::AlignmentFault { .. }) => {
                Some(to_alignment_exception(word, &registers))
            }
            Err(unknown) => panic!("{unknown:?}"),
        };
        *processor = to_processor(&registers, exception);
        for (page, before) in writable.iter().zip(before) {
            report_changed_bytes(*page, &before, &read_page(*page), &mut on_memory_mutated);
        }
    }
}

/// Raises an interrupt in the one execution it was made with.
pub struct StandInInterrupter {
    raised: Arc<AtomicBool>,
}

impl ExecutionInterrupter for StandInInterrupter {
    fn interrupt(&self) {
        self.raised.store(true, Ordering::Release);
    }
}

/// Returns the instruction word at the address.
fn read_instruction(space: &UnicornSpace, address: u64) -> u32 {
    let mut word = [u8::MIN; INSTRUCTION_BYTES as usize];
    space
        .borrow(address..address + INSTRUCTION_BYTES)
        .expect("the program counter lies inside the space")
        .read(&mut word);
    u32::from_le_bytes(word)
}

fn to_permissions(permission: PagePermission) -> Permissions {
    match permission {
        PagePermission::Unmapped => Permissions::empty(),
        PagePermission::Mapped {
            data_access,
            instruction_fetch,
        } => {
            let data = match data_access {
                DataAccess::Forbidden => Permissions::empty(),
                DataAccess::ReadOnly => Permissions::READ,
                DataAccess::ReadWrite => Permissions::READ | Permissions::WRITE,
            };
            let fetch = match instruction_fetch {
                InstructionFetch::Forbidden => Permissions::empty(),
                InstructionFetch::Allowed => Permissions::EXECUTE,
            };
            data | fetch
        }
    }
}

/// Returns the architecture's page permission for what a space's page allows, a page allowing nothing unmapped.
fn to_page_permission(allowed: Permissions) -> PagePermission {
    if allowed.is_empty() {
        return PagePermission::Unmapped;
    }
    let data_access = if allowed.contains(Permissions::WRITE) {
        DataAccess::ReadWrite
    } else if allowed.contains(Permissions::READ) {
        DataAccess::ReadOnly
    } else {
        DataAccess::Forbidden
    };
    let instruction_fetch = if allowed.contains(Permissions::EXECUTE) {
        InstructionFetch::Allowed
    } else {
        InstructionFetch::Forbidden
    };
    PagePermission::Mapped {
        data_access,
        instruction_fetch,
    }
}

fn to_register_block(processor: &Processor) -> RegisterBlock {
    RegisterBlock {
        x: processor.general_purpose_registers,
        sp: processor.stack_pointer,
        pc: processor.program_counter,
        pstate: u64::from(processor.condition_flags.bits()),
        v: processor
            .vector_registers
            .map(|value| [value as u64, (value >> u64::BITS) as u64]),
        fpsr: u32::try_from(processor.floating_point_status)
            .expect("FPSR's upper half is reserved as zero"),
        fpcr: u32::try_from(processor.floating_point_control)
            .expect("FPCR's upper half is reserved as zero"),
        tpidr_el0: processor.software_thread_id,
    }
}

fn to_processor(registers: &RegisterBlock, exception: Option<Exception>) -> Processor {
    Processor {
        general_purpose_registers: registers.x,
        stack_pointer: registers.sp,
        program_counter: registers.pc,
        condition_flags: ConditionFlags::from_bits_truncate(registers.pstate as u32),
        vector_registers: registers
            .v
            .map(|[low, high]| u128::from(low) | u128::from(high) << u64::BITS),
        floating_point_control: u64::from(registers.fpcr),
        floating_point_status: u64::from(registers.fpsr),
        software_thread_id: registers.tpidr_el0,
        exception,
    }
}

/// Returns the exception an exit is, or none when the instruction completed; `word` is the instruction at `pc`.
fn to_exception(
    exit: Exit,
    word: u32,
    pc: u64,
    permission_at: impl Fn(u64) -> PagePermission,
) -> Option<Exception> {
    let fault_status = |address| match permission_at(address) {
        PagePermission::Unmapped => FaultStatus::TranslationFaultLevel3 as u64,
        PagePermission::Mapped { .. } => FaultStatus::PermissionFaultLevel3 as u64,
    };
    let (class, specific, fault_address) = match exit {
        Exit::Stop => return None,
        Exit::Syscall(_) => (ESR_EL1::EC::Value::SVC64, u64::from(IMM16.read(word)), None),
        Exit::MemoryFault {
            address,
            access: Access::Execute,
        } => (
            ESR_EL1::EC::Value::InstrAbortLowerEL,
            fault_status(address),
            Some(address),
        ),
        Exit::MemoryFault {
            address,
            access: Access::Read,
        } => (
            ESR_EL1::EC::Value::DataAbortLowerEL,
            fault_status(address),
            Some(address),
        ),
        Exit::MemoryFault {
            address,
            access: Access::Write,
        } => (
            ESR_EL1::EC::Value::DataAbortLowerEL,
            WRITE_NOT_READ | fault_status(address),
            Some(address),
        ),
        Exit::AlignmentFault { address } if address == pc => {
            (ESR_EL1::EC::Value::PCAlignmentFault, u64::MIN, Some(pc))
        }
        Exit::AlignmentFault { .. } => (ESR_EL1::EC::Value::SPAlignmentFault, u64::MIN, None),
        Exit::IllegalInstruction => (ESR_EL1::EC::Value::Unknown, u64::MIN, None),
        Exit::Breakpoint { comment } => (ESR_EL1::EC::Value::Brk64, u64::from(comment), None),
        Exit::MemoryFaultWithoutRegisters => {
            panic!("the instruction at {pc:#x} reached beyond the space")
        }
    };
    Some(Exception {
        syndrome: encode_syndrome(class, specific),
        fault_address,
    })
}

/// Returns the exception of an alignment fault of the instruction `word`, which takes its address from its base register.
fn to_alignment_exception(word: u32, registers: &RegisterBlock) -> Exception {
    // QEMU checks alignment only on the exclusive, compare-and-swap and atomic instructions, which all take their
    // address from Rn with no offset; real ARM64 faults the same way on an exclusive one.
    let base = usize::try_from(RN.read(word)).expect("a register number fits in an index");
    match registers.x.get(base) {
        Some(address) => Exception {
            syndrome: encode_syndrome(ESR_EL1::EC::Value::DataAbortLowerEL, ALIGNMENT_FAULT_STATUS),
            fault_address: Some(*address),
        },
        // Rn 31 names the stack pointer, which is checked first: misaligned for the access, it is misaligned for
        // the stack pointer alignment check Linux turns on.
        None => Exception {
            syndrome: encode_syndrome(ESR_EL1::EC::Value::SPAlignmentFault, u64::MIN),
            fault_address: None,
        },
    }
}

impl Cpu for StandIn {
    fn address_space(&self) -> AddressSpace {
        AddressSpace { end: self.end }
    }

    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        let memory = Memory::new(self.end)?;
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
        if let Some(mut dropped) = self.memories.remove(&memory)
            && let Some(digests) = &mut self.digests
        {
            digests.push((memory, dropped.digest()));
        }
        self.let_go_of_kept();
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
        self.let_go_of_kept();
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
        let base = block.bytes.as_mut_ptr().wrapping_add(offset as usize);
        let memory = self.memories.get_mut(&memory).expect("memory exists");
        memory.range(address, length)?;
        memory.unshare(address, address + length);
        memory.shared.insert(address, (address + length, base));
        memory.remap(address, address + length);
        self.let_go_of_kept();
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
            #[expect(
                unsafe_code,
                reason = "a slice of the part of a block a memory showed until now, its length from shared_map's check; the block lives, as shared_drop keeps a shown one; unicorn's mem_read replaces it in step one"
            )]
            let bytes = unsafe { std::slice::from_raw_parts(base, (end - start) as usize) };
            memory.bytes[start as usize..end as usize].copy_from_slice(bytes);
            memory.touch(start, end);
        }
        memory.remap(address, address + length);
        self.let_go_of_kept();
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

    /// The block's host memory is unmapped, unless a memory still shows it: unicorn and the stand-in's own
    /// copies would reach freed memory through that memory, so the drop is refused ([`Refused`]) and the
    /// block kept until no memory shows it.
    fn shared_drop(&mut self, shared: SharedId) {
        let Some(block) = self.blocks.remove(&shared) else {
            return;
        };
        let shown_in = self.showing(&block);
        if shown_in.is_empty() {
            return;
        }
        let refused = Refused::DroppingAShownBlock {
            block: shared,
            shown_in,
        };
        eprintln!("the stand-in refused the kernel's request (abi/cpu.rs:88): {refused:?}");
        self.refused.push(refused);
        self.kept.push(block);
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

/// Copies the pages of `from` that hold anything into `to`, which is fresh zeroed memory of the
/// same length, so a block much larger than what its mappings touched (a mapping far into a
/// file) takes host memory only for what they touched.
fn copy_touched(from: &MmapMut, to: &mut MmapMut) {
    const CHUNK: usize = wcontainers_abi::PAGE_SIZE as usize;
    for (index, chunk) in from.chunks(CHUNK).enumerate() {
        if chunk.iter().any(|byte| *byte != 0) {
            let start = index * CHUNK;
            to[start..start + chunk.len()].copy_from_slice(chunk);
        }
    }
}

#[cfg(test)]
mod tests {
    use unicorn_engine::RegisterARM64;

    use super::{q_register, x_register};

    #[test]
    fn register_numbers_end_where_unicorn_numbers_them() {
        assert_eq!(x_register(28), RegisterARM64::X28 as i32);
        assert_eq!(x_register(29), RegisterARM64::X29 as i32);
        assert_eq!(x_register(30), RegisterARM64::X30 as i32);
        assert_eq!(q_register(31), RegisterARM64::Q31 as i32);
    }

    /// Before, this index became a value of unicorn's register enum that names no register, which
    /// is undefined behavior from a safe function.
    #[test]
    #[should_panic(expected = "no general register X31")]
    fn a_general_register_past_x30_is_refused() {
        x_register(31);
    }

    /// As for the general registers.
    #[test]
    #[should_panic(expected = "no vector register Q32")]
    fn a_vector_register_past_q31_is_refused() {
        q_register(32);
    }
}
