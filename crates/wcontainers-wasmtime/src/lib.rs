//! The WebAssembly back end of the runtime: the translator's output for a guest `G`, run by wasmtime,
//! behind the kernel's `Cpu` side. One store per process; a thread's instances are made at its first
//! run; code is translated at first use and kept in a cache shared by every back end of a [`Compiler`].

mod memory;
pub mod starts;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::convert::Infallible;
use std::mem::MaybeUninit;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use wasmtime::{
    AsContextMut, Caller, Config, Engine, Extern, Func, FuncType, Global, GlobalType, Instance,
    Memory, MemoryType, Module, Mutability, Ref, SharedMemory, Store, StoreLimits,
    StoreLimitsBuilder, Table, Trap, TypedFunc, Val, ValRaw,
};
use wcontainers_abi::{
    Access, AddressSpace, CounterReading, Cpu, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions,
    RegisterBlock, Resume, SharedId, Syscall, ThreadId, Warning,
};
use wcontainers_guest_cpu::{Guest, HookBody, MemoryFault};
use wcontainers_runtime::{Answer, Carry, Execution, Leaving, Turn};
use wcontainers_translator::contract::{self, Reason, TrapKind};
use wcontainers_translator::translate;
use zerocopy::{FromBytes, IntoBytes};

pub use memory::Layout;
pub use starts::Sources;
pub use wcontainers_translator::{LookAhead, Optimise};

/// The back end's own slow paths, besides the translator's [`Reason`]s.
pub mod reason {
    /// Progress is counted by stop checks passed until the translator emits the guest's own count.
    pub const PROGRESS_BY_STOP_CHECKS: &str = "progress counted by stop checks";
    /// The guest's first write to a page that shows a block of shared pages since its copies were made.
    pub const FIRST_WRITE_TO_SHARED_PAGE: &str = "first write to a shared page";
    /// Code of a range already translated was announced changed, so it is translated at run time only.
    pub const CODE_CHANGED_AFTER_IT_RAN: &str = "code changed after it ran";
    /// Code ran outside every range the kernel announced.
    pub const CODE_OUTSIDE_ANNOUNCED_RANGE: &str = "code outside an announced range";
    /// The thread left `run` because code it may be running was dropped.
    pub const LEFT_CODE_CHANGED_UNDER_IT: &str = "left run: code changed under it";
    /// Leaving `run` from a hook, the slow twin of answering in it, by why. The detail is the call
    /// number, or [`NOT_A_CALL`] after a stop.
    pub const LEFT_CALL_WAITS: &str = "left run: the call waits";
    pub const LEFT_ANOTHER_THREAD: &str = "left run: another thread runs";
    pub const LEFT_NEW_REGISTERS: &str = "left run: new registers";
    pub const LEFT_FINISHED: &str = "left run: the container finished";
    pub const LEFT_OUTSIDE_ASKED: &str = "left run: the outside asked";
    /// The detail of a leave after an exit that is not a system call.
    pub const NOT_A_CALL: u64 = u64::MAX;
}

/// What every back end of one engine shares: the engine, the thread module, the pool translation and
/// compiling run in, the probe's answer, the compiled translations kept in memory, and the directory
/// they are kept in between runs.
pub struct Compiler<G: Guest> {
    engine: Engine,
    thread_module: Module,
    pool: rayon::ThreadPool,
    host: G::Host,
    modules: Mutex<HashMap<Key, Arc<Compiled>>>,
    directory: Option<PathBuf>,
}

/// Where a translation a back end used came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Compiled modules kept in memory.
    Memory,
    /// The cache's directory, in the nanoseconds reading and loading took.
    Disk(u64),
    /// Made now, in the nanoseconds translating and compiling took.
    Made { translating: u64, compiling: u64 },
}

/// Names the build of the translator and its guests, made from their sources by the build script.
const TRANSLATOR_BUILD: &str = env!("WCONTAINERS_TRANSLATOR_BUILD");

/// The first bytes of a cache file.
const CACHE_MAGIC: &[u8; 8] = b"wcxlate1";

/// What a compiled translation is kept under.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    hash: [u8; 32],
    starts: Vec<u64>,
    look_ahead: LookAhead,
    optimise: Optimise,
    offer: u32,
}

/// A translation as wasmtime compiled it.
struct Compiled {
    modules: Vec<Module>,
    /// Each module's pieces, as blob offsets in slot order.
    pieces: Vec<Vec<u64>>,
    count: u32,
}

impl<G: Guest> Compiler<G> {
    /// A compiler keeping translations in memory, and in `directory` between runs when given one.
    /// Files there are loaded as compiled code, so the directory must be written only by compilers.
    pub fn new(directory: Option<PathBuf>) -> Arc<Self> {
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
            .memory_may_move(false)
            .with_host_memory(Arc::new(memory::HostMemories));
        // Unix signals, not Mach ports, so a copy of the process made by fork can still run guests.
        #[cfg(target_os = "macos")]
        config.macos_use_mach_ports(false);
        let engine = Engine::new(&config).expect("a wasmtime engine");
        let pool = rayon::ThreadPoolBuilder::new()
            .build()
            .expect("the back end's pool");
        let thread_module = pool
            .install(|| Module::new(&engine, wcontainers_translator::thread_module::<G>()))
            .expect("the thread module");
        let probe = Module::new(&engine, G::probe_module()).expect("the probe module");
        let mut store = Store::new(&engine, ());
        let probe = Instance::new(&mut store, &probe, &[]).expect("the probe instance");
        let probe = probe
            .get_typed_func::<(), i32>(&mut store, contract::PROBE)
            .expect("the probe export");
        let host = G::host_from_probe(probe.call(&mut store, ()).expect("the probe runs"));
        Arc::new(Self {
            engine,
            thread_module,
            pool,
            host,
            modules: Mutex::new(HashMap::new()),
            directory,
        })
    }

    /// The file a key's translation is kept in: every part of the key and everything else the
    /// compiled code depends on, hashed.
    fn file(&self, directory: &Path, key: &Key) -> PathBuf {
        let mut hasher = blake3::Hasher::new();
        hasher.update(TRANSLATOR_BUILD.as_bytes());
        // The back end, the guest, the host's answer, and the output's memory widths.
        hasher.update(b"wasmtime guest-memory-64 state-memory-32");
        hasher.update(&(G::ID as u32).to_le_bytes());
        hasher.update(&G::host_key(self.host).to_le_bytes());
        hasher.update(&key.offer.to_le_bytes());
        hasher.update(format!("{:?} {:?}", key.look_ahead, key.optimise).as_bytes());
        hasher.update(&key.hash);
        for start in &key.starts {
            hasher.update(&start.to_le_bytes());
        }
        directory.join(hasher.finalize().to_hex().as_str())
    }

    /// A translation from the cache's directory; none when it is not there or wasmtime refuses it.
    fn load(&self, file: &Path) -> Option<Compiled> {
        let bytes = std::fs::read(file).ok()?;
        let mut rest = bytes.strip_prefix(CACHE_MAGIC)?;
        let mut take = |length: usize| -> Option<&[u8]> {
            let (taken, after) = rest.split_at_checked(length)?;
            rest = after;
            Some(taken)
        };
        let word = |bytes: &[u8]| u64::from_le_bytes(bytes.try_into().expect("eight bytes"));
        let count = word(take(8)?);
        let (mut modules, mut pieces) = (Vec::new(), Vec::new());
        for _ in 0..count {
            let length = usize::try_from(word(take(8)?)).ok()?;
            let list: Vec<u64> = take(length.checked_mul(8)?)?
                .as_chunks::<8>()
                .0
                .iter()
                .map(|bytes| u64::from_le_bytes(*bytes))
                .collect();
            let length = usize::try_from(word(take(8)?)).ok()?;
            // SAFETY: the file was written by `store` from wasmtime's own serialisation, and
            // wasmtime checks it was made by a compatible engine and build.
            let module = unsafe { Module::deserialize(&self.engine, take(length)?) }.ok()?;
            modules.push(module);
            pieces.push(list);
        }
        Some(Compiled {
            modules,
            count: pieces.iter().map(|p| p.len() as u32).sum(),
            pieces,
        })
    }

    /// Keeps a translation in the cache's directory, written whole or not at all.
    fn store(&self, file: &Path, compiled: &Compiled) {
        let mut bytes = CACHE_MAGIC.to_vec();
        bytes.extend((compiled.modules.len() as u64).to_le_bytes());
        for (module, pieces) in compiled.modules.iter().zip(&compiled.pieces) {
            bytes.extend((pieces.len() as u64).to_le_bytes());
            bytes.extend(pieces.iter().flat_map(|p| p.to_le_bytes()));
            let serialised = module.serialize().expect("a compiled module serialises");
            bytes.extend((serialised.len() as u64).to_le_bytes());
            bytes.extend(serialised);
        }
        let partial = file.with_extension(format!("partial-{}", std::process::id()));
        let written = std::fs::create_dir_all(file.parent().expect("a directory"))
            .and_then(|()| std::fs::write(&partial, &bytes))
            .and_then(|()| std::fs::rename(&partial, file));
        if let Err(error) = written {
            panic!("keeping a translation in {}: {error}", file.display());
        }
    }

