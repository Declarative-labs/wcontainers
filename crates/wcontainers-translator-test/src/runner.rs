//! A small wasmtime runner: one guest thread, its memory, the hooks, and a stand-in for the kernel that
//! answers `write`, `writev`, `exit`, `exit_group` and `brk`.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use wasmtime::{
    Caller, Config, Engine, Extern, Global, Instance, Linker, Memory, MemoryType, Module,
    SharedMemory, Store, Trap, TypedFunc, Val,
};
use wcontainers_abi::{
    Access, CounterReading, Exit, PAGE_SIZE, Permissions, RegisterBlock, STOP_FLAG_OFFSET, Syscall,
    Warning,
};
use wcontainers_translator::contract::{self, Reason};
use wcontainers_translator::{
    LookAhead, Omit, Translation, translate_in_modules, translate_omitting,
};
use zerocopy::{FromBytes, IntoBytes};

/// Where the thread block sits in the state memory.
const BLOCK_AT: u32 = 0;
/// Where the permission table starts in the state memory.
const PERMISSIONS_AT: u32 = 4096;

/// What a system call does in this runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syscalls {
    /// Stop the thread at every system call, before it runs.
    Stop,
    /// Answer the stand-in's five calls; every other number returns "not implemented".
    StandIn,
}

/// Why `run` came back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stopped {
    Exit(Exit),
    /// The address allows running code, but the lookup table has no slot for it, so this runner cannot run
    /// it.
    NoCode(u64),
    /// The program called `exit` or `exit_group` with this status.
    Finished(i32),
}

/// Where the thread is paused when the pause function runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pause {
    /// At a stop check, before the thread carries on.
    Stop,
    /// After a system call the stand-in answered, before the thread carries on.
    Syscall,
}

/// What a pause function may look at and change while the thread is paused.
pub struct Paused<'a, 'b> {
    caller: &'a mut Caller<'b, Host>,
}

impl Paused<'_, '_> {
    pub fn registers(&mut self) -> RegisterBlock {
        snapshot(self.caller)
    }

    pub fn read(&mut self, address: u64, into: &mut [u8]) {
        let memory = self.caller.data().memory.unwrap();
        memory
            .read(&*self.caller, address as usize, into)
            .expect("read inside guest memory");
    }

    /// Say what a page aligned range allows, as a kernel would for another thread or a system call.
    pub fn set_permissions(&mut self, range: Range<u64>, permissions: Permissions) {
        set_permissions(self.caller.data_mut(), range, permissions);
    }

    /// Raise the thread's stop flag, so it stops at the next stop check it reaches.
    pub fn raise_stop_flag(&mut self) {
        raise_stop_flag(self.caller);
    }

    /// At a stop: the WebAssembly stack is lost here. The thread leaves, and carries on from its registers in
    /// a fresh thread instance.
    pub fn lose_stack(&mut self) {
        self.caller.data_mut().lose_stack = true;
    }

    /// Addresses the lookup table has been given a piece for, in this thread instance.
    pub fn placed(&self) -> &[u64] {
        &self.caller.data().placed
    }

    /// The lookup table forgets the piece for an address, as if none had been made yet.
    pub fn forget(&mut self, address: u64) {
        set_slot(self.caller.data(), address, 0);
    }
}

/// Called at every pause of the thread.
type PauseFunction = Box<dyn FnMut(Pause, &mut Paused<'_, '_>)>;

pub struct Host {
    /// What the engine does where WebAssembly leaves results open; translations are made for it.
    facts: wcontainers_arm64::Host,
    /// When the machine was made, for the clock.
    started: Instant,
    syscalls: Syscalls,
    stopped: Option<Stopped>,
    /// Registers at the moment the thread stopped.
    snapshot: RegisterBlock,
    /// What the guest wrote to file descriptors 1 and 2.
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    counters: HashMap<Reason, (u64, Duration)>,
    /// How often each slow path was taken at each address or number.
    places: HashMap<(Reason, u64), u64>,
    warnings: Vec<Warning<'static>>,
    /// Where the code of each blob loaded is, as guest addresses.
    code_ranges: Vec<Range<u64>>,
    /// By address, the piece of look-ahead none of the one instruction there, for the table's last slot.
    exact_pieces: HashMap<u64, wasmtime::Func>,
    /// The look-ahead code reached at run time with no piece is translated at, chosen each time.
    run_time: Box<dyn FnMut() -> LookAhead>,
    /// The code translated ahead of time, in the order it was loaded, made again in a fresh thread instance.
    loads: Vec<Load>,
    /// After every load, the piece each of these addresses gets: the one of this load.
    picks: Vec<(u64, usize)>,
    /// Addresses the lookup table has been given a piece for.
    placed: Vec<u64>,
    /// The pause function asked for the stack to be lost: the thread leaves and carries on in a fresh thread
    /// instance.
    lose_stack: bool,
    /// Entries of the array of slots: the instructions from `table_base` it covers.
    slots: u32,
    /// Where the array of slots is in the state memory.
    slots_at: u32,
    /// The lookup table's next free slot.
    next_slot: u32,
    state: Option<SharedMemory>,
    /// The program break the stand-in reports: the page after the highest blob.
    program_break: u64,
    engine: Engine,
    thread: Option<Instance>,
    memory: Option<Memory>,
    table_base: u64,
    /// The stop hook raises the flag again before it says carry on, so the thread stops at every check.
    stop_everywhere: bool,
    /// What each page allows, as the permission table says.
    permissions: Vec<Permissions>,
    pause: Option<PauseFunction>,
    /// No limit on instances and tables: each piece made at run time and each fresh thread instance is one.
    limits: wasmtime::StoreLimits,
}

pub struct Machine {
    store: Store<Host>,
    memory: Memory,
    state: SharedMemory,
    run: TypedFunc<(), ()>,
}

