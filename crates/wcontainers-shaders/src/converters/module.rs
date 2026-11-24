//! The converters' WebAssembly module under wasmtime: compiled once per process, instantiated afresh
//! for every call, so a shader that breaks the converter's memory reaches no other shader and every
//! call starts from the same state. Its run is bounded in memory and in instructions (wasmtime's
//! fuel), both deterministic, so the same shader is refused the same way on every machine. Instances
//! come from wasmtime's pool, whose memories keep their first pages mapped between instances, so a
//! fresh instance costs no page faults where the converters work.

use super::{BUFFER_SLOTS, Entry, Failure, SUCCEEDED, exports, no_reason, slot};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use std::sync::OnceLock;
use wasmtime::{
    Caller, Config, Engine, Export, Instance, InstanceAllocationStrategy, InstancePre, Linker,
    Memory, Module, PoolingAllocationConfig, ResourceLimiter, Store, Trap, TypedFunc, Val,
};

/// The module, built by `build.rs` from the converters' sources.
const MODULE: &[u8] = include_bytes!("../../converters.wasm");

/// The most memory one shader's instance may grow to: 26 times the most any shader of the corpora
/// reaches (9.9 MB).
// standalone 256: the bound in MiB, chosen as above
const MEMORY_BOUND: usize = 256 << 20;

/// The first bytes of a pooled memory kept mapped and zeroed between instances: the module's stack,
/// its data and the heap a typical conversion reaches.
const KEPT_RESIDENT: usize = 4 << 20;

/// The most instructions (wasmtime's fuel units) one call may run: twice what the slowest shader of
/// the corpora needs (5.9e11, a game's DXBC shader that runs for a minute), and 900 times what any
/// other needs.
const FUEL_BOUND: u64 = 1 << 40;

/// The most of what the module writes to its standard error that a refusal keeps.
const KEPT_ERROR_BYTES: usize = 4096;

/// The WASI module the C library imports from.
const WASI: &str = "wasi_snapshot_preview1";

/// The reactor's initialiser (the C++ static constructors), run once per instance.
const INITIALIZE: &str = "_initialize";

/// The file descriptor of standard error.
const STANDARD_ERROR: i32 = 2;

/// WASI's errno values the host gives
/// ([`errno`](https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md#-errno-variant)):
/// success, `badf` for every descriptor but standard error (the module has no files), and `nosys`
/// for any other call.
const SUCCESS: i32 = 0;
const BAD_DESCRIPTOR: i32 = 8;
const NO_SYSTEM_CALL: i32 = 52;

/// The prefix of WASI's calls on a file descriptor.
const DESCRIPTOR_CALL: &str = "fd_";

/// The engine and the module, compiled once.
struct Loaded {
    engine: Engine,
    module: InstancePre<State>,
}

/// What one call's store holds: its memory bound's state and the standard error it wrote.
#[derive(Default)]
struct State {
    /// Whether the module asked to grow past `MEMORY_BOUND`.
    past_memory_bound: bool,
    /// The instance's memory, once it exists.
    memory: Option<Memory>,
    error: Vec<u8>,
}

impl ResourceLimiter for State {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.past_memory_bound |= desired > MEMORY_BOUND;
        Ok(!self.past_memory_bound)
    }

    /// The module's one table is its function table, made at its declared size, which the C++ never
    /// grows.
    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(true)
    }
}