    /// The compiled translation of a blob from its starts, from the cache or made now, and where it
    /// came from.
    fn compiled(
        &self,
        bytes: &[u8],
        hash: [u8; 32],
        mut starts: Vec<u64>,
        look_ahead: LookAhead,
        optimise: Optimise,
        offer: G::Offer,
    ) -> (Arc<Compiled>, Origin) {
        starts.sort_unstable();
        starts.dedup();
        let key = Key {
            hash,
            starts,
            look_ahead,
            optimise,
            offer: G::offer_key(offer),
        };
        if let Some(compiled) = self.modules.lock().expect("the cache").get(&key) {
            return (compiled.clone(), Origin::Memory);
        }
        let file = self.directory.as_deref().map(|d| self.file(d, &key));
        let started = Instant::now();
        if let Some(compiled) = file.as_deref().and_then(|file| self.load(file)) {
            let compiled = Arc::new(compiled);
            let nanoseconds = started.elapsed().as_nanos() as u64;
            self.modules
                .lock()
                .expect("the cache")
                .insert(key, compiled.clone());
            return (compiled, Origin::Disk(nanoseconds));
        }
        let started = Instant::now();
        let length = bytes.len() as u64;
        let translation = self
            .pool
            .install(|| {
                translate::<G>(
                    bytes,
                    0..length,
                    &key.starts,
                    look_ahead,
                    optimise,
                    self.host,
                    offer,
                )
            })
            .unwrap_or_else(|error| panic!("translating {length} bytes: {error:?}"));
        let translated = started.elapsed().as_nanos() as u64;
        let started = Instant::now();
        let modules: Vec<Module> = self.pool.install(|| {
            translation
                .modules
                .iter()
                .map(|module| {
                    Module::new(&self.engine, &module.wasm)
                        .expect("wasmtime accepts what the translator makes")
                })
                .collect()
        });
        let compiled_in = started.elapsed().as_nanos() as u64;
        let pieces: Vec<Vec<u64>> = translation
            .modules
            .iter()
            .map(|m| m.pieces.clone())
            .collect();
        let compiled = Arc::new(Compiled {
            modules,
            count: pieces.iter().map(|p| p.len() as u32).sum(),
            pieces,
        });
        if let Some(file) = &file {
            self.store(file, &compiled);
        }
        self.modules
            .lock()
            .expect("the cache")
            .insert(key, compiled.clone());
        let origin = Origin::Made {
            translating: translated,
            compiling: compiled_in,
        };
        (compiled, origin)
    }
}

/// One translation a back end used: where, how, and what it cost.
#[derive(Clone, Debug)]
pub struct Translated {
    pub start: u64,
    pub length: u64,
    pub look_ahead: LookAhead,
    /// Made at run time from one address, rather than the whole range from its starts.
    pub at_run_time: bool,
    pub starts: usize,
    /// The starts the range's ELF image gave, by source; none for a run-time piece or no image.
    pub sources: Option<Sources>,
    pub pieces: u32,
    pub origin: Origin,
}

/// The WebAssembly back end for guest `G`.
pub struct CpuSide<G: Guest> {
    compiler: Arc<Compiler<G>>,
    look_ahead: LookAhead,
    optimise: Optimise,
    offer: G::Offer,
    layout: Layout,
    processes: HashMap<MemoryId, Process<G>>,
    threads: HashMap<ThreadId, Thread>,
    blocks: HashMap<SharedId, Vec<u8>>,
    /// The last handle given out, of any kind.
    last: u32,
    running: Option<Running>,
    counters: BTreeMap<&'static str, (u64, u64)>,
    /// The (reason, detail) pairs already warned of.
    warned: HashSet<(&'static str, u64)>,
    /// Warnings made while no thread ran, given to the outside at the next run.
    unsent: Vec<Warning<'static>>,
    translated: Vec<Translated>,
}

struct Running {
    memory: MemoryId,
    thread: ThreadId,
    /// Code of the running process was dropped while the thread was in a hook.
    code_dropped: bool,
}

/// One guest process: its store, its memories and what the back end keeps about its pages and code.
struct Process<G: Guest> {
    /// None while one of its threads runs.
    store: Option<Store<Data<G>>>,
    memory: Memory,
    base: *mut u8,
    state: SharedMemory,
    state_base: *mut u8,
    hooks: Vec<(&'static str, Func)>,
    /// What the kernel last said each page allows.
    permissions: Vec<Permissions>,
    /// The back end's write locks on each page, by reason bit.
    locks: Vec<u8>,
    /// Pages that may hold anything but zeros: given access or written since last zeroed.
    data: Pages,
    /// Ranges that show a block of shared pages, by start.
    shown: BTreeMap<u64, Shown>,
    /// Pages showing a block that the guest has written since their copies were last made.
    written: BTreeSet<u64>,
    ranges: Vec<CodeRange>,
    loads: Vec<Load>,
    next_slot: u32,
    free_blocks: Vec<u32>,
    next_block: u32,
    dropped: bool,
}

/// Write lock: the page shows a block of shared pages.
const LOCK_SHARED: u8 = 1;

/// One bit per page.
#[derive(Clone)]
struct Pages(Vec<u64>);

impl Pages {
    fn new(pages: u64) -> Self {
        Pages(vec![0; pages.div_ceil(64) as usize])
    }

    fn set(&mut self, pages: Range<u64>, on: bool) {
        for page in pages {
            let (word, bit) = ((page / 64) as usize, page % 64);
            if on {
                self.0[word] |= 1 << bit;
            } else {
                self.0[word] &= !(1 << bit);
            }
        }
    }

    /// The runs of pages that are set, in order.
    fn runs(&self) -> Vec<Range<u64>> {
        let mut runs: Vec<Range<u64>> = Vec::new();
        for (index, &word) in self.0.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let page = index as u64 * 64 + u64::from(bits.trailing_zeros());
                bits &= bits - 1;
                match runs.last_mut() {
                    Some(run) if run.end == page => run.end += 1,
                    _ => runs.push(page..page + 1),
                }
            }
        }
        runs
    }
}

#[derive(Clone, Copy)]
struct Shown {
    end: u64,
    block: SharedId,
    offset: u64,
}

/// The bytes of a range of code and their hash.
struct Code {
    start: u64,
    bytes: Vec<u8>,
    hash: [u8; 32],
}

/// A range the kernel announced as code.
#[derive(Clone)]
struct CodeRange {
    range: Range<u64>,
    starts: Vec<u64>,
    translated: bool,
    /// Its bytes changed after it was translated, so it is translated at run time only.
    changed: bool,
    /// The hash of its bytes when it was last translated whole, kept across notices of the same range.
    hash: Option<[u8; 32]>,
}

/// A compiled translation placed in a process's lookup tables.
#[derive(Clone)]
struct Load {
    compiled: Arc<Compiled>,
    load_address: u64,
    slot_base: u32,
    /// False once its code was dropped: new threads do not instantiate it.
    live: bool,
}

struct Thread {
    memory: MemoryId,
    block_at: u32,
    instance: Option<ThreadInstance>,
    /// By address, the piece of the one instruction there for the exact slot.
    exact: HashMap<u64, Func>,
    /// Registers the kernel set while the thread was in a hook, written once it has left.
    held: Option<RegisterBlock>,
    dropped: bool,
}

#[derive(Clone)]
struct ThreadInstance {
    instance: Instance,
    run: TypedFunc<(), ()>,
    table: Table,
}

/// What a process's store holds for its hooks while a thread runs.
struct Data<G: Guest> {
    side: *mut CpuSide<G>,
    answer: Option<NonNull<dyn Answer>>,
    memory: MemoryId,
    thread: ThreadId,
    block_at: u32,
    state_base: *mut u8,
    /// Progress in this turn, which its limit is compared with.
    progress: u64,
    /// The part of `progress` the outside was already told by a clock read.
    reported: u64,
    limit: Option<u64>,
    leaving: bool,
    /// Leave and run again at once for this turn, as if carrying on.
    rerun: Option<Turn>,
    /// An exit to hand on once the thread has left.
    pending: Option<Exit>,
    /// A write to a page under a write lock, to settle before the thread carries on.
    lock_fault: Option<u64>,
    limits: StoreLimits,
}

impl<G: Guest> Data<G> {
    /// Progress the outside has not been told of, for a clock read; the turn's count goes on.
    fn report(&mut self) -> u64 {
        let progress = self.progress - self.reported;
        self.reported = self.progress;
        progress
    }

    /// Progress the outside has not been told of, at an exit, which ends the turn's count.
    fn end_turn(&mut self) -> u64 {
        let progress = self.progress - self.reported;
        (self.progress, self.reported) = (0, 0);
        progress
    }
}

