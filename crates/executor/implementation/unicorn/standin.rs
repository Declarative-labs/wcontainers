//! The stand-in CPU: the unicorn-engine ARM64 emulator behind the owner's [`Executor`] and the abi's [`Cpu`] trait.
//!
//! Each memory is one unicorn instance, because a unicorn instance is one address space ([`crate::memory`]). Every
//! unicorn instance of a stand-in lives on its engine's host thread ([`crate::engine`]): the stand-in and its spaces
//! are handles that may move between threads, and its memories, threads and blocks are its [`Machine`] there.

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::future::Ready;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wcontainers_abi::{
    Access, AddressSpace, Cpu, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock,
    Resume, SharedId, SystemCallRequest, ThreadId,
};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, PerThreadWatch, SystemCallResponse, TrapHandler,
};
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture,
};
use wcontainers_processor_architecture_arm64::encoding::{IMM16, RN};
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_processor_architecture_arm64::system_register::{
    NEOVERSE_V2_SYSTEM_REGISTERS, simulate_system_register_move,
};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, ConditionFlags, DataAccess, Exception, ExclusiveMonitor, FaultStatus,
    INSTRUCTION_SIZE, Instruction, InstructionFetch, PagePermission, Processor, User,
    WaitInstruction, ZERO_PROCESSOR,
};
use wcontainers_unicorn::uc_error;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::engine::EngineHandle;
use crate::memory::{Memory, Predicates, SharedPages, UNTAGGED};
use crate::space::{UnicornPageTable, UnicornSpace};

/// A process's address space on a host with 32 bit memories.
pub const SPACE_32_BIT: u64 = 1 << 32;
/// A process's address space on a host with 64 bit memories, at the smallest cap such a host
/// sets (Chrome's 16 GiB).
pub const SPACE_64_BIT: u64 = 16 << 30;

/// How many guest instructions an execution retires before the stand-in asks its handler, as an interrupt from outside
/// does, unless configured otherwise.
pub const DEFAULT_SLICE_INSTRUCTIONS: u64 = 1_000_000;

/// What the stand-in is configured with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandInConfig {
    /// How many guest instructions an execution retires before it calls `TrapHandler::handle_interrupt`, counted from
    /// its start and again after each interrupt, so where a slice ends repeats exactly.
    pub slice_instructions: u64,
}