/// One engine for the whole process, so compiled modules can be shared.
fn engine() -> Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(new_engine).clone()
}

/// The thread module, compiled once.
fn thread_module(engine: &Engine) -> Module {
    static MODULE: OnceLock<Module> = OnceLock::new();
    MODULE
        .get_or_init(|| {
            Module::new(engine, wcontainers_translator::thread_module()).expect("thread module")
        })
        .clone()
}

/// The probe of this engine, run once per process.
pub fn probe() -> wcontainers_arm64::Host {
    static FACTS: OnceLock<wcontainers_arm64::Host> = OnceLock::new();
    *FACTS.get_or_init(|| {
        let engine = engine();
        let module =
            Module::new(&engine, wcontainers_arm64::host::probe_module()).expect("probe module");
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[]).expect("probe instance");
        let probe = instance
            .get_typed_func::<(), i32>(&mut store, wcontainers_arm64::host::PROBE)
            .expect("probe export");
        wcontainers_arm64::Host::from_probe(probe.call(&mut store, ()).expect("probe runs"))
    })
}

fn new_engine() -> Engine {
    let mut config = Config::new();
    config
        .wasm_tail_call(true)
        .wasm_function_references(true)
        .wasm_gc(true)
        .wasm_exceptions(true)
        .wasm_threads(true)
        .wasm_memory64(true)
        .wasm_multi_memory(true)
        .shared_memory(true)
        .memory_may_move(false);
    Engine::new(&config).expect("engine")
}

/// A module compiled with the runner's settings, as wasmtime stores it (an ELF file), for reading its machine
/// code.
pub fn precompile(wasm: &[u8]) -> Vec<u8> {
    engine().precompile_module(wasm).expect("a valid module")
}

/// How long the engine takes to compile a module.
pub fn compile_time(wasm: &[u8]) -> std::time::Duration {
    let engine = engine();
    let start = std::time::Instant::now();
    Module::new(&engine, wasm).expect("a valid module");
    start.elapsed()
}

impl Machine {
    /// A machine with `memory_bytes` of guest memory, nothing allowed anywhere, and an array of slots of
    /// `slots` entries, for the instructions from guest address `table_base`.
    pub fn new(memory_bytes: u64, table_base: u64, slots: u32, syscalls: Syscalls) -> Self {
        let engine = engine();
        // The table covers the whole memory, and one more byte, always zero, follows it.
        let permission_pages = memory_bytes.div_ceil(65536) * 65536 / PAGE_SIZE;
        let mut store = Store::new(
            &engine,
            Host {
                facts: probe(),
                started: Instant::now(),
                syscalls,
                stopped: None,
                snapshot: RegisterBlock::ZERO,
                stdout: Vec::new(),
                stderr: Vec::new(),
                counters: HashMap::new(),
                places: HashMap::new(),
                warnings: Vec::new(),
                code_ranges: Vec::new(),
                exact_pieces: HashMap::new(),
                run_time: Box::new(|| LookAhead::AT_RUN_TIME),
                loads: Vec::new(),
                picks: Vec::new(),
                placed: Vec::new(),
                lose_stack: false,
                slots,
                slots_at: 0,
                next_slot: contract::EXACT_SLOT as u32 + 1,
                state: None,
                program_break: 0,
                engine: engine.clone(),
                thread: None,
                memory: None,
                table_base,
                stop_everywhere: false,
                permissions: vec![Permissions::empty(); permission_pages as usize],
                pause: None,
                limits: wasmtime::StoreLimitsBuilder::new()
                    .instances(usize::MAX)
                    .tables(usize::MAX)
                    .memories(usize::MAX)
                    .build(),
            },
        );
        store.limiter(|host| &mut host.limits);
        let pages = memory_bytes.div_ceil(65536);
        let memory =
            Memory::new(&mut store, MemoryType::new64(pages, Some(pages))).expect("memory");
        let slots_at = (u64::from(PERMISSIONS_AT) + permission_pages + 1).next_multiple_of(4);
        let state_pages = (slots_at + 4 * u64::from(slots)).div_ceil(65536) as u32;
        let state = SharedMemory::new(&engine, MemoryType::shared(state_pages, state_pages))
            .expect("state memory");

        let host = store.data_mut();
        host.state = Some(state.clone());
        host.memory = Some(memory);
        host.slots_at = slots_at as u32;
        let run = thread_instance(&mut store);
        Machine {
            store,
            memory,
            state,
            run,
        }
    }

    /// Lose the WebAssembly stack: carry on from the registers in a fresh thread instance, with the code
    /// translated ahead of time made again and nothing translated since.
    fn fresh_thread(&mut self) {
        self.run = thread_instance(&mut self.store);
        let host = self.store.data_mut();
        host.exact_pieces.clear();
        host.placed.clear();
        host.next_slot = contract::EXACT_SLOT as u32 + 1;
        let entries = host.slots as usize * 4;
        write_shared(
            host.state.as_ref().unwrap(),
            host.slots_at as usize,
            &vec![0; entries],
        );
        let loads = std::mem::take(&mut host.loads);
        let mut pieces: Vec<HashMap<u64, u32>> = Vec::new();
        for load in &loads {
            pieces.push(instantiate(&mut self.store, load, false));
        }
        let picks = self.store.data().picks.clone();
        apply_picks(&mut self.store, &pieces, &picks);
        self.store.data_mut().loads = loads;
    }

