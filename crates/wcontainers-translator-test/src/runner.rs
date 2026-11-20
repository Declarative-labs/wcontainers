//! A small wasmtime runner: one guest thread, its memory, the hooks, and a stand-in for the kernel that
//! answers `write`, `writev`, `exit`, `exit_group` and `brk`.

use crate::guest::GuestTest;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::ops::Range;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use wasmtime::{
    Caller, Config, Engine, Extern, Global, Instance, Linker, Memory, MemoryType, Module,
    SharedMemory, Store, Trap, TypedFunc, Val, ValRaw,
};
use wcontainers_abi::{Access, CounterReading, Exit, PAGE_SIZE, Permissions, Syscall, Warning};
use wcontainers_guest_cpu::{CodeChanges, HookBody, HookType, MemoryFault};
pub use wcontainers_translator::contract::thread_state_size;
use wcontainers_translator::contract::{self, Reason};
use wcontainers_translator::{
    LookAhead, Omit, Optimise, Translation, translate_in_modules, translate_omitting,
};
use zerocopy::{FromBytes, IntoBytes};

/// Where the thread block sits in the state memory.
const BLOCK_AT: u32 = 0;
/// Where the permission table starts in the state memory, after the thread state.
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
    /// A fault the abi's exits do not name yet (x86-64's, added to the abi with its kernel side).
    Fault(Fault),
}

/// A fault of a guest CPU other than those the abi's exits name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// `SIGFPE`, `FPE_INTDIV`.
    DivideError,
    /// `SIGSEGV`, `SI_KERNEL`.
    GeneralProtection,
    /// `SIGFPE` with this `si_code`.
    FloatException(i64),
    /// `int 4`: `SIGSEGV`, `SI_KERNEL`.
    Overflow,
    /// x86's stack fault: `SIGBUS`, `SI_KERNEL`.
    Stack,
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
pub struct Paused<'a, 'b, G: GuestTest> {
    caller: &'a mut Caller<'b, Host<G>>,
}