impl Default for StandInConfig {
    fn default() -> Self {
        Self {
            slice_instructions: DEFAULT_SLICE_INSTRUCTIONS,
        }
    }
}

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
    /// The emulator ends the whole host process on this instruction word instead of raising an exception, so the
    /// stand-in does not run it.
    EmulatorAborts { pc: u64, word: u32 },
    /// A system instruction stopped as undefined, which is either undefined or trapped to EL1: unicorn gives no
    /// syndrome to tell them apart.
    TrapOrUndefined { pc: u64, word: u32 },
    /// The state names memory past the end of the stand-in's spaces.
    BeyondSpace { end: u64 },
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
        let key = self.key;
        self.engine.call(move |engine| {
            engine.machine(key).digests.get_or_insert_default();
        });
    }

    fn digests(&mut self) -> crate::replay::MemoryDigests {
        let key = self.key;
        self.engine.call(move |engine| {
            let machine = engine.machine(key);
            let mut digests = machine.digests.clone().unwrap_or_default();
            let mut left: Vec<MemoryId> = machine.memories.keys().copied().collect();
            left.sort();
            for memory in left {
                let digest = digest(&machine.memories[&memory]);
                digests.push((memory, digest));
            }
            digests
        })
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

/// The stand-in CPU side: a handle to its machine on its engine's thread.
pub struct StandIn {
    engine: EngineHandle,
    /// The key of its machine on the engine.
    key: u64,
    /// Where every memory's address space ends.
    end: u64,
    /// How many instructions the last turn ran, about: whole blocks are counted.
    pub last_instructions: u64,
    /// Whether the kernel's clock is the machine's, so a slice is also a span of it.
    wall_clock: bool,
    config: StandInConfig,
}

/// The memories, threads and blocks of a stand-in, on its engine's thread.
pub(crate) struct Machine {
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
    /// What each memory held as it was dropped, once asked for.
    #[cfg(feature = "harness")]
    digests: Option<crate::replay::MemoryDigests>,
}

/// A kernel request the stand-in refused breaks the [`Cpu`] contract, so the test it ran in fails,
/// unless the test took the refusal ([`StandIn::take_refused`]) or is failing already.
impl Drop for StandIn {
    fn drop(&mut self) {
        let key = self.key;
        let refused = self.engine.call(move |engine| {
            engine
                .machines
                .remove(&key)
                .map(|machine| machine.refused)
                .unwrap_or_default()
        });
        if !refused.is_empty() && !std::thread::panicking() {
            panic!("the stand-in refused the kernel's requests (abi/cpu.rs): {refused:?}");
        }
    }
}

impl StandIn {
    /// A CPU side as on a host with 32 bit memories.
    pub fn new() -> Self {
        Self::with_space(SPACE_32_BIT)
    }

    /// A CPU side whose memories' address spaces end at `end`, on an engine of its own.
    pub fn with_space(end: u64) -> Self {
        Self::on_engine(EngineHandle::start(), end, Machine::new)
    }

    /// A CPU side on the engine with the machine `make` makes there.
    fn on_engine(
        engine: EngineHandle,
        end: u64,
        make: impl FnOnce(u64) -> Machine + Send + 'static,
    ) -> Self {
        let key = engine.call(move |engine| {
            let key = engine.take_key();
            engine.machines.insert(key, make(end));
            key
        });
        Self {
            engine,
            key,
            end,
            last_instructions: 0,
            wall_clock: false,
            config: StandInConfig::default(),
        }
    }

    /// The same CPU side with the configuration given.
    pub fn configured(mut self, config: StandInConfig) -> Self {
        self.config = config;
        self
    }

    /// The same CPU side for a kernel on the machine's clock: a turn also ends once its slice has
    /// passed on that clock, since an instruction then takes what it takes.
    pub fn on_wall_clock(mut self) -> Self {
        self.wall_clock = true;
        self
    }

    /// A new CPU side holding what this one holds, as bringing back a saved container makes it: every
    /// memory, block of shared pages and thread's registers copied under the same handles, on new
    /// emulator instances with nothing translated or cached, on the same engine.
    pub fn restored(&self) -> Self {
        let key = self.key;
        let copy = self.engine.call(move |engine| {
            let copy = engine.machine(key).restored();
            let copy_key = engine.take_key();
            engine.machines.insert(copy_key, copy);
            copy_key
        });
        Self {
            engine: self.engine.clone(),
            key: copy,
            end: self.end,
            last_instructions: 0,
            wall_clock: self.wall_clock,
            config: self.config,
        }
    }

    /// What the stand-in refused of the kernel's requests so far, in order, taken: a stand-in
    /// dropped with a refusal not taken fails its test.
    pub fn take_refused(&mut self) -> Vec<Refused> {
        let key = self.key;
        self.engine
            .call(move |engine| std::mem::take(&mut engine.machine(key).refused))
    }

    /// Adds what the stand-in of a copy made at a save refused, which the copy reported as it
    /// ended, so they fail the test as this stand-in goes, as its own refusals do.
    #[cfg(all(feature = "harness", not(feature = "runtime")))]
    pub(crate) fn refused_in_a_copy(&mut self, refused: Vec<Refused>) {
        let key = self.key;
        self.engine
            .call(move |engine| engine.machine(key).refused.extend(refused));
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
        let key = self.key;
        let (exit, instructions) = self.engine.call(move |engine| {
            engine
                .machine(key)
                .run(thread, resume, slice_nanoseconds, deadline)
        });
        self.last_instructions = instructions;
        exit
    }

    /// Runs a thread of a kernel that keeps its threads' registers and memories itself, as
    /// [`StandIn::run`] runs one of its own; the memory must be one of this stand-in's.
    pub fn run_on(
        &mut self,
        registers: &mut RegisterBlock,
        memory: &UnicornSpace,
        resume: Resume,
        slice_nanoseconds: Option<u64>,
    ) -> Result<Exit, Unsupported> {
        assert!(
            memory.is_on(&self.engine),
            "the memory is one of this stand-in's"
        );
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

    /// Runs the call on the stand-in's machine, on its engine's thread.
    fn with_machine<Answer: Send + 'static>(
        &self,
        call: impl FnOnce(&mut Machine) -> Answer + Send + 'static,
    ) -> Answer {
        let key = self.key;
        self.engine.call(move |engine| call(engine.machine(key)))
    }
}

impl Default for StandIn {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    fn new(end: u64) -> Self {
        Self {
            end,
            memories: HashMap::new(),
            threads: HashMap::new(),
            blocks: HashMap::new(),
            refused: Vec::new(),
            last_memory: 0,
            last_thread: 0,
            last_block: 0,
            #[cfg(feature = "harness")]
            digests: None,
        }
    }

    /// A copy of every memory, block and thread under the same handles, on new unicorn instances.
    fn restored(&self) -> Self {
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
            #[cfg(feature = "harness")]
            digests: None,
        }
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

    /// Runs a thread until it comes back, as [`StandIn::run`] describes, with the number of instructions run.
    fn run(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        slice_nanoseconds: Option<u64>,
        deadline: Option<coarsetime::Instant>,
    ) -> (Result<Exit, Unsupported>, u64) {
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
        memory.run_turn(&mut state.registers, slice_nanoseconds, deadline, None)
    }

    fn memory_mut(&mut self, memory: MemoryId) -> &mut Memory {
        self.memories.get_mut(&memory).expect("memory exists")
    }
}

/// Each writable page's address with its bytes before and after an instruction.
type ChangedPages = Vec<(u64, Vec<u8>, Vec<u8>)>;

/// Runs the one instruction at the processor's program counter on the cleared memory given the state's pages, as
/// [`StandIn`]'s `execute_single_instruction_in_place` describes, on the engine's thread.
fn execute_in_place(
    memory: &mut Memory,
    processor: &mut Processor<()>,
    mutations: &[MemoryDelta<'static, Arm64<User>>],
) -> (Result<ChangedPages, Unsupported>, u64) {
    let named_end = mutations
        .iter()
        .map(|delta| match delta {
            MemoryDelta::PermissionAssignment { addresses, .. } => addresses.end,
            MemoryDelta::ValueAssignment { start, bytes } => {
                start + u64::try_from(bytes.len()).expect("a count of bytes fits in an address")
            }
        })
        .fold(processor.program_counter + INSTRUCTION_SIZE, u64::max);
    // The scratch page the predicates pass through lies after the named pages.
    let scratch = named_end.div_ceil(PAGE_SIZE).saturating_mul(PAGE_SIZE);
    if scratch >= memory.end() {
        return (Err(Unsupported::BeyondSpace { end: named_end }), 0);
    }
    let mut pages: BTreeMap<u64, PagePermission> = BTreeMap::new();
    for delta in mutations {
        match delta {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => {
                assert!(
                    addresses.start.is_multiple_of(PAGE_SIZE)
                        && addresses.end.is_multiple_of(PAGE_SIZE),
                    "a permission is given to whole pages"
                );
                memory.set_permissions(addresses.clone(), to_permissions(*permission));
                for page in addresses.clone().step_by(PAGE_BYTES) {
                    pages.insert(page, *permission);
                }
            }
            MemoryDelta::ValueAssignment { start, bytes } => {
                memory
                    .write(*start, bytes)
                    .expect("the bytes lie inside the space");
            }
        }
    }
    let pc = processor.program_counter;
    let mut word = [u8::MIN; INSTRUCTION_SIZE as usize];
    memory
        .read(pc, &mut word)
        .expect("the program counter lies inside the space");
    let word = u32::from_le_bytes(word);
    if aborts_emulator(word) {
        return (Err(Unsupported::EmulatorAborts { pc, word }), 0);
    }
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
    let read_page = |memory: &Memory, page: u64| {
        let mut bytes = vec![u8::MIN; PAGE_BYTES];
        memory
            .read(page, &mut bytes)
            .expect("a page the state gave a permission");
        bytes
    };
    let before: Vec<Vec<u8>> = writable
        .iter()
        .map(|page| read_page(memory, *page))
        .collect();
    let mut registers = to_register_block(processor);
    let mut predicates = Predicates {
        registers: processor.predicate_registers,
        first_fault: processor.first_fault_register,
    };
    let (exit, instructions) = memory.run_instruction(&mut registers, &mut predicates, scratch);
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
        Ok(Exit::IllegalInstruction) => match to_undefined_exception(word, pc) {
            Ok(exception) => Some(exception),
            Err(unsupported) => return (Err(unsupported), instructions),
        },
        Ok(exit) => to_exception(exit, word, pc, permission_at),
        Err(Unsupported::AlignmentFault { .. }) => Some(to_alignment_exception(word, &registers)),
        Err(unknown) => return (Err(unknown), instructions),
    };
    // Every exception but a supervisor call is taken at its instruction; unicorn leaves the program counter after an
    // SVE store that faults.
    if exception.is_some_and(|exception| !exception.is_taken_after_instruction()) {
        registers.pc = pc;
    }
    *processor = to_processor(
        &registers,
        predicates,
        processor.process_state_controls,
        exception,
    );
    let changed = writable
        .into_iter()
        .zip(before)
        .map(|(page, before)| (page, before, read_page(memory, page)))
        .collect();
    (Ok(changed), instructions)
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
        let mutations: Vec<MemoryDelta<'static, Arm64<User>>> = memory
            .mutations
            .iter()
            .cloned()
            .map(MemoryDelta::into_owned)
            .collect();
        let mut moved = processor.clone();
        let space_end = self.end;
        let (changed, moved, instructions) = self.engine.call(move |engine| {
            let memory = engine.single_instruction_memory(space_end);
            let (changed, instructions) = execute_in_place(memory, &mut moved, &mutations);
            (changed, moved, instructions)
        });
        self.last_instructions = instructions;
        for (page, before, after) in changed? {
            report_changed_bytes(page, &before, &after, mutation_hook);
        }
        *processor = moved;
        Ok(())
    }
}