    /// Place a blob at `load_address`, translate it at each look-ahead given, from its starts, and give each
    /// address that has pieces the piece of one of them, picked at random from `seed`.
    pub fn load_mixed(
        &mut self,
        bytes: &[u8],
        code: Range<u64>,
        load_address: u64,
        translations: &[(LookAhead, Vec<u64>)],
        seed: u64,
    ) {
        use proptest::prelude::RngExt;
        use proptest::test_runner::{RngAlgorithm, TestRng};
        self.place(bytes, code.clone(), load_address);
        let mut pieces: Vec<HashMap<u64, u32>> = Vec::new();
        for (look_ahead, starts) in translations {
            let load = translate_load(
                &mut self.store,
                (bytes, load_address),
                code.clone(),
                starts,
                *look_ahead,
                Omit::default(),
            );
            pieces.push(instantiate(&mut self.store, &load, false));
            self.store.data_mut().loads.push(load);
        }
        let mut key = [0u8; 32];
        key[..8].copy_from_slice(&seed.to_le_bytes());
        let mut rng = TestRng::from_seed(RngAlgorithm::ChaCha, &key);
        let first = self.store.data().loads.len() - translations.len();
        let mut addresses: Vec<u64> = pieces.iter().flat_map(|p| p.keys().copied()).collect();
        addresses.sort_unstable();
        addresses.dedup();
        let mut picks = Vec::new();
        for address in addresses {
            let having: Vec<usize> = (0..pieces.len())
                .filter(|&k| pieces[k].contains_key(&address))
                .collect();
            let k = having[rng.random_range(0..having.len())];
            picks.push((address, first + k));
        }
        let mut all = vec![HashMap::new(); first];
        all.extend(pieces);
        apply_picks(&mut self.store, &all, &picks);
        self.store.data_mut().picks.extend(picks);
    }

    pub fn write(&mut self, address: u64, bytes: &[u8]) {
        self.memory
            .write(&mut self.store, address as usize, bytes)
            .expect("write inside guest memory");
    }

    pub fn read(&mut self, address: u64, into: &mut [u8]) {
        self.memory
            .read(&self.store, address as usize, into)
            .expect("read inside guest memory");
    }

    /// Say what a page aligned range allows.
    pub fn set_permissions(&mut self, range: Range<u64>, permissions: Permissions) {
        set_permissions(self.store.data_mut(), range, permissions);
    }

    /// From now on `pause` runs whenever the thread pauses at a stop check or after a system call, and may
    /// change page permissions there.
    pub fn on_pause(&mut self, pause: impl FnMut(Pause, &mut Paused<'_, '_>) + 'static) {
        self.store.data_mut().pause = Some(Box::new(pause));
    }

    /// Place a blob at `load_address`, translate it from `starts` (blob offsets) with a look-ahead and add its
    /// pieces. Code reached at run time with no piece is translated as [`Machine::translate_at_run_time`] says.
    pub fn load(
        &mut self,
        bytes: &[u8],
        code: Range<u64>,
        load_address: u64,
        starts: &[u64],
        look_ahead: LookAhead,
    ) {
        self.load_omitting(
            bytes,
            code,
            load_address,
            starts,
            look_ahead,
            Omit::default(),
        );
    }

    /// [`Machine::load`] without some checks, only to measure what they cost.
    pub fn load_omitting(
        &mut self,
        bytes: &[u8],
        code: Range<u64>,
        load_address: u64,
        starts: &[u64],
        look_ahead: LookAhead,
        omit: Omit,
    ) {
        self.place(bytes, code.clone(), load_address);
        let load = translate_load(
            &mut self.store,
            (bytes, load_address),
            code,
            starts,
            look_ahead,
            omit,
        );
        instantiate(&mut self.store, &load, false);
        self.store.data_mut().loads.push(load);
    }

    /// [`Machine::load`] at full look-ahead with the translation split into code modules of at most
    /// `module_bytes` of code each, so that pieces reach each other across modules.
    pub fn load_split(
        &mut self,
        bytes: &[u8],
        code: Range<u64>,
        load_address: u64,
        starts: &[u64],
        module_bytes: usize,
    ) {
        self.place(bytes, code.clone(), load_address);
        let host = self.store.data();
        let translation = translate_in_modules(
            bytes,
            code,
            starts,
            LookAhead::Full,
            host.facts,
            module_bytes,
        )
        .expect("translate");
        let load = compile(&host.engine, load_address, translation);
        instantiate(&mut self.store, &load, false);
        self.store.data_mut().loads.push(load);
    }

    /// Place a blob and load a translation made of it: how long wasmtime took to compile its modules, how long
    /// to instantiate them, and the bytes of machine code it made.
    pub fn load_translation(
        &mut self,
        bytes: &[u8],
        code: Range<u64>,
        load_address: u64,
        translation: Translation,
    ) -> (Duration, Duration, usize) {
        self.place(bytes, code, load_address);
        let started = Instant::now();
        let load = compile(self.store.engine(), load_address, translation);
        let compiled = started.elapsed();
        let machine_code = load
            .modules
            .iter()
            .map(|m| m.image_range().end as usize - m.image_range().start as usize)
            .sum();
        let started = Instant::now();
        instantiate(&mut self.store, &load, false);
        let instantiated = started.elapsed();
        self.store.data_mut().loads.push(load);
        (compiled, instantiated, machine_code)
    }

    /// Put a blob's bytes in guest memory.
    fn place(&mut self, bytes: &[u8], code: Range<u64>, load_address: u64) {
        self.write(load_address, bytes);
        let host = self.store.data_mut();
        host.code_ranges
            .push(load_address + code.start..load_address + code.end);
        let end = (load_address + bytes.len() as u64).next_multiple_of(PAGE_SIZE);
        host.program_break = host.program_break.max(end);
    }

    /// From now on code reached with no piece is translated at the look-ahead `choose` gives each time.
    pub fn translate_at_run_time(&mut self, choose: impl FnMut() -> LookAhead + 'static) {
        self.store.data_mut().run_time = Box::new(choose);
    }

