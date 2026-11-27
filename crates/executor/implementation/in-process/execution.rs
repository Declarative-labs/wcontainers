//! How form I is driven: [`InProcess`] implements the executor interface, each execution a loop of "run until the next
//! trap, then ask the handler" on the thread that runs it. Every guest instruction is a piece of its own, translated
//! for the in-process way, and every exit of a piece reaches the slot table, of which no page is mapped: the reference
//! path, correct and slow.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use wcontainers_abi::{Access, Permissions, SystemCallNumber, SystemCallRequest};
use wcontainers_arm64::thread_state::RegisterPage;
use wcontainers_arm64::way::Kept;
use wcontainers_arm64::{Arm64 as Arm64Translation, Instruction, Way, decode};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_executor_arm64_stop::in_process::{self, Resolution};
use wcontainers_executor_arm64_stop::{
    FloatStatus, GuestState, Piece, Resolved, SYSCALL_NUMBER, Stop, Taken,
};
use wcontainers_guest_cpu::HostCode;
use wcontainers_linux_process::{LinuxProcess, Pid, RUN_STOP_SIGNAL};
use wcontainers_linux_process_direct::{DirectCodeMapping, DirectLinuxProcess};
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture,
};
use wcontainers_processor_architecture_arm64::encoding::IMM16;
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_processor_architecture_arm64::system_register::{
    NEOVERSE_V2_SYSTEM_REGISTERS, simulate_system_register_move,
};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, ConditionFlags, DataAccess, Exception, ExclusiveMonitor, FaultStatus,
    INSTRUCTION_SIZE, InstructionFetch, PagePermission, Processor, User, ZERO_PROCESSOR,
};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::memory::{
    CreateMemoryError, GUEST_PAGE, InProcessMemory, InProcessPageTable, InProcessPool,
};
use crate::run::{Exit, InProcessThread};

/// Where the pieces are put in this process: past the floor, in the area no guest address reaches.
const CODE_AREA: u64 = (1 << 45) + (1 << 34);
/// The bytes of the code area.
const CODE_AREA_BYTES: u64 = 1 << 40;
/// The top byte of an address, which the chip ignores in an address of the lower half at EL0 (TCR_EL1.TBI0, as Linux
/// sets it).
const TAG: u64 = 0xff << 56;

/// Form I's executor: guest memories are windows of its pool, and its executions run on the thread that runs them.
#[derive(Debug)]
pub struct InProcess {
    pool: Arc<InProcessPool>,
    /// The pieces loaded, by the slot and the displacement of the way they were translated for, their guest address
    /// and their instruction.
    pieces: HashMap<PieceKey, Rc<Loaded>>,
}

/// What a piece is translated from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PieceKey {
    slot: u16,
    displaced: bool,
    guest: u64,
    word: u32,
}

/// A piece of one guest instruction, translated and mapped as code where its runs find it.
#[derive(Debug)]
struct Loaded {
    code: HostCode,
    at: u64,
    _mapping: DirectCodeMapping,
}

impl InProcess {
    /// An executor whose guest memories are slots of the pool.
    pub fn new(pool: Arc<InProcessPool>) -> Self {
        Self {
            pool,
            pieces: HashMap::new(),
        }
    }

    /// The piece of the guest instruction `word` at `guest`, run in `way`, translated and mapped on first use; none
    /// where the instruction cannot be translated or the code area is full.
    fn piece(&mut self, way: Way, guest: u64, word: u32) -> Option<Rc<Loaded>> {
        let Way::InProcess {
            slot, displaced, ..
        } = way
        else {
            return None;
        };
        let key = PieceKey {
            slot,
            displaced,
            guest,
            word,
        };
        if let Some(loaded) = self.pieces.get(&key) {
            return Some(Rc::clone(loaded));
        }
        let code = translate_native_for::<Arm64Translation>(
            &word.to_le_bytes(),
            guest,
            LookAhead::None,
            true,
            &way,
            &(),
        )?;
        let mut bytes = code.bytes.clone();
        bytes.resize(bytes.len().next_multiple_of(GUEST_PAGE as usize), 0);
        let (at, mapping) = map_in_code_area(&bytes)?;
        let loaded = Rc::new(Loaded {
            code,
            at,
            _mapping: mapping,
        });
        self.pieces.insert(key, Rc::clone(&loaded));
        Some(loaded)
    }
}