/// The stand-in runs guest code as it is, with no translator to configure; its slice is set as it is built
/// ([`StandIn::configured`]).
impl Executor for StandIn {
    type TranslatorConfig = ();
    type GuestMemory = UnicornSpace;
    type CreateGuestMemoryError = CpuError;
    type Interrupter = StandInInterrupter;
    type Execution<'execution> = StandInExecution<'execution>;

    fn create_guest_memory(&self) -> Result<(UnicornSpace, UnicornPageTable), CpuError> {
        UnicornSpace::new(&self.engine, self.end)
    }

    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: Processor<()>,
        memory: &'execution UnicornSpace,
    ) -> (StandInExecution<'execution>, StandInInterrupter) {
        assert!(
            memory.is_on(&self.engine),
            "the memory is one of this stand-in's"
        );
        let raised = Arc::new(AtomicBool::new(false));
        let interrupter = StandInInterrupter {
            raised: Arc::clone(&raised),
        };
        let slice = self.config.slice_instructions;
        let execution = StandInExecution {
            slice,
            left_in_slice: slice,
            stand_in: self,
            registers: to_register_block(&processor),
            predicates: Predicates {
                registers: processor.predicate_registers,
                first_fault: processor.first_fault_register,
            },
            process_state_controls: processor.process_state_controls,
            memory,
            raised,
        };
        (execution, interrupter)
    }
}