    /// From now on the thread stops at every stop check it reaches (each counted), and carries on each time.
    pub fn stop_at_every_check(&mut self) {
        self.store.data_mut().stop_everywhere = true;
        self.raise_stop_flag();
    }

    /// Raise the thread's stop flag while it is not running, as something outside the thread would.
    pub fn raise_stop_flag(&self) {
        write_shared(
            &self.state,
            BLOCK_AT as usize + STOP_FLAG_OFFSET,
            &1u32.to_le_bytes(),
        );
    }

    /// Something another host thread can call to raise the stop flag while the guest runs.
    pub fn stop_flag_raiser(&self) -> impl Fn() + Send + 'static {
        let state = self.state.clone();
        move || {
            let word = &state.data()[BLOCK_AT as usize + STOP_FLAG_OFFSET];
            // The flag is a 4 byte aligned word that both threads reach only through atomics or the guest's
            // own loads.
            let flag = unsafe { &*(word.get() as *const std::sync::atomic::AtomicU32) };
            flag.store(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    pub fn set_registers(&mut self, registers: &RegisterBlock) {
        write_shared(&self.state, BLOCK_AT as usize, registers.as_bytes());
    }

    /// Keep FPSR's flags exact from now on, as if the program had written FPSR.
    pub fn track_float_flags(&mut self) {
        write_shared(
            &self.state,
            BLOCK_AT as usize + contract::FLOAT_TRACKING,
            &1u32.to_le_bytes(),
        );
    }

    /// Translate for other facts about the engine than the probe found, to test the paths other hosts
    /// take. Only blobs loaded afterwards are affected.
    pub fn set_host(&mut self, facts: wcontainers_arm64::Host) {
        self.store.data_mut().facts = facts;
    }

    pub fn registers(&mut self) -> RegisterBlock {
        snapshot(&mut self.store)
    }

    pub fn stdout(&self) -> &[u8] {
        &self.store.data().stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.store.data().stderr
    }

    /// Every slow path taken so far: how often and the time spent.
    pub fn counters(&self) -> Vec<CounterReading> {
        let counters = &self.store.data().counters;
        Reason::ALL
            .into_iter()
            .filter_map(|reason| {
                let &(count, time) = counters.get(&reason)?;
                Some(CounterReading {
                    reason: reason.name(),
                    count,
                    nanoseconds: time.as_nanos() as u64,
                })
            })
            .collect()
    }

    /// How often a slow path was taken.
    pub fn count(&self, reason: Reason) -> u64 {
        self.store.data().counters.get(&reason).map_or(0, |c| c.0)
    }

    /// The warnings raised so far.
    pub fn warnings(&self) -> &[Warning<'static>] {
        &self.store.data().warnings
    }

    /// Run from the registers set until the thread stops. Returns why, and the registers at that moment.
    pub fn run(&mut self) -> (Stopped, RegisterBlock) {
        self.store.data_mut().stopped = None;
        let mut outcome = self.run.call(&mut self.store, ());
        while outcome.is_ok() && std::mem::take(&mut self.store.data_mut().lose_stack) {
            self.fresh_thread();
            outcome = self.run.call(&mut self.store, ());
        }
        match outcome {
            Ok(()) => {
                let host = self.store.data();
                (host.stopped.expect("left without a reason"), host.snapshot)
            }
            Err(error) => match error.downcast_ref::<Trap>() {
                Some(Trap::MemoryOutOfBounds) => {
                    slow_path(
                        self.store.data_mut(),
                        Reason::FaultWithoutExactRegisters,
                        0,
                        Duration::ZERO,
                    );
                    (
                        Stopped::Exit(Exit::MemoryFaultWithoutRegisters),
                        RegisterBlock::ZERO,
                    )
                }
                _ => panic!("engine stopped the guest: {error:?}"),
            },
        }
    }
}

fn set_permissions(host: &mut Host, range: Range<u64>, permissions: Permissions) {
    let pages = range.start / PAGE_SIZE..range.end.div_ceil(PAGE_SIZE);
    for page in pages.clone() {
        host.permissions[page as usize] = permissions;
    }
    let state = host.state.as_ref().unwrap();
    // Each byte also says what the page after it allows, so rewrite the one before the range too.
    for page in pages.start.saturating_sub(1)..pages.end.min(host.permissions.len() as u64) {
        let next = host
            .permissions
            .get(page as usize + 1)
            .copied()
            .unwrap_or(Permissions::empty());
        let byte = contract::permission_byte(host.permissions[page as usize], next);
        write_shared(state, (u64::from(PERMISSIONS_AT) + page) as usize, &[byte]);
    }
}

/// Run the pause function, if there is one.
fn pause(caller: &mut Caller<'_, Host>, at: Pause) {
    if let Some(mut function) = caller.data_mut().pause.take() {
        function(at, &mut Paused { caller });
        caller.data_mut().pause = Some(function);
    }
}

fn write_shared(memory: &SharedMemory, at: usize, bytes: &[u8]) {
    let data = memory.data();
    assert!(at + bytes.len() <= data.len());
    for (i, &byte) in bytes.iter().enumerate() {
        // The guest thread is not running, so nothing else touches these bytes.
        unsafe { *data[at + i].get() = byte };
    }
}

fn read_shared(memory: &SharedMemory, at: usize, into: &mut [u8]) {
    let data = memory.data();
    for (i, byte) in into.iter_mut().enumerate() {
        *byte = unsafe { *data[at + i].get() };
    }
}

/// The registers, from the thread block, where they all are whenever a hook runs or the thread has left.
fn snapshot(store: &mut impl wasmtime::AsContextMut<Data = Host>) -> RegisterBlock {
    let mut bytes = [0u8; size_of::<RegisterBlock>()];
    let state = store.as_context().data().state.clone().unwrap();
    read_shared(&state, BLOCK_AT as usize, &mut bytes);
    RegisterBlock::read_from_bytes(&bytes).unwrap()
}

/// Count a slow path taken at an address (or for a number) and the time it took; warn once per address
/// when its count there reaches the threshold.
fn slow_path(host: &mut Host, reason: Reason, detail: u64, time: Duration) {
    let counter = host.counters.entry(reason).or_default();
    counter.0 += 1;
    counter.1 += time;
    let total = counter.0;
    let here = host.places.entry((reason, detail)).or_default();
    *here += 1;
    if *here == contract::WARNING_THRESHOLD {
        let warning = Warning {
            reason: reason.name(),
            count: total,
            detail,
        };
        eprintln!(
            "wcontainers warning: {}, {detail:#x}, {total} so far",
            warning.reason
        );
        host.warnings.push(warning);
    }
}

/// Record why the thread leaves, with its registers now.
fn stop(caller: &mut Caller<'_, Host>, stopped: Stopped) {
    let registers = snapshot(caller);
    let host = caller.data_mut();
    host.snapshot = registers;
    host.stopped = Some(stopped);
}

fn raise_stop_flag(caller: &Caller<'_, Host>) {
    let state = caller.data().state.clone().unwrap();
    write_shared(
        &state,
        BLOCK_AT as usize + STOP_FLAG_OFFSET,
        &1u32.to_le_bytes(),
    );
}

fn hooks(linker: &mut Linker<Host>) {
    use contract::*;
    linker
        .func_wrap(
            HOOKS,
            HOOK_SYSCALL,
            |mut caller: Caller<'_, Host>,
             number: i64,
             a0: i64,
             a1: i64,
             a2: i64,
             a3: i64,
             a4: i64,
             a5: i64|
             -> i64 {
                let call = Syscall {
                    number: number as u64,
                    arguments: [a0, a1, a2, a3, a4, a5].map(|a| a as u64),
                };
                if caller.data().syscalls == Syscalls::Stop {
                    stop(&mut caller, Stopped::Exit(Exit::Syscall(call)));
                    raise_stop_flag(&caller);
                    return 0;
                }
                let result = stand_in(&mut caller, call);
                if caller.data().stopped.is_none() {
                    pause(&mut caller, Pause::Syscall);
                }
                result
            },
        )
        .unwrap();
    linker
        .func_wrap(HOOKS, HOOK_STOP, |mut caller: Caller<'_, Host>| -> i32 {
            let state = caller.data().state.clone().unwrap();
            write_shared(
                &state,
                BLOCK_AT as usize + STOP_FLAG_OFFSET,
                &0u32.to_le_bytes(),
            );
            // The runner raised the flag itself to make the thread leave.
            if caller.data().stopped.is_some() {
                return 1;
            }
            // Raised from outside: the stand-in clears it and the thread carries on.
            let pc = snapshot(&mut caller).pc;
            slow_path(caller.data_mut(), Reason::StoppedByFlag, pc, Duration::ZERO);
            if caller.data().stop_everywhere {
                raise_stop_flag(&caller);
            }
            pause(&mut caller, Pause::Stop);
            i32::from(caller.data().lose_stack)
        })
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_MEMORY_FAULT,
            |mut caller: Caller<'_, Host>, address: i64, access: i32| {
                slow_path(
                    caller.data_mut(),
                    Reason::MemoryFault,
                    address as u64,
                    Duration::ZERO,
                );
                let access = if access == 1 {
                    Access::Write
                } else {
                    Access::Read
                };
                stop(
                    &mut caller,
                    Stopped::Exit(Exit::MemoryFault {
                        address: address as u64,
                        access,
                    }),
                );
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_LOAD_CODE,
            |mut caller: Caller<'_, Host>, address: i64, exact: i32| -> i32 {
                let address = address as u64;
                let table = lookup_table(&mut caller);
                let exact_slot = contract::EXACT_SLOT as u64;
                if exact == 1
                    && let Some(&piece) = caller.data().exact_pieces.get(&address)
                {
                    table
                        .set(&mut caller, exact_slot, wasmtime::Ref::Func(Some(piece)))
                        .unwrap();
                    return 1;
                }
                // Code runs only from pages that allow it, as on Linux: a jump anywhere else faults there.
                let Some(run) = executable_run(caller.data(), address) else {
                    slow_path(
                        caller.data_mut(),
                        Reason::MemoryFault,
                        address,
                        Duration::ZERO,
                    );
                    let access = Access::Execute;
                    stop(
                        &mut caller,
                        Stopped::Exit(Exit::MemoryFault { address, access }),
                    );
                    return 0;
                };
                // The array has slots only for the addresses it covers; the runner cannot run code past them.
                let covered = covered(caller.data());
                if !covered.contains(&address) {
                    stop(&mut caller, Stopped::NoCode(address));
                    return 0;
                }
                let start = Instant::now();
                // The code as it is in memory now, which may be code the program wrote: a blob's code, or the
                // run of pages that allow running code within what the table covers.
                let known = caller
                    .data()
                    .code_ranges
                    .iter()
                    .find(|r| r.contains(&address));
                let run = match known {
                    Some(code) => code.clone(),
                    None => run.start.max(covered.start)..run.end.min(covered.end),
                };
                let mut bytes = vec![0; (run.end - run.start) as usize];
                let memory = caller.data().memory.unwrap();
                memory
                    .read(&caller, run.start as usize, &mut bytes)
                    .expect("code inside guest memory");
                let offset = address - run.start;
                if exact == 1 {
                    let one = LookAhead::Bounded(1);
                    let code = (&bytes[..], run.start);
                    instantiate_code(&mut caller, code, offset, one, true);
                    let Some(wasmtime::Ref::Func(Some(piece))) = table.get(&mut caller, exact_slot)
                    else {
                        panic!("the piece is in the exact slot");
                    };
                    caller.data_mut().exact_pieces.insert(address, piece);
                } else {
                    // Code made at run time never takes the place of a piece the lookup finds.
                    assert_eq!(
                        slot(caller.data(), address),
                        0,
                        "code asked for at {address:#x}, which has a piece"
                    );
                    let look_ahead = (caller.data_mut().run_time)();
                    let code = (&bytes[..], run.start);
                    instantiate_code(&mut caller, code, offset, look_ahead, false);
                }
                let time = start.elapsed();
                slow_path(
                    caller.data_mut(),
                    Reason::TranslatedAtRunTime,
                    address,
                    time,
                );
                1
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_WARN,
            |mut caller: Caller<'_, Host>, reason: i32, detail: i64| {
                let reason = Reason::from_i32(reason).expect("known reason");
                slow_path(caller.data_mut(), reason, detail as u64, Duration::ZERO);
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_FLOAT,
            |mut caller: Caller<'_, Host>, operation: i32, a: i64, b: i64, c: i64| -> i64 {
                let state = caller.data().state.clone().unwrap();
                let word = |at: usize| {
                    let mut bytes = [0u8; 4];
                    read_shared(&state, BLOCK_AT as usize + at, &mut bytes);
                    u32::from_le_bytes(bytes)
                };
                let fpsr_at = core::mem::offset_of!(RegisterBlock, fpsr);
                let fpcr = word(core::mem::offset_of!(RegisterBlock, fpcr));
                let mut fpsr = word(fpsr_at);
                let tracking = word(contract::FLOAT_TRACKING);
                let result = wcontainers_arm64::exact::run(
                    operation, a as u64, b as u64, c as u64, fpcr, &mut fpsr,
                );
                write_shared(&state, BLOCK_AT as usize + fpsr_at, &fpsr.to_le_bytes());
                let (kind, _) = wcontainers_arm64::exact::Operation::decode(operation);
                let reason = if kind.fused() && fpcr == 0 && tracking == 0 {
                    Reason::SoftwareFusedMultiplyAdd
                } else {
                    Reason::ExactFloatingPointRequested
                };
                // Registers are not saved here, so the detail is the operation, not the address.
                slow_path(
                    caller.data_mut(),
                    reason,
                    u64::from(operation as u32),
                    Duration::ZERO,
                );
                result as i64
            },
        )
        .unwrap();
    linker
        .func_wrap(HOOKS, HOOK_CLOCK, |caller: Caller<'_, Host>| -> i64 {
            caller.data().started.elapsed().as_nanos() as i64
        })
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_CODE_CHANGED,
            |mut caller: Caller<'_, Host>, address: i64| {
                forget_code(&mut caller, address as u64);
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_TRAP,
            |mut caller: Caller<'_, Host>, kind: i32, detail: i64| {
                let kind = TrapKind::from_i32(kind).expect("known trap");
                let exit = match kind {
                    TrapKind::Undefined | TrapKind::Unsupported => Exit::IllegalInstruction,
                    TrapKind::Breakpoint => Exit::Breakpoint {
                        comment: detail as u16,
                    },
                    TrapKind::Alignment => Exit::AlignmentFault {
                        address: detail as u64,
                    },
                };
                stop(&mut caller, Stopped::Exit(exit));
                if kind == TrapKind::Unsupported {
                    let pc = caller.data().snapshot.pc;
                    let reason = Reason::UnsupportedInstructionReached;
                    slow_path(caller.data_mut(), reason, pc, Duration::ZERO);
                }
            },
        )
        .unwrap();
}