fn loaded() -> &'static Loaded {
    static LOADED: OnceLock<Loaded> = OnceLock::new();
    LOADED.get_or_init(|| {
        let mut config = Config::new();
        let mut pool = PoolingAllocationConfig::default();
        pool.max_memory_size(MEMORY_BOUND)
            .linear_memory_keep_resident(KEPT_RESIDENT);
        config
            .consume_fuel(true)
            .allocation_strategy(InstanceAllocationStrategy::Pooling(pool));
        let engine = Engine::new(&config).expect("wasmtime runs on this host");
        let module = Module::new(&engine, MODULE).expect("the converters' module compiles");
        let mut linker = Linker::new(&engine);
        for import in module.imports() {
            let ty = import.ty();
            let Some(ty) = ty.func() else { continue };
            let (wasi, name) = (import.module(), String::from(import.name()));
            assert_eq!(wasi, WASI, "the converters' module imports only WASI");
            linker
                .func_new(
                    wasi,
                    &name.clone(),
                    ty.clone(),
                    move |caller, params, results| system_call(caller, &name, params, results),
                )
                .expect("one definition per import");
        }
        let module = linker
            .instantiate_pre(&module)
            .expect("every import of the converters' module is defined");
        Loaded { engine, module }
    })
}

/// The C library's calls into WASI: what it writes to standard error is kept, no other descriptor
/// exists, the environment and arguments are empty, exit ends the run, and anything else is refused
/// as `nosys`.
fn system_call(
    mut caller: Caller<'_, State>,
    name: &str,
    params: &[Val],
    results: &mut [Val],
) -> wasmtime::Result<()> {
    let arg = |i: usize| params.get(i).and_then(Val::i32).unwrap_or_default();
    let status = match name {
        wasi::WRITE if arg(wasi::write::DESCRIPTOR) == STANDARD_ERROR => {
            let memory = memory_of(&caller)?;
            let mut written = 0u32;
            let data = memory.data(&caller);
            let mut kept = Vec::new();
            for vector in 0..arg(wasi::write::VECTOR_COUNT) {
                let at = address(arg(wasi::write::VECTORS))
                    + address(vector) * wasi::write::VECTOR_BYTES;
                let (base, len) = (word(data, at)?, word(data, at + size_of::<u32>())?);
                let bytes = data
                    .get(address(base as i32)..address(base as i32) + len as usize)
                    .ok_or_else(|| wasmtime::Error::msg("a write outside the module's memory"))?;
                kept.extend_from_slice(bytes);
                written = written.wrapping_add(len);
            }
            let state = caller.data_mut();
            let room = KEPT_ERROR_BYTES.saturating_sub(state.error.len());
            state.error.extend(kept.into_iter().take(room));
            let at = address(arg(wasi::write::WRITTEN));
            memory.write(&mut caller, at, &written.to_le_bytes())?;
            SUCCESS
        }
        wasi::ENVIRONMENT_SIZES | wasi::ARGUMENT_SIZES => {
            let memory = memory_of(&caller)?;
            for at in wasi::SIZES.map(arg) {
                memory.write(&mut caller, address(at), &0u32.to_le_bytes())?;
            }
            SUCCESS
        }
        wasi::ENVIRONMENT | wasi::ARGUMENTS => SUCCESS,
        wasi::EXIT => return Err(wasmtime::Error::msg(format!("exit {}", arg(0)))),
        name if name.starts_with(DESCRIPTOR_CALL) => BAD_DESCRIPTOR,
        _ => NO_SYSTEM_CALL,
    };
    if let Some(result) = results.first_mut() {
        *result = Val::I32(status);
    }
    Ok(())
}

/// The calls of WASI's
/// [first preview](https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md#-wasi_snapshot_preview1)
/// the host answers by name, and the parameters it reads.
mod wasi {
    pub const WRITE: &str = "fd_write";
    pub const ENVIRONMENT_SIZES: &str = "environ_sizes_get";
    pub const ARGUMENT_SIZES: &str = "args_sizes_get";
    pub const ENVIRONMENT: &str = "environ_get";
    pub const ARGUMENTS: &str = "args_get";
    pub const EXIT: &str = "proc_exit";
    /// The two counts' addresses a size call writes.
    pub const SIZES: [usize; 2] = [0, 1];