/// The next free place in the code area, taken for good.
static NEXT_CODE: Mutex<u64> = Mutex::new(CODE_AREA);

/// Maps `bytes` at the next free place of the code area: where something else of this process maps, the place after.
fn map_in_code_area(bytes: &[u8]) -> Option<(u64, DirectCodeMapping)> {
    let mut next = NEXT_CODE.lock().unwrap_or_else(PoisonError::into_inner);
    while *next + bytes.len() as u64 <= CODE_AREA + CODE_AREA_BYTES {
        let at = *next;
        *next += bytes.len() as u64;
        if let Ok(mapping) = DirectLinuxProcess.map_code(at as usize, bytes) {
            return Some((at, mapping));
        }
    }
    None
}

/// Why an instruction run alone was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleInstructionRefusal {
    /// The state's exclusive monitor holds an address, which no run of form I sets.
    ExclusiveMonitorHeld(ExclusiveMonitor),
    /// The pool gave no memory, or the host refused this thread a run.
    Host,
}

impl BehaviorSource for InProcess {
    type Architecture = Arm64<User>;
    type Error = SingleInstructionRefusal;

    /// Runs the instruction alone in a memory made of the deltas until its first landing; the pages it may write are
    /// compared before and after.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), SingleInstructionRefusal> {
        if processor.exclusive_monitor != ExclusiveMonitor::Open {
            return Err(SingleInstructionRefusal::ExclusiveMonitorHeld(
                processor.exclusive_monitor,
            ));
        }
        let (space, mut table) = self
            .pool
            .create_memory()
            .map_err(|_| SingleInstructionRefusal::Host)?;
        let mut writable = Vec::new();
        for delta in memory.mutations {
            match delta {
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => {
                    table
                        .set_permissions(addresses.clone(), to_permissions(*permission))
                        .map_err(|_| SingleInstructionRefusal::Host)?;
                    let pages = addresses.clone().step_by(GUEST_PAGE as usize);
                    writable.retain(|page| !addresses.contains(page));
                    if to_permissions(*permission).contains(Permissions::WRITE) {
                        writable.extend(pages);
                    }
                }
                MemoryDelta::ValueAssignment { start, bytes } => {
                    let into = space
                        .borrow(*start..*start + bytes.len() as u64)
                        .map_err(|_| SingleInstructionRefusal::Host)?;
                    into.write(bytes);
                }
            }
        }
        let read_page = |page: u64| {
            let mut bytes = vec![0; GUEST_PAGE as usize];
            if let Ok(from) = space.borrow(page..page + GUEST_PAGE) {
                from.read(&mut bytes);
            }
            bytes
        };
        let before: Vec<Vec<u8>> = writable.iter().map(|page| read_page(*page)).collect();
        let mut thread = InProcessThread::new().map_err(|_| SingleInstructionRefusal::Host)?;
        let mut run = Run::new(self, &space, processor, &mut thread);
        let trap = run.until_trap(&Interruption::default(), true);
        let pc = processor.program_counter;
        let exception = match trap {
            Trap::Landed | Trap::Interrupt => None,
            Trap::SystemCall(_) => Some(system_call(word_at(&space, pc))),
            Trap::MemoryFault { address, access } => Some(abort(&space, address, access)),
            Trap::Ended(exception) => Some(exception),
        };
        *processor = run.processor(exception);
        for (page, before) in writable.iter().zip(before) {
            report_changed_bytes(*page, &before, &read_page(*page), mutation_hook);
        }
        Ok(())
    }
}

impl Executor for InProcess {
    type TranslatorConfig = ();
    type GuestMemory = InProcessMemory;
    type Interrupter = InProcessInterrupter;
    type Execution<'execution> = InProcessExecution<'execution>;
    type CreateGuestMemoryError = CreateMemoryError;