/// The run of pages around an address that allow running code, if its own page does.
fn executable_run(host: &Host, address: u64) -> Option<Range<u64>> {
    let executable = |page: u64| {
        host.permissions
            .get(page as usize)
            .is_some_and(|p| p.contains(Permissions::EXECUTE))
    };
    let page = address / PAGE_SIZE;
    if !executable(page) {
        return None;
    }
    let first = (0..page)
        .rev()
        .take_while(|&p| executable(p))
        .last()
        .unwrap_or(page);
    let end = (page + 1..)
        .take_while(|&p| executable(p))
        .last()
        .unwrap_or(page)
        + 1;
    Some(first * PAGE_SIZE..end * PAGE_SIZE)
}

/// The thread instance's lookup table.
fn lookup_table(store: &mut impl wasmtime::AsContextMut<Data = Host>) -> wasmtime::Table {
    let thread = store.as_context().data().thread.unwrap();
    let Some(Extern::Table(table)) = thread.get_export(store, contract::LOOKUP) else {
        panic!("the thread module exports its lookup table");
    };
    table
}

/// `ic ivau`: the stand-in forgets the pieces the lookup finds for the 64 byte line, so the next jump there
/// translates the code again.
fn forget_code(caller: &mut Caller<'_, Host>, address: u64) {
    let line = address & !63;
    for at in (line..line + 64).step_by(4) {
        caller.data_mut().exact_pieces.remove(&at);
        set_slot(caller.data(), at, 0);
    }
}

