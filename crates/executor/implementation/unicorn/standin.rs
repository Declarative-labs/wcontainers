//! The stand-in CPU: the unicorn-engine ARM64 emulator behind the owner's [`Executor`] and the abi's [`Cpu`] trait.
//!
//! Each memory is one unicorn instance, because a unicorn instance is one address space ([`crate::memory`]). A unicorn
//! instance stays on the host thread that made it, so neither the stand-in nor its spaces move between threads.

use std::collections::{BTreeMap, HashMap};
use std::future::Ready;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use unicorn_engine::uc_error;
use wcontainers_abi::{
    Access, AddressSpace, Cpu, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock,
    Resume, SharedId, SystemCallRequest, ThreadId,
};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture,
};
use wcontainers_processor_architecture_arm64::encoding::{IMM16, RN};
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, ConditionFlags, DataAccess, Exception, ExclusiveMonitor, FaultStatus,
    INSTRUCTION_SIZE, InstructionFetch, PagePermission, Processor, User,
};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::memory::{Memory, ONE_INSTRUCTION, SharedPages};
use crate::space::{UnicornPageTable, UnicornSpace};

/// A process's address space on a host with 32 bit memories.
pub const SPACE_32_BIT: u64 = 1 << 32;
/// A process's address space on a host with 64 bit memories, at the smallest cap such a host
/// sets (Chrome's 16 GiB).
pub const SPACE_64_BIT: u64 = 16 << 30;

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

/// A request of the kernel's that breaks the [`Cpu`] contract, which the stand-in refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// `shared_drop` of a block these memories still show, where the contract says nothing maps it any more
    /// (abi/cpu.rs:88). The memories keep showing the block's pages.
    DroppingAShownBlock {
        block: SharedId,
        shown_in: Vec<MemoryId>,
    },
}

struct Thread {
    memory: MemoryId,
    registers: RegisterBlock,
}

#[cfg(feature = "harness")]
impl crate::replay::DigestMemories for StandIn {
    fn keep_digests(&mut self) {
        self.digests.get_or_insert_default();
    }

    fn digests(&mut self) -> crate::replay::MemoryDigests {
        let mut digests = self.digests.clone().unwrap_or_default();
        let mut left: Vec<MemoryId> = self.memories.keys().copied().collect();
        left.sort();
        for memory in left {
            let digest = digest(&self.memories[&memory]);
            digests.push((memory, digest));
        }
        digests
    }
}

/// A digest of what the memory holds, its shared ranges' pages included.
#[cfg(feature = "harness")]
fn digest(memory: &Memory) -> u64 {
    let contents: Vec<(u64, Vec<u8>)> = memory
        .list_held_pages()
        .into_iter()
        .map(|page| {
            let mut bytes = vec![u8::MIN; PAGE_BYTES];
            memory
                .read(page * PAGE_SIZE, &mut bytes)
                .expect("a page of the memory");
            (page * PAGE_SIZE, bytes)
        })
        .collect();
    crate::replay::digest_pages(
        contents
            .iter()
            .map(|(address, bytes)| (*address, bytes.as_slice())),
    )
}