    fn create_guest_memory(
        &self,
    ) -> Result<(InProcessMemory, InProcessPageTable), CreateMemoryError> {
        self.pool.create_memory()
    }

    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: Processor<()>,
        memory: &'execution InProcessMemory,
    ) -> (InProcessExecution<'execution>, InProcessInterrupter) {
        let interruption = Arc::new(Interruption::default());
        (
            InProcessExecution {
                executor: self,
                memory,
                processor,
                interruption: Arc::clone(&interruption),
            },
            InProcessInterrupter { interruption },
        )
    }
}

/// One execution of form I, run on the thread that calls [`Execution::run`].
pub struct InProcessExecution<'execution> {
    executor: &'execution mut InProcess,
    memory: &'execution InProcessMemory,
    processor: Processor<()>,
    interruption: Arc<Interruption>,
}

impl Execution for InProcessExecution<'_> {
    type ProcessorState = Processor<()>;
    /// Never made: an execution waits for nothing, and runs to its end.
    type Wait = core::future::Pending<()>;

    /// Runs until the next trap, then asks the handler, until an answer terminates the execution.
    fn run(self, handler: &mut impl TrapHandler) -> ExecutionStop<Processor<()>, Self::Wait> {
        let Self {
            executor,
            memory,
            mut processor,
            interruption,
        } = self;
        let Ok(mut thread) = InProcessThread::new() else {
            processor.exception = Some(host_refused());
            return ExecutionStop::Terminated(processor);
        };
        *interruption.target() = Some(rustix::thread::gettid());
        let mut run = Run::new(executor, memory, &processor, &mut thread);
        let exception = loop {
            let trap = run.until_trap(&interruption, false);
            match ask_handler(handler, trap, &mut run.state) {
                Ok(()) => {}
                Err(Ended::SystemCall) => {
                    break Some(system_call(word_at(memory, run.state.pc - INSTRUCTION_SIZE)));
                }
                Err(Ended::MemoryFault { address, access }) => {
                    break Some(abort(memory, address, access));
                }
                Err(Ended::Interrupt) => break None,
                Err(Ended::Exception(exception)) => break Some(exception),
            }
        };
        *interruption.target() = None;
        let processor = run.processor(exception);
        drop(thread);
        ExecutionStop::Terminated(processor)
    }
}

/// Where a run stopped for the handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trap {
    SystemCall(SystemCallRequest),
    /// A fault at the address as the guest made it, its top byte kept.
    MemoryFault { address: u64, access: Access },
    /// An interrupt asked for, before the guest entered or while it ran.
    Interrupt,
    /// The first landing of a run alone: its one instruction is done.
    Landed,
    /// An exception no handler is given, which ends the execution.
    Ended(Exception),
}

/// How an execution ended, the handler having answered terminate.
enum Ended {
    /// At a system call: the state is after it.
    SystemCall,
    MemoryFault { address: u64, access: Access },
    Interrupt,
    Exception(Exception),
}

/// The handler's answer to a trap: the state goes on (a system call's result in x0, a fault's instruction again), or
/// the execution ends.
fn ask_handler(
    handler: &mut impl TrapHandler,
    trap: Trap,
    state: &mut GuestState,
) -> Result<(), Ended> {
    match trap {
        Trap::SystemCall(request) => match handler.handle_system_call(request) {
            SystemCallResponse::Return(value) => {
                state.x[0] = value;
                Ok(())
            }
            SystemCallResponse::Terminate => Err(Ended::SystemCall),
        },
        // The handler is given the address its pages are found at; the state the execution ends with keeps the tag.
        Trap::MemoryFault { address, access } => {
            match handler.handle_memory_fault(address & !TAG, access) {
                MemoryFaultResponse::Retry => Ok(()),
                MemoryFaultResponse::Terminate => Err(Ended::MemoryFault { address, access }),
            }
        }
        Trap::Interrupt => match handler.handle_interrupt() {
            InterruptResponse::Continue => Ok(()),
            InterruptResponse::Terminate => Err(Ended::Interrupt),
        },
        Trap::Landed => Ok(()),
        Trap::Ended(exception) => Err(Ended::Exception(exception)),
    }
}

/// One execution's guest thread as it runs: its state between pieces and the host thread that runs them.
struct Run<'run> {
    executor: &'run mut InProcess,
    memory: &'run InProcessMemory,
    thread: &'run mut InProcessThread,
    state: GuestState,
    page: RegisterPage,
    float: FloatStatus,
    /// The next piece runs displaced: its instruction writes a stack pointer outside the window.
    displace: bool,
}