/// The guest addresses the array of slots covers.
fn covered(host: &Host) -> Range<u64> {
    host.table_base..host.table_base + 4 * u64::from(host.slots)
}

/// Where in the state memory the array holds the slot for an address, if it covers it.
fn slot_entry(host: &Host, address: u64) -> Option<usize> {
    (covered(host).contains(&address) && address.is_multiple_of(4))
        .then(|| host.slots_at as usize + (address - host.table_base) as usize)
}

/// The lookup table slot of the piece for an address, 0 for none.
fn slot(host: &Host, address: u64) -> u32 {
    slot_entry(host, address).map_or(0, |at| {
        let mut bytes = [0; 4];
        read_shared(host.state.as_ref().unwrap(), at, &mut bytes);
        u32::from_le_bytes(bytes)
    })
}

/// Give an address the piece in a lookup table slot, 0 for none.
fn set_slot(host: &Host, address: u64, slot: u32) {
    if let Some(at) = slot_entry(host, address) {
        write_shared(host.state.as_ref().unwrap(), at, &slot.to_le_bytes());
    }
}

/// The code modules made for a blob: where the blob is, and the blob offsets of the pieces they put in the
/// table.
struct Load {
    load_address: u64,
    modules: Vec<Module>,
    /// The blob offsets of each module's pieces, in slot order.
    module_pieces: Vec<Vec<u64>>,
    pieces: Vec<u64>,
}