impl<G: Guest> CpuSide<G>
where
    G: Guest<Block = RegisterBlock>,
{
    /// A back end translating at `look_ahead` for `optimise`, with the CPU features `offer` claims.
    pub fn new(
        compiler: Arc<Compiler<G>>,
        look_ahead: LookAhead,
        optimise: Optimise,
        offer: G::Offer,
    ) -> Self {
        Self {
            compiler,
            look_ahead,
            optimise,
            offer,
            layout: Layout::of::<G>(),
            processes: HashMap::new(),
            threads: HashMap::new(),
            blocks: HashMap::new(),
            last: 0,
            running: None,
            counters: BTreeMap::new(),
            warned: HashSet::new(),
            unsent: Vec::new(),
            translated: Vec::new(),
        }
    }

    /// Every slow path taken so far: how often and the time spent.
    pub fn counters(&self) -> Vec<CounterReading> {
        self.counters
            .iter()
            .map(|(reason, (count, nanoseconds))| CounterReading {
                reason,
                detail: 0,
                count: *count,
                nanoseconds: *nanoseconds,
            })
            .collect()
    }

    /// Every translation used, in order.
    pub fn translated(&self) -> &[Translated] {
        &self.translated
    }

    fn fresh(&mut self) -> u32 {
        self.last += 1;
        self.last
    }

    /// Counts a slow path, and warns of it the first time at each detail.
    fn slow_path(&mut self, reason: &'static str, detail: u64, nanoseconds: u64) {
        let counter = self.counters.entry(reason).or_default();
        counter.0 += 1;
        counter.1 += nanoseconds;
        let count = counter.0;
        if self.warned.insert((reason, detail)) {
            self.unsent.push(Warning {
                reason,
                count,
                detail,
            });
        }
    }

    fn send_warnings(&mut self, answer: NonNull<dyn Answer>) {
        for warning in std::mem::take(&mut self.unsent) {
            // SAFETY: the answer of the run in progress, alive until `run` returns.
            unsafe { &mut *answer.as_ptr() }.warn(warning);
        }
    }

    fn process(&self, memory: MemoryId) -> &Process<G> {
        self.processes.get(&memory).expect("a memory that exists")
    }

    fn check(&self, address: u64, length: u64) -> Result<(), CpuError> {
        match address.checked_add(length) {
            Some(end) if end <= self.layout.ceiling => Ok(()),
            _ => Err(CpuError::BadAddress),
        }
    }

    /// Bytes of a thread's state. They are the state memory's, which is shared, not the back end's.
    #[allow(clippy::mut_from_ref)]
    fn block_bytes(&self, memory: MemoryId, block_at: u32, length: usize) -> &mut [u8] {
        let process = self.process(memory);
        // SAFETY: inside the state memory's thread blocks; no guest thread runs or is in a hook that
        // reads these bytes while the back end changes them.
        unsafe { std::slice::from_raw_parts_mut(process.state_base.add(block_at as usize), length) }
    }

    fn read_block(&self, thread: &Thread) -> RegisterBlock {
        let bytes = self.block_bytes(thread.memory, thread.block_at, size_of::<RegisterBlock>());
        RegisterBlock::read_from_bytes(bytes).expect("a register block")
    }

    fn write_block(&self, thread: &Thread, registers: &RegisterBlock) {
        self.block_bytes(thread.memory, thread.block_at, size_of::<RegisterBlock>())
            .copy_from_slice(registers.as_bytes());
    }

    /// What a page allows once the write locks are taken off what the kernel said.
    fn effective(process: &Process<G>, page: u64) -> Permissions {
        let Some(&permissions) = process.permissions.get(page as usize) else {
            return Permissions::empty();
        };
        if process.locks[page as usize] != 0 {
            permissions - Permissions::WRITE
        } else {
            permissions
        }
    }

    /// Rewrites the permission table's bytes for these pages and the one before them.
    fn write_table(&self, memory: MemoryId, pages: Range<u64>) {
        let process = self.process(memory);
        let last = self.layout.permission_pages;
        for page in pages.start.saturating_sub(1)..pages.end.min(last) {
            let byte = contract::permission_byte(
                Self::effective(process, page),
                Self::effective(process, page + 1),
            );
            // SAFETY: inside the permission table; no thread of the process is running guest code.
            unsafe {
                *process
                    .state_base
                    .add(self.layout.permissions_at as usize + page as usize) = byte;
            }
        }
    }

    fn slot_entry(&self, address: u64) -> Option<usize> {
        (address < self.layout.ceiling && address.is_multiple_of(G::ALIGN))
            .then(|| self.layout.slots_at as usize + 4 * (address / G::ALIGN) as usize)
    }

    fn slot(&self, memory: MemoryId, address: u64) -> u32 {
        let Some(at) = self.slot_entry(address) else {
            return 0;
        };
        // SAFETY: an aligned entry of the array of slots in the state memory.
        unsafe { (self.process(memory).state_base.add(at) as *const u32).read() }
    }

    fn set_slot(&self, memory: MemoryId, address: u64, slot: u32) {
        if let Some(at) = self.slot_entry(address) {
            // SAFETY: as for `slot`; no thread of the process is running guest code.
            unsafe { (self.process(memory).state_base.add(at) as *mut u32).write(slot) }
        }
    }

    /// Drops every piece the lookup finds for addresses in a range; whether there was one.
    fn forget(&mut self, memory: MemoryId, range: Range<u64>) -> bool {
        let start = range.start.next_multiple_of(G::ALIGN);
        let end = range.end.min(self.layout.ceiling);
        let mut dropped = false;
        if start < end {
            let first = self.slot_entry(start).expect("an entry");
            let count = ((end - start).div_ceil(G::ALIGN)) as usize;
            // SAFETY: entries of the array of slots for addresses under the ceiling.
            let entries = unsafe {
                std::slice::from_raw_parts_mut(
                    self.process(memory).state_base.add(first) as *mut u32,
                    count,
                )
            };
            dropped = entries.iter().any(|&slot| slot != 0);
            entries.fill(0);
        }
        for thread in self.threads.values_mut().filter(|t| t.memory == memory) {
            thread.exact.retain(|address, _| !range.contains(address));
        }
        dropped
    }

    /// The range of code an address is in: the announced one, or else the run of pages around it that
    /// allow running code, counted as a slow path.
    fn range_at(&mut self, memory: MemoryId, address: u64) -> usize {
        if let Some(index) = self
            .process(memory)
            .ranges
            .iter()
            .position(|r| r.range.contains(&address))
        {
            return index;
        }
        self.slow_path(reason::CODE_OUTSIDE_ANNOUNCED_RANGE, address, 0);
        let process = self.process(memory);
        let executable = |page: u64| {
            process
                .permissions
                .get(page as usize)
                .is_some_and(|p| p.contains(Permissions::EXECUTE))
        };
        let page = address / PAGE_SIZE;
        let mut first = page;
        while first > 0 && executable(first - 1) {
            first -= 1;
        }
        let mut end = page + 1;
        while executable(end) {
            end += 1;
        }
        let process = self.processes.get_mut(&memory).expect("a memory");
        process.ranges.push(CodeRange {
            range: first * PAGE_SIZE..end * PAGE_SIZE,
            starts: Vec::new(),
            translated: true,
            changed: false,
            hash: None,
        });
        process.ranges.len() - 1
    }

    fn code_bytes(&self, memory: MemoryId, range: &Range<u64>) -> Vec<u8> {
        let mut bytes = vec![0; (range.end - range.start) as usize];
        self.copy_out(memory, range.start, &mut bytes);
        bytes
    }

    fn copy_out(&self, memory: MemoryId, address: u64, into: &mut [u8]) {
        // SAFETY: the caller checked the range is under the ceiling, inside the guest memory.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.process(memory).base.add(address as usize),
                into.as_mut_ptr(),
                into.len(),
            )
        }
    }

    fn copy_in(&self, memory: MemoryId, address: u64, from: &[u8]) {
        // SAFETY: as for `copy_out`; no guest thread of the process is running guest code.
        unsafe {
            std::ptr::copy_nonoverlapping(
                from.as_ptr(),
                self.process(memory).base.add(address as usize),
                from.len(),
            )
        }
    }

    /// The look-ahead of code reached at run time with no piece.
    fn at_run_time(&self) -> LookAhead {
        match self.look_ahead {
            LookAhead::Full => LookAhead::AT_RUN_TIME,
            other => other,
        }
    }

    /// A range's bytes as they are now, with their hash.
    fn code(&self, memory: MemoryId, range: &Range<u64>) -> Code {
        let bytes = self.code_bytes(memory, range);
        Code {
            start: range.start,
            hash: *blake3::hash(&bytes).as_bytes(),
            bytes,
        }
    }

    /// The compiled translation of `code` from `starts`, recorded.
    fn compile(
        &mut self,
        code: &Code,
        starts: Vec<u64>,
        look_ahead: LookAhead,
        at_run_time: bool,
        sources: Option<Sources>,
    ) -> Arc<Compiled> {
        let count = starts.len();
        let (compiled, origin) = self.compiler.compiled(
            &code.bytes,
            code.hash,
            starts,
            look_ahead,
            self.optimise,
            self.offer,
        );
        self.translated.push(Translated {
            start: code.start,
            length: code.bytes.len() as u64,
            look_ahead,
            at_run_time,
            starts: count,
            sources,
            pieces: compiled.count,
            origin,
        });
        compiled
    }

    /// Places a compiled translation in every thread of a process that has its instances, from the
    /// next free slot, and gives each piece's address its slot.
    fn add_load(
        &mut self,
        mut store: impl AsContextMut<Data = Data<G>>,
        memory: MemoryId,
        compiled: Arc<Compiled>,
        load_address: u64,
    ) {
        let process = self.processes.get_mut(&memory).expect("a memory");
        let load = Load {
            compiled,
            load_address,
            slot_base: process.next_slot,
            live: true,
        };
        process.next_slot += load.compiled.count;
        let process = self.process(memory);
        for thread in self.threads.values().filter(|t| t.memory == memory) {
            if let Some(instance) = &thread.instance {
                grow(&mut store, instance, process.next_slot);
                self.instantiate_load(&mut store, process, thread, instance, &load, load.slot_base);
            }
        }
        for (slot, offset) in (load.slot_base..).zip(load.compiled.pieces.iter().flatten()) {
            self.set_slot(memory, load_address + offset, slot);
        }
        self.processes
            .get_mut(&memory)
            .expect("a memory")
            .loads
            .push(load);
    }

    fn instantiate_load(
        &self,
        mut store: impl AsContextMut<Data = Data<G>>,
        process: &Process<G>,
        thread: &Thread,
        instance: &ThreadInstance,
        load: &Load,
        slot_base: u32,
    ) {
        let mut slot = slot_base;
        for (module, pieces) in load.compiled.modules.iter().zip(&load.compiled.pieces) {
            let values = self.values(thread.block_at, Some((load.load_address, slot)));
            instantiate(
                &mut store,
                module,
                process,
                Some(instance.instance),
                &values,
            );
            slot += pieces.len() as u32;
        }
    }

    /// The constants a thread module or a code module (with its load address and slot base) imports.
    fn values(&self, block_at: u32, code: Option<(u64, u32)>) -> Vec<(&'static str, Val)> {
        let layout = &self.layout;
        let mut values = vec![
            (contract::TABLE_BASE, Val::I64(0)),
            (contract::THREAD_BLOCK, Val::I32(block_at as i32)),
            (contract::SLOTS, Val::I32(layout.slots_at as i32)),
            (contract::SLOTS_COVER, Val::I64(layout.ceiling as i64)),
            (
                contract::PERMISSIONS,
                Val::I32(layout.permissions_at as i32),
            ),
            (
                contract::PERMISSION_PAGES,
                Val::I64(layout.permission_pages as i64),
            ),
        ];
        if let Some((load_address, slot_base)) = code {
            values.push((contract::LOAD_ADDRESS, Val::I64(load_address as i64)));
            values.push((contract::SLOT_BASE, Val::I32(slot_base as i32)));
        }
        values
    }

    /// The thread's instances, made at its first run with every live translation of its process.
    fn thread_instance(&mut self, thread: ThreadId, store: &mut Store<Data<G>>) -> ThreadInstance {
        let record = self.threads.get(&thread).expect("a thread that exists");
        if let Some(instance) = &record.instance {
            return instance.clone();
        }
        let process = self.process(record.memory);
        let values = self.values(record.block_at, None);
        let instance = instantiate(
            &mut *store,
            &self.compiler.thread_module,
            process,
            None,
            &values,
        );
        let Some(table) = instance.get_table(&mut *store, contract::LOOKUP) else {
            panic!("the thread module exports its lookup table");
        };
        let run = instance
            .get_typed_func::<(), ()>(&mut *store, contract::RUN)
            .expect("the thread module exports run");
        let made = ThreadInstance {
            instance,
            run,
            table,
        };
        grow(&mut *store, &made, process.next_slot);
        for load in process.loads.iter().filter(|load| load.live) {
            self.instantiate_load(&mut *store, process, record, &made, load, load.slot_base);
        }
        self.threads.get_mut(&thread).expect("a thread").instance = Some(made.clone());
        made
    }

    /// Sets a thread up to carry on as `resume` says.
    fn start_from(&mut self, thread: ThreadId, resume: Resume) {
        let record = self.threads.get(&thread).expect("a thread that exists");
        match resume {
            Resume::Return(value) => {
                let mut registers = self.read_block(record);
                registers.x[0] = value;
                self.write_block(record, &registers);
            }
            Resume::Continue => {}
            Resume::FromRegisters => {
                // The CPU side's own words start as a new thread's.
                self.block_bytes(
                    record.memory,
                    record.block_at + G::BLOCK_SIZE as u32,
                    G::PRIVATE_BYTES,
                )
                .fill(0);
            }
        }
    }

    /// A process with a new store and memories, all zeros, where every page allows nothing.
    fn new_process(&self, id: MemoryId) -> Result<Process<G>, CpuError> {
        let engine = self.compiler.engine.clone();
        let mut store = Store::new(
            &engine,
            Data {
                side: std::ptr::null_mut(),
                answer: None,
                memory: id,
                thread: ThreadId(0),
                block_at: 0,
                state_base: std::ptr::null_mut(),
                progress: 0,
                reported: 0,
                limit: None,
                leaving: false,
                rerun: None,
                pending: None,
                lock_fault: None,
                // Each translation and thread is an instance and a table.
                limits: StoreLimitsBuilder::new()
                    .instances(usize::MAX)
                    .tables(usize::MAX)
                    .memories(usize::MAX)
                    .build(),
            },
        );
        store.limiter(|data| &mut data.limits);
        let layout = self.layout;
        let pages = layout.ceiling / 65536;
        let memory = Memory::new(&mut store, MemoryType::new64(pages, Some(pages)))
            .map_err(|_| CpuError::OutOfMemory)?;
        let base = memory.data_ptr(&store);
        let state = SharedMemory::new(
            &engine,
            MemoryType::shared(layout.state_pages, layout.state_pages),
        )
        .map_err(|_| CpuError::OutOfMemory)?;
        let state_base = state.data().as_ptr() as *mut u8;
        let hooks = hooks::<G>(&mut store);
        let pages = layout.permission_pages as usize;
        Ok(Process {
            store: Some(store),
            memory,
            base,
            state,
            state_base,
            hooks,
            permissions: vec![Permissions::empty(); pages],
            locks: vec![0; pages],
            data: Pages::new(pages as u64),
            shown: BTreeMap::new(),
            written: BTreeSet::new(),
            ranges: Vec::new(),
            loads: Vec::new(),
            next_slot: contract::EXACT_SLOT as u32 + 1,
            free_blocks: Vec::new(),
            next_block: 0,
            dropped: false,
        })
    }

    /// Makes `into`, a new process, a copy of `source` (of this back end or another of the same guest):
    /// every page that may hold data copied exactly, the same pages' permissions, locks and blocks
    /// shown, and, with `code`, the same translations at the same slots. Threads are not copied.
    fn copy_from(&self, into: &mut Process<G>, source: &Process<G>, code: bool) {
        debug_assert!(
            source.written.is_empty(),
            "the copies of shared pages are made first"
        );
        let host_page = memory::host_page();
        let copy = |from: *mut u8, to: *mut u8, runs: &mut dyn Iterator<Item = Range<u64>>| {
            let mut merged: Vec<Range<u64>> = Vec::new();
            for run in runs {
                let run = run.start / host_page * host_page..run.end.next_multiple_of(host_page);
                match merged.last_mut() {
                    Some(last) if last.end >= run.start => last.end = last.end.max(run.end),
                    _ => merged.push(run),
                }
            }
            for run in merged {
                // SAFETY: whole host pages inside the two processes' own mappings, the new one still
                // all zeros; no guest of either runs meanwhile.
                unsafe {
                    memory::copy_pages(
                        from.add(run.start as usize),
                        to.add(run.start as usize),
                        (run.end - run.start) as usize,
                    )
                }
            }
        };
        copy(
            source.base,
            into.base,
            &mut source
                .data
                .runs()
                .into_iter()
                .map(|pages| pages.start * PAGE_SIZE..pages.end * PAGE_SIZE),
        );
        into.permissions.clone_from(&source.permissions);
        into.locks.clone_from(&source.locks);
        into.data.clone_from(&source.data);
        into.shown.clone_from(&source.shown);
        let layout = self.layout;
        if code {
            // The table is the same record's, and its slots are the source's, so both are copied.
            let table = u64::from(layout.permissions_at)
                ..u64::from(layout.permissions_at) + layout.permission_pages + 1;
            let slots = source.ranges.iter().map(|code| {
                let entry = |address: u64| u64::from(layout.slots_at) + 4 * (address / G::ALIGN);
                entry(code.range.start)..entry(code.range.end.min(layout.ceiling))
            });
            let mut runs: Vec<Range<u64>> = std::iter::once(table).chain(slots).collect();
            runs.sort_by_key(|run| run.start);
            copy(source.state_base, into.state_base, &mut runs.into_iter());
            into.ranges.clone_from(&source.ranges);
            into.loads.clone_from(&source.loads);
            into.next_slot = source.next_slot;
        } else {
            into.ranges = source
                .ranges
                .iter()
                .map(|range| CodeRange {
                    translated: false,
                    ..range.clone()
                })
                .collect();
        }
    }

    /// Makes the copies of every page the guest wrote under a shared lock, and locks them again.
    fn flush(&mut self, memory: MemoryId) {
        let pages = std::mem::take(&mut self.processes.get_mut(&memory).expect("a memory").written);
        for &page in &pages {
            let address = page * PAGE_SIZE;
            let Some((block, offset)) = self.shown_at(memory, address) else {
                continue;
            };
            let mut bytes = vec![0; PAGE_SIZE as usize];
            self.copy_out(memory, address, &mut bytes);
            self.block_write(block, offset, &bytes);
            self.processes.get_mut(&memory).expect("a memory").locks[page as usize] |= LOCK_SHARED;
            self.write_table(memory, page..page + 1);
        }
    }

    fn flush_all(&mut self) {
        let written: Vec<MemoryId> = self
            .processes
            .iter()
            .filter(|(_, p)| !p.written.is_empty())
            .map(|(id, _)| *id)
            .collect();
        for memory in written {
            self.flush(memory);
        }
    }

    /// The block and offset an address of a memory shows, if it shows one.
    fn shown_at(&self, memory: MemoryId, address: u64) -> Option<(SharedId, u64)> {
        let (start, shown) = self.process(memory).shown.range(..=address).next_back()?;
        (address < shown.end).then(|| (shown.block, shown.offset + (address - start)))
    }

    /// Writes bytes into a block and into every memory that shows them.
    fn block_write(&mut self, block: SharedId, offset: u64, bytes: &[u8]) {
        let end = offset + bytes.len() as u64;
        self.blocks.get_mut(&block).expect("a block that exists")[offset as usize..end as usize]
            .copy_from_slice(bytes);
        for process in self.processes.values() {
            for (start, shown) in &process.shown {
                if shown.block != block {
                    continue;
                }
                let shown_end = shown.offset + (shown.end - start);
                let (from, to) = (offset.max(shown.offset), end.min(shown_end));
                if from < to {
                    let into = start + (from - shown.offset);
                    let part = &bytes[(from - offset) as usize..(to - offset) as usize];
                    // SAFETY: inside the memory's shown range, under the ceiling; no guest runs.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            part.as_ptr(),
                            process.base.add(into as usize),
                            part.len(),
                        )
                    }
                }
            }
        }
    }

    /// Ends the sharing in a range of a memory; the memory keeps what the pages hold.
    fn unshow(&mut self, memory: MemoryId, range: Range<u64>) {
        let process = self.processes.get_mut(&memory).expect("a memory");
        let overlapping: Vec<(u64, Shown)> = process
            .shown
            .range(..range.end)
            .filter(|(_, shown)| shown.end > range.start)
            .map(|(start, shown)| (*start, *shown))
            .collect();
        if overlapping.is_empty() {
            return;
        }
        for (start, shown) in overlapping {
            process.shown.remove(&start);
            if start < range.start {
                process.shown.insert(
                    start,
                    Shown {
                        end: range.start,
                        ..shown
                    },
                );
            }
            if shown.end > range.end {
                process.shown.insert(
                    range.end,
                    Shown {
                        offset: shown.offset + (range.end - start),
                        ..shown
                    },
                );
            }
        }
        let pages = range.start / PAGE_SIZE..range.end.div_ceil(PAGE_SIZE);
        for page in pages.clone() {
            process.locks[page as usize] &= !LOCK_SHARED;
            process.written.remove(&page);
        }
        self.write_table(memory, pages);
    }

    /// A guest write to a page under the shared lock that the kernel allows: the page is marked
    /// written and unlocked until its copies are next made.
    fn settle_write(&mut self, memory: MemoryId, page: u64) {
        self.slow_path(reason::FIRST_WRITE_TO_SHARED_PAGE, page * PAGE_SIZE, 0);
        let process = self.processes.get_mut(&memory).expect("a memory");
        process.locks[page as usize] &= !LOCK_SHARED;
        process.written.insert(page);
        self.write_table(memory, page..page + 1);
    }
}

