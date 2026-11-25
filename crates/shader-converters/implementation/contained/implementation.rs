//! The converters' WebAssembly module under wasmtime: compiled once per process, instantiated afresh
//! for every conversion, so a shader that breaks the converter's memory reaches no other shader and
//! every conversion starts from the same state. Its run is bounded in memory and in instructions
//! (wasmtime's fuel), both deterministic, so the same shader is refused the same way on every
//! machine. Instances come from wasmtime's pool, whose memories keep their first pages mapped between
//! instances, so a fresh instance costs no page faults where the converters work; while the pool has
//! none free, a conversion waits for one.

use core::cell::{Cell, RefCell};
use std::sync::{Condvar, Mutex, OnceLock};

use wasmtime::{
    Caller, Config, Engine, Export, Instance, InstanceAllocationStrategy, InstancePre, Linker,
    Memory, Module, PoolConcurrencyLimitError, PoolingAllocationConfig, ResourceLimiter, Store,
    Trap, TypedFunc, Val,
};
use wcontainers_shader_converters::{
    ALLOCATE, BUFFER_SLOTS, Buffers, CONVERTER_NAMES, ELEMENT_SIZES, EXPORT_NAMES, FREE, Failure,
    Function, NO_BUFFER, SLOT_COUNT, SLOT_FIRST, SLOT_REASON, SLOT_REASON_LENGTH, SLOT_SECOND,
    SUCCEEDED, ShaderConverters, StandaloneShaderConverters,
};

/// The module, built by the linked implementation's `build.rs` from the converters' sources.
const MODULE: &[u8] = include_bytes!("converters.wasm");

/// The most memory one shader's instance may grow to, 256 MiB: 26 times the most any shader of the
/// corpora reaches (9.9 MB).
const MEMORY_BOUND: usize = 1 << 28;

/// The first bytes of a pooled memory kept mapped and zeroed between instances: the module's stack,
/// its data and the heap a typical conversion reaches.
const KEPT_RESIDENT: usize = 4 << 20;

/// The most instructions (wasmtime's fuel units) one conversion may run, over all its calls: twice
/// what the slowest shader of the corpora needs (5.9e11, a game's DXBC shader that runs for a
/// minute), and 900 times what any other needs.
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
        // As many instances at once as the host runs threads: a conversion is bound by the
        // processor, so more gain nothing, and the converters' memory together stays within that
        // many bounds. A conversion finding every instance taken waits for one.
        let instances = std::thread::available_parallelism()
            .map_or(1, |n| u32::try_from(n.get()).unwrap_or(u32::MAX));
        let mut pool = PoolingAllocationConfig::default();
        pool.max_memory_size(MEMORY_BOUND)
            .linear_memory_keep_resident(KEPT_RESIDENT)
            .total_core_instances(instances)
            .total_memories(instances)
            .total_tables(instances);
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

/// One conversion's instance, kept for the calls that follow on the same shader (a DXIL conversion
/// makes several) until the conversion ends, so every conversion starts from a fresh instance and
/// pays for one.
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
    /// Declared last, so it tells waiting conversions after the instance went back to the pool.
    _returned: Returned,
}

/// How many instances went back to the pool, for conversions waiting on a full pool.
static RETURNED: (Mutex<u64>, Condvar) = (Mutex::new(0), Condvar::new());

/// Counts its instance back into the pool when dropped.
struct Returned;

impl Drop for Returned {
    fn drop(&mut self) {
        returned();
    }
}

/// Tells a conversion waiting on a full pool that an instance went back.
fn returned() {
    let (count, returned) = &RETURNED;
    *count.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    returned.notify_one();
}

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    /// How many `Conversion`s are open on this thread.
    static OPEN: Cell<u32> = const { Cell::new(0) };
}

/// The converters run as their own module, one instance a conversion: while values live on a thread, its calls on
/// the same shader share one instance, which goes back to the pool when the last of them is dropped.
pub struct ContainedShaderConverters(());

impl StandaloneShaderConverters for ContainedShaderConverters {
    type Config = ();