/// Translate the blob of `bytes` at a load address from `starts` and compile it.
fn translate_load(
    store: &mut impl wasmtime::AsContextMut<Data = Host>,
    (bytes, load_address): (&[u8], u64),
    code: Range<u64>,
    starts: &[u64],
    look_ahead: LookAhead,
    omit: Omit,
) -> Load {
    let host = store.as_context().data();
    let translation =
        translate_omitting(bytes, code, starts, look_ahead, host.facts, omit).expect("translate");
    compile(&host.engine, load_address, translation)
}

/// Compile a translation's code modules.
fn compile(engine: &Engine, load_address: u64, translation: Translation) -> Load {
    Load {
        load_address,
        pieces: translation.pieces().collect(),
        module_pieces: translation
            .modules
            .iter()
            .map(|m| m.pieces.clone())
            .collect(),
        modules: translation
            .modules
            .iter()
            .map(|m| Module::new(engine, &m.wasm).expect("code module"))
            .collect(),
    }
}

/// Translate and instantiate code reached at run time: the code bytes at a load address, from one start.
fn instantiate_code(
    store: &mut impl wasmtime::AsContextMut<Data = Host>,
    code: (&[u8], u64),
    start: u64,
    look_ahead: LookAhead,
    exact: bool,
) {
    let range = 0..code.0.len() as u64;
    let load = translate_load(store, code, range, &[start], look_ahead, Omit::default());
    instantiate(store, &load, exact);
}

/// Give each address picked the piece of the load picked for it, from each load's pieces by address and
/// their slots.
fn apply_picks(
    store: &mut impl wasmtime::AsContextMut<Data = Host>,
    pieces: &[HashMap<u64, u32>],
    picks: &[(u64, usize)],
) {
    let host = store.as_context().data();
    for &(address, load) in picks {
        set_slot(host, address, pieces[load][&address]);
    }
}

/// Instantiate a load's code modules, putting their pieces in new slots of the lookup table and giving each
/// piece's address its slot. Returns the slot of each piece, by address. With `exact`, the load is one
/// piece, put in [`contract::EXACT_SLOT`] and given to no address.
fn instantiate(
    store: &mut impl wasmtime::AsContextMut<Data = Host>,
    load: &Load,
    exact: bool,
) -> HashMap<u64, u32> {
    let mut store = store.as_context_mut();
    let table = lookup_table(&mut store);
    let host = store.data();
    let thread = host.thread.unwrap();
    let memory = host.memory.unwrap();
    let state = host.state.clone().unwrap();
    let load_address = load.load_address;
    let first = if exact {
        contract::EXACT_SLOT as u32
    } else {
        host.next_slot
    };
    if !exact {
        let pieces = load.pieces.len() as u32;
        let more = (first + pieces).saturating_sub(table.size(&store) as u32);
        table
            .grow(&mut store, u64::from(more), wasmtime::Ref::Func(None))
            .expect("a lookup table that grows");
        store.data_mut().next_slot = first + pieces;
        let placed: Vec<u64> = load.pieces.iter().map(|o| load_address + o).collect();
        store.data_mut().placed.extend(placed);
    }

    let mut linker = Linker::new(store.engine());
    linker.allow_shadowing(true);
    hooks(&mut linker);
    linker
        .define(&store, contract::ENV, contract::MEMORY, memory)
        .unwrap();
    linker
        .define(&store, contract::ENV, contract::STATE, state)
        .unwrap();
    let pages = memory.size(&store);
    for (name, value) in [
        (contract::LOAD_ADDRESS, Val::I64(load_address as i64)),
        (
            contract::TABLE_BASE,
            Val::I64(store.data().table_base as i64),
        ),
        (contract::THREAD_BLOCK, Val::I32(BLOCK_AT as i32)),
        (contract::PERMISSIONS, Val::I32(PERMISSIONS_AT as i32)),
        (
            contract::PERMISSION_PAGES,
            Val::I64((pages * 65536 / PAGE_SIZE) as i64),
        ),
        (contract::SLOTS, Val::I32(store.data().slots_at as i32)),
        (
            contract::SLOTS_COVER,
            Val::I64(4 * i64::from(store.data().slots)),
        ),
    ] {
        let ty = wasmtime::GlobalType::new(value.ty(&store).unwrap(), wasmtime::Mutability::Const);
        let global = Global::new(&mut store, ty, value).unwrap();
        linker.define(&store, contract::ENV, name, global).unwrap();
    }
    let exports: Vec<(String, Extern)> = thread
        .exports(&mut store)
        .map(|export| (export.name().to_string(), export.into_extern()))
        .collect();
    for (name, item) in exports {
        linker
            .define(&store, contract::THREAD, &name, item)
            .unwrap();
    }
    let mut slot = first;
    let mut slots = HashMap::new();
    for (module, pieces) in load.modules.iter().zip(&load.module_pieces) {
        let base = wasmtime::GlobalType::new(wasmtime::ValType::I32, wasmtime::Mutability::Const);
        let base = Global::new(&mut store, base, Val::I32(slot as i32)).unwrap();
        linker
            .define(&store, contract::ENV, contract::SLOT_BASE, base)
            .unwrap();
        linker
            .instantiate(&mut store, module)
            .expect("code instance");
        for offset in pieces {
            slots.insert(load_address + offset, slot);
            if !exact {
                set_slot(store.data(), load_address + offset, slot);
            }
            slot += 1;
        }
    }
    slots
}