/// A thread's stop flag.
fn flag<G: Guest>(state_base: *mut u8, block_at: u32) -> &'static AtomicU32 {
    // SAFETY: an aligned word inside the state memory, which lives as long as the process; it is
    // reached only through atomics or the guest's own loads.
    unsafe { &*(state_base.add(block_at as usize + G::STOP_FLAG_OFFSET) as *const AtomicU32) }
}

/// Grows a thread's lookup table to `slots`.
fn grow<G: Guest>(
    mut store: impl AsContextMut<Data = Data<G>>,
    instance: &ThreadInstance,
    slots: u32,
) {
    let size = instance.table.size(&store);
    let more = u64::from(slots).saturating_sub(size);
    if more > 0 {
        instance
            .table
            .grow(&mut store, more, Ref::Func(None))
            .expect("a lookup table that grows");
    }
}

/// An instance of a module with its imports: the process's memories and hooks, the constants given,
/// and what the thread module exports.
fn instantiate<G: Guest>(
    mut store: impl AsContextMut<Data = Data<G>>,
    module: &Module,
    process: &Process<G>,
    thread: Option<Instance>,
    values: &[(&'static str, Val)],
) -> Instance {
    let mut store = store.as_context_mut();
    let mut imports = Vec::new();
    for import in module.imports() {
        let item: Extern = match (import.module(), import.name()) {
            (contract::ENV, contract::MEMORY) => process.memory.into(),
            (contract::ENV, contract::STATE) => process.state.clone().into(),
            (contract::ENV, name) => {
                let (_, value) = values
                    .iter()
                    .find(|(n, _)| *n == name)
                    .unwrap_or_else(|| panic!("no value for the import {name}"));
                let ty =
                    GlobalType::new(value.ty(&store).expect("a value type"), Mutability::Const);
                Global::new(&mut store, ty, *value)
                    .expect("a global")
                    .into()
            }
            (contract::HOOKS, name) => process
                .hooks
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or_else(|| panic!("no hook {name}"))
                .1
                .into(),
            (contract::THREAD, name) => thread
                .expect("a code module imports from its thread")
                .get_export(&mut store, name)
                .unwrap_or_else(|| panic!("the thread module exports {name}")),
            (module, name) => panic!("an import the contract does not name: {module}.{name}"),
        };
        imports.push(item);
    }
    Instance::new(&mut store, module, &imports).expect("an instance of a translated module")
}

/// The back end a hook's store belongs to.
fn side<'a, G: Guest>(caller: &Caller<'_, Data<G>>) -> &'a mut CpuSide<G> {
    // SAFETY: set by `run` for the call in progress, which has given up its own use of the back end
    // until the call returns; the running store is not inside the back end.
    unsafe { &mut *caller.data().side }
}