/// The kernel watches one thread of a memory itself, before each of its turns, so the stand-in has no watch of its own.
impl PerThreadWatch for StandIn {
    fn set_thread_permissions(
        &mut self,
        _thread: wcontainers_abi::ThreadId,
        _range: std::ops::Range<u64>,
        _allowed: Permissions,
    ) -> Result<(), CpuError> {
        Err(CpuError::NotSupported)
    }
}

/// One execution of the stand-in, not yet run.
pub struct StandInExecution<'execution> {
    stand_in: &'execution mut StandIn,
    registers: RegisterBlock,
    /// The predicate registers and FFR as the execution started: unicorn's interface reaches neither, and a guest's
    /// whole run has no page to pass them through, so the execution ends with them as they were.
    predicates: Predicates,
    /// SSBS, DIT and TCO as the execution started, which it ends with, as unicorn's interface does not reach them.
    process_state_controls: u64,
    memory: &'execution UnicornSpace,
    raised: Arc<AtomicBool>,
    /// The instructions of a slice, and how many of the current slice the guest has yet to retire.
    slice: u64,
    left_in_slice: u64,
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
        ExecutionStop::Terminated(to_processor(
            &self.registers,
            self.predicates,
            self.process_state_controls,
            exception,
        ))
    }
}

impl StandInExecution<'_> {
    /// Runs the guest until it traps, an interrupt is raised, which unicorn answers at the next block, or its slice
    /// ends, which unicorn counts exactly.
    fn run_until_trap(&mut self) -> Stop {
        loop {
            if self.raised.swap(false, Ordering::AcqRel) {
                self.left_in_slice = self.slice;
                return Stop::Interrupted;
            }
            let (exit, instructions) = self.memory.run_turn(
                &mut self.registers,
                Some(self.left_in_slice),
                None,
                Some(Arc::clone(&self.raised)),
            );
            self.stand_in.last_instructions = instructions;
            // Unicorn counts whole blocks as they start, so a trap inside a block counts the block's rest too.
            self.left_in_slice = self.left_in_slice.saturating_sub(instructions);
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
                // A stop with no interrupt raised is the slice's end; an interrupt is answered at the top.
                Exit::Stop if !self.raised.load(Ordering::Acquire) => {
                    self.left_in_slice = self.slice;
                    return Stop::Interrupted;
                }
                Exit::Stop => {}
                Exit::IllegalInstruction => {
                    let pc = self.registers.pc;
                    match to_undefined_exception(read_instruction(self.memory, pc), pc) {
                        Ok(exception) => return Stop::Final(exception),
                        Err(unsupported) => panic!("{unsupported:?}"),
                    }
                }
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

/// Returns the processor unicorn's registers give, with the predicates and SSBS, DIT and TCO the stand-in carries itself.
fn to_processor(
    registers: &RegisterBlock,
    predicates: Predicates,
    process_state_controls: u64,
    exception: Option<Exception>,
) -> Processor<()> {
    Processor {
        general_purpose_registers: registers.x,
        stack_pointer: registers.sp,
        program_counter: registers.pc,
        condition_flags: ConditionFlags::from_bits_truncate(registers.pstate as u32),
        vector_registers: registers
            .v
            .map(|[low, high]| u128::from(low) | u128::from(high) << u64::BITS),
        predicate_registers: predicates.registers,
        first_fault_register: predicates.first_fault,
        // TODO: an MSR (immediate) that writes SSBS, DIT or TCO is not seen, since unicorn's interface reaches none.
        process_state_controls,
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
    // A data address's top byte is its tag, which Linux ignores; an instruction fetch's address has none here.
    let status = |address: u64| {
        let address = address & UNTAGGED;
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

/// Returns the exception an undefined-instruction stop of the instruction `word` at `pc` is.
///
/// QEMU raises an instruction EL1 traps as undefined too, telling the class only in a syndrome unicorn does not give.
/// Outside the system instructions nothing traps at EL0 here (floating point and SVE are on, the pointer authentication
/// keys off); among them WFI always traps, as SCTLR_EL1.nTWI is clear, and the others are refused.
fn to_undefined_exception(word: u32, pc: u64) -> Result<Exception, Unsupported> {
    if word == WAIT_FOR_INTERRUPT {
        Ok(Exception::WaitTrapped(WaitInstruction::WaitForInterrupt))
    } else if let Some(direction) = to_register_move_direction(word) {
        // QEMU raises an access EL1 traps and an undefined one alike; what EL0 reaches of each register, as the
        // architecture lists it, tells them apart. An access the list lets complete stays a refusal.
        let mut processor = ZERO_PROCESSOR;
        match simulate_system_register_move(
            &mut processor,
            word,
            direction,
            NEOVERSE_V2_SYSTEM_REGISTERS,
        ) {
            Err(
                exception @ (Exception::SystemAccessTrapped(_) | Exception::UndefinedInstruction),
            ) => Ok(exception),
            _ => Err(Unsupported::TrapOrUndefined { pc, word }),
        }
    } else if word & SYSTEM_INSTRUCTION_MASK == SYSTEM_INSTRUCTION_VALUE {
        Err(Unsupported::TrapOrUndefined { pc, word })
    } else {
        Ok(Exception::UndefinedInstruction)
    }
}

/// The direction of an MRS (a read) or MSR (register) word (a write), none for another word.
fn to_register_move_direction(word: u32) -> Option<AccessDirection> {
    let matches = |instruction: Instruction<Infallible>| {
        let fixed = instruction.to_fixed_bits();
        word & fixed.mask == fixed.value
    };
    if matches(Instruction::Mrs) {
        Some(AccessDirection::Read)
    } else if matches(Instruction::MsrRegister) {
        Some(AccessDirection::Write)
    } else {
        None
    }
}

/// [WFI](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/WFI--Wait-for-interrupt-).
const WAIT_FOR_INTERRUPT: u32 = 0xD503_207F;
/// The [system instruction class](https://developer.arm.com/documentation/ddi0602/latest/Index-by-Encoding/Branches--Exception-Generating-and-System-instructions):
/// bits 31 to 22 are 0b1101010100 (hints, barriers, PSTATE, SYS, SYSL, MSR and MRS).
const SYSTEM_INSTRUCTION_MASK: u32 = 0xFFC0_0000;
const SYSTEM_INSTRUCTION_VALUE: u32 = 0xD500_0000;

/// The [Advanced SIMD three same (FP16)](https://developer.arm.com/documentation/ddi0602/latest/Index-by-Encoding/Data-Processing----Scalar-Floating-Point-and-Advanced-SIMD)
/// class: `0 Q U 01110 a 10 Rm 00 opcode 1 Rn Rd`.
const THREE_SAME_HALF_MASK: u32 = 0x9F60_C400;
const THREE_SAME_HALF_VALUE: u32 = 0x0E40_0400;
/// The values of U, a and opcode (`U:a:opcode`) the class leaves unallocated, on which QEMU's decoder asserts.
const THREE_SAME_HALF_UNALLOCATED: [u32; 8] = [
    0b00101, 0b01011, 0b01100, 0b01101, 0b10001, 0b11001, 0b11011, 0b11111,
];

/// The first-fault gathers of elements wider than a byte, on some state of each of which unicorn ends the host process
/// with SIGSEGV (MEASURED: each instruction alone in a probe over its generated states, 2026-11-26; the byte gathers, the
/// contiguous first-fault and the non-fault loads ran to the end).
const CRASHING_GATHERS: [Instruction<Infallible>; 27] = [
    Instruction::Ldff1wGatherExtendedOffset,
    Instruction::Ldff1shGatherSCALEDExtendedOffset32,
    Instruction::Ldff1hGatherSCALEDExtendedOffset32,
    Instruction::Ldff1wGatherSCALEDExtendedOffset,
    Instruction::Ldff1shGatherImmediate32,
    Instruction::Ldff1wGatherImmediate,
    Instruction::Ldff1hGatherImmediate32,
    Instruction::Ldff1shGatherExtendedOffset64,
    Instruction::Ldff1swGatherExtendedOffset64,
    Instruction::Ldff1dGatherExtendedOffset,
    Instruction::Ldff1hGatherExtendedOffset64,
    Instruction::Ldff1wGatherExtendedOffset64,
    Instruction::Ldff1shGatherSCALEDExtendedOffset64,
    Instruction::Ldff1swGatherSCALEDExtendedOffset64,
    Instruction::Ldff1dGatherSCALEDExtendedOffset,
    Instruction::Ldff1hGatherSCALEDExtendedOffset64,
    Instruction::Ldff1wGatherSCALEDExtendedOffset64,
    Instruction::Ldff1shGatherImmediate64,
    Instruction::Ldff1swGatherImmediate64,
    Instruction::Ldff1dGatherImmediate,
    Instruction::Ldff1hGatherImmediate64,
    Instruction::Ldff1wGatherImmediate64,
    Instruction::Ldff1shGatherSCALED64,
    Instruction::Ldff1swGatherSCALED64,
    Instruction::Ldff1dGatherSCALED,
    Instruction::Ldff1hGatherSCALED64,
    Instruction::Ldff1wGatherSCALED64,
];

/// Returns whether the emulator ends the host process on the word: an unallocated Advanced SIMD three same (FP16)
/// encoding, which its decoder reaches an assertion on instead of raising an undefined instruction, or one of
/// [`CRASHING_GATHERS`].
fn aborts_emulator(word: u32) -> bool {
    if CRASHING_GATHERS
        .iter()
        .any(|gather| gather.to_fixed_bits().is_matching(word))
    {
        return true;
    }
    if word & THREE_SAME_HALF_MASK != THREE_SAME_HALF_VALUE {
        return false;
    }
    let opcode = word >> 11 & 0b111;
    let a = word >> 23 & 1;
    let u = word >> 29 & 1;
    THREE_SAME_HALF_UNALLOCATED.contains(&(u << 4 | a << 3 | opcode))
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

/// Each call runs on the stand-in's machine, on its engine's thread.
impl Cpu for StandIn {
    fn address_space(&self) -> AddressSpace {
        AddressSpace { end: self.end }
    }

    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        self.with_machine(Machine::memory_new)
    }

    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        self.with_machine(move |machine| machine.memory_duplicate(source))
    }

    fn memory_drop(&mut self, memory: MemoryId) {
        self.with_machine(move |machine| machine.memory_drop(memory));
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        let length = into.len();
        let read = self.with_machine(move |machine| {
            let mut read = vec![u8::MIN; length];
            machine.memory_mut(memory).read(address, &mut read)?;
            Ok::<_, CpuError>(read)
        })?;
        into.copy_from_slice(&read);
        Ok(())
    }

    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        let from = from.to_vec();
        self.with_machine(move |machine| machine.memory_mut(memory).write(address, &from))
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        let end = address.checked_add(length).ok_or(CpuError::BadAddress)?;
        self.with_machine(move |machine| machine.memory_mut(memory).zero(address..end))
    }

    fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        self.with_machine(move |machine| {
            let memory = machine.memory_mut(memory);
            let range = memory.range(address, length)?;
            memory.set_permissions(range.start as u64..range.end as u64, permissions);
            Ok(())
        })
    }

    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, _starts: &[u64]) {
        self.with_machine(move |machine| {
            machine
                .memory_mut(memory)
                .forget_code(address..address.saturating_add(length));
        });
    }

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        self.with_machine(move |machine| machine.shared_new(length))
    }

    fn shared_map(
        &mut self,
        shared: SharedId,
        offset: u64,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.with_machine(move |machine| {
            machine.shared_map(shared, offset, memory, address, length)
        })
    }

    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.with_machine(move |machine| {
            let memory = machine.memory_mut(memory);
            memory.range(address, length)?;
            memory.keep_shared_bytes(address..address + length);
            Ok(())
        })
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        let length = into.len();
        let read = self.with_machine(move |machine| {
            let block = machine.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
            check_block(block, offset, length)?;
            let mut read = vec![u8::MIN; length];
            block.read(offset, &mut read);
            Ok::<_, CpuError>(read)
        })?;
        into.copy_from_slice(&read);
        Ok(())
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        let from = from.to_vec();
        self.with_machine(move |machine| {
            let block = machine.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
            check_block(block, offset, from.len())?;
            block.write(offset, &from);
            Ok(())
        })
    }

    /// The block's handle goes; a memory that still shows it is refused ([`Refused`]) and keeps showing its pages.
    fn shared_drop(&mut self, shared: SharedId) {
        self.with_machine(move |machine| machine.shared_drop(shared));
    }

    fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        self.with_machine(move |machine| machine.thread_new(memory))
    }

    fn thread_drop(&mut self, thread: ThreadId) {
        self.with_machine(move |machine| {
            machine.threads.remove(&thread);
        });
    }

    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        *into = self.with_machine(move |machine| machine.threads[&thread].registers);
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        let from = *from;
        self.with_machine(move |machine| {
            machine
                .threads
                .get_mut(&thread)
                .expect("thread exists")
                .registers = from;
        });
    }
}