/// The stand-in CPU side.
pub struct StandIn {
    /// Where every memory's address space ends.
    end: u64,
    memories: HashMap<MemoryId, Memory>,
    threads: HashMap<ThreadId, Thread>,
    blocks: HashMap<SharedId, Rc<SharedPages>>,
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
    #[cfg(feature = "harness")]
    digests: Option<crate::replay::MemoryDigests>,
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
            refused: Vec::new(),
            last_memory: 0,
            last_thread: 0,
            last_block: 0,
            last_instructions: 0,
            wall_clock: false,
            #[cfg(feature = "harness")]
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
        let mut copies: Vec<(Rc<SharedPages>, Rc<SharedPages>)> = Vec::new();
        let mut copy_of = |pages: &Rc<SharedPages>| {
            if let Some((_, copy)) = copies.iter().find(|(old, _)| Rc::ptr_eq(old, pages)) {
                return Rc::clone(copy);
            }
            let copy = Rc::new(pages.copy().expect("host memory for a block"));
            copies.push((Rc::clone(pages), Rc::clone(&copy)));
            copy
        };
        let blocks = self
            .blocks
            .iter()
            .map(|(id, pages)| (*id, copy_of(pages)))
            .collect();
        let memories = self
            .memories
            .iter()
            .map(|(id, memory)| {
                let mut copy = memory.duplicate().expect("a new emulator instance");
                copy.retarget(&mut copy_of);
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
            refused: self.refused.clone(),
            last_memory: self.last_memory,
            last_thread: self.last_thread,
            last_block: self.last_block,
            last_instructions: 0,
            wall_clock: self.wall_clock,
            #[cfg(feature = "harness")]
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
    #[cfg(all(feature = "harness", not(feature = "runtime")))]
    pub(crate) fn refused_in_a_copy(&mut self, refused: Vec<Refused>) {
        self.refused.extend(refused);
    }

    /// The memories that show any of the block's pages, in order.
    fn showing(&self, block: &Rc<SharedPages>) -> Vec<MemoryId> {
        let mut shown_in: Vec<MemoryId> = self
            .memories
            .iter()
            .filter(|(_, memory)| {
                memory
                    .list_parts(0..memory.end())
                    .iter()
                    .any(|(_, part)| matches!(part, crate::memory::Part::Shown { pages, .. } if Rc::ptr_eq(pages, block)))
            })
            .map(|(id, _)| *id)
            .collect();
        shown_in.sort();
        shown_in
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
            state.registers.x[RESULT_REGISTER] = value;
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

    fn memory_mut(&mut self, memory: MemoryId) -> &mut Memory {
        self.memories.get_mut(&memory).expect("memory exists")
    }
}

impl Default for StandIn {
    fn default() -> Self {
        Self::new()
    }
}

impl BehaviorSource for StandIn {
    type Architecture = Arm64<User>;
    type Error = Unsupported;

    /// Runs the one instruction at the program counter on a space holding the snapshot's pages.
    ///
    /// Unicorn keeps its exclusive monitor to itself, so the processor leaves with it open.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), Unsupported> {
        // The space ends after the last byte the state names, so a state costs no more than it holds.
        let named_end = memory
            .mutations
            .iter()
            .map(|delta| match delta {
                MemoryDelta::PermissionAssignment { addresses, .. } => addresses.end,
                MemoryDelta::ValueAssignment { start, bytes } => {
                    start + u64::try_from(bytes.len()).expect("a count of bytes fits in an address")
                }
            })
            .fold(processor.program_counter + INSTRUCTION_SIZE, u64::max);
        let end = named_end
            .div_ceil(PAGE_SIZE)
            .saturating_mul(PAGE_SIZE)
            .min(self.end);
        let (space, mut table) = UnicornSpace::new(end).expect("host memory for one more space");
        let mut pages: BTreeMap<u64, PagePermission> = BTreeMap::new();
        for delta in memory.mutations {
            match delta {
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => {
                    table
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
        let permission_at = |address: u64| {
            pages
                .get(&(address - address % PAGE_SIZE))
                .copied()
                .unwrap_or(PagePermission::Unmapped)
        };
        let exception = match exit {
            // Unicorn stops a counted turn as it fetches the next instruction, so a fetch fault anywhere but at the
            // instruction run belongs to the next one: the instruction completed.
            Ok(
                Exit::MemoryFault {
                    address,
                    access: Access::Execute,
                }
                | Exit::AlignmentFault { address },
            ) if address != pc => None,
            Ok(exit) => to_exception(exit, word, pc, permission_at),
            Err(Unsupported::AlignmentFault { .. }) => {
                Some(to_alignment_exception(word, &registers))
            }
            Err(unknown) => return Err(unknown),
        };
        *processor = to_processor(&registers, exception);
        for (page, before) in writable.iter().zip(before) {
            report_changed_bytes(*page, &before, &read_page(*page), mutation_hook);
        }
        Ok(())
    }
}

/// The stand-in runs guest code as it is, with no translator to configure.
impl Executor for StandIn {
    type TranslatorConfig = ();
    type GuestMemory = UnicornSpace;
    type CreateGuestMemoryError = CpuError;
    type Interrupter = StandInInterrupter;
    type Execution<'execution> = StandInExecution<'execution>;

    fn create_guest_memory(&self) -> Result<(UnicornSpace, UnicornPageTable), CpuError> {
        UnicornSpace::new(self.end)
    }

    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: Processor<()>,
        memory: &'execution UnicornSpace,
    ) -> (StandInExecution<'execution>, StandInInterrupter) {
        let raised = Arc::new(AtomicBool::new(false));
        let interrupter = StandInInterrupter {
            raised: Arc::clone(&raised),
        };
        let execution = StandInExecution {
            stand_in: self,
            registers: to_register_block(&processor),
            memory,
            raised,
        };
        (execution, interrupter)
    }
}

/// One execution of the stand-in, not yet run.
pub struct StandInExecution<'execution> {
    stand_in: &'execution mut StandIn,
    registers: RegisterBlock,
    memory: &'execution UnicornSpace,
    raised: Arc<AtomicBool>,
}

/// Why the stand-in stopped running the guest.
enum Stop {
    Interrupted,
    SystemCall(SystemCallRequest),
    MemoryFault {
        address: u64,
        access: Access,
    },
    /// The guest raised an exception that always ends execution.
    Final(Exception),
}

impl Execution for StandInExecution<'_> {
    type ProcessorState = Processor<()>;
    /// Never made, since the stand-in never waits.
    type Wait = Ready<()>;

    /// Runs until the next trap, then asks the handler, until the handler ends the execution.
    ///
    /// Panics where unicorn stops for a reason the stand-in cannot give as the processor's ([`Unsupported`]).
    fn run(mut self, handler: &mut impl TrapHandler) -> ExecutionStop<Processor<()>, Ready<()>> {
        let exception = loop {
            match self.run_until_trap() {
                Stop::Interrupted => match handler.handle_interrupt() {
                    InterruptResponse::Continue => {}
                    InterruptResponse::Terminate => break None,
                },
                Stop::SystemCall(request) => match handler.handle_system_call(request) {
                    SystemCallResponse::Return(value) => {
                        self.registers.x[RESULT_REGISTER] = value;
                    }
                    SystemCallResponse::Terminate => {
                        // A supervisor call is taken after its instruction, where unicorn leaves the program counter.
                        let word =
                            read_instruction(self.memory, self.registers.pc - INSTRUCTION_SIZE);
                        break Some(Exception::SupervisorCall {
                            immediate: to_immediate(word),
                        });
                    }
                },
                Stop::MemoryFault { address, access } => {
                    match handler.handle_memory_fault(address, access) {
                        MemoryFaultResponse::Retry => {}
                        MemoryFaultResponse::Terminate => {
                            let memory = self.memory;
                            break to_exception(
                                Exit::MemoryFault { address, access },
                                u32::MIN,
                                self.registers.pc,
                                |address| to_page_permission(memory.read_permissions(address)),
                            );
                        }
                    }
                }
                Stop::Final(exception) => break Some(exception),
            }
        };
        ExecutionStop::Terminated(to_processor(&self.registers, exception))
    }
}

impl StandInExecution<'_> {
    /// Runs the guest until it traps or an interrupt is raised, which unicorn answers at the next block.
    fn run_until_trap(&mut self) -> Stop {
        loop {
            if self.raised.swap(false, Ordering::AcqRel) {
                return Stop::Interrupted;
            }
            let (exit, instructions) = self.memory.run_turn(
                &mut self.registers,
                None,
                None,
                Some(Arc::clone(&self.raised)),
            );
            self.stand_in.last_instructions = instructions;
            let exit = match exit {
                Ok(exit) => exit,
                Err(Unsupported::AlignmentFault { pc }) => {
                    let word = read_instruction(self.memory, pc);
                    return Stop::Final(to_alignment_exception(word, &self.registers));
                }
                Err(unknown) => panic!("{unknown:?}"),
            };
            match exit {
                Exit::Syscall(request) => return Stop::SystemCall(request),
                Exit::MemoryFault { address, access } => {
                    return Stop::MemoryFault { address, access };
                }
                // Only an interrupt stops a turn with no limit; it is answered at the top.
                Exit::Stop => {}
                _ => {
                    let memory = self.memory;
                    let exception = to_exception(exit, u32::MIN, self.registers.pc, |address| {
                        to_page_permission(memory.read_permissions(address))
                    })
                    .expect("an exit other than a stop is an exception");
                    return Stop::Final(exception);
                }
            }
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
    let mut word = [u8::MIN; INSTRUCTION_SIZE as usize];
    space
        .borrow(address..address + INSTRUCTION_SIZE)
        .expect("the program counter lies inside the space")
        .read(&mut word);
    u32::from_le_bytes(word)
}

/// Returns the 16 bit immediate of a supervisor call or a breakpoint.
fn to_immediate(word: u32) -> u16 {
    u16::try_from(IMM16.read(word)).expect("the immediate is 16 bits")
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

fn to_register_block(processor: &Processor<()>) -> RegisterBlock {
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

fn to_processor(registers: &RegisterBlock, exception: Option<Exception>) -> Processor<()> {
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
        exclusive_monitor: ExclusiveMonitor::Open,
        exception,
        level_specific: (),
    }
}

/// Returns the exception an exit is, or none when the instruction completed; `word` is the instruction at `pc`.
fn to_exception(
    exit: Exit,
    word: u32,
    pc: u64,
    permission_at: impl Fn(u64) -> PagePermission,
) -> Option<Exception> {
    let status = |address: u64| {
        if address >= Arm64::<User>::ADDRESSABLE.end {
            FaultStatus::TranslationFaultLevel0
        } else {
            match permission_at(address) {
                PagePermission::Unmapped => FaultStatus::TranslationFaultLevel3,
                PagePermission::Mapped { .. } => FaultStatus::PermissionFaultLevel3,
            }
        }
    };
    Some(match exit {
        Exit::Stop => return None,
        Exit::Syscall(_) => Exception::SupervisorCall {
            immediate: to_immediate(word),
        },
        Exit::MemoryFault {
            address,
            access: Access::Execute,
        } => Exception::InstructionAbort {
            status: status(address),
            address,
        },
        Exit::MemoryFault {
            address,
            access: Access::Read,
        } => Exception::DataAbort {
            status: status(address),
            access: AccessDirection::Read,
            address,
        },
        Exit::MemoryFault {
            address,
            access: Access::Write,
        } => Exception::DataAbort {
            status: status(address),
            access: AccessDirection::Write,
            address,
        },
        Exit::AlignmentFault { address } if address == pc => {
            Exception::ProgramCounterAlignmentFault { address: pc }
        }
        Exit::AlignmentFault { .. } => Exception::StackPointerAlignmentFault,
        Exit::IllegalInstruction => Exception::UndefinedInstruction,
        Exit::Breakpoint { comment } => Exception::Breakpoint { comment },
        Exit::MemoryFaultWithoutRegisters => {
            panic!("the instruction at {pc:#x} reached beyond the space")
        }
    })
}

/// Returns the exception of an alignment fault of the instruction `word`, which takes its address from its base register.
fn to_alignment_exception(word: u32, registers: &RegisterBlock) -> Exception {
    // QEMU checks alignment only on the exclusive, compare-and-swap and atomic instructions, which all take their
    // address from Rn with no offset; real ARM64 faults the same way on an exclusive one.
    let base = usize::try_from(RN.read(word)).expect("a register number fits in an index");
    match registers.x.get(base) {
        Some(address) => Exception::DataAbort {
            status: FaultStatus::AlignmentFault,
            access: AccessDirection::Read,
            address: *address,
        },
        // Rn 31 names the stack pointer, which is checked first: misaligned for the access, it is misaligned for
        // the stack pointer alignment check Linux turns on.
        None => Exception::StackPointerAlignmentFault,
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
        let dropped = self.memories.remove(&memory);
        #[cfg(feature = "harness")]
        if let Some(dropped) = dropped
            && let Some(digests) = &mut self.digests
        {
            digests.push((memory, digest(&dropped)));
        }
        #[cfg(not(feature = "harness"))]
        drop(dropped);
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        self.memory_mut(memory).read(address, into)
    }

    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        self.memory_mut(memory).write(address, from)
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        let end = address.checked_add(length).ok_or(CpuError::BadAddress)?;
        self.memory_mut(memory).zero(address..end)
    }

    fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        let memory = self.memory_mut(memory);
        let range = memory.range(address, length)?;
        memory.set_permissions(range.start as u64..range.end as u64, permissions);
        Ok(())
    }

    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, _starts: &[u64]) {
        self.memory_mut(memory)
            .forget_code(address..address.saturating_add(length));
    }

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        let pages = SharedPages::new(length)?;
        self.last_block += 1;
        let id = SharedId(self.last_block);
        self.blocks.insert(id, Rc::new(pages));
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
        let block = Rc::clone(self.blocks.get(&shared).ok_or(CpuError::BadAddress)?);
        if offset
            .checked_add(length)
            .is_none_or(|end| end > block.len())
        {
            return Err(CpuError::BadAddress);
        }
        let memory = self.memory_mut(memory);
        memory.range(address, length)?;
        memory.show(address..address + length, block, offset, true);
        Ok(())
    }

    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        let memory = self.memory_mut(memory);
        memory.range(address, length)?;
        memory.keep_shared_bytes(address..address + length);
        Ok(())
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        let block = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
        check_block(block, offset, into.len())?;
        block.read(offset, into);
        Ok(())
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        let block = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
        check_block(block, offset, from.len())?;
        block.write(offset, from);
        Ok(())
    }

    /// The block's handle goes; a memory that still shows it is refused ([`Refused`]) and keeps showing its pages.
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