/// The answer of the run in progress.
fn answer<'a, G: Guest>(caller: &Caller<'_, Data<G>>) -> &'a mut dyn Answer {
    let answer = caller.data().answer.expect("a run in progress");
    // SAFETY: the answer `run` was given, alive until it returns.
    unsafe { &mut *answer.as_ptr() }
}

/// Hands an exit to the answer from inside a hook, after making the copies of shared pages.
fn hand<G>(caller: &mut Caller<'_, Data<G>>, exit: Exit) -> Carry
where
    G: Guest<Block = RegisterBlock>,
{
    let side = side(caller);
    let memory = caller.data().memory;
    side.flush(memory);
    let progress = caller.data_mut().end_turn();
    let answer = answer(caller);
    side.send_warnings(caller.data().answer.expect("a run"));
    let carry = answer.exit(side, exit, progress);
    side.send_warnings(caller.data().answer.expect("a run"));
    carry
}

/// After an answer to carry on: leave and run again at once when code was dropped under the thread.
fn carry_on<G>(caller: &mut Caller<'_, Data<G>>, turn: Turn)
where
    G: Guest<Block = RegisterBlock>,
{
    let side = side(caller);
    let data = caller.data_mut();
    data.limit = turn.limit;
    let running = side.running.as_mut().expect("a run");
    if std::mem::take(&mut running.code_dropped) {
        data.rerun = Some(turn);
        leave(data);
        side.slow_path(reason::LEFT_CODE_CHANGED_UNDER_IT, 0, 0);
    }
}

fn leave<G: Guest>(data: &mut Data<G>) {
    data.leaving = true;
    flag::<G>(data.state_base, data.block_at).store(1, Ordering::SeqCst);
}

/// Leaves `run` from a hook because the answer said so: counted by why, with the call number.
fn leave_for<G>(caller: &mut Caller<'_, Data<G>>, why: Leaving, call: u64)
where
    G: Guest<Block = RegisterBlock>,
{
    leave(caller.data_mut());
    let name = match why {
        Leaving::Waits => reason::LEFT_CALL_WAITS,
        Leaving::AnotherThread => reason::LEFT_ANOTHER_THREAD,
        Leaving::NewRegisters => reason::LEFT_NEW_REGISTERS,
        Leaving::Finished => reason::LEFT_FINISHED,
        Leaving::OutsideAsked => reason::LEFT_OUTSIDE_ASKED,
    };
    side(caller).slow_path(name, call, 0);
}

fn syscall<G>(caller: &mut Caller<'_, Data<G>>, number: i64, arguments: [i64; 6]) -> i64
where
    G: Guest<Block = RegisterBlock>,
{
    let call = Syscall {
        number: number as u64,
        arguments: arguments.map(|a| a as u64),
    };
    match hand(caller, Exit::Syscall(call)) {
        Carry::On {
            resume: Resume::Return(value),
            turn,
        } => {
            carry_on(caller, turn);
            value as i64
        }
        Carry::On { resume, .. } => panic!("{resume:?} after a system call"),
        Carry::Leave(why) => {
            leave_for(caller, why, call.number);
            // x0 stays as it was at the call, as it is for a thread that leaves.
            arguments[0]
        }
    }
}

fn stop<G>(caller: &mut Caller<'_, Data<G>>) -> i32
where
    G: Guest<Block = RegisterBlock>,
{
    let data = caller.data_mut();
    let flag = flag::<G>(data.state_base, data.block_at);
    flag.store(0, Ordering::SeqCst);
    if data.leaving {
        return 1;
    }
    data.progress += 1;
    if data.limit.is_some_and(|limit| data.progress >= limit) {
        match hand(caller, Exit::Stop) {
            Carry::On {
                resume: Resume::Continue,
                turn,
            } => {
                carry_on(caller, turn);
                if caller.data().leaving {
                    return 1;
                }
            }
            Carry::On { resume, .. } => panic!("{resume:?} after a stop"),
            Carry::Leave(why) => {
                leave_for(caller, why, reason::NOT_A_CALL);
                return 1;
            }
        }
    }
    // Every check is counted while progress is counted by them.
    flag.store(1, Ordering::SeqCst);
    0
}

fn memory_fault<G>(caller: &mut Caller<'_, Data<G>>, address: u64, access: i32)
where
    G: Guest<Block = RegisterBlock>,
{
    let side = side(caller);
    let memory = caller.data().memory;
    let page = address / PAGE_SIZE;
    let process = side.process(memory);
    let locked = process.locks.get(page as usize).is_some_and(|l| *l != 0);
    if access == 1 && locked && process.permissions[page as usize].contains(Permissions::WRITE) {
        caller.data_mut().lock_fault = Some(page);
        return;
    }
    side.slow_path(Reason::MemoryFault.name(), address, 0);
    let thread = &side.threads[&caller.data().thread];
    let registers = side.read_block(thread);
    let mut code = [0u8; 16];
    let length = side.layout.ceiling.saturating_sub(registers.pc).min(16) as usize;
    side.copy_out(
        memory,
        registers.pc.min(side.layout.ceiling),
        &mut code[..length],
    );
    let access = if access == 1 {
        Access::Write
    } else {
        Access::Read
    };
    let exit = match G::memory_fault(address, &code[..length], &registers) {
        MemoryFault::Page => Exit::MemoryFault { address, access },
        other => panic!("a {other:?} fault, which the abi has no exit for yet"),
    };
    caller.data_mut().pending = Some(exit);
}

fn trap<G>(caller: &mut Caller<'_, Data<G>>, kind: i32, detail: i64)
where
    G: Guest<Block = RegisterBlock>,
{
    let kind = TrapKind::from_i32(kind).expect("a trap kind the contract names");
    let exit = match kind {
        TrapKind::Undefined | TrapKind::Unsupported => Exit::IllegalInstruction,
        TrapKind::Breakpoint => Exit::Breakpoint {
            comment: detail as u16,
        },
        TrapKind::Alignment => Exit::AlignmentFault {
            address: detail as u64,
        },
        other => panic!("a {other:?} trap, which the abi has no exit for yet"),
    };
    if kind == TrapKind::Unsupported {
        let side = side(caller);
        let pc = side.read_block(&side.threads[&caller.data().thread]).pc;
        side.slow_path(Reason::UnsupportedInstructionReached.name(), pc, 0);
    }
    caller.data_mut().pending = Some(exit);
}

fn load_code<G>(caller: &mut Caller<'_, Data<G>>, address: u64, exact: bool) -> i32
where
    G: Guest<Block = RegisterBlock>,
{
    let started = Instant::now();
    let side = side(caller);
    let (memory, thread) = (caller.data().memory, caller.data().thread);
    // Code runs only from pages that allow it, as on Linux.
    let page = address / PAGE_SIZE;
    let runs = side
        .process(memory)
        .permissions
        .get(page as usize)
        .is_some_and(|p| p.contains(Permissions::EXECUTE));
    if !runs {
        side.slow_path(Reason::MemoryFault.name(), address, 0);
        caller.data_mut().pending = Some(Exit::MemoryFault {
            address,
            access: Access::Execute,
        });
        return 0;
    }
    let index = side.range_at(memory, address);
    let range = side.process(memory).ranges[index].range.clone();
    let instance = side.threads[&thread]
        .instance
        .clone()
        .expect("a running thread's instances");
    if exact {
        let table_slot = contract::EXACT_SLOT as u64;
        if let Some(piece) = side.threads[&thread].exact.get(&address) {
            instance
                .table
                .set(&mut *caller, table_slot, Ref::Func(Some(*piece)))
                .expect("the exact slot");
            return 1;
        }
        let code = side.code(memory, &range);
        let one = LookAhead::Bounded(1);
        let compiled = side.compile(&code, vec![address - range.start], one, true, None);
        let load = Load {
            compiled,
            load_address: range.start,
            slot_base: contract::EXACT_SLOT as u32,
            live: false,
        };
        let record = &side.threads[&thread];
        side.instantiate_load(
            &mut *caller,
            side.process(memory),
            record,
            &instance,
            &load,
            load.slot_base,
        );
        let Some(Ref::Func(Some(piece))) = instance.table.get(&mut *caller, table_slot) else {
            panic!("the piece is in the exact slot");
        };
        side.threads
            .get_mut(&thread)
            .expect("a thread")
            .exact
            .insert(address, piece);
        let nanoseconds = started.elapsed().as_nanos() as u64;
        side.slow_path(Reason::TranslatedAtRunTime.name(), address, nanoseconds);
        return 1;
    }
    let code = &side.process(memory).ranges[index];
    if !code.translated && !code.changed {
        let before = code.hash;
        let mut starts = code.starts.clone();
        let code = side.code(memory, &range);
        let reader = Reader { side, memory };
        let found = starts::image_starts(&reader, range.clone());
        let sources = found.as_ref().map(|(_, sources)| *sources);
        if let Some((more, _)) = found {
            starts.extend(more);
        }
        let record = &mut side.processes.get_mut(&memory).expect("a memory").ranges[index];
        record.translated = true;
        if before.is_some_and(|before| before != code.hash) {
            record.changed = true;
            side.slow_path(reason::CODE_CHANGED_AFTER_IT_RAN, range.start, 0);
        } else if !starts.is_empty() {
            record.hash = Some(code.hash);
            let look_ahead = side.look_ahead;
            let compiled = side.compile(&code, starts, look_ahead, false, sources);
            side.add_load(&mut *caller, memory, compiled, range.start);
        }
    }
    if side.slot(memory, address) == 0 {
        let started = Instant::now();
        let code = side.code(memory, &range);
        let look_ahead = side.at_run_time();
        let start = vec![address - range.start];
        let compiled = side.compile(&code, start, look_ahead, true, None);
        side.add_load(&mut *caller, memory, compiled, range.start);
        let nanoseconds = started.elapsed().as_nanos() as u64;
        side.slow_path(Reason::TranslatedAtRunTime.name(), address, nanoseconds);
    }
    1
}

/// Guest memory as the reader of starts sees it.
struct Reader<'a, G: Guest> {
    side: &'a CpuSide<G>,
    memory: MemoryId,
}

impl<G: Guest<Block = RegisterBlock>> starts::Guest for Reader<'_, G> {
    fn read(&self, address: u64, into: &mut [u8]) -> bool {
        if self.side.check(address, into.len() as u64).is_err() {
            return false;
        }
        self.side.copy_out(self.memory, address, into);
        true
    }

    fn readable(&self, address: u64) -> bool {
        self.side
            .process(self.memory)
            .permissions
            .get((address / PAGE_SIZE) as usize)
            .is_some_and(|p| p.contains(Permissions::READ))
    }
}

/// The guest's own code changed hook (ARM64 `ic ivau`): drops the pieces of the line at the address.
fn code_line_changed<G>(caller: &mut Caller<'_, Data<G>>, address: u64)
where
    G: Guest<Block = RegisterBlock>,
{
    let side = side(caller);
    let memory = caller.data().memory;
    let line = address & !63;
    if side.forget(memory, line..line + 64) {
        let process = side.processes.get_mut(&memory).expect("a memory");
        if let Some(range) = process.ranges.iter_mut().find(|r| r.range.contains(&line)) {
            range.changed = true;
        }
        side.slow_path(reason::CODE_CHANGED_AFTER_IT_RAN, line, 0);
    }
}

/// The value types of a guest's own hook, as wasmtime has them.
fn val_type(ty: wasm_encoder::ValType) -> wasmtime::ValType {
    match ty {
        wasm_encoder::ValType::I32 => wasmtime::ValType::I32,
        wasm_encoder::ValType::I64 => wasmtime::ValType::I64,
        other => unreachable!("a hook takes and gives integers, not {other:?}"),
    }
}

/// The hooks of one process's store, by import name.
fn hooks<G>(store: &mut Store<Data<G>>) -> Vec<(&'static str, Func)>
where
    G: Guest<Block = RegisterBlock>,
{
    let mut hooks = vec![
        (
            contract::HOOK_SYSCALL,
            Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, Data<G>>,
                 number: i64,
                 a0: i64,
                 a1: i64,
                 a2: i64,
                 a3: i64,
                 a4: i64,
                 a5: i64|
                 -> i64 { syscall(&mut caller, number, [a0, a1, a2, a3, a4, a5]) },
            ),
        ),
        (
            contract::HOOK_STOP,
            Func::wrap(&mut *store, |mut caller: Caller<'_, Data<G>>| -> i32 {
                stop(&mut caller)
            }),
        ),
        (
            contract::HOOK_MEMORY_FAULT,
            Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, Data<G>>, address: i64, access: i32| {
                    memory_fault(&mut caller, address as u64, access)
                },
            ),
        ),
        (
            contract::HOOK_LOAD_CODE,
            Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, Data<G>>, address: i64, exact: i32| -> i32 {
                    load_code(&mut caller, address as u64, exact == 1)
                },
            ),
        ),
        (
            contract::HOOK_WARN,
            Func::wrap(
                &mut *store,
                |caller: Caller<'_, Data<G>>, reason: i32, detail: i64| {
                    let name = Reason::name_of::<G>(reason as u32).expect("a known reason");
                    side(&caller).slow_path(name, detail as u64, 0);
                },
            ),
        ),
        (
            contract::HOOK_TRAP,
            Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, Data<G>>, kind: i32, detail: i64| {
                    trap(&mut caller, kind, detail)
                },
            ),
        ),
    ];
    for (index, hook) in G::HOOKS.iter().enumerate() {
        let func = match hook.body {
            HookBody::Clock => Func::wrap(&mut *store, |mut caller: Caller<'_, Data<G>>| -> i64 {
                let progress = caller.data_mut().report();
                answer(&caller).clock(progress) as i64
            }),
            HookBody::CodeChanged => Func::wrap(
                &mut *store,
                |mut caller: Caller<'_, Data<G>>, address: i64| {
                    code_line_changed(&mut caller, address as u64)
                },
            ),
            HookBody::Cpu => {
                const {
                    assert!(G::HOOKS.len() <= CPU_HOOKS);
                    let mut i = 0;
                    while i < G::HOOKS.len() {
                        let hook = &G::HOOKS[i];
                        assert!(hook.params.len() <= CPU_HOOK_ARGUMENTS && hook.results.len() <= 2);
                        i += 1;
                    }
                };
                let ty = FuncType::new(
                    store.engine(),
                    hook.params.iter().map(|&t| val_type(t)),
                    hook.results.iter().map(|&t| val_type(t)),
                );
                [
                    cpu_hook_func::<G, 0>,
                    cpu_hook_func::<G, 1>,
                    cpu_hook_func::<G, 2>,
                    cpu_hook_func::<G, 3>,
                    cpu_hook_func::<G, 4>,
                    cpu_hook_func::<G, 5>,
                    cpu_hook_func::<G, 6>,
                    cpu_hook_func::<G, 7>,
                ][index](store, ty)
            }
        };
        hooks.push((hook.name, func));
    }
    hooks
}