impl Machine {
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
        // Unicorn maps the block's host memory itself, which it does only from a whole page to whole pages.
        let whole_pages = [offset, address, length]
            .iter()
            .all(|value| value.is_multiple_of(PAGE_SIZE));
        if !whole_pages
            || offset
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
    use wcontainers_abi::{Cpu, CpuError, PAGE_SIZE, Permissions};

    use super::{Processor, SPACE_32_BIT, StandIn};

    /// The stand-in and its spaces may move to another host thread, though unicorn's instances may not.
    #[test]
    fn the_stand_in_moves_between_threads() {
        fn is_sendable<T: Send>(_: &T) {}
        let mut stand_in = StandIn::with_space(SPACE_32_BIT);
        is_sendable(&stand_in);
        let memory = stand_in.memory_new().expect("a memory");
        let moved = std::thread::spawn(move || {
            stand_in
                .write(memory, PAGE_SIZE, &[u8::MAX])
                .expect("a byte written on another thread");
            stand_in
        })
        .join()
        .expect("the other thread ends");
        let mut stand_in = moved;
        let mut read = [u8::MIN];
        stand_in
            .read(memory, PAGE_SIZE, &mut read)
            .expect("the byte read back");
        assert_eq!(read, [u8::MAX]);
    }

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

    /// A block is shown only from a whole page to whole pages, which is all unicorn maps; a part of a page is refused.
    #[test]
    fn a_block_is_shown_only_as_whole_pages() {
        let mut stand_in = StandIn::with_space(SPACE_32_BIT);
        let memory = stand_in.memory_new().expect("a memory");
        let block = stand_in.shared_new(2 * PAGE_SIZE).expect("a block");
        for (offset, address, length) in [
            (1, PAGE_SIZE, PAGE_SIZE),
            (0, PAGE_SIZE + 1, PAGE_SIZE),
            (0, PAGE_SIZE, PAGE_SIZE - 1),
        ] {
            assert_eq!(
                stand_in.shared_map(block, offset, memory, address, length),
                Err(CpuError::BadAddress),
                "offset {offset}, address {address}, length {length}"
            );
        }
        stand_in
            .shared_map(block, PAGE_SIZE, memory, PAGE_SIZE, PAGE_SIZE)
            .expect("the block's second page shown");
    }