    /// `fd_write(fd, iovs, iovs_len, nwritten)`.
    pub mod write {
        pub const DESCRIPTOR: usize = 0;
        pub const VECTORS: usize = 1;
        pub const VECTOR_COUNT: usize = 2;
        pub const WRITTEN: usize = 3;
        /// An `iovec`: its base and its length, a word each.
        pub const VECTOR_BYTES: usize = 2 * size_of::<u32>();
    }
}

/// The memory of the instance the caller runs in.
fn memory_of(caller: &Caller<'_, State>) -> wasmtime::Result<Memory> {
    caller
        .data()
        .memory
        .ok_or_else(|| wasmtime::Error::msg("the module exports no memory"))
}

/// A 32-bit address of the module as an index of its memory.
fn address(value: i32) -> usize {
    value as u32 as usize
}

/// The little-endian word at `at` of `data`.
fn word(data: &[u8], at: usize) -> wasmtime::Result<u32> {
    data.get(at..at + size_of::<u32>())
        .and_then(|w| w.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| wasmtime::Error::msg("a read outside the module's memory"))
}

/// One shader's instance, kept for the calls that follow on the same shader (a DXIL conversion
/// makes several), so every shader starts from a fresh instance and pays for one.
struct Session {
    store: Store<State>,
    instance: Instance,
    memory: Memory,
    free: TypedFunc<i32, ()>,
    /// The shader in the module's memory, its length, and the slots the boundary fills.
    input: i32,
    length: i32,
    slots: i32,
    shader: Vec<u8>,
}

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

pub(super) fn call<const BUFFERS: usize>(
    entry: &Entry<BUFFERS>,
    data: &[u8],
    samples: u32,
) -> Result<[Vec<u8>; BUFFERS], Failure> {
    SESSION.with_borrow_mut(|session| {
        if session.as_ref().is_none_or(|s| s.shader != data) {
            *session = None;
            let loaded = loaded();
            let mut store = Store::new(&loaded.engine, State::default());
            store.limiter(|state| state);
            store
                .set_fuel(FUEL_BOUND)
                .expect("fuel is on in the engine");
            match start(store, &loaded.module, data) {
                Ok(started) => *session = Some(started),
                Err((store, e)) => return Err(Failure::Ended(ended(entry, &store, &e))),
            }
        }
        let started = session.as_mut().expect("a session for this shader");
        let state = started.store.data_mut();
        state.past_memory_bound = false;
        state.error.clear();
        started
            .store
            .set_fuel(FUEL_BOUND)
            .expect("fuel is on in the engine");
        match run(started, entry, samples) {
            Ok(outputs) => Ok(outputs),
            Err(Run::Refused(reason)) => Err(Failure::Refused(reason)),
            Err(Run::Ended(e)) => {
                let reason = ended(entry, &started.store, &e);
                *session = None;
                Err(Failure::Ended(reason))
            }
        }
    })
}

/// A fresh instance holding `data`, or the store and why it ended.
fn start(
    mut store: Store<State>,
    module: &InstancePre<State>,
    data: &[u8],
) -> Result<Session, (Store<State>, wasmtime::Error)> {
    let parts = (|| {
        let instance: Instance = module.instantiate(&mut store)?;
        if let Ok(initialize) = instance.get_typed_func::<(), ()>(&mut store, INITIALIZE) {
            initialize.call(&mut store, ())?;
        }
        let memory = instance
            .exports(&mut store)
            .find_map(Export::into_memory)
            .ok_or_else(|| wasmtime::Error::msg("the module exports no memory"))?;
        store.data_mut().memory = Some(memory);
        let allocate = instance.get_typed_func::<i32, i32>(&mut store, exports::ALLOCATE)?;
        let free = instance.get_typed_func::<i32, ()>(&mut store, exports::FREE)?;
        let length = i32::try_from(data.len())
            .map_err(|_| wasmtime::Error::msg("a shader larger than the module's memory"))?;
        let input = allocate.call(&mut store, length)?;
        let slots = allocate.call(&mut store, (slot::COUNT * size_of::<u32>()) as i32)?;
        if input == 0 || slots == 0 {
            return Err(wasmtime::Error::msg("the module's allocator gave nothing"));
        }
        memory
            .write(&mut store, address(input), data)
            .map_err(wasmtime::Error::from)?;
        Ok((instance, memory, free, input, length, slots))
    })();
    match parts {
        Ok((instance, memory, free, input, length, slots)) => Ok(Session {
            store,
            instance,
            memory,
            free,
            input,
            length,
            slots,
            shader: data.to_vec(),
        }),
        Err(e) => Err((store, e)),
    }
}

