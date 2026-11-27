//! One thread's store on one space: its guest memory, its thread instance, its lookup table and its hooks.
//!
//! The hooks the translated code calls ask the trap handler of the run in progress from inside the run.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use wasmtime::{
    AsContextMut, Caller, Extern, Func, FuncType, Global, GlobalType, Instance, Memory, MemoryType,
    Module, Mutability, Ref, Store, StoreLimits, StoreLimitsBuilder, Table, TypedFunc, Val,
    ValType,
};
use wcontainers_abi::{
    Access, PAGE_SIZE, Permissions, RegisterBlock, SystemCallNumber, SystemCallRequest,
};
use wcontainers_arm64::thread_state::untag_address;
use wcontainers_arm64::{
    Arm64, EXCLUSIVE_ADDRESS, EXCLUSIVE_BYTES, FLOAT_TRACKING, INSTRUCTION_BYTES, Reg,
};
use wcontainers_executor::{
    InterruptResponse, MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_guest_cpu::{Guest, HookBody, Personality, TrapKind};
use wcontainers_processor_architecture_arm64::encoding::{Field, IMM16, RT};
use wcontainers_processor_architecture_arm64::system_instruction::to_cache_maintenance_syndrome;
use wcontainers_processor_architecture_arm64::system_register::{
    SYSTEM_REGISTER, SystemRegisterEncoding,
};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, ConditionFlags, Exception, ExclusiveMonitor, FaultStatus, Processor,
    SystemAccess, VIRTUAL_ADDRESS_BITS,
};
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::{self, progress_limit_offset, thread_state_size};
use wcontainers_wasmtime::Wasmtime;
use zerocopy::{FromBytes, IntoBytes};

use crate::space::{GUEST_WASM_PAGES, PERMISSIONS_AT, SLOTS_AT, SLOTS_COVER, Space};

/// Tuning number: the pages of code before a run-time start's page that its translation sees, to reach its loops.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const PAGES_BEHIND: u64 = 1;
/// Tuning number: the pages of code from a run-time start's page on that its translation sees.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const PAGES_AHEAD: u64 = 4;
/// Tuning number: room for the arguments of the guest's own hooks, which a test checks every hook fits.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const CPU_HOOK_ARGUMENTS: usize = 8;
/// The import that gives a code module the guest's own hook at an index of `Arm64::HOOKS`.
type CpuHookIndex = usize;
/// The bit of an MRS or MSR word that is set in an MRS, [L](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MRS--Move-system-register-to-general-purpose-register-).
const SYSTEM_MOVE_READ_BIT: u32 = 21;
/// The op0 field of a system instruction's word: 2 or 3 in an MRS or MSR (register), 0 in an MSR (immediate).
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MRS--Move-system-register-to-general-purpose-register->
const SYSTEM_OP0: Field = Field::new(19, 2);

/// How a thread runs.
#[derive(Clone, Copy)]
pub(crate) enum Mode {
    /// Until the handler ends it, at the look-ahead given for code reached at run time.
    Execute(LookAhead),
    /// Exactly the one instruction at `start`, whose piece is loaded once.
    ///
    /// The thread stops at the second stop check or wherever control goes after the instruction.
    Step {
        start: u64,
        stops: u32,
        loaded: bool,
    },
}

/// Why a run ended.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Ending {
    /// The instruction or a trap raised the exception, or the handler ended the run at it.
    Raised(Exception),
    /// The handler ended the run at an interrupt.
    Interrupted,
    /// The one instruction of a step completed.
    Stepped,
}

/// The interrupter's side of one execution: whether an interrupt waits, and the stop flag to raise while it runs.
#[derive(Default)]
pub(crate) struct InterruptState {
    pub(crate) raised: AtomicBool,
    /// The space and block of the execution, until it ends.
    pub(crate) target: Mutex<Option<(Arc<Space>, u32)>>,
}