    /// A read of the virtual counter at EL0 traps to EL1, so the kernel answers it from its own clock.
    #[test]
    fn a_counter_read_traps() {
        use wcontainers_abi::{Access, SystemCallRequest};
        use wcontainers_executor::memory_space::{MemorySpace, PageTable};
        use wcontainers_executor::{
            Execution, ExecutionStop, Executor, InterruptResponse, MemoryFaultResponse,
            SystemCallResponse, TrapHandler,
        };
        use wcontainers_processor_architecture_arm64::{
            AccessDirection, Exception, SystemAccess, ZERO_PROCESSOR,
        };
        use wcontainers_volatile_bytes::VolatileBytes;

        /// Ends the execution at any trap a handler is given.
        struct Ending;
        impl TrapHandler for Ending {
            fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
                SystemCallResponse::Terminate
            }
            fn handle_memory_fault(&mut self, _: u64, _: Access) -> MemoryFaultResponse {
                MemoryFaultResponse::Terminate
            }
            fn handle_interrupt(&mut self) -> InterruptResponse {
                InterruptResponse::Terminate
            }
        }

        /// [MRS](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MRS--Move-system-register-to-general-purpose-register-)
        /// of X0 from CNTVCT_EL0, op0 3, op1 3, CRn 14, CRm 0, op2 2.
        const READ_VIRTUAL_COUNTER: u32 = 0xD53B_E040;
        let mut stand_in = StandIn::with_space(SPACE_32_BIT);
        let (memory, mut table) = stand_in.create_guest_memory().expect("a memory");
        let code = PAGE_SIZE..PAGE_SIZE * 2;
        table
            .set_permissions(code.clone(), Permissions::READ | Permissions::EXECUTE)
            .expect("the code's page allowed");
        memory
            .borrow(code.start..code.start + super::INSTRUCTION_SIZE)
            .expect("the code's word")
            .write(&READ_VIRTUAL_COUNTER.to_le_bytes());
        let start = Processor {
            program_counter: code.start,
            ..ZERO_PROCESSOR
        };
        let (execution, _interrupter) = stand_in.prepare_execution(start.clone(), &memory);
        let ExecutionStop::Terminated(stopped) = execution.run(&mut Ending) else {
            panic!("the stand-in never waits");
        };
        let trapped = Exception::SystemAccessTrapped(SystemAccess {
            op0: 3,
            op1: 3,
            crn: 14,
            crm: 0,
            op2: 2,
            rt: 0,
            direction: AccessDirection::Read,
        });
        assert_eq!(
            stopped,
            Processor {
                exception: Some(trapped),
                ..start
            }
        );
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