/// Returns whether the bytes from the offset lie inside the block.
fn check_block(block: &SharedPages, offset: u64, length: usize) -> Result<(), CpuError> {
    let end = offset
        .checked_add(u64::try_from(length).map_err(|_| CpuError::BadAddress)?)
        .ok_or(CpuError::BadAddress)?;
    if end > block.len() {
        return Err(CpuError::BadAddress);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use wcontainers_abi::{Cpu, PAGE_SIZE, Permissions};

    use super::{SPACE_32_BIT, StandIn};

    /// A block a memory shows keeps its bytes for the memory after the block's handle is dropped.
    #[test]
    fn a_dropped_block_a_memory_shows_keeps_its_bytes() {
        let mut stand_in = StandIn::with_space(SPACE_32_BIT);
        let memory = stand_in.memory_new().expect("a memory");
        let block = stand_in.shared_new(PAGE_SIZE).expect("a block");
        stand_in
            .shared_map(block, 0, memory, PAGE_SIZE, PAGE_SIZE)
            .expect("the block shown");
        stand_in
            .shared_write(block, 0, &[u8::MAX])
            .expect("a byte of the block written");
        stand_in.shared_drop(block);
        let mut read = [u8::MIN];
        stand_in
            .read(memory, PAGE_SIZE, &mut read)
            .expect("the shown byte read");
        assert_eq!(read, [u8::MAX]);
        assert_eq!(stand_in.take_refused().len(), 1, "the refusals");
    }

    /// Two memories that show one block see each other's writes, the guest's among them.
    #[test]
    fn memories_showing_a_block_share_its_bytes() {
        let mut stand_in = StandIn::with_space(SPACE_32_BIT);
        let first = stand_in.memory_new().expect("a memory");
        let second = stand_in.memory_duplicate(first).expect("a copy");
        let block = stand_in.shared_new(PAGE_SIZE).expect("a block");
        for memory in [first, second] {
            stand_in
                .shared_map(block, 0, memory, PAGE_SIZE, PAGE_SIZE)
                .expect("the block shown");
            stand_in
                .set_permissions(
                    memory,
                    PAGE_SIZE,
                    PAGE_SIZE,
                    Permissions::READ | Permissions::WRITE,
                )
                .expect("the page allowed");
        }
        stand_in
            .write(first, PAGE_SIZE, &[u8::MAX])
            .expect("a byte written through the first");
        let mut read = [u8::MIN];
        stand_in
            .read(second, PAGE_SIZE, &mut read)
            .expect("the byte read through the second");
        assert_eq!(read, [u8::MAX]);
        stand_in
            .shared_unmap(second, PAGE_SIZE, PAGE_SIZE)
            .expect("the block unmapped from the second");
        stand_in
            .write(first, PAGE_SIZE, &[u8::MIN])
            .expect("a byte written through the first");
        stand_in
            .read(second, PAGE_SIZE, &mut read)
            .expect("the kept byte read");
        assert_eq!(read, [u8::MAX], "the second keeps what the block held");
    }
}