/// What a thread's store holds for its hooks.
pub(crate) struct ThreadData {
    /// The space, while a run is prepared or in progress; the store otherwise holds it weakly.
    pub(crate) space: Option<Arc<Space>>,
    pub(crate) block: u32,
    memory: Option<Memory>,
    hooks: Vec<(&'static str, Func)>,
    thread: Option<Instance>,
    table: Option<Table>,
    /// How many of the space's loads this store has instantiated.
    synced: usize,
    /// The space's count of narrowings when this thread last entered guest code.
    narrowed_seen: u64,
    /// By address and instruction word, the piece of that one instruction for the exact slot.
    exact: HashMap<(u64, u32), Func>,
    pub(crate) mode: Mode,
    pub(crate) ending: Option<Ending>,
    /// The thread left to run again at once from its registers, without the pieces it was in.
    rerun: bool,
    /// Whether the thread is counted in guest code.
    entered: bool,
    pub(crate) interrupt: Arc<InterruptState>,
    /// The progress events a run goes on for before it asks the handler as at an interrupt; none for no bound.
    slice: Option<NonZeroU64>,
    /// The progress count at which the slice in progress ends: the thread's limit while no stop is raised.
    slice_end: u64,
    limits: StoreLimits,
}

/// A thread's store on one space, and its thread module's run function.
pub(crate) struct ThreadStore {
    pub(crate) store: Store<ThreadData>,
    run: TypedFunc<(), ()>,
    pub(crate) space: Weak<Space>,
}

impl Drop for ThreadStore {
    fn drop(&mut self) {
        if let Some(space) = self.space.upgrade() {
            space.free_block(self.store.data().block);
        }
    }
}

fn space_of(data: &ThreadData) -> Arc<Space> {
    data.space.clone().expect("a thread's space while it runs")
}

impl ThreadStore {
    /// Makes a thread on the space: a block, a store whose guest memory is the space's, and its thread instance.
    pub(crate) fn new(space: &Arc<Space>) -> ThreadStore {
        let block = space
            .allocate_block()
            .expect("a thread block left in the space's state memory");
        let shared = space.shared.clone();
        let data = ThreadData {
            space: Some(space.clone()),
            block,
            memory: None,
            hooks: Vec::new(),
            thread: None,
            table: None,
            synced: usize::MIN,
            narrowed_seen: u64::MIN,
            exact: HashMap::new(),
            mode: Mode::Execute(LookAhead::AT_RUN_TIME),
            ending: None,
            rerun: false,
            entered: false,
            interrupt: Arc::default(),
            slice: None,
            slice_end: u64::MAX,
            limits: StoreLimitsBuilder::new()
                .instances(usize::MAX)
                .tables(usize::MAX)
                .memories(usize::MAX)
                .table_elements(usize::MAX)
                .build(),
        };
        let mut store = Store::new(&shared.engine, data);
        store.limiter(|data| &mut data.limits);
        let ty = MemoryType::new64(GUEST_WASM_PAGES, Some(GUEST_WASM_PAGES));
        let memory = shared
            .wasmtime
            .new_memory(&mut store, ty, &space.host)
            .expect("a store's guest memory over its space's host memory");
        let hooks = make_hooks(&mut store);
        let data = store.data_mut();
        data.memory = Some(memory);
        data.hooks = hooks;
        let thread = instantiate(&mut store, &shared.thread_module, u64::MIN, u32::MIN);
        let Some(Extern::Table(table)) = thread.get_export(&mut store, contract::LOOKUP) else {
            panic!("the thread module exports its lookup table");
        };
        let run = thread
            .get_typed_func::<(), ()>(&mut store, contract::RUN)
            .expect("the thread module exports its run function");
        let data = store.data_mut();
        data.thread = Some(thread);
        data.table = Some(table);
        data.space = None;
        ThreadStore {
            store,
            run,
            space: Arc::downgrade(space),
        }
    }

    /// Readies the thread to run from the processor in the mode, with its interrupts and slices as given.
    ///
    /// Its interrupts go to the state given, and the run is bounded by slices of the progress events given.
    pub(crate) fn prepare(
        &mut self,
        space: &Arc<Space>,
        processor: &Processor<()>,
        (mode, slice): (Mode, Option<NonZeroU64>),
        interrupt: Arc<InterruptState>,
    ) {
        let data = self.store.data_mut();
        data.space = Some(space.clone());
        data.mode = mode;
        data.ending = None;
        data.rerun = false;
        data.interrupt = interrupt;
        data.slice = slice;
        data.slice_end = slice.map_or(u64::MAX, NonZeroU64::get);
        let block = data.block;
        space.state.write(
            usize::try_from(block).expect("a block offset"),
            &thread_state(processor, data.slice_end),
        );
    }

    /// Runs the thread until it ends, running it again from its registers whenever it leaves to, and returns why.
    pub(crate) fn run_to_end(&mut self) -> Ending {
        // A piece that only loops checks the stop flag at its jump back and not at its entry, so an interrupt raised
        // before the run is answered here, before any guest code runs.
        let data = self.store.data();
        if matches!(data.mode, Mode::Execute(_))
            && data.interrupt.raised.swap(false, Ordering::SeqCst)
        {
            let answer = space_of(data)
                .shared
                .wasmtime
                .with_trap_handler(|handler| handler.handle_interrupt())
                .expect("the trap handler of the run in progress");
            if matches!(answer, InterruptResponse::Terminate) {
                return Ending::Interrupted;
            }
        }
        loop {
            self.store.data_mut().rerun = false;
            enter_guest(&mut self.store);
            let result = self.run.call(&mut self.store, ());
            leave_guest(self.store.data_mut());
            if let Err(error) = result {
                panic!("wasmtime stopped the guest outside every hook: {error:?}");
            }
            let data = self.store.data_mut();
            if let Some(ending) = data.ending.take() {
                return ending;
            }
            assert!(
                data.rerun,
                "the thread left with no ending and no reason to run again"
            );
        }
    }