/// Why a run gave nothing, before it is described.
enum Run {
    Refused(String),
    Ended(wasmtime::Error),
}

impl From<wasmtime::Error> for Run {
    fn from(e: wasmtime::Error) -> Self {
        Run::Ended(e)
    }
}

/// The refusal of a run that ended: by its bound where it reached one, otherwise the trap, with what
/// the converter wrote to standard error.
fn ended<const BUFFERS: usize>(
    entry: &Entry<BUFFERS>,
    store: &Store<State>,
    e: &wasmtime::Error,
) -> String {
    let converter = entry.converter;
    let state = store.data();
    let what = if state.past_memory_bound {
        format!("ran past its memory bound of {MEMORY_BOUND} bytes")
    } else if e.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
        format!("ran past its bound of {FUEL_BOUND} instructions")
    } else {
        match e.downcast_ref::<Trap>() {
            Some(trap) => format!("stopped: {trap}"),
            None => format!("stopped: {e}"),
        }
    };
    let error = String::from_utf8_lossy(&state.error);
    match error.trim() {
        "" => format!("{converter} {what}"),
        error => format!("{converter} {what}: {error}"),
    }
}

/// Calls `entry` on the session's shader, copies out what it gave and frees that in the module.
fn run<const BUFFERS: usize>(
    session: &mut Session,
    entry: &Entry<BUFFERS>,
    samples: u32,
) -> Result<[Vec<u8>; BUFFERS], Run> {
    let Session {
        store,
        instance,
        memory,
        free,
        input,
        length,
        slots,
        ..
    } = session;
    let function = instance.get_typed_func::<(i32, i32, i32, i32), i32>(&mut *store, entry.name)?;
    let status = function.call(&mut *store, (*input, *length, samples as i32, *slots))?;
    let data = memory.data(&*store);
    let at = |slot: usize| word(data, address(*slots) + slot * size_of::<u32>());
    let given = [slot::FIRST, slot::SECOND, slot::REASON].map(&at);
    let result = if status != SUCCEEDED {
        let (reason, length) = (address(at(slot::REASON)? as i32), at(slot::REASON_LENGTH)?);
        Err(Run::Refused(match reason {
            0 => no_reason(entry),
            _ => {
                let bytes = data
                    .get(reason..reason.saturating_add(length as usize))
                    .ok_or_else(|| wasmtime::Error::msg("a reason outside the module's memory"))?;
                String::from_utf8_lossy(bytes).into_owned()
            }
        }))
    } else {
        let mut outputs = core::array::from_fn(|_| Vec::new());
        for ((output, unit), [buffer, count]) in
            outputs.iter_mut().zip(entry.outputs).zip(BUFFER_SLOTS)
        {
            let (out, count) = (address(at(buffer)? as i32), at(count)? as usize);
            let bytes = count
                .checked_mul(unit.bytes())
                .and_then(|n| data.get(out..out.checked_add(n)?))
                .ok_or_else(|| wasmtime::Error::msg("an output outside the module's memory"))?;
            *output = bytes.to_vec();
        }
        Ok(outputs)
    };
    for address in given {
        match address? {
            0 => {}
            address => free.call(&mut *store, address as i32)?,
        }
    }
    result
}