impl<'run> Run<'run> {
    fn new(
        executor: &'run mut InProcess,
        memory: &'run InProcessMemory,
        processor: &Processor<()>,
        thread: &'run mut InProcessThread,
    ) -> Self {
        thread.set_vectors(processor.vector_registers);
        thread.set_thread_pointer(processor.software_thread_id);
        Self {
            executor,
            memory,
            thread,
            state: GuestState {
                x: processor.general_purpose_registers,
                sp: processor.stack_pointer,
                pc: processor.program_counter,
                flags: u64::from(processor.condition_flags.bits()),
                count: 0,
                exclusive: [0; 4],
            },
            page: RegisterPage::default(),
            float: FloatStatus {
                fpsr: processor.floating_point_status as u32,
                fpcr: processor.floating_point_control as u32,
                tracking: false,
            },
            displace: false,
        }
    }

    /// The processor the run leaves, with the exception it ended at; its exclusive monitor open.
    fn processor(&self, exception: Option<Exception>) -> Processor<()> {
        Processor {
            general_purpose_registers: self.state.x,
            stack_pointer: self.state.sp,
            program_counter: self.state.pc,
            condition_flags: ConditionFlags::from_bits_truncate(self.state.flags as u32),
            vector_registers: self.thread.vectors(),
            floating_point_control: u64::from(self.float.fpcr),
            floating_point_status: u64::from(self.float.fpsr),
            software_thread_id: self.thread.thread_pointer(),
            exception,
            ..ZERO_PROCESSOR
        }
    }

    /// The way the next piece runs in.
    fn way(&self) -> Way {
        Way::InProcess {
            slot: self.memory.slot(),
            displaced: self.displace || in_process::outside(self.state.sp),
            kept: Kept::USUAL,
        }
    }

    /// Runs piece after piece from the state until a trap; `alone` ends the run at its first landing too.
    fn until_trap(&mut self, interruption: &Interruption, alone: bool) -> Trap {
        loop {
            if interruption.requested.swap(false, Ordering::AcqRel) {
                return Trap::Interrupt;
            }
            let pc = self.state.pc;
            if !pc.is_multiple_of(INSTRUCTION_SIZE) {
                return Trap::Ended(Exception::ProgramCounterAlignmentFault { address: pc });
            }
            if !self.memory.allowed(pc).contains(Permissions::EXECUTE) {
                return Trap::MemoryFault {
                    address: pc,
                    access: Access::Execute,
                };
            }
            let word = word_at(self.memory, pc);
            let way = self.way();
            self.displace = false;
            let Some(piece) = self.executor.piece(way, pc & !TAG, word) else {
                return Trap::Ended(Exception::UndefinedInstruction);
            };
            self.thread
                .enter(way, &self.state, self.page.clone(), &self.float, piece.at);
            match self.run_piece(way, &piece, interruption) {
                Some(Trap::Landed) if !alone => {}
                Some(trap) => return trap,
                None => {}
            }
        }
    }