    fn new(_config: ()) -> ContainedShaderConverters {
        OPEN.set(OPEN.get() + 1);
        ContainedShaderConverters(())
    }
}

/// Ends the outermost conversion's session, giving its instance back to the pool.
impl Drop for ContainedShaderConverters {
    fn drop(&mut self) {
        OPEN.set(OPEN.get() - 1);
        if OPEN.get() == 0 {
            SESSION.take();
        }
    }
}

impl ShaderConverters for ContainedShaderConverters {
    fn call(&self, function: Function, bytes: &[u8], flags: u32) -> Result<Buffers, Failure> {
        call(function, bytes, flags)
    }
}

/// Runs `function` on `data` in the session of this thread's conversion, started afresh for another shader.
fn call(entry: Function, data: &[u8], samples: u32) -> Result<Buffers, Failure> {
    SESSION.with_borrow_mut(|session| {
        if session.as_ref().is_none_or(|s| s.shader != data) {
            *session = None;
            *session = Some(started(entry, data)?);
        }
        let started = session.as_mut().expect("a session for this shader");
        let state = started.store.data_mut();
        state.past_memory_bound = false;
        state.error.clear();
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

/// A fresh session holding `data`, once the pool has an instance free; or why it ended.
fn started(entry: Function, data: &[u8]) -> Result<Session, Failure> {
    let loaded = loaded();
    loop {
        let seen = *RETURNED.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut store = Store::new(&loaded.engine, State::default());
        store.limiter(|state| state);
        store
            .set_fuel(FUEL_BOUND)
            .expect("fuel is on in the engine");
        match start(store, &loaded.module, data) {
            Err((_, e)) if e.downcast_ref::<PoolConcurrencyLimitError>().is_some() => {
                let (count, returned) = &RETURNED;
                let count = count.lock().unwrap_or_else(|e| e.into_inner());
                drop(
                    returned
                        .wait_while(count, |count| *count == seen)
                        .unwrap_or_else(|e| e.into_inner()),
                );
            }
            Err((store, e)) => {
                let reason = ended(entry, &store, &e);
                drop(store);
                returned();
                return Err(Failure::Ended(reason));
            }
            Ok(started) => return Ok(started),
        }
    }
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
        let allocate = instance.get_typed_func::<i32, i32>(&mut store, ALLOCATE)?;
        let free = instance.get_typed_func::<i32, ()>(&mut store, FREE)?;
        let length = i32::try_from(data.len())
            .map_err(|_| wasmtime::Error::msg("a shader larger than the module's memory"))?;
        let input = allocate.call(&mut store, length)?;
        let slots = allocate.call(&mut store, (SLOT_COUNT * size_of::<u32>()) as i32)?;
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
            _returned: Returned,
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
fn ended(entry: Function, store: &Store<State>, e: &wasmtime::Error) -> String {
    let converter = CONVERTER_NAMES[entry as usize];
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
fn run(session: &mut Session, entry: Function, samples: u32) -> Result<Buffers, Run> {
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
    let function = instance
        .get_typed_func::<(i32, i32, i32, i32), i32>(&mut *store, EXPORT_NAMES[entry as usize])?;
    let status = function.call(&mut *store, (*input, *length, samples as i32, *slots))?;
    let data = memory.data(&*store);
    let at = |slot: usize| word(data, address(*slots) + slot * size_of::<u32>());
    let given = [SLOT_FIRST, SLOT_SECOND, SLOT_REASON].map(&at);
    let result = if status != SUCCEEDED {
        let (reason, length) = (address(at(SLOT_REASON)? as i32), at(SLOT_REASON_LENGTH)?);
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
        let sizes = ELEMENT_SIZES[entry as usize]
            .into_iter()
            .take_while(|size| *size != NO_BUFFER);
        for ((output, size), [buffer, count]) in outputs.iter_mut().zip(sizes).zip(BUFFER_SLOTS) {
            let (out, count) = (address(at(buffer)? as i32), at(count)? as usize);
            let bytes = count
                .checked_mul(size)
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

/// Returns the reason given when the converter set none.
fn no_reason(function: Function) -> String {
    format!("{} gave no reason", CONVERTER_NAMES[function as usize])
}