/// The most hooks of its own a guest has, and the most arguments one takes.
const CPU_HOOKS: usize = 8;
const CPU_HOOK_ARGUMENTS: usize = 8;

/// The guest's own hook `I` as a function of the store, made for that index at compile time so its
/// types and state range are constants in its body.
fn cpu_hook_func<G, const I: usize>(store: &mut Store<Data<G>>, ty: FuncType) -> Func
where
    G: Guest<Block = RegisterBlock>,
{
    // SAFETY: `cpu_hook` reads the arguments and writes the results of exactly this type.
    unsafe {
        Func::new_unchecked(store, ty, |caller, values| {
            cpu_hook::<G, I>(&caller, values);
            Ok(())
        })
    }
}

/// A guest's own hook: its work by `Guest::run_hook`, in place on the bytes of the thread's state it
/// names, counted as the slow path it says it was. `values` holds the arguments and takes the results.
fn cpu_hook<G, const I: usize>(caller: &Caller<'_, Data<G>>, values: &mut [MaybeUninit<ValRaw>])
where
    G: Guest<Block = RegisterBlock>,
{
    let hook = &G::HOOKS[I];
    let mut arguments = [0i64; CPU_HOOK_ARGUMENTS];
    for (i, ty) in hook.params.iter().enumerate() {
        // SAFETY: wasmtime has checked the arguments against the hook's type.
        let value = unsafe { values[i].assume_init() };
        arguments[i] = match ty {
            wasm_encoder::ValType::I32 => i64::from(value.get_i32()),
            _ => value.get_i64(),
        };
    }
    let side = side(caller);
    let data = caller.data();
    let range = hook.state.range();
    let bytes = side.block_bytes(data.memory, data.block_at + range.start as u32, range.len());
    let done = G::run_hook(I, &arguments[..hook.params.len()], bytes);
    for (i, ty) in hook.results.iter().enumerate() {
        values[i] = MaybeUninit::new(match ty {
            wasm_encoder::ValType::I32 => ValRaw::i32(done.results[i] as i32),
            _ => ValRaw::i64(done.results[i]),
        });
    }
    if let Some(reason) = done.reason {
        let name = Reason::name_of::<G>(reason).expect("a known reason");
        side.slow_path(name, arguments[0] as u32 as u64, 0);
    }
}

impl<G> Execution for CpuSide<G>
where
    G: Guest<Block = RegisterBlock>,
{
    /// Everything the guest does becomes an exit.
    type Failure = Infallible;

    fn run(
        &mut self,
        thread: ThreadId,
        mut resume: Resume,
        turn: Turn,
        answer: &mut dyn Answer,
    ) -> Result<(), Infallible> {
        // SAFETY: only the lifetime is erased; the pointer is used only until this call returns and is
        // cleared from the store before then.
        let answer: NonNull<dyn Answer> = unsafe {
            std::mem::transmute::<NonNull<dyn Answer + '_>, NonNull<dyn Answer + 'static>>(
                NonNull::from(answer),
            )
        };
        let memory = self.threads[&thread].memory;
        let mut store = self
            .processes
            .get_mut(&memory)
            .expect("a memory that exists")
            .store
            .take()
            .expect("one thread of a process runs at a time");
        self.running = Some(Running {
            memory,
            thread,
            code_dropped: false,
        });
        self.slow_path(reason::PROGRESS_BY_STOP_CHECKS, 0, 0);
        // A new turn to count from, or none when the thread goes on with the turn it had.
        let mut fresh = Some(turn);
        loop {
            self.send_warnings(answer);
            self.start_from(thread, resume);
            let instance = self.thread_instance(thread, &mut store);
            let block_at = self.threads[&thread].block_at;
            let state_base = self.process(memory).state_base;
            let data = store.data_mut();
            data.side = self;
            data.answer = Some(answer);
            data.memory = memory;
            data.thread = thread;
            data.block_at = block_at;
            data.state_base = state_base;
            if let Some(turn) = fresh.take() {
                (data.progress, data.reported, data.limit) = (0, 0, turn.limit);
            }
            data.leaving = false;
            let flag = flag::<G>(state_base, block_at);
            flag.store(1, Ordering::SeqCst);
            let result = instance.run.call(&mut store, ());
            flag.store(0, Ordering::SeqCst);
            let data = store.data_mut();
            data.answer = None;
            let mut pending = data.pending.take();
            let (lock_fault, rerun) = (data.lock_fault.take(), data.rerun.take());
            if let Err(error) = result {
                match error.downcast_ref::<Trap>() {
                    Some(Trap::MemoryOutOfBounds) => {
                        let name = Reason::FaultWithoutExactRegisters.name();
                        self.slow_path(name, 0, 0);
                        pending = Some(Exit::MemoryFaultWithoutRegisters);
                    }
                    _ => panic!("the engine stopped the guest: {error:?}"),
                }
            }
            if let Some(record) = self.threads.get(&thread)
                && let Some(held) = record.held
            {
                self.write_block(record, &held);
                self.threads.get_mut(&thread).expect("a thread").held = None;
            }
            let gone = self.process(memory).dropped || self.threads[&thread].dropped;
            if gone {
                break;
            }
            if let Some(page) = lock_fault {
                self.settle_write(memory, page);
                resume = Resume::Continue;
                continue;
            }
            if let Some(again) = rerun {
                (resume, fresh) = (Resume::Continue, Some(again));
                continue;
            }
            let Some(exit) = pending else {
                break;
            };
            let progress = store.data_mut().end_turn();
            self.flush(memory);
            self.send_warnings(answer);
            // SAFETY: the answer this call was given.
            match unsafe { &mut *answer.as_ptr() }.exit(self, exit, progress) {
                Carry::On {
                    resume: then,
                    turn: next,
                } => (resume, fresh) = (then, Some(next)),
                // The thread is out of the guest already, so leaving costs nothing more here.
                Carry::Leave(_) => break,
            }
        }
        self.send_warnings(answer);
        self.running = None;
        let process = self.processes.get_mut(&memory).expect("a memory");
        if process.dropped {
            drop(store);
            self.processes.remove(&memory);
        } else {
            process.store = Some(store);
        }
        if self.threads[&thread].dropped {
            let record = self.threads.remove(&thread).expect("a thread");
            if let Some(process) = self.processes.get_mut(&memory) {
                process.free_blocks.push(record.block_at);
            }
        }
        Ok(())
    }

    fn restored(&self) -> Self {
        let compiler = Compiler::new(self.compiler.directory.clone());
        let mut side = Self::new(compiler, self.look_ahead, self.optimise, self.offer);
        side.last = self.last;
        side.blocks.clone_from(&self.blocks);
        side.counters.clone_from(&self.counters);
        side.warned.clone_from(&self.warned);
        side.translated.clone_from(&self.translated);
        for (&id, source) in &self.processes {
            let mut process = side
                .new_process(id)
                .expect("memory for a process brought back");
            side.copy_from(&mut process, source, false);
            process.free_blocks.clone_from(&source.free_blocks);
            process.next_block = source.next_block;
            side.processes.insert(id, process);
            side.write_table(id, 0..side.layout.permission_pages);
        }
        let bytes = self.layout.thread_bytes as usize;
        for (&id, thread) in &self.threads {
            side.block_bytes(thread.memory, thread.block_at, bytes)
                .copy_from_slice(self.block_bytes(thread.memory, thread.block_at, bytes));
            side.threads.insert(
                id,
                Thread {
                    memory: thread.memory,
                    block_at: thread.block_at,
                    instance: None,
                    exact: HashMap::new(),
                    held: thread.held,
                    dropped: false,
                },
            );
        }
        side
    }
}

impl<G> Cpu for CpuSide<G>
where
    G: Guest<Block = RegisterBlock>,
{
    fn address_space(&self) -> AddressSpace {
        AddressSpace {
            end: self.layout.ceiling,
        }
    }

    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        let id = MemoryId(self.fresh());
        let process = self.new_process(id)?;
        self.processes.insert(id, process);
        Ok(id)
    }

    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        self.flush(source);
        let id = MemoryId(self.fresh());
        let mut process = self.new_process(id)?;
        self.copy_from(&mut process, self.process(source), true);
        self.processes.insert(id, process);
        Ok(id)
    }

    fn memory_drop(&mut self, memory: MemoryId) {
        self.flush(memory);
        let shown: Vec<u64> = self.process(memory).shown.keys().copied().collect();
        let process = self.processes.get_mut(&memory).expect("a memory");
        for start in shown {
            process.shown.remove(&start);
        }
        if self.running.as_ref().is_some_and(|r| r.memory == memory) {
            process.dropped = true;
        } else {
            self.processes.remove(&memory);
        }
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        self.check(address, into.len() as u64)?;
        self.copy_out(memory, address, into);
        Ok(())
    }

    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        self.check(address, from.len() as u64)?;
        self.flush(memory);
        self.copy_in(memory, address, from);
        let pages = address / PAGE_SIZE..(address + from.len() as u64).div_ceil(PAGE_SIZE);
        let process = self.processes.get_mut(&memory).expect("a memory");
        process.data.set(pages, true);
        let end = address + from.len() as u64;
        let shown: Vec<(u64, Shown)> = self
            .process(memory)
            .shown
            .range(..end)
            .filter(|(_, shown)| shown.end > address)
            .map(|(start, shown)| (*start, *shown))
            .collect();
        for (start, shown) in shown {
            let (from_at, to) = (address.max(start), end.min(shown.end));
            let part = &from[(from_at - address) as usize..(to - address) as usize];
            self.block_write(shown.block, shown.offset + (from_at - start), part);
        }
        Ok(())
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        self.check(address, length)?;
        self.flush(memory);
        self.unshow(memory, address..address + length);
        // Only pages zeroed whole are known to hold nothing.
        let whole = address.div_ceil(PAGE_SIZE)..(address + length) / PAGE_SIZE;
        if whole.start < whole.end {
            let process = self.processes.get_mut(&memory).expect("a memory");
            process.data.set(whole, false);
        }
        let host_page = memory::host_page();
        let (first, end) = (
            address.next_multiple_of(host_page),
            (address + length) / host_page * host_page,
        );
        let base = self.process(memory).base;
        let zero_bytes = |from: u64, to: u64| {
            // SAFETY: inside the guest memory, under the ceiling; no guest runs meanwhile.
            unsafe { std::ptr::write_bytes(base.add(from as usize), 0, (to - from) as usize) }
        };
        if first < end {
            zero_bytes(address, first);
            // SAFETY: whole host pages inside the guest memory, which no guest runs in meanwhile.
            unsafe { memory::fresh_pages(base.add(first as usize), (end - first) as usize) };
            zero_bytes(end, address + length);
        } else {
            zero_bytes(address, address + length);
        }
        Ok(())
    }

    fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        self.check(address, length)?;
        let pages = address / PAGE_SIZE..(address + length).div_ceil(PAGE_SIZE);
        let process = self.processes.get_mut(&memory).expect("a memory");
        process.permissions[pages.start as usize..pages.end as usize].fill(permissions);
        if !permissions.is_empty() {
            process.data.set(pages.clone(), true);
        }
        self.write_table(memory, pages);
        Ok(())
    }

    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, starts: &[u64]) {
        let range = address..address + length;
        let dropped = self.forget(memory, range.clone());
        let process = self.processes.get_mut(&memory).expect("a memory");
        for load in &mut process.loads {
            if range.contains(&load.load_address) {
                load.live = false;
            }
        }
        let mut kept: Vec<u64> = starts.to_vec();
        let (mut changed, mut hash) = (false, None);
        // Ranges of other extents translated before, to compare with their bytes now.
        let mut compare = Vec::new();
        process.ranges.retain(|code| {
            let overlaps = code.range.start < range.end && range.start < code.range.end;
            if overlaps {
                changed |= code.changed;
                match code.hash {
                    Some(old) if code.range == range => hash = Some(old),
                    Some(old) => compare.push((code.range.clone(), old)),
                    None => {}
                }
                kept.extend(
                    code.starts
                        .iter()
                        .map(|offset| code.range.start + offset)
                        .filter(|at| range.contains(at)),
                );
            }
            !overlaps
        });
        let mut offsets: Vec<u64> = kept.into_iter().map(|at| at - address).collect();
        offsets.sort_unstable();
        offsets.dedup();
        let now_changed = !changed
            && compare
                .iter()
                .any(|(old, before)| self.code(memory, old).hash != *before);
        self.processes
            .get_mut(&memory)
            .expect("a memory")
            .ranges
            .push(CodeRange {
                range,
                starts: offsets,
                translated: false,
                changed: changed || now_changed,
                hash,
            });
        if now_changed {
            self.slow_path(reason::CODE_CHANGED_AFTER_IT_RAN, address, 0);
        }
        if dropped && let Some(running) = self.running.as_mut().filter(|r| r.memory == memory) {
            running.code_dropped = true;
        }
    }

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        let id = SharedId(self.fresh());
        let length = usize::try_from(length).map_err(|_| CpuError::OutOfMemory)?;
        self.blocks.insert(id, vec![0; length]);
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
        self.check(address, length)?;
        let block = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > block.len() as u64)
        {
            return Err(CpuError::BadAddress);
        }
        self.flush(memory);
        self.unshow(memory, address..address + length);
        let bytes = &self.blocks[&shared][offset as usize..(offset + length) as usize];
        self.copy_in(memory, address, bytes);
        let process = self.processes.get_mut(&memory).expect("a memory");
        process.shown.insert(
            address,
            Shown {
                end: address + length,
                block: shared,
                offset,
            },
        );
        let pages = address / PAGE_SIZE..(address + length).div_ceil(PAGE_SIZE);
        for page in pages.clone() {
            process.locks[page as usize] |= LOCK_SHARED;
        }
        process.data.set(pages.clone(), true);
        self.write_table(memory, pages);
        Ok(())
    }

    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.check(address, length)?;
        self.flush(memory);
        self.unshow(memory, address..address + length);
        Ok(())
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        self.flush_all();
        let block = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
        let end = offset
            .checked_add(into.len() as u64)
            .filter(|end| *end <= block.len() as u64)
            .ok_or(CpuError::BadAddress)?;
        into.copy_from_slice(&block[offset as usize..end as usize]);
        Ok(())
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        self.flush_all();
        let block = self.blocks.get(&shared).ok_or(CpuError::BadAddress)?;
        if offset
            .checked_add(from.len() as u64)
            .is_none_or(|end| end > block.len() as u64)
        {
            return Err(CpuError::BadAddress);
        }
        self.block_write(shared, offset, from);
        Ok(())
    }

    fn shared_drop(&mut self, shared: SharedId) {
        self.blocks.remove(&shared);
    }

    fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        let blocks = self.layout.blocks();
        let bytes = self.layout.thread_bytes;
        let process = self.processes.get_mut(&memory).expect("a memory");
        let block_at = match process.free_blocks.pop() {
            Some(at) => at,
            None if process.next_block < blocks => {
                process.next_block += 1;
                (process.next_block - 1) * bytes
            }
            None => return Err(CpuError::OutOfMemory),
        };
        self.block_bytes(memory, block_at, bytes as usize).fill(0);
        let id = ThreadId(self.fresh());
        self.threads.insert(
            id,
            Thread {
                memory,
                block_at,
                instance: None,
                exact: HashMap::new(),
                held: None,
                dropped: false,
            },
        );
        Ok(id)
    }

    fn thread_drop(&mut self, thread: ThreadId) {
        if self.running.as_ref().is_some_and(|r| r.thread == thread) {
            self.threads.get_mut(&thread).expect("a thread").dropped = true;
            return;
        }
        let record = self.threads.remove(&thread).expect("a thread that exists");
        if let Some(process) = self.processes.get_mut(&record.memory) {
            process.free_blocks.push(record.block_at);
        }
    }

    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        let record = &self.threads[&thread];
        *into = record.held.unwrap_or_else(|| self.read_block(record));
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        if self.running.as_ref().is_some_and(|r| r.thread == thread) {
            self.threads.get_mut(&thread).expect("a thread").held = Some(*from);
        } else {
            self.write_block(&self.threads[&thread], from);
        }
    }
}