    /// Runs the entered piece until the guest leaves it: none where it goes on at its state's pc, else the trap.
    fn run_piece(
        &mut self,
        way: Way,
        loaded: &Loaded,
        interruption: &Interruption,
    ) -> Option<Trap> {
        let piece = Piece {
            base: loaded.at,
            length: loaded.code.bytes.len(),
            spans: &loaded.code.spans,
        };
        loop {
            let taken = match self.thread.run() {
                Exit::Call(code) => Taken::Call(code),
                Exit::Taken(taken) => taken,
                // A stop that came after the interrupt it was sent for was taken: the run goes on as it was entered.
                Exit::NotEntered if !interruption.requested.load(Ordering::Acquire) => continue,
                Exit::NotEntered | Exit::Stopped => {
                    let asked = interruption.requested.swap(false, Ordering::AcqRel);
                    let memory = self.memory;
                    let state = in_process::interrupted(
                        way,
                        &piece,
                        &self.thread.hardware(),
                        &self.thread.page(),
                        &self.thread.after(),
                        &mut self.float,
                        &mut |at, into| read_into(memory, at, into),
                    );
                    let Some(state) = state else {
                        return Some(Trap::Ended(host_refused()));
                    };
                    self.state = state;
                    self.page = self.thread.page();
                    return asked.then_some(Trap::Interrupt);
                }
                Exit::Elsewhere => return Some(Trap::Ended(host_refused())),
            };
            let memory = self.memory;
            let resolution = in_process::resolve(
                way,
                &piece,
                taken,
                &self.thread.hardware(),
                &self.thread.page(),
                &self.thread.after(),
                &mut self.float,
                &mut |at, into| read_into(memory, at, into),
            );
            let page = self.thread.page();
            match resolution {
                None => return Some(Trap::Ended(host_refused())),
                Some(Resolution::Outside { address, state }) => {
                    let access = access_of(word_at(self.memory, state.pc), false);
                    self.state = state;
                    self.page = page;
                    return Some(Trap::MemoryFault { address, access });
                }
                Some(Resolution::Displaced(state)) => {
                    self.state = state;
                    self.page = page;
                    self.displace = true;
                    return None;
                }
                Some(Resolution::Probed(state)) => {
                    self.state = state;
                    self.page = page;
                    return None;
                }
                Some(Resolution::Resolved(resolved)) => match resolved {
                    Resolved::Again(then, then_page)
                    | Resolved::FloatStatusNotKept(then, then_page)
                    | Resolved::LastTarget {
                        then,
                        page: then_page,
                        ..
                    } => self.thread.resume(&then, &then_page),
                    Resolved::Clock { at, .. } => {
                        let word = word_at(self.memory, at.pc);
                        self.state = *at;
                        self.page = page;
                        return Some(Trap::Ended(counter_read(word)));
                    }
                    Resolved::CodeChanged { address, at, .. } => {
                        self.state = *at;
                        self.page = page;
                        let readable = self
                            .memory
                            .allowed(address)
                            .intersects(Permissions::READ | Permissions::WRITE);
                        if !readable {
                            return Some(Trap::MemoryFault {
                                address,
                                access: Access::Read,
                            });
                        }
                        // The announcement changes nothing the guest sees, and every piece is found by its own
                        // instruction: the guest goes on after it.
                        self.state.pc += INSTRUCTION_SIZE;
                        return None;
                    }
                    Resolved::Stop(stop, state) => {
                        self.state = state;
                        self.page = page;
                        return self.stopped(stop);
                    }
                },
            }
        }
    }

    /// The trap a stop resolved to comes to; none where the guest goes on at its state's pc.
    fn stopped(&mut self, stop: Stop) -> Option<Trap> {
        let pc = self.state.pc;
        Some(match stop {
            Stop::Landed(_) => Trap::Landed,
            Stop::Syscall => {
                let mut request = SystemCallRequest {
                    number: SystemCallNumber(self.state.x[SYSCALL_NUMBER]),
                    arguments: Default::default(),
                };
                let count = request.arguments.len();
                request.arguments.copy_from_slice(&self.state.x[..count]);
                Trap::SystemCall(request)
            }
            Stop::Breakpoint(comment) => Trap::Ended(Exception::Breakpoint { comment }),
            Stop::Illegal => Trap::Ended(illegal(word_at(self.memory, pc))),
            Stop::Alignment(address) if address == pc => {
                Trap::Ended(Exception::ProgramCounterAlignmentFault { address })
            }
            Stop::Alignment(_) => Trap::Ended(Exception::StackPointerAlignmentFault),
            Stop::MemoryFault { address, write } => Trap::MemoryFault {
                address,
                access: access_of(word_at(self.memory, pc), write),
            },
        })
    }
}

/// Reads guest memory at `at` into `into`, zeros where it cannot.
fn read_into(memory: &InProcessMemory, at: u64, into: &mut [u8]) {
    into.fill(0);
    if let Ok(bytes) = memory.borrow(at..at.saturating_add(into.len() as u64)) {
        bytes.read(into);
    }
}

/// The instruction word at `pc`, zero where it cannot be read.
fn word_at(memory: &InProcessMemory, pc: u64) -> u32 {
    let mut word = [0; INSTRUCTION_SIZE as usize];
    read_into(memory, pc & !TAG, &mut word);
    u32::from_le_bytes(word)
}

