//! A small wasmtime runner: one guest thread, its memory, the hooks, and a stand-in for the kernel that
//! answers `write`, `writev`, `exit`, `exit_group` and `brk`.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Mutex, OnceLock};
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
use wcontainers_translator::{LookAhead, translate};
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
    /// No code is known for this address.
    NoCode(u64),
    /// The program called `exit` or `exit_group` with this status.
    Finished(i32),
}

/// One blob placed in guest memory and translated.
struct Blob {
    bytes: Vec<u8>,
    code: Range<u64>,
    load_address: u64,
}

pub struct Host {
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
    blobs: Vec<Blob>,
    state: Option<SharedMemory>,
    save_state: Option<TypedFunc<(), ()>>,
    /// The program break the stand-in reports: the page after the highest blob.
    program_break: u64,
    engine: Engine,
    thread: Option<Instance>,
    memory: Option<Memory>,
    table_base: u64,
}

pub struct Machine {
    store: Store<Host>,
    memory: Memory,
    state: SharedMemory,
    run: TypedFunc<(), ()>,
    load_state: TypedFunc<(), ()>,
    permission_pages: u64,
    permissions: Vec<Permissions>,
}

/// One engine for the whole process, so compiled modules can be shared.
fn engine() -> Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(new_engine).clone()
}

/// The thread module for a table size, compiled once.
fn thread_module(engine: &Engine, slots: u32) -> Module {
    static MODULES: OnceLock<Mutex<HashMap<u32, Module>>> = OnceLock::new();
    let mut modules = MODULES.get_or_init(Default::default).lock().unwrap();
    modules
        .entry(slots)
        .or_insert_with(|| {
            Module::new(engine, wcontainers_translator::thread_module(slots))
                .expect("thread module")
        })
        .clone()
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
        .shared_memory(true);
    Engine::new(&config).expect("engine")
}

/// How long the engine takes to compile a module.
pub fn compile_time(wasm: &[u8]) -> std::time::Duration {
    let engine = engine();
    let start = std::time::Instant::now();
    Module::new(&engine, wasm).expect("a valid module");
    start.elapsed()
}

impl Machine {
    /// A machine with `memory_bytes` of guest memory, nothing allowed anywhere, and a lookup table of
    /// `slots` entries starting at guest address `table_base`.
    pub fn new(memory_bytes: u64, table_base: u64, slots: u32, syscalls: Syscalls) -> Self {
        let engine = engine();
        let permission_pages = memory_bytes.div_ceil(PAGE_SIZE);
        let mut store = Store::new(
            &engine,
            Host {
                syscalls,
                stopped: None,
                snapshot: RegisterBlock::ZERO,
                stdout: Vec::new(),
                stderr: Vec::new(),
                counters: HashMap::new(),
                places: HashMap::new(),
                warnings: Vec::new(),
                blobs: Vec::new(),
                state: None,
                save_state: None,
                program_break: 0,
                engine: engine.clone(),
                thread: None,
                memory: None,
                table_base,
            },
        );
        let pages = memory_bytes.div_ceil(65536);
        let memory =
            Memory::new(&mut store, MemoryType::new64(pages, Some(pages))).expect("memory");
        let state_pages = (u64::from(PERMISSIONS_AT) + permission_pages).div_ceil(65536) as u32;
        let state = SharedMemory::new(&engine, MemoryType::shared(state_pages, state_pages))
            .expect("state memory");

        let thread_module = thread_module(&engine, slots);
        let mut linker = Linker::new(&engine);
        hooks(&mut linker);
        linker
            .define(&store, contract::ENV, contract::STATE, state.clone())
            .unwrap();
        let constant = |store: &mut Store<Host>, value: Val| {
            let ty = value.ty(&*store).unwrap();
            let ty = wasmtime::GlobalType::new(ty, wasmtime::Mutability::Const);
            Global::new(&mut *store, ty, value).unwrap()
        };
        let table_base_global = constant(&mut store, Val::I64(table_base as i64));
        let block_global = constant(&mut store, Val::I32(BLOCK_AT as i32));
        linker
            .define(
                &store,
                contract::ENV,
                contract::TABLE_BASE,
                table_base_global,
            )
            .unwrap();
        linker
            .define(&store, contract::ENV, contract::THREAD_BLOCK, block_global)
            .unwrap();
        let thread = linker
            .instantiate(&mut store, &thread_module)
            .expect("thread instance");

        let run = thread.get_typed_func(&mut store, contract::RUN).unwrap();
        let save_state = thread
            .get_typed_func(&mut store, contract::SAVE_STATE)
            .unwrap();
        let load_state = thread
            .get_typed_func(&mut store, contract::LOAD_STATE)
            .unwrap();
        let host = store.data_mut();
        host.state = Some(state.clone());
        host.save_state = Some(save_state);
        host.thread = Some(thread);
        host.memory = Some(memory);
        Machine {
            store,
            memory,
            state,
            run,
            load_state,
            permission_pages,
            permissions: vec![Permissions::empty(); permission_pages as usize],
        }
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
        let pages = range.start / PAGE_SIZE..range.end.div_ceil(PAGE_SIZE);
        for page in pages.clone() {
            self.permissions[page as usize] = permissions;
        }
        // Each byte also says what the page after it allows, so rewrite the one before the range too.
        for page in pages.start.saturating_sub(1)..pages.end.min(self.permission_pages) {
            let next = self
                .permissions
                .get(page as usize + 1)
                .copied()
                .unwrap_or(Permissions::empty());
            let byte = contract::permission_byte(self.permissions[page as usize], next);
            write_shared(
                &self.state,
                (u64::from(PERMISSIONS_AT) + page) as usize,
                &[byte],
            );
        }
    }