/// Make a thread instance, with an empty lookup table, for the machine's memories; its run function.
fn thread_instance(store: &mut Store<Host>) -> TypedFunc<(), ()> {
    let engine = store.engine().clone();
    let host = store.data();
    let module = thread_module(&engine);
    let state = host.state.clone().unwrap();
    let (table_base, slots_at, slots) = (host.table_base, host.slots_at, host.slots);
    let mut linker = Linker::new(&engine);
    hooks(&mut linker);
    linker
        .define(&*store, contract::ENV, contract::STATE, state)
        .unwrap();
    let constant = |store: &mut Store<Host>, value: Val| {
        let ty = value.ty(&*store).unwrap();
        let ty = wasmtime::GlobalType::new(ty, wasmtime::Mutability::Const);
        Global::new(&mut *store, ty, value).unwrap()
    };
    for (name, value) in [
        (contract::TABLE_BASE, Val::I64(table_base as i64)),
        (contract::THREAD_BLOCK, Val::I32(BLOCK_AT as i32)),
        (contract::SLOTS, Val::I32(slots_at as i32)),
        (contract::SLOTS_COVER, Val::I64(4 * i64::from(slots))),
    ] {
        let global = constant(store, value);
        linker.define(&*store, contract::ENV, name, global).unwrap();
    }
    let thread = linker
        .instantiate(&mut *store, &module)
        .expect("thread instance");
    store.data_mut().thread = Some(thread);
    thread.get_typed_func(&mut *store, contract::RUN).unwrap()
}

/// Where the stand-in sends what is written to a file descriptor.
fn file(host: &mut Host, fd: u64) -> Option<&mut Vec<u8>> {
    match fd {
        1 => Some(&mut host.stdout),
        2 => Some(&mut host.stderr),
        _ => None,
    }
}

/// The stand-in kernel.
fn stand_in(caller: &mut Caller<'_, Host>, call: Syscall) -> i64 {
    const EBADF: i64 = 9;
    const EFAULT: i64 = 14;
    const ENOSYS: i64 = 38;
    const EINVAL: i64 = 22;
    // Linux's limit on the vectors of one writev.
    const IOV_MAX: u64 = 1024;
    let [a0, a1, a2, ..] = call.arguments;
    let memory = caller.data().memory.unwrap();
    // Guest bytes, if every one is on a page the guest may read. Like a write to a pipe on Linux, a buffer
    // that is only partly readable gives a fault and writes nothing.
    let read = |caller: &mut Caller<'_, Host>, address: u64, length: u64| -> Option<Vec<u8>> {
        if length > 0 {
            let last = address.checked_add(length - 1)?;
            if last >= memory.data_size(&*caller) as u64 {
                return None;
            }
            let state = caller.data().state.clone().unwrap();
            let table = state.data();
            let readable = |page: u64| {
                table
                    .get(PERMISSIONS_AT as usize + page as usize)
                    .is_some_and(|byte| unsafe { *byte.get() } & contract::PAGE_READ != 0)
            };
            if !(address / PAGE_SIZE..=last / PAGE_SIZE).all(readable) {
                return None;
            }
        }
        let mut bytes = vec![0; usize::try_from(length).ok()?];
        memory.read(&*caller, address as usize, &mut bytes).ok()?;
        Some(bytes)
    };
    match call.number {
        // write
        64 => {
            if file(caller.data_mut(), a0).is_none() {
                return -EBADF;
            }
            let Some(bytes) = read(caller, a1, a2) else {
                return -EFAULT;
            };
            file(caller.data_mut(), a0)
                .unwrap()
                .extend_from_slice(&bytes);
            a2 as i64
        }
        // writev
        66 => {
            if file(caller.data_mut(), a0).is_none() {
                return -EBADF;
            }
            // Linux takes the vector count as a 32 bit number.
            let count = u64::from(a2 as u32);
            if count > IOV_MAX {
                return -EINVAL;
            }
            let Some(vectors) = read(caller, a1, 16 * count) else {
                return -EFAULT;
            };
            let vectors: Vec<(u64, u64)> = vectors
                .chunks(16)
                .map(|v| {
                    let half = |at: usize| u64::from_le_bytes(v[at..at + 8].try_into().unwrap());
                    (half(0), half(8))
                })
                .collect();
            // A length that is negative as a signed number is invalid, checked before anything is read.
            if vectors.iter().any(|&(_, length)| (length as i64) < 0) {
                return -EINVAL;
            }
            let mut gathered = Vec::new();
            for (base, length) in vectors {
                let Some(bytes) = read(caller, base, length) else {
                    return -EFAULT;
                };
                gathered.extend_from_slice(&bytes);
            }
            file(caller.data_mut(), a0)
                .unwrap()
                .extend_from_slice(&gathered);
            gathered.len() as i64
        }
        // exit, exit_group
        93 | 94 => {
            stop(caller, Stopped::Finished(a0 as i32));
            raise_stop_flag(caller);
            0
        }
        // brk: this stand-in never moves the break, and answers with the current one as Linux does when it
        // cannot.
        214 => caller.data().program_break as i64,
        number => {
            let reason = Reason::SystemCallNotImplemented;
            slow_path(caller.data_mut(), reason, number, Duration::ZERO);
            -ENOSYS
        }
    }
}