impl<G: GuestTest> Paused<'_, '_, G> {
    pub fn registers(&mut self) -> G::Block {
        block::<G>(&snapshot(self.caller))
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
type PauseFunction<G> = Box<dyn FnMut(Pause, &mut Paused<'_, '_, G>)>;

pub struct Host<G: GuestTest> {
    /// What the engine does where WebAssembly leaves results open; translations are made for it.
    facts: G::Host,
    /// When the machine was made, for the clock.
    started: Instant,
    syscalls: Syscalls,
    stopped: Option<Stopped>,
    /// The thread state at the moment the thread stopped.
    snapshot: Vec<u8>,
    /// What the guest wrote to file descriptors 1 and 2.
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    counters: FxHashMap<u32, (u64, Duration)>,
    /// How often each slow path was taken at each address or number.
    places: FxHashMap<(u32, u64), u64>,
    warnings: Vec<Warning<'static>>,
    /// Where the code of each blob loaded is, as guest addresses.
    code_ranges: Vec<Range<u64>>,
    /// By address, the piece of look-ahead none of the one instruction there, for the table's last slot.
    exact_pieces: HashMap<u64, wasmtime::Func>,
    /// The look-ahead code reached at run time with no piece is translated at, chosen each time.
    run_time: Box<dyn FnMut() -> LookAhead>,
    /// What translations are made for, unless a load says.
    optimise: Optimise,
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
    /// What each page allows, as the kernel last said; the table leaves out what the locks hold back.
    permissions: Vec<Permissions>,
    /// The write locks the CPU side holds on each page, a bit for each reason (`LOCK_CODE`).
    locks: Vec<u8>,
    /// Translate the instruction at this address without locking its page, at the thread's next exact
    /// `load_code` there: the write it faulted on runs once on the unlocked page.
    exact_mark: Option<u64>,
    /// Code translated at run time: the guest bytes each translation was made from, and the addresses it
    /// gave its pieces.
    run_time_loads: Vec<(Vec<Range<u64>>, Vec<u64>)>,
    pause: Option<PauseFunction<G>>,
    /// No limit on instances and tables: each piece made at run time and each fresh thread instance is one.
    limits: wasmtime::StoreLimits,
}

pub struct Machine<G: GuestTest> {
    store: Store<Host<G>>,
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
fn thread_module<G: GuestTest>(engine: &Engine) -> Module {
    per_guest::<G, _>(|| {
        Module::new(engine, wcontainers_translator::thread_module::<G>()).expect("thread module")
    })
}

/// A value made once per process for each guest. A static inside a generic function is one for all of its
/// instances, so each guest's is kept by its type.
fn per_guest<G: 'static, T: Clone + Send + Sync + 'static>(make: impl FnOnce() -> T) -> T {
    use std::any::{Any, TypeId};
    use std::sync::Mutex;
    type Made = HashMap<(TypeId, TypeId), Box<dyn Any + Send + Sync>>;
    static MADE: OnceLock<Mutex<Made>> = OnceLock::new();
    let key = (TypeId::of::<G>(), TypeId::of::<T>());
    let made = MADE.get_or_init(Default::default);
    if let Some(value) = made.lock().unwrap().get(&key) {
        return value.downcast_ref::<T>().unwrap().clone();
    }
    let value = make();
    made.lock()
        .unwrap()
        .entry(key)
        .or_insert_with(|| Box::new(value))
        .downcast_ref::<T>()
        .unwrap()
        .clone()
}

/// A block's values as the state both sides of a comparison hold.
pub fn fields<G: GuestTest>(block: &G::Block) -> crate::guest::State {
    let mut thread = vec![0; thread_state_size::<G>()];
    thread[..size_of::<G::Block>()].copy_from_slice(block.as_bytes());
    G::to_state(&thread)
}

/// The guest's pc in a thread state.
fn pc_of<G: GuestTest>(thread: &[u8]) -> u64 {
    u64::from_le_bytes(thread[G::PC_OFFSET..][..8].try_into().unwrap())
}

/// The block at the start of a thread state.
fn block<G: GuestTest>(thread: &[u8]) -> G::Block {
    G::Block::read_from_bytes(&thread[..size_of::<G::Block>()]).expect("a thread block")
}

/// Names an answer the host probe can give, which then stands in for this engine's own, so that what the
/// translator emits for another host runs and is checked here too.
pub const PROBE_ANSWER_SETTING: &str = "WC_PROBE_ANSWER";

/// The probe of this engine, run once per process, or the answer `PROBE_ANSWER_SETTING` names.
pub fn probe<G: GuestTest>() -> G::Host {
    per_guest::<G, _>(|| {
        if let Some(answer) = std::env::var(PROBE_ANSWER_SETTING)
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
        {
            assert!(
                G::PROBE_ANSWERS.contains(&answer),
                "{PROBE_ANSWER_SETTING} {answer} is not an answer of the probe"
            );
            return G::host_from_probe(answer);
        }
        let engine = engine();
        let module = Module::new(&engine, G::probe_module()).expect("probe module");
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[]).expect("probe instance");
        let probe = instance
            .get_typed_func::<(), i32>(&mut store, contract::PROBE)
            .expect("probe export");
        G::host_from_probe(probe.call(&mut store, ()).expect("probe runs"))
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

impl<G: GuestTest> Machine<G> {
    /// A machine with `memory_bytes` of guest memory, nothing allowed anywhere, and an array of slots of
    /// `slots` entries, for the instructions from guest address `table_base`.
    pub fn new(memory_bytes: u64, table_base: u64, slots: u32, syscalls: Syscalls) -> Self {
        const {
            assert!(
                BLOCK_AT as usize + thread_state_size::<G>() <= PERMISSIONS_AT as usize,
                "the thread state fits before the permission table"
            )
        };
        let engine = engine();
        // The table covers the whole memory, and one more byte, always zero, follows it.
        let permission_pages = memory_bytes.div_ceil(65536) * 65536 / PAGE_SIZE;
        let mut store = Store::new(
            &engine,
            Host {
                facts: probe::<G>(),
                started: Instant::now(),
                syscalls,
                stopped: None,
                snapshot: vec![0; thread_state_size::<G>()],
                stdout: Vec::new(),
                stderr: Vec::new(),
                counters: FxHashMap::default(),
                places: FxHashMap::default(),
                warnings: Vec::new(),
                code_ranges: Vec::new(),
                exact_pieces: HashMap::new(),
                run_time: Box::new(|| LookAhead::AT_RUN_TIME),
                optimise: Optimise::Speed,
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
                locks: vec![0; permission_pages as usize],
                exact_mark: None,
                run_time_loads: Vec::new(),
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
        host.run_time_loads.clear();
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
            pieces.push(if load.dropped {
                HashMap::new()
            } else {
                instantiate(&mut self.store, load, false)
            });
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
        translations: &[(LookAhead, Optimise, Vec<u64>)],
        seed: u64,
    ) {
        use proptest::prelude::RngExt;
        use proptest::test_runner::{RngAlgorithm, TestRng};
        self.place(bytes, code.clone(), load_address);
        let mut pieces: Vec<HashMap<u64, u32>> = Vec::new();
        for (look_ahead, optimise, starts) in translations {
            let load = translate_load(
                &mut self.store,
                (bytes, load_address),
                code.clone(),
                starts,
                (*look_ahead, *optimise),
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

    /// Write guest memory as the kernel does, past the page permissions; a page locked for code drops its
    /// pieces first.
    pub fn write(&mut self, address: u64, bytes: &[u8]) {
        let pages = address / PAGE_SIZE..(address + bytes.len() as u64).div_ceil(PAGE_SIZE);
        for page in pages {
            if self
                .store
                .data()
                .locks
                .get(page as usize)
                .is_some_and(|l| l & LOCK_CODE != 0)
            {
                code_written(self.store.data_mut(), page);
            }
        }
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
    pub fn on_pause(&mut self, pause: impl FnMut(Pause, &mut Paused<'_, '_, G>) + 'static) {
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
        let optimise = self.store.data().optimise;
        let load = translate_load(
            &mut self.store,
            (bytes, load_address),
            code,
            starts,
            (look_ahead, optimise),
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
        let translation = translate_in_modules::<G>(
            bytes,
            code,
            starts,
            LookAhead::Full,
            host.optimise,
            host.facts,
            G::Offer::default(),
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

    /// From now on translations are made for `optimise`, those made at run time included.
    pub fn optimise_for(&mut self, optimise: Optimise) {
        self.store.data_mut().optimise = optimise;
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
            BLOCK_AT as usize + G::STOP_FLAG_OFFSET,
            &1u32.to_le_bytes(),
        );
    }

    /// Something another host thread can call to raise the stop flag while the guest runs.
    pub fn stop_flag_raiser(&self) -> impl Fn() + Send + 'static {
        let state = self.state.clone();
        move || {
            let word = &state.data()[BLOCK_AT as usize + G::STOP_FLAG_OFFSET];
            // The flag is a 4 byte aligned word that both threads reach only through atomics or the guest's
            // own loads.
            let flag = unsafe { &*(word.get() as *const std::sync::atomic::AtomicU32) };
            flag.store(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    pub fn set_registers(&mut self, registers: &G::Block) {
        write_shared(&self.state, BLOCK_AT as usize, registers.as_bytes());
    }

    /// Set the whole thread state: the block and the CPU's private bytes.
    pub fn set_thread_state(&mut self, thread: &[u8]) {
        assert_eq!(thread.len(), thread_state_size::<G>());
        write_shared(&self.state, BLOCK_AT as usize, thread);
    }

    /// Keep the float exception flags exact from now on, as if the program had written them.
    pub fn track_float_flags(&mut self) {
        let mut thread = vec![0; thread_state_size::<G>()];
        read_shared(&self.state, BLOCK_AT as usize, &mut thread);
        G::track(&mut thread);
        write_shared(&self.state, BLOCK_AT as usize, &thread);
    }

    /// The clock reads `nanos` now and runs on from there.
    pub fn set_clock(&mut self, nanos: u64) {
        self.store.data_mut().started = Instant::now()
            .checked_sub(Duration::from_nanos(nanos))
            .expect("a clock that far back");
    }

    /// Translate for other facts about the engine than the probe found, to test the paths other hosts
    /// take. Only blobs loaded afterwards are affected.
    pub fn set_host(&mut self, facts: G::Host) {
        self.store.data_mut().facts = facts;
    }

    pub fn registers(&mut self) -> G::Block {
        block::<G>(&snapshot(&mut self.store))
    }

    /// The whole thread state at the moment the thread last stopped.
    pub fn stopped_state(&self) -> &[u8] {
        &self.store.data().snapshot
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
                let &(count, time) = counters.get(&reason.number())?;
                Some(CounterReading {
                    reason: reason.name(),
                    detail: 0,
                    count,
                    nanoseconds: time.as_nanos() as u64,
                })
            })
            .collect()
    }

    /// How often a slow path was taken.
    pub fn count(&self, reason: Reason) -> u64 {
        self.store
            .data()
            .counters
            .get(&reason.number())
            .map_or(0, |c| c.0)
    }

    /// How often the slow path of this name, the translator's or the guest's own, was taken.
    pub fn count_named(&self, name: &str) -> u64 {
        assert!(
            Reason::ALL.into_iter().any(|r| r.name() == name)
                || G::REASONS.iter().any(|r| r.1 == name),
            "no slow path is named {name:?}"
        );
        self.store
            .data()
            .counters
            .iter()
            .filter(|&(&number, _)| Reason::name_of::<G>(number) == Some(name))
            .map(|(_, c)| c.0)
            .sum()
    }

    /// The warnings raised so far.
    pub fn warnings(&self) -> &[Warning<'static>] {
        &self.store.data().warnings
    }

    /// Run from the registers set until the thread stops. Returns why, and the registers at that moment.
    pub fn run(&mut self) -> (Stopped, G::Block) {
        self.store.data_mut().stopped = None;
        let mut outcome = self.run.call(&mut self.store, ());
        while outcome.is_ok() && std::mem::take(&mut self.store.data_mut().lose_stack) {
            self.fresh_thread();
            outcome = self.run.call(&mut self.store, ());
        }
        match outcome {
            Ok(()) => {
                let host = self.store.data();
                (
                    host.stopped.expect("left without a reason"),
                    block::<G>(&host.snapshot),
                )
            }
            Err(error) => match error.downcast_ref::<Trap>() {
                Some(Trap::MemoryOutOfBounds) => {
                    slow_path(
                        self.store.data_mut(),
                        Reason::FaultWithoutExactRegisters.number(),
                        0,
                        Duration::ZERO,
                    );
                    let lost = vec![0; thread_state_size::<G>()];
                    self.store.data_mut().snapshot = lost.clone();
                    (
                        Stopped::Exit(Exit::MemoryFaultWithoutRegisters),
                        block::<G>(&lost),
                    )
                }
                _ => panic!("engine stopped the guest: {error:?}"),
            },
        }
    }
}

fn set_permissions<G: GuestTest>(host: &mut Host<G>, range: Range<u64>, permissions: Permissions) {
    let pages = range.start / PAGE_SIZE..range.end.div_ceil(PAGE_SIZE);
    for page in pages.clone() {
        host.permissions[page as usize] = permissions;
    }
    let state = host.state.as_ref().unwrap();
    // Each byte also says what the page after it allows, so rewrite the one before the range too.
    for page in pages.start.saturating_sub(1)..pages.end.min(host.permissions.len() as u64) {
        write_permission_byte(host, state, page);
    }
}

/// A write lock the CPU side holds on a page: the page holds code it translated.
const LOCK_CODE: u8 = 1;

/// What the table says a page allows: what the kernel said, without writing while a lock is held.
fn allowed<G: GuestTest>(host: &Host<G>, page: u64) -> Permissions {
    let Some(&permissions) = host.permissions.get(page as usize) else {
        return Permissions::empty();
    };
    if host.locks[page as usize] != 0 {
        permissions - Permissions::WRITE
    } else {
        permissions
    }
}

/// Write the permission table's byte for a page from what it and the next page allow.
fn write_permission_byte<G: GuestTest>(host: &Host<G>, state: &SharedMemory, page: u64) {
    let byte = contract::permission_byte(allowed(host, page), allowed(host, page + 1));
    write_shared(state, (u64::from(PERMISSIONS_AT) + page) as usize, &[byte]);
}

/// Take or give back a write lock on a page, and write the table's bytes that show it.
fn set_lock<G: GuestTest>(host: &mut Host<G>, page: u64, lock: u8, held: bool) {
    let locks = &mut host.locks[page as usize];
    let before = *locks;
    if held {
        *locks |= lock;
    } else {
        *locks &= !lock;
    }
    if (before == 0) != (*locks == 0) {
        let state = host.state.clone().unwrap();
        write_permission_byte(host, &state, page);
        if let Some(previous) = page.checked_sub(1) {
            write_permission_byte(host, &state, previous);
        }
    }
}

/// Lock for code every page a translation was made from (guest addresses), where code changes are detected
/// by such writes.
fn lock_code<G: GuestTest>(host: &mut Host<G>, read: &[Range<u64>]) {
    if G::CODE_CHANGES != CodeChanges::Detected {
        return;
    }
    for range in read {
        for page in range.start / PAGE_SIZE..range.end.div_ceil(PAGE_SIZE) {
            if (page as usize) < host.locks.len() {
                set_lock(host, page, LOCK_CODE, true);
            }
        }
    }
}

/// Code was written on a page locked for code: every piece translated from the page is dropped (the lookup
/// forgets the addresses of every translation made from it), and the lock is given back. Counted.
fn code_written<G: GuestTest>(host: &mut Host<G>, page: u64) {
    let bytes = page * PAGE_SIZE..(page + 1) * PAGE_SIZE;
    let touches = |read: &[Range<u64>]| {
        read.iter()
            .any(|r| r.start < bytes.end && bytes.start < r.end)
    };
    let mut forget: Vec<u64> = Vec::new();
    for load in &mut host.loads {
        if !load.dropped && touches(&load.read) {
            load.dropped = true;
            forget.extend(load.pieces.iter().map(|o| load.load_address + o));
        }
    }
    host.run_time_loads.retain(|(read, addresses)| {
        let stale = touches(read);
        if stale {
            forget.extend(addresses);
        }
        !stale
    });
    for address in forget {
        set_slot(host, address, 0);
    }
    host.exact_pieces.clear();
    set_lock(host, page, LOCK_CODE, false);
    slow_path(
        host,
        Reason::CodeWrittenWithoutAnnouncement.number(),
        page * PAGE_SIZE,
        Duration::ZERO,
    );
}

/// Run the pause function, if there is one.
fn pause<G: GuestTest>(caller: &mut Caller<'_, Host<G>>, at: Pause) {
    if let Some(mut function) = caller.data_mut().pause.take() {
        function(at, &mut Paused { caller });
        caller.data_mut().pause = Some(function);
    }
}

fn write_shared(memory: &SharedMemory, at: usize, bytes: &[u8]) {
    let data = &memory.data()[at..at + bytes.len()];
    // SAFETY: the range is inside the memory, and the guest thread is not running, so nothing else
    // touches these bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.as_ptr() as *mut u8, bytes.len()) };
}

fn read_shared(memory: &SharedMemory, at: usize, into: &mut [u8]) {
    let data = &memory.data()[at..at + into.len()];
    // SAFETY: as for `write_shared`.
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, into.as_mut_ptr(), into.len())
    };
}

/// The thread state, where every register is whenever a hook runs or the thread has left.
fn snapshot<G: GuestTest>(store: &mut impl wasmtime::AsContextMut<Data = Host<G>>) -> Vec<u8> {
    let mut bytes = vec![0u8; thread_state_size::<G>()];
    let state = store.as_context().data().state.clone().unwrap();
    read_shared(&state, BLOCK_AT as usize, &mut bytes);
    bytes
}

/// The WebAssembly type of a hook's parameter or result.
fn val_type(ty: wasm_encoder::ValType) -> wasmtime::ValType {
    match ty {
        wasm_encoder::ValType::I32 => wasmtime::ValType::I32,
        wasm_encoder::ValType::I64 => wasmtime::ValType::I64,
        other => unreachable!("a hook takes and gives integers, not {other:?}"),
    }
}

/// The guest's own hook `I`: its work by `Guest::run_hook`, in place on the bytes of the thread's state it
/// names, counted and warned as the slow path it says it was. `values` holds the arguments and takes the
/// results. Made for each index at compile time (`CpuHooks::REGISTER`), so the hook's types and state range
/// are constants here.
fn cpu_hook<G: GuestTest, const I: usize>(
    caller: &mut Caller<'_, Host<G>>,
    values: &mut [MaybeUninit<ValRaw>],
) {
    let hook = &G::HOOKS[I];
    let mut args = [0i64; HookType::MAX_PARAMS];
    for (i, ty) in hook.params.iter().enumerate() {
        // SAFETY: wasmtime has checked the arguments against the hook's type.
        let value = unsafe { values[i].assume_init() };
        args[i] = match ty {
            wasm_encoder::ValType::I32 => i64::from(value.get_i32()),
            _ => value.get_i64(),
        };
    }
    let at = BLOCK_AT as usize + usize::from(hook.state.at);
    let cells = &caller.data().state.as_ref().unwrap().data()[at..][..usize::from(hook.state.len)];
    // SAFETY: the range is inside the memory, and the guest thread is in this hook, so nothing else touches
    // these bytes while the hook works on them in place.
    let bytes = unsafe { std::slice::from_raw_parts_mut(cells.as_ptr() as *mut u8, cells.len()) };
    let done = G::run_hook(I, &args[..hook.params.len()], bytes);
    for (i, ty) in hook.results.iter().enumerate() {
        values[i] = MaybeUninit::new(match ty {
            wasm_encoder::ValType::I32 => ValRaw::i32(done.results[i] as i32),
            _ => ValRaw::i64(done.results[i]),
        });
    }
    if let Some(reason) = done.reason {
        // Registers are not saved here, so the detail is the hook's first argument, not the address.
        slow_path(
            caller.data_mut(),
            reason,
            args[0] as u32 as u64,
            Duration::ZERO,
        );
    }
}

/// Registers the guest's own hook `I` with a closure made for that index.
fn register_cpu_hook<G: GuestTest, const I: usize>(linker: &mut Linker<Host<G>>) {
    let hook = &G::HOOKS[I];
    let ty = wasmtime::FuncType::new(
        linker.engine(),
        hook.params.iter().map(|&t| val_type(t)),
        hook.results.iter().map(|&t| val_type(t)),
    );
    // SAFETY: `cpu_hook` reads the arguments and writes the results of exactly this type.
    unsafe {
        linker.func_new_unchecked(contract::HOOKS, hook.name, ty, |mut caller, values| {
            cpu_hook::<G, I>(&mut caller, values);
            Ok(())
        })
    }
    .unwrap();
}

/// `register_cpu_hook` for each index a guest's own hook may have.
struct CpuHooks<G>(std::marker::PhantomData<G>);

impl<G: GuestTest> CpuHooks<G> {
    const REGISTER: [fn(&mut Linker<Host<G>>); 8] = [
        register_cpu_hook::<G, 0>,
        register_cpu_hook::<G, 1>,
        register_cpu_hook::<G, 2>,
        register_cpu_hook::<G, 3>,
        register_cpu_hook::<G, 4>,
        register_cpu_hook::<G, 5>,
        register_cpu_hook::<G, 6>,
        register_cpu_hook::<G, 7>,
    ];
    /// Every hook of the guest fits the closures made for it, checked when the runner is built for it.
    const FIT: () = {
        assert!(G::HOOKS.len() <= Self::REGISTER.len());
        let mut i = 0;
        while i < G::HOOKS.len() {
            assert!(
                G::HOOKS[i].params.len() <= HookType::MAX_PARAMS
                    && G::HOOKS[i].results.len() <= HookType::MAX_RESULTS
            );
            i += 1;
        }
    };
}

/// The guest's bytes from `pc`, as many as an instruction may take and the memory has.
fn code_at<G: GuestTest>(caller: &mut Caller<'_, Host<G>>, pc: u64) -> Vec<u8> {
    let memory = caller.data().memory.unwrap();
    let data = memory.data(&*caller);
    let at = (pc as usize).min(data.len());
    data[at..(at + G::MAX_LENGTH as usize).min(data.len())].to_vec()
}

/// Count a slow path taken at an address (or for a number) and the time it took; warn once per address
/// when its count there reaches the threshold.
fn slow_path<G: GuestTest>(host: &mut Host<G>, reason: u32, detail: u64, time: Duration) {
    let counter = host.counters.entry(reason).or_default();
    counter.0 += 1;
    counter.1 += time;
    let total = counter.0;
    let here = host.places.entry((reason, detail)).or_default();
    *here += 1;
    if *here == contract::WARNING_THRESHOLD {
        let warning = Warning {
            reason: Reason::name_of::<G>(reason).expect("a known reason"),
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
fn stop<G: GuestTest>(caller: &mut Caller<'_, Host<G>>, stopped: Stopped) {
    let registers = snapshot(caller);
    let host = caller.data_mut();
    host.snapshot = registers;
    host.stopped = Some(stopped);
}

fn raise_stop_flag<G: GuestTest>(caller: &Caller<'_, Host<G>>) {
    let state = caller.data().state.clone().unwrap();
    write_shared(
        &state,
        BLOCK_AT as usize + G::STOP_FLAG_OFFSET,
        &1u32.to_le_bytes(),
    );
}

fn hooks<G: GuestTest>(linker: &mut Linker<Host<G>>) {
    use contract::*;
    linker
        .func_wrap(
            HOOKS,
            HOOK_SYSCALL,
            |mut caller: Caller<'_, Host<G>>,
             number: i64,
             a0: i64,
             a1: i64,
             a2: i64,
             a3: i64,
             a4: i64,
             a5: i64|
             -> i64 {
                caller.data_mut().exact_mark = None;
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
        .func_wrap(HOOKS, HOOK_STOP, |mut caller: Caller<'_, Host<G>>| -> i32 {
            caller.data_mut().exact_mark = None;
            let state = caller.data().state.clone().unwrap();
            write_shared(
                &state,
                BLOCK_AT as usize + G::STOP_FLAG_OFFSET,
                &0u32.to_le_bytes(),
            );
            // The runner raised the flag itself to make the thread leave.
            if caller.data().stopped.is_some() {
                return UNWIND_LEAVE;
            }
            // Raised from outside: the stand-in clears it and the thread carries on.
            let mut pc = [0u8; 8];
            read_shared(&state, BLOCK_AT as usize + G::PC_OFFSET, &mut pc);
            let pc = u64::from_le_bytes(pc);
            slow_path(
                caller.data_mut(),
                Reason::StoppedByFlag.number(),
                pc,
                Duration::ZERO,
            );
            if caller.data().stop_everywhere {
                raise_stop_flag(&caller);
            }
            pause(&mut caller, Pause::Stop);
            if caller.data().lose_stack {
                UNWIND_LEAVE
            } else {
                0
            }
        })
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_MEMORY_FAULT,
            |mut caller: Caller<'_, Host<G>>, address: i64, access: i32| -> i32 {
                let page = address as u64 / PAGE_SIZE;
                let host = caller.data_mut();
                host.exact_mark = None;
                let locked_write = access == 1
                    && host
                        .locks
                        .get(page as usize)
                        .is_some_and(|l| l & LOCK_CODE != 0)
                    && host.permissions[page as usize].contains(Permissions::WRITE);
                if locked_write {
                    code_written(host, page);
                    let pc = pc_of::<G>(&snapshot(&mut caller));
                    caller.data_mut().exact_mark = Some(pc);
                    return UNWIND_GO_TO_EXACT;
                }
                slow_path(
                    caller.data_mut(),
                    Reason::MemoryFault.number(),
                    address as u64,
                    Duration::ZERO,
                );
                let thread = snapshot(&mut caller);
                let pc = pc_of::<G>(&thread);
                let registers = block::<G>(&thread);
                let code = code_at(&mut caller, pc);
                let access = if access == 1 {
                    Access::Write
                } else {
                    Access::Read
                };
                let stopped = match G::memory_fault(address as u64, &code, &registers) {
                    MemoryFault::Page => Stopped::Exit(Exit::MemoryFault {
                        address: address as u64,
                        access,
                    }),
                    MemoryFault::GeneralProtection => Stopped::Fault(Fault::GeneralProtection),
                    MemoryFault::Stack => Stopped::Fault(Fault::Stack),
                };
                stop(&mut caller, stopped);
                UNWIND_LEAVE
            },
        )
        .unwrap();
    linker
        .func_wrap(
            HOOKS,
            HOOK_LOAD_CODE,
            |mut caller: Caller<'_, Host<G>>, address: i64, exact: i32| -> i32 {
                let address = address as u64;
                let table = lookup_table(&mut caller);
                let exact_slot = contract::EXACT_SLOT as u64;
                let unlocked = caller.data_mut().exact_mark.take() == Some(address) && exact == 1;
                if exact == 1
                    && !unlocked
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
                        Reason::MemoryFault.number(),
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
                    instantiate_code(&mut caller, code, offset, one, (true, unlocked));
                    let Some(wasmtime::Ref::Func(Some(piece))) = table.get(&mut caller, exact_slot)
                    else {
                        panic!("the piece is in the exact slot");
                    };
                    // A piece of an unlocked page may be written over unseen, so it is not kept.
                    if !unlocked {
                        caller.data_mut().exact_pieces.insert(address, piece);
                    }
                } else {
                    // Code made at run time never takes the place of a piece the lookup finds.
                    assert_eq!(
                        slot(caller.data(), address),
                        0,
                        "code asked for at {address:#x}, which has a piece"
                    );
                    let look_ahead = (caller.data_mut().run_time)();
                    let code = (&bytes[..], run.start);
                    instantiate_code(&mut caller, code, offset, look_ahead, (false, false));
                }
                let time = start.elapsed();
                slow_path(
                    caller.data_mut(),
                    Reason::TranslatedAtRunTime.number(),
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
            |mut caller: Caller<'_, Host<G>>, reason: i32, detail: i64| {
                slow_path(
                    caller.data_mut(),
                    reason as u32,
                    detail as u64,
                    Duration::ZERO,
                );
            },
        )
        .unwrap();
    for (index, hook) in G::HOOKS.iter().enumerate() {
        match hook.body {
            HookBody::Clock => {
                linker
                    .func_wrap(HOOKS, hook.name, |caller: Caller<'_, Host<G>>| -> i64 {
                        caller.data().started.elapsed().as_nanos() as i64
                    })
                    .unwrap();
            }
            HookBody::CodeChanged { line } => {
                linker
                    .func_wrap(
                        HOOKS,
                        hook.name,
                        move |mut caller: Caller<'_, Host<G>>, address: i64| {
                            forget_code(&mut caller, address as u64, u64::from(line));
                        },
                    )
                    .unwrap();
            }
            HookBody::Cpu => {
                let () = CpuHooks::<G>::FIT;
                CpuHooks::<G>::REGISTER[index](linker)
            }
        }
    }
    linker
        .func_wrap(
            HOOKS,
            HOOK_TRAP,
            |mut caller: Caller<'_, Host<G>>, kind: i32, detail: i64| {
                caller.data_mut().exact_mark = None;
                let kind = TrapKind::from_i32(kind).expect("known trap");
                let stopped = match kind {
                    TrapKind::Undefined | TrapKind::Unsupported => {
                        Stopped::Exit(Exit::IllegalInstruction)
                    }
                    TrapKind::Breakpoint => Stopped::Exit(Exit::Breakpoint {
                        comment: detail as u16,
                    }),
                    TrapKind::Alignment => Stopped::Exit(Exit::AlignmentFault {
                        address: detail as u64,
                    }),
                    TrapKind::DivideError => Stopped::Fault(Fault::DivideError),
                    TrapKind::GeneralProtection => Stopped::Fault(Fault::GeneralProtection),
                    TrapKind::FloatException => Stopped::Fault(Fault::FloatException(detail)),
                    TrapKind::Overflow => Stopped::Fault(Fault::Overflow),
                };
                stop(&mut caller, stopped);
                if kind == TrapKind::Unsupported {
                    let pc = pc_of::<G>(&caller.data().snapshot);
                    let reason = Reason::UnsupportedInstructionReached.number();
                    slow_path(caller.data_mut(), reason, pc, Duration::ZERO);
                }
            },
        )
        .unwrap();
}

/// The run of pages around an address that allow running code, if its own page does.
fn executable_run<G: GuestTest>(host: &Host<G>, address: u64) -> Option<Range<u64>> {
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
fn lookup_table<G: GuestTest>(
    store: &mut impl wasmtime::AsContextMut<Data = Host<G>>,
) -> wasmtime::Table {
    let thread = store.as_context().data().thread.unwrap();
    let Some(Extern::Table(table)) = thread.get_export(store, contract::LOOKUP) else {
        panic!("the thread module exports its lookup table");
    };
    table
}

/// The stand-in forgets the pieces the lookup finds for the `line` bytes of code holding the address, so
/// the next jump there translates the code again.
fn forget_code<G: GuestTest>(caller: &mut Caller<'_, Host<G>>, address: u64, line: u64) {
    let start = address & !(line - 1);
    for at in (start..start + line).step_by(G::ALIGN as usize) {
        caller.data_mut().exact_pieces.remove(&at);
        set_slot(caller.data(), at, 0);
    }
}

/// The guest addresses the array of slots covers.
fn covered<G: GuestTest>(host: &Host<G>) -> Range<u64> {
    host.table_base..host.table_base + G::ALIGN * u64::from(host.slots)
}

/// Where in the state memory the array holds the slot for an address, if it covers it.
fn slot_entry<G: GuestTest>(host: &Host<G>, address: u64) -> Option<usize> {
    (covered(host).contains(&address) && address.is_multiple_of(G::ALIGN)).then(|| {
        host.slots_at as usize + contract::slot_entry::<G>(address - host.table_base) as usize
    })
}

/// The lookup table slot of the piece for an address, 0 for none.
fn slot<G: GuestTest>(host: &Host<G>, address: u64) -> u32 {
    slot_entry(host, address).map_or(0, |at| {
        let mut bytes = [0; 4];
        read_shared(host.state.as_ref().unwrap(), at, &mut bytes);
        u32::from_le_bytes(bytes)
    })
}

/// Give an address the piece in a lookup table slot, 0 for none.
fn set_slot<G: GuestTest>(host: &Host<G>, address: u64, slot: u32) {
    if let Some(at) = slot_entry(host, address) {
        write_shared(host.state.as_ref().unwrap(), at, &slot.to_le_bytes());
    }
}

/// The code modules made for a blob: where the blob is, and the blob offsets of the pieces they put in the
/// table.
struct Load {
    load_address: u64,
    /// The guest bytes the translation was made from.
    read: Vec<Range<u64>>,
    /// Code was written over what it was made from, so its pieces are gone.
    dropped: bool,
    modules: Vec<Module>,
    /// The blob offsets of each module's pieces, in slot order.
    module_pieces: Vec<Vec<u64>>,
    pieces: Vec<u64>,
}

/// Translate the blob of `bytes` at a load address from `starts` and compile it.
fn translate_load<G: GuestTest>(
    store: &mut impl wasmtime::AsContextMut<Data = Host<G>>,
    (bytes, load_address): (&[u8], u64),
    code: Range<u64>,
    starts: &[u64],
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
) -> Load {
    let host = store.as_context().data();
    let translation = translate_omitting::<G>(
        bytes,
        code,
        starts,
        look_ahead,
        optimise,
        host.facts,
        G::Offer::default(),
        omit,
    )
    .expect("translate");
    compile(&host.engine, load_address, translation)
}

/// Compile a translation's code modules.
fn compile(engine: &Engine, load_address: u64, translation: Translation) -> Load {
    Load {
        load_address,
        read: translation
            .read
            .iter()
            .map(|r| load_address + r.start..load_address + r.end)
            .collect(),
        dropped: false,
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
/// With `exact`, the one piece goes in the exact slot; with `unlocked` too, its page is not locked for code.
fn instantiate_code<G: GuestTest>(
    store: &mut impl wasmtime::AsContextMut<Data = Host<G>>,
    code: (&[u8], u64),
    start: u64,
    look_ahead: LookAhead,
    (exact, unlocked): (bool, bool),
) {
    let range = 0..code.0.len() as u64;
    let how = (look_ahead, store.as_context().data().optimise);
    let load = translate_load(store, code, range, &[start], how, Omit::default());
    let mut store = store.as_context_mut();
    // `instantiate` locks the pages of every load but an exact one.
    if exact && !unlocked {
        lock_code(store.data_mut(), &load.read);
    }
    let slots = instantiate(&mut store, &load, exact);
    if !exact {
        let addresses = slots.into_keys().collect();
        store.data_mut().run_time_loads.push((load.read, addresses));
    }
}

/// Give each address picked the piece of the load picked for it, from each load's pieces by address and
/// their slots.
fn apply_picks<G: GuestTest>(
    store: &mut impl wasmtime::AsContextMut<Data = Host<G>>,
    pieces: &[HashMap<u64, u32>],
    picks: &[(u64, usize)],
) {
    let host = store.as_context().data();
    for &(address, load) in picks {
        if let Some(&slot) = pieces[load].get(&address) {
            set_slot(host, address, slot);
        }
    }
}

/// Instantiate a load's code modules, putting their pieces in new slots of the lookup table and giving each
/// piece's address its slot. Returns the slot of each piece, by address. With `exact`, the load is one
/// piece, put in [`contract::EXACT_SLOT`] and given to no address.
fn instantiate<G: GuestTest>(
    store: &mut impl wasmtime::AsContextMut<Data = Host<G>>,
    load: &Load,
    exact: bool,
) -> HashMap<u64, u32> {
    let mut store = store.as_context_mut();
    if !exact {
        lock_code(store.data_mut(), &load.read);
    }
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
            Val::I64(G::ALIGN as i64 * i64::from(store.data().slots)),
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
fn thread_instance<G: GuestTest>(store: &mut Store<Host<G>>) -> TypedFunc<(), ()> {
    let engine = store.engine().clone();
    let host = store.data();
    let module = thread_module::<G>(&engine);
    let state = host.state.clone().unwrap();
    let (table_base, slots_at, slots) = (host.table_base, host.slots_at, host.slots);
    let mut linker = Linker::new(&engine);
    hooks(&mut linker);
    linker
        .define(&*store, contract::ENV, contract::STATE, state)
        .unwrap();
    let constant = |store: &mut Store<Host<G>>, value: Val| {
        let ty = value.ty(&*store).unwrap();
        let ty = wasmtime::GlobalType::new(ty, wasmtime::Mutability::Const);
        Global::new(&mut *store, ty, value).unwrap()
    };
    for (name, value) in [
        (contract::TABLE_BASE, Val::I64(table_base as i64)),
        (contract::THREAD_BLOCK, Val::I32(BLOCK_AT as i32)),
        (contract::SLOTS, Val::I32(slots_at as i32)),
        (
            contract::SLOTS_COVER,
            Val::I64(G::ALIGN as i64 * i64::from(slots)),
        ),
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
fn file<G: GuestTest>(host: &mut Host<G>, fd: u64) -> Option<&mut Vec<u8>> {
    match fd {
        1 => Some(&mut host.stdout),
        2 => Some(&mut host.stderr),
        _ => None,
    }
}

/// The stand-in kernel.
fn stand_in<G: GuestTest>(caller: &mut Caller<'_, Host<G>>, call: Syscall) -> i64 {
    const EPERM: i64 = 1;
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
    let read = |caller: &mut Caller<'_, Host<G>>, address: u64, length: u64| -> Option<Vec<u8>> {
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
    let numbers = G::STAND_IN;
    match call.number {
        n if n == numbers.write => {
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
        n if n == numbers.writev => {
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
        n if n == numbers.exit || n == numbers.exit_group => {
            stop(caller, Stopped::Finished(a0 as i32));
            raise_stop_flag(caller);
            0
        }
        // brk: this stand-in never moves the break, and answers with the current one as Linux does when it
        // cannot.
        n if n == numbers.brk => caller.data().program_break as i64,
        number => {
            if let Some(set) = &numbers.set_base
                && number == set.number
                && let Some(&(_, at)) = set.bases.iter().find(|&&(code, _)| code == a0)
            {
                if a1 >= set.limit {
                    return -EPERM;
                }
                let state = caller.data().state.clone().unwrap();
                write_shared(&state, BLOCK_AT as usize + at, &a1.to_le_bytes());
                return 0;
            }
            let reason = Reason::SystemCallNotImplemented.number();
            slow_path(caller.data_mut(), reason, number, Duration::ZERO);
            -ENOSYS
        }
    }
}