/// What the faulting instruction `word` did, as Linux reports it: a write where the host said so or where it stores.
fn access_of(word: u32, write: bool) -> Access {
    let stores = matches!(
        decode(word),
        Instruction::Store { .. }
            | Instruction::StorePair { .. }
            | Instruction::StoreVector { .. }
            | Instruction::StoreVectorPair { .. }
            | Instruction::StoreExclusive { .. }
            | Instruction::StoreRelease { .. }
            | Instruction::SimdMemory { load: false, .. }
    );
    if write || stores {
        Access::Write
    } else {
        Access::Read
    }
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

/// The exception of the system call instruction `word`.
fn system_call(word: u32) -> Exception {
    Exception::SupervisorCall {
        immediate: IMM16.read(word) as u16,
    }
}

/// The abort of an access to `address` of the memory, whose page allows nothing unless the guest gave it permissions.
fn abort(memory: &InProcessMemory, address: u64, access: Access) -> Exception {
    let untagged = address & !TAG;
    let status = if untagged >= Arm64::<User>::ADDRESSABLE.end {
        FaultStatus::TranslationFaultLevel0
    } else if !memory.allowed(untagged).is_empty() {
        FaultStatus::PermissionFaultLevel3
    } else {
        FaultStatus::TranslationFaultLevel3
    };
    match access {
        Access::Execute => Exception::InstructionAbort { status, address },
        Access::Read => Exception::DataAbort {
            status,
            access: AccessDirection::Read,
            address,
        },
        Access::Write => Exception::DataAbort {
            status,
            access: AccessDirection::Write,
            address,
        },
    }
}

/// The exception of a read of the counter `word`, which the chip refuses at EL0 and the system answers.
fn counter_read(word: u32) -> Exception {
    trapped(word).unwrap_or(Exception::UndefinedInstruction)
}

/// The exception of an instruction the translation refused: the trap of a system register move the chip traps at EL0,
/// else an undefined instruction.
fn illegal(word: u32) -> Exception {
    trapped(word).unwrap_or(Exception::UndefinedInstruction)
}

/// The trap of the system register move `word` where the chip traps it at EL0, as its instruction list says.
fn trapped(word: u32) -> Option<Exception> {
    let direction = match decode(word) {
        Instruction::ReadSystem { .. } => AccessDirection::Read,
        Instruction::WriteSystem { .. } => AccessDirection::Write,
        _ => return None,
    };
    let mut processor = ZERO_PROCESSOR;
    match simulate_system_register_move(
        &mut processor,
        word,
        direction,
        NEOVERSE_V2_SYSTEM_REGISTERS,
    ) {
        Err(trapped @ Exception::SystemAccessTrapped(_)) => Some(trapped),
        Ok(()) | Err(_) => None,
    }
}

/// The exception an execution ends with where the host refused it a run or its code reported what no piece does: an
/// SError, the interface having no other way to say it.
fn host_refused() -> Exception {
    Exception::Undecoded {
        syndrome: aarch64_cpu::registers::ESR_EL1::EC::SError.value,
        fault_address: u64::MIN,
    }
}

/// What an execution's interrupter and its run share.
#[derive(Debug, Default)]
struct Interruption {
    /// An interrupt asked for and not yet given to the handler.
    requested: AtomicBool,
    /// The host thread the run is on, while it runs.
    target: Mutex<Option<Pid>>,
}

impl Interruption {
    fn target(&self) -> MutexGuard<'_, Option<Pid>> {
        self.target.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Interrupts one execution of form I: a request its run takes before its next entry, and the stop signal to its
/// thread, which ends a run of its code.
#[derive(Debug)]
pub struct InProcessInterrupter {
    interruption: Arc<Interruption>,
}

impl ExecutionInterrupter for InProcessInterrupter {
    fn interrupt(&self) {
        self.interruption.requested.store(true, Ordering::Release);
        if let Some(thread) = *self.interruption.target() {
            let _ = DirectLinuxProcess.tgkill(rustix::process::getpid(), thread, RUN_STOP_SIGNAL);
        }
    }
}