    /// Place a blob at `load_address`, translate it from `starts` (blob offsets) and add its pieces.
    pub fn load(&mut self, bytes: &[u8], code: Range<u64>, load_address: u64, starts: &[u64]) {
        self.write(load_address, bytes);
        let host = self.store.data_mut();
        host.blobs.push(Blob {
            bytes: bytes.to_vec(),
            code,
            load_address,
        });
        let end = (load_address + bytes.len() as u64).next_multiple_of(PAGE_SIZE);
        host.program_break = host.program_break.max(end);
        let index = host.blobs.len() - 1;
        instantiate_code(&mut self.store, index, starts);
    }

    pub fn set_registers(&mut self, registers: &RegisterBlock) {
        write_shared(&self.state, BLOCK_AT as usize, registers.as_bytes());
        self.load_state.call(&mut self.store, ()).unwrap();
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
        match self.run.call(&mut self.store, ()) {
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

fn snapshot(store: &mut impl wasmtime::AsContextMut<Data = Host>) -> RegisterBlock {
    let save = store.as_context().data().save_state.clone().unwrap();
    save.call(&mut *store, ()).unwrap();
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
                stand_in(&mut caller, call)
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
            if caller.data().stopped.is_none() {
                stop(&mut caller, Stopped::Exit(Exit::Stop));
            }
            let pc = snapshot(&mut caller).pc;
            slow_path(caller.data_mut(), Reason::StoppedByFlag, pc, Duration::ZERO);
            1
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
            |mut caller: Caller<'_, Host>, address: i64| -> i32 {
                let address = address as u64;
                let found = caller.data().blobs.iter().position(|blob| {
                    let start = blob.load_address + blob.code.start;
                    let end = blob.load_address + blob.code.end;
                    (start..end).contains(&address) && address.is_multiple_of(4)
                });
                match found {
                    Some(index) => {
                        let start = Instant::now();
                        let offset = address - caller.data().blobs[index].load_address;
                        instantiate_code(&mut caller, index, &[offset]);
                        let time = start.elapsed();
                        slow_path(
                            caller.data_mut(),
                            Reason::TranslatedAtRunTime,
                            address,
                            time,
                        );
                        1
                    }
                    None => {
                        stop(&mut caller, Stopped::NoCode(address));
                        0
                    }
                }
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

/// Translate blob `index` from `starts` and instantiate it, adding its pieces to the lookup table.
fn instantiate_code(
    store: &mut impl wasmtime::AsContextMut<Data = Host>,
    index: usize,
    starts: &[u64],
) {
    let mut store = store.as_context_mut();
    let host = store.data();
    let blob = &host.blobs[index];
    let translation =
        translate(&blob.bytes, blob.code.clone(), starts, LookAhead::None).expect("translate");
    let module = Module::new(&host.engine, &translation.wasm).expect("code module");
    let thread = host.thread.unwrap();
    let memory = host.memory.unwrap();
    let state = host.state.clone().unwrap();
    let slot_base = (blob.load_address - host.table_base) / 4;
    let load_address = blob.load_address;

    let mut linker = Linker::new(store.engine());
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
        (contract::SLOT_BASE, Val::I32(slot_base as i32)),
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
    linker
        .instantiate(&mut store, &module)
        .expect("code instance");
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
    let [a0, a1, a2, ..] = call.arguments;
    let memory = caller.data().memory.unwrap();
    let read = |caller: &mut Caller<'_, Host>, address: u64, length: u64| -> Option<Vec<u8>> {
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
            let Some(vectors) = a2
                .checked_mul(16)
                .and_then(|length| read(caller, a1, length))
            else {
                return -EFAULT;
            };
            let mut gathered = Vec::new();
            for vector in vectors.chunks(16) {
                let base = u64::from_le_bytes(vector[..8].try_into().unwrap());
                let length = u64::from_le_bytes(vector[8..].try_into().unwrap());
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