    /// Returns the processor the thread holds, from the start state for what the translated code does not keep.
    pub(crate) fn processor(
        &self,
        start: &Processor<()>,
        exception: Option<Exception>,
    ) -> Processor<()> {
        let data = self.store.data();
        let space = space_of(data);
        let mut state = vec![u8::MIN; thread_state_size::<Arm64>()];
        space.state.read(
            usize::try_from(data.block).expect("a block offset"),
            &mut state,
        );
        let mut processor = to_processor(&state, start, exception);
        // The translated code keeps the place in the guest memory of the address an exclusive load reached.
        if let ExclusiveMonitor::Exclusive { address } = &mut processor.exclusive_monitor {
            *address = space.address_of_place(*address);
        }
        processor
    }

    /// Lets go of the space and of the interrupter after a run.
    pub(crate) fn finish(&mut self) {
        let data = self.store.data_mut();
        *data
            .interrupt
            .target
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        data.space = None;
    }
}

/// Returns the thread's state bytes for the processor: its registers, the progress limit and the start of a run.
///
/// The progress count is zero, the float status is kept exact from the start and the exclusive monitor is open.
fn thread_state(processor: &Processor<()>, limit: u64) -> Vec<u8> {
    let block = RegisterBlock {
        x: processor.general_purpose_registers,
        sp: processor.stack_pointer,
        pc: processor.program_counter,
        pstate: u64::from(processor.condition_flags.bits()),
        v: processor
            .vector_registers
            .map(|value| [value as u64, (value >> u64::BITS) as u64]),
        fpsr: u32::try_from(processor.floating_point_status & u64::from(u32::MAX))
            .expect("the low half of FPSR"),
        fpcr: u32::try_from(processor.floating_point_control & u64::from(u32::MAX))
            .expect("the low half of FPCR"),
        tpidr_el0: processor.software_thread_id,
    };
    let mut state = vec![u8::MIN; thread_state_size::<Arm64>()];
    state[..size_of::<RegisterBlock>()].copy_from_slice(block.as_bytes());
    state[FLOAT_TRACKING..FLOAT_TRACKING + size_of::<u32>()]
        .copy_from_slice(&u32::from(true).to_le_bytes());
    let at = progress_limit_offset::<Arm64>();
    state[at..at + size_of::<u64>()].copy_from_slice(&limit.to_le_bytes());
    state
}

/// Returns the processor of the thread's state bytes, with the exception and what the code does not change.
///
/// The start gives the SVE predicates and PSTATE controls, which the translated code does not change.
fn to_processor(
    state: &[u8],
    start: &Processor<()>,
    exception: Option<Exception>,
) -> Processor<()> {
    let block = RegisterBlock::read_from_bytes(&state[..size_of::<RegisterBlock>()])
        .expect("a register block at the start of a thread's state");
    let word = |at: usize| {
        u64::from_le_bytes(
            state[at..at + size_of::<u64>()]
                .try_into()
                .expect("a word of the thread's state"),
        )
    };
    let exclusive_bytes = u32::from_le_bytes(
        state[EXCLUSIVE_BYTES..EXCLUSIVE_BYTES + size_of::<u32>()]
            .try_into()
            .expect("a word of the thread's state"),
    );
    Processor {
        general_purpose_registers: block.x,
        stack_pointer: block.sp,
        program_counter: block.pc,
        condition_flags: ConditionFlags::from_bits_truncate(
            u32::try_from(block.pstate & u64::from(u32::MAX)).expect("the low half of PSTATE"),
        ),
        vector_registers: block
            .v
            .map(|[low, high]| u128::from(low) | u128::from(high) << u64::BITS),
        predicate_registers: start.predicate_registers,
        first_fault_register: start.first_fault_register,
        process_state_controls: start.process_state_controls,
        floating_point_control: u64::from(block.fpcr),
        floating_point_status: u64::from(block.fpsr),
        software_thread_id: block.tpidr_el0,
        exclusive_monitor: if exclusive_bytes == 0 {
            ExclusiveMonitor::Open
        } else {
            ExclusiveMonitor::Exclusive {
                address: word(EXCLUSIVE_ADDRESS),
            }
        },
        exception,
        level_specific: (),
    }
}

/// Counts the thread in guest code, instantiates the loads other threads added, and notes the narrowings so far.
fn enter_guest(store: &mut impl AsContextMut<Data = ThreadData>) {
    let mut store = store.as_context_mut();
    let space = space_of(store.data());
    space.enter();
    let data = store.data_mut();
    data.entered = true;
    data.narrowed_seen = space.narrowed.load(Ordering::SeqCst);
    sync(&mut store);
}

/// Counts the thread out of guest code, if it was in.
fn leave_guest(data: &mut ThreadData) {
    if std::mem::take(&mut data.entered) {
        space_of(data).leave();
    }
}

/// Goes back into guest code after a hook, or stays out and returns true when a narrowing makes it run again.
///
/// A narrowing since the thread last entered means it must leave and run again from its registers.
fn resume(caller: &mut Caller<'_, ThreadData>) -> bool {
    let seen = caller.data().narrowed_seen;
    enter_guest(caller);
    if caller.data().narrowed_seen == seen {
        return false;
    }
    let data = caller.data_mut();
    data.rerun = true;
    leave_guest(data);
    true
}

/// Resumes from a hook that returns no unwind payload, as [`resume`] does.
///
/// The thread raises its own stop flag to leave at its next stop check, and stays counted in guest code until then.
fn resume_in_place(caller: &mut Caller<'_, ThreadData>) {
    let seen = caller.data().narrowed_seen;
    enter_guest(caller);
    if caller.data().narrowed_seen != seen {
        caller.data_mut().rerun = true;
        let data = caller.data();
        space_of(data).raise_stop_flag(data.block);
    }
}

/// Instantiates every load of the space this store does not hold yet, but those dropped.
fn sync(store: &mut impl AsContextMut<Data = ThreadData>) {
    let mut store = store.as_context_mut();
    let space = space_of(store.data());
    let synced = store.data().synced;
    if space.load_count.load(Ordering::SeqCst) == synced {
        return;
    }
    let loads: Vec<_> = space
        .code
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .loads[synced..]
        .to_vec();
    for load in &loads {
        if !load.dropped.load(Ordering::SeqCst) {
            let mut slot = load.slot_base;
            for (module, offsets) in &load.translated.modules {
                let pieces = u32::try_from(offsets.len()).expect("a count of pieces");
                grow_table(&mut store, slot + pieces);
                instantiate(&mut store, module, load.load_address, slot);
                slot += pieces;
            }
        }
    }
    store.data_mut().synced = synced + loads.len();
}

/// Grows the thread's lookup table to hold the slots.
fn grow_table(store: &mut impl AsContextMut<Data = ThreadData>, slots: u32) {
    let mut store = store.as_context_mut();
    let table = store.data().table.expect("a thread's lookup table");
    let more = u64::from(slots).saturating_sub(table.size(&store));
    if more > 0 {
        table
            .grow(&mut store, more, Ref::Func(None))
            .expect("a lookup table that grows");
    }
}

/// Instantiates a module of the translator with its imports from the space, the thread and the thread module.
///
/// The imports are the space's memories, the thread's constants and hooks, and the thread module's exports.
fn instantiate(
    store: &mut impl AsContextMut<Data = ThreadData>,
    module: &Module,
    load_address: u64,
    slot_base: u32,
) -> Instance {
    let mut store = store.as_context_mut();
    let space = space_of(store.data());
    let mut imports = Vec::new();
    for import in module.imports() {
        let constant = |value: Val| -> Option<Val> { Some(value) };
        let item: Extern = match (import.module(), import.name()) {
            (contract::ENV, contract::MEMORY) => {
                store.data().memory.expect("a thread's guest memory").into()
            }
            (contract::ENV, contract::STATE) => space.state.shared_memory().into(),
            (contract::ENV, name) => {
                let block = store.data().block;
                let value = match name {
                    contract::LOAD_ADDRESS => constant(Val::I64(load_address.cast_signed())),
                    contract::SLOT_BASE => constant(Val::I32(slot_base.cast_signed())),
                    contract::TABLE_BASE => constant(Val::I64(i64::default())),
                    contract::SLOTS => constant(Val::I32(
                        i32::try_from(SLOTS_AT).expect("an offset in the state memory"),
                    )),
                    contract::SLOTS_COVER => constant(Val::I64(SLOTS_COVER.cast_signed())),
                    contract::THREAD_BLOCK => constant(Val::I32(block.cast_signed())),
                    contract::PERMISSIONS => constant(Val::I32(
                        i32::try_from(PERMISSIONS_AT).expect("an offset in the state memory"),
                    )),
                    contract::PERMISSION_PAGES => {
                        constant(Val::I64(crate::space::GUEST_PAGES.cast_signed()))
                    }
                    _ => None,
                };
                let value = value.unwrap_or_else(|| panic!("no value for the import {name}"));
                let ty = match value {
                    Val::I32(_) => ValType::I32,
                    _ => ValType::I64,
                };
                Global::new(&mut store, GlobalType::new(ty, Mutability::Const), value)
                    .expect("a constant global")
                    .into()
            }
            (contract::HOOKS, name) => store
                .data()
                .hooks
                .iter()
                .find(|(hook, _)| *hook == name)
                .unwrap_or_else(|| panic!("no hook {name}"))
                .1
                .into(),
            (contract::THREAD, name) => {
                let thread = store
                    .data()
                    .thread
                    .expect("a code module imports from its thread");
                thread
                    .get_export(&mut store, name)
                    .unwrap_or_else(|| panic!("the thread module exports {name}"))
            }
            (module, name) => panic!("an import the contract does not name: {module}.{name}"),
        };
        imports.push(item);
    }
    Instance::new(&mut store, module, &imports).expect("an instance of a translated module")
}

/// Returns the thread's program counter as its block holds it.
fn program_counter(data: &ThreadData) -> u64 {
    let mut pc = [u8::MIN; size_of::<u64>()];
    let at = usize::try_from(data.block).expect("a block offset") + Arm64::PC_OFFSET;
    space_of(data).state.read(at, &mut pc);
    u64::from_le_bytes(pc)
}

/// Returns the fault status of an access to the address: of translation where nothing is mapped, else of permission.
///
/// Nothing is mapped outside the 48 bits of addresses and on a page never given permissions.
fn fault_status(space: &Space, address: u64) -> FaultStatus {
    if address >> VIRTUAL_ADDRESS_BITS != 0 {
        FaultStatus::TranslationFaultLevel0
    } else if space.is_mapped(address / PAGE_SIZE) {
        FaultStatus::PermissionFaultLevel3
    } else {
        FaultStatus::TranslationFaultLevel3
    }
}

/// Asks the run's handler; the hooks run only inside a run that lent one.
fn ask<Returned>(
    caller: &Caller<'_, ThreadData>,
    answer: impl FnOnce(&mut dyn TrapHandler) -> Returned,
) -> Returned {
    space_of(caller.data())
        .shared
        .wasmtime
        .with_trap_handler(answer)
        .expect("the trap handler of the run in progress")
}

/// Answers the system call hook: the handler's answer goes to x0, or the run ends at the call.
fn answer_system_call(caller: &mut Caller<'_, ThreadData>, request: SystemCallRequest) -> i64 {
    leave_guest(caller.data_mut());
    let first = request.arguments[0].cast_signed();
    let returned = match caller.data().mode {
        Mode::Step { .. } => None,
        Mode::Execute(_) => match ask(caller, |handler| handler.handle_system_call(request)) {
            SystemCallResponse::Return(value) => Some(value),
            SystemCallResponse::Terminate => None,
        },
    };
    match returned {
        Some(value) => {
            resume_in_place(caller);
            value.cast_signed()
        }
        None => {
            // The block's program counter is after the call, whose immediate is in its word.
            let data = caller.data();
            let space = space_of(data);
            let call = program_counter(data).wrapping_sub(INSTRUCTION_BYTES as u64);
            let mut word = [u8::MIN; INSTRUCTION_BYTES];
            let immediate = if space.read(call, &mut word) {
                u16::try_from(IMM16.read(u32::from_le_bytes(word)))
                    .expect("an immediate of the field's width")
            } else {
                u16::MIN
            };
            space.raise_stop_flag(data.block);
            caller.data_mut().ending =
                Some(Ending::Raised(Exception::SupervisorCall { immediate }));
            enter_guest(caller);
            // x0 keeps what it held at the call.
            first
        }
    }
}

/// Answers the stop hook: a change, an interrupt, the end of a slice, or the end of a step or run stopped the thread.
fn stop(caller: &mut Caller<'_, ThreadData>) -> i32 {
    leave_guest(caller.data_mut());
    let data = caller.data();
    let space = space_of(data);
    let count = space.progress_count(data.block);
    // The slice ends once the count reaches its end; the next slice starts from where the thread stopped.
    let sliced = matches!(data.mode, Mode::Execute(_)) && count >= data.slice_end;
    if sliced && let Some(slice) = data.slice {
        caller.data_mut().slice_end = count.saturating_add(slice.get());
    }
    // The limit is set again before the interrupter's flag is read, so an interrupt raised meanwhile is answered now
    // or stops the thread again.
    let data = caller.data();
    space.set_progress_limit(data.block, data.slice_end);
    if data.ending.is_some() || data.rerun {
        return contract::UNWIND_LEAVE;
    }
    match caller.data().mode {
        Mode::Step {
            start,
            stops,
            loaded,
        } => {
            if stops == 0 {
                caller.data_mut().mode = Mode::Step {
                    start,
                    stops: stops + 1,
                    loaded,
                };
                let data = caller.data();
                space_of(data).raise_stop_flag(data.block);
                if resume(caller) {
                    return contract::UNWIND_LEAVE;
                }
                return contract::STOP_CARRY_ON;
            }
            caller.data_mut().ending = Some(Ending::Stepped);
            contract::UNWIND_LEAVE
        }
        Mode::Execute(_) => {
            let raised = caller.data().interrupt.raised.swap(false, Ordering::SeqCst);
            if (raised || sliced)
                && matches!(
                    ask(caller, |handler| handler.handle_interrupt()),
                    InterruptResponse::Terminate
                )
            {
                caller.data_mut().ending = Some(Ending::Interrupted);
                return contract::UNWIND_LEAVE;
            }
            if resume(caller) {
                contract::UNWIND_LEAVE
            } else {
                contract::STOP_CARRY_ON
            }
        }
    }
}

/// Answers the memory fault hook: a write to code drops the code and runs again; the handler takes any other fault.
fn memory_fault(caller: &mut Caller<'_, ThreadData>, computed: u64, write: bool) -> i32 {
    leave_guest(caller.data_mut());
    // The hook is given the address as the program computed it, tag and all; the pages and the handler see the address
    // the access reaches, and the exception keeps the tag, as FAR_EL1 does.
    let address = untag_address(computed);
    let data = caller.data();
    let space = space_of(data);
    let page = address / PAGE_SIZE;
    if write && space.holds_code(page) && space.permissions(page).contains(Permissions::WRITE) {
        let block = data.block;
        space.change(Some(block), || {
            space.drop_code_locked(page * PAGE_SIZE..(page + 1) * PAGE_SIZE)
        });
        return if resume(caller) {
            contract::UNWIND_LEAVE
        } else {
            contract::UNWIND_GO_TO_EXACT
        };
    }
    let access = if write { Access::Write } else { Access::Read };
    let retried = match caller.data().mode {
        Mode::Step { .. } => false,
        Mode::Execute(_) => matches!(
            ask(caller, |handler| handler
                .handle_memory_fault(address, access)),
            MemoryFaultResponse::Retry
        ),
    };
    if retried {
        return if resume(caller) {
            contract::UNWIND_LEAVE
        } else {
            contract::UNWIND_GO_TO_EXACT
        };
    }
    let direction = if write {
        AccessDirection::Write
    } else {
        AccessDirection::Read
    };
    let status = fault_status(&space, address);
    let exception = if is_cache_maintenance(caller.data()) {
        Exception::Undecoded {
            syndrome: to_cache_maintenance_syndrome(status),
            fault_address: computed,
        }
    } else {
        Exception::DataAbort {
            status,
            access: direction,
            address: computed,
        }
    };
    caller.data_mut().ending = Some(Ending::Raised(exception));
    contract::UNWIND_LEAVE
}

/// Returns whether the thread's instruction is DC CVAC or DC CIVAC, whose fault is a data abort of cache maintenance.
fn is_cache_maintenance(data: &ThreadData) -> bool {
    let pc = program_counter(data);
    let mut word = [u8::MIN; INSTRUCTION_BYTES];
    pc.is_multiple_of(INSTRUCTION_BYTES as u64)
        && space_of(data).read(pc, &mut word)
        && matches!(
            wcontainers_arm64::decode(u32::from_le_bytes(word)),
            wcontainers_arm64::Instruction::CleanCache { .. }
        )
}

/// Answers the load code hook: puts a piece for the address in place, or ends the run at a fetch fault or step end.
fn load_code(caller: &mut Caller<'_, ThreadData>, address: u64, exact: bool) -> i32 {
    leave_guest(caller.data_mut());
    let space = space_of(caller.data());
    let look_ahead = match caller.data().mode {
        Mode::Step {
            start,
            stops,
            loaded,
        } => {
            // A step ends wherever control goes after its one instruction.
            if !exact && (address != start || loaded) {
                caller.data_mut().ending = Some(Ending::Stepped);
                return contract::LOAD_CODE_LEAVE;
            }
            caller.data_mut().mode = Mode::Step {
                start,
                stops,
                loaded: true,
            };
            LookAhead::None
        }
        Mode::Execute(look_ahead) => look_ahead,
    };
    // Code runs only from pages that allow it, and from addresses the array of slots covers.
    loop {
        let page = address / PAGE_SIZE;
        if space.permissions(page).contains(Permissions::EXECUTE) && address < SLOTS_COVER {
            break;
        }
        let retried = match caller.data().mode {
            Mode::Step { .. } => false,
            Mode::Execute(_) => matches!(
                ask(caller, |handler| handler
                    .handle_memory_fault(address, Access::Execute)),
                MemoryFaultResponse::Retry
            ),
        };
        if !retried {
            caller.data_mut().ending = Some(Ending::Raised(Exception::InstructionAbort {
                status: fault_status(&space, address),
                address,
            }));
            return contract::LOAD_CODE_LEAVE;
        }
    }
    if exact {
        place_exact(caller, &space, address);
    } else if !place_load(caller, &space, address, look_ahead) {
        caller.data_mut().ending = Some(Ending::Raised(Exception::UndefinedInstruction));
        return contract::LOAD_CODE_LEAVE;
    }
    if resume(caller) {
        contract::LOAD_CODE_LEAVE
    } else {
        contract::LOAD_CODE_PLACED
    }
}

/// Puts the piece of the one instruction at the address in the exact slot, from those made for its word before.
fn place_exact(caller: &mut Caller<'_, ThreadData>, space: &Space, address: u64) {
    let mut word = [u8::MIN; INSTRUCTION_BYTES];
    let _ = space.read(address, &mut word);
    let key = (address, u32::from_le_bytes(word));
    let exact_slot = contract::EXACT_SLOT as u64;
    let table = caller.data().table.expect("a thread's lookup table");
    if let Some(&piece) = caller.data().exact.get(&key) {
        table
            .set(&mut *caller, exact_slot, Ref::Func(Some(piece)))
            .expect("the exact slot");
        return;
    }
    let translated = space
        .shared
        .translate(&word, u64::MIN, LookAhead::None)
        .expect("the translator takes one instruction");
    for (module, _) in &translated.modules {
        instantiate(caller, module, address, contract::EXACT_SLOT as u32);
    }
    let Some(Ref::Func(Some(piece))) = table.get(&mut *caller, exact_slot) else {
        panic!("the exact piece is in the exact slot");
    };
    caller.data_mut().exact.insert(key, piece);
}

/// Translates the code around the address and places it in every thread's lookup table; false when it is refused.
fn place_load(
    caller: &mut Caller<'_, ThreadData>,
    space: &Arc<Space>,
    address: u64,
    look_ahead: LookAhead,
) -> bool {
    let stepping = matches!(caller.data().mode, Mode::Step { .. });
    let window = if stepping {
        address..address + INSTRUCTION_BYTES as u64
    } else {
        let run = space
            .find_executable_run(address)
            .expect("an address on a page that allows running code");
        let page = address - address % PAGE_SIZE;
        run.start.max(page.saturating_sub(PAGES_BEHIND * PAGE_SIZE))
            ..run.end.min(page + PAGES_AHEAD * PAGE_SIZE).min(SLOTS_COVER)
    };
    let mut code =
        vec![u8::MIN; usize::try_from(window.end - window.start).expect("a window of code")];
    if !space.read(window.start, &mut code) {
        return false;
    }
    let Some(translated) = space
        .shared
        .translate(&code, address - window.start, look_ahead)
    else {
        return false;
    };
    let block = caller.data().block;
    space.change(Some(block), || {
        space.add_load_locked(translated, window.start, !stepping);
        sync(caller);
    });
    true
}

/// Answers the trap hook: the instruction raises its exception, which ends the run.
fn trap(caller: &mut Caller<'_, ThreadData>, kind: i32, detail: i64) {
    leave_guest(caller.data_mut());
    let pc = program_counter(caller.data());
    let exception = match TrapKind::from_i32(kind) {
        Some(TrapKind::SystemAccess) => Exception::SystemAccessTrapped(to_system_access(
            u32::try_from(detail & i64::from(u32::MAX)).expect("an instruction word"),
        )),
        Some(TrapKind::Breakpoint) => Exception::Breakpoint {
            comment: u16::try_from(detail & i64::from(u16::MAX))
                .expect("a comment of the field's width"),
        },
        Some(TrapKind::Alignment) => alignment_fault(caller.data(), pc, detail.cast_unsigned()),
        _ => Exception::UndefinedInstruction,
    };
    // A branch to a misaligned address completes; the fault is the fetch of the next instruction, which a step leaves
    // out, as it leaves out any other fetch after its instruction.
    let next_fetch = matches!(exception, Exception::ProgramCounterAlignmentFault { .. })
        && matches!(caller.data().mode, Mode::Step { start, .. } if start != pc);
    caller.data_mut().ending = Some(if next_fetch {
        Ending::Stepped
    } else {
        Ending::Raised(exception)
    });
}

/// Returns the exception of an alignment trap at `pc` whose detail is `address`.
///
/// It is a data abort for an access that must be aligned and is not, at the address it computed; a fault of the
/// program counter, which a branch left at `address`; or else a fault of the stack pointer, which an access through it
/// found misaligned.
fn alignment_fault(data: &ThreadData, pc: u64, address: u64) -> Exception {
    use wcontainers_arm64::Instruction as I;
    let mut word = [u8::MIN; INSTRUCTION_BYTES];
    let read = pc.is_multiple_of(INSTRUCTION_BYTES as u64) && space_of(data).read(pc, &mut word);
    let access = match read.then(|| wcontainers_arm64::decode(u32::from_le_bytes(word))) {
        Some(I::StoreExclusive { rn, .. } | I::StoreRelease { rn, .. }) if rn != Reg::Sp => {
            Some(AccessDirection::Write)
        }
        Some(
            I::LoadExclusive { rn, .. }
            | I::LoadAcquire { rn, .. }
            | I::Atomic { rn, .. }
            | I::CompareSwap { rn, .. },
        ) if rn != Reg::Sp => Some(AccessDirection::Read),
        _ => None,
    };
    match access {
        Some(access) => Exception::DataAbort {
            status: FaultStatus::AlignmentFault,
            access,
            address,
        },
        None if address == pc => Exception::ProgramCounterAlignmentFault { address },
        None => Exception::StackPointerAlignmentFault,
    }
}

/// Returns the fields EL1 is given of a trapped MRS, MSR (register) or MSR (immediate), the instruction word given.
fn to_system_access(word: u32) -> SystemAccess {
    let encoding = SystemRegisterEncoding::from_field(SYSTEM_REGISTER.read(word));
    let field = |value: u32| u8::try_from(value).expect("a field of a system register's encoding");
    SystemAccess {
        op0: field(SYSTEM_OP0.read(word)),
        op1: field(encoding.op1),
        crn: field(encoding.crn),
        crm: field(encoding.crm),
        op2: field(encoding.op2),
        rt: field(RT.read(word)),
        direction: if word >> SYSTEM_MOVE_READ_BIT & 1 == 1 {
            AccessDirection::Read
        } else {
            AccessDirection::Write
        },
    }
}

/// Answers the guest's own code changed hook (ARM64 `ic ivau`): drops the code of the line holding the address.
fn drop_changed_code(caller: &mut Caller<'_, ThreadData>, address: u64, line: u64) {
    leave_guest(caller.data_mut());
    let data = caller.data();
    let space = space_of(data);
    let start = address & !(line - 1);
    space.change(Some(data.block), || {
        space.drop_code_locked(start..start.saturating_add(line))
    });
    caller.data_mut().exact.clear();
    resume_in_place(caller);
}

/// Answers the guest's own hook at the index with `Guest::run_hook` on the bytes of the thread's state it names.
fn run_cpu_hook(
    caller: &Caller<'_, ThreadData>,
    index: CpuHookIndex,
    values: &[Val],
    results: &mut [Val],
) {
    let hook = &Arm64::HOOKS[index];
    let mut arguments = [i64::default(); CPU_HOOK_ARGUMENTS];
    for (argument, value) in arguments.iter_mut().zip(values) {
        *argument = match value {
            Val::I32(value) => i64::from(*value),
            Val::I64(value) => *value,
            other => unreachable!("a hook takes integers, not {other:?}"),
        };
    }
    let data = caller.data();
    let space = space_of(data);
    let range = hook.state.range();
    let at = usize::try_from(data.block).expect("a block offset") + range.start;
    let mut bytes = vec![u8::MIN; range.len()];
    space.state.read(at, &mut bytes);
    let before = bytes.clone();
    let done = Arm64::run_hook(index, &arguments[..hook.params.len()], &mut bytes);
    space.state.write_changed(at, &before, &bytes);
    for ((result, ty), value) in results.iter_mut().zip(hook.results).zip(done.results) {
        *result = match ty {
            wasm_encoder::ValType::I32 => Val::I32(value as i32),
            _ => Val::I64(value),
        };
    }
}

/// Converts a hook's parameter or result type to wasmtime's.
fn val_type(ty: wasm_encoder::ValType) -> ValType {
    match ty {
        wasm_encoder::ValType::I32 => ValType::I32,
        wasm_encoder::ValType::I64 => ValType::I64,
        other => unreachable!("a hook takes and gives integers, not {other:?}"),
    }
}

/// Makes the hooks of one store, by import name.
fn make_hooks(store: &mut Store<ThreadData>) -> Vec<(&'static str, Func)> {
    let mut hooks = vec![
        (
            contract::HOOK_SYSCALL,
            Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, ThreadData>,
                 number: i64,
                 a0: i64,
                 a1: i64,
                 a2: i64,
                 a3: i64,
                 a4: i64,
                 a5: i64|
                 -> i64 {
                    let request = SystemCallRequest {
                        number: SystemCallNumber(number.cast_unsigned()),
                        arguments: [a0, a1, a2, a3, a4, a5].map(i64::cast_unsigned),
                    };
                    answer_system_call(&mut caller, request)
                },
            ),
        ),
        (
            contract::HOOK_STOP,
            Func::wrap(&mut *store, |mut caller: Caller<'_, ThreadData>| -> i32 {
                stop(&mut caller)
            }),
        ),
        (
            contract::HOOK_MEMORY_FAULT,
            Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, ThreadData>, address: i64, access: i32| -> i32 {
                    memory_fault(&mut caller, address.cast_unsigned(), access == 1)
                },
            ),
        ),
        (
            contract::HOOK_LOAD_CODE,
            Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, ThreadData>, address: i64, exact: i32| -> i32 {
                    load_code(&mut caller, address.cast_unsigned(), exact == 1)
                },
            ),
        ),
        (
            contract::HOOK_WARN,
            Func::wrap(&mut *store, |_: Caller<'_, ThreadData>, _: i32, _: i64| {}),
        ),
        (
            contract::HOOK_TRAP,
            Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, ThreadData>, kind: i32, detail: i64| {
                    trap(&mut caller, kind, detail);
                },
            ),
        ),
    ];
    for (index, hook) in Arm64::HOOKS.iter().enumerate() {
        let func = match hook.body {
            HookBody::Clock => Func::wrap(&mut *store, |_: Caller<'_, ThreadData>| -> i64 {
                unreachable!("the executor's offer makes every counter read trap")
            }),
            HookBody::CodeChanged { line } => Func::wrap(
                &mut *store,
                move |mut caller: Caller<'_, ThreadData>, address: i64| {
                    drop_changed_code(&mut caller, address.cast_unsigned(), u64::from(line));
                },
            ),
            HookBody::Cpu => {
                assert!(
                    hook.params.len() <= CPU_HOOK_ARGUMENTS,
                    "a guest hook takes at most {CPU_HOOK_ARGUMENTS} arguments"
                );
                let ty = FuncType::new(
                    store.engine(),
                    hook.params.iter().map(|&ty| val_type(ty)),
                    hook.results.iter().map(|&ty| val_type(ty)),
                );
                Func::new(&mut *store, ty, move |caller, values, results| {
                    run_cpu_hook(&caller, index, values, results);
                    Ok(())
                })
            }
        };
        hooks.push((hook.name, func));
    }
    hooks
}
