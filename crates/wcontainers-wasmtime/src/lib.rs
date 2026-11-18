//! The WebAssembly back end of the runtime: the translator's output for a guest `G`, run by wasmtime,
//! behind the kernel's `Cpu` side. One store per process; a thread's instances are made at its first
//! run; code is translated at first use and kept in a cache shared by every back end of a [`Compiler`].

mod memory;
pub mod starts;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::convert::Infallible;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
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
    /// The guest reached a page of a block that it was writing through another page of the same memory,
    /// so the copies were made first.
    pub const SHARED_PAGE_WRITTEN_ELSEWHERE: &str = "shared page written through another page";
    /// A write to a page of a block that the neighbouring page shows too is made one instruction at
    /// a time, since one write could cross from one copy of the block's page into the other.
    pub const WRITE_STEPPED: &str = "write next to another view made one instruction at a time";
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
    /// A translation could not be written to the cache's directory; the detail is the host's error
    /// number (`u64::MAX` when it gave none).
    pub const TRANSLATION_NOT_KEPT: &str = "translation not kept in the cache's directory";
    /// A file of the cache's directory could not be read whole or wasmtime refused it, so its
    /// translation is made again; the detail is the host's error number (`u64::MAX` when it gave none).
    pub const TRANSLATION_REFUSED: &str = "translation in the cache's directory refused";
    /// The guest read the counter on a back end brought back from a save, whose clock hook answers the
    /// outside's monotonic clock without the kernel's offset until the kernel makes its clock public.
    pub const COUNTER_WITHOUT_KERNEL_OFFSET: &str = "counter read without the kernel's offset";
    /// The detail of a leave after an exit that is not a system call.
    pub const NOT_A_CALL: u64 = u64::MAX;
}

/// What every back end of one engine shares: the engine, the thread module, the pool translation and
/// compiling run in, the probe's answer, the compiled translations kept in memory, and the directory
/// they are kept in between runs.
pub struct Compiler<G: Guest> {
    engine: Engine,
    thread_module: Module,
    /// Dropped only in the process that made it: in a copy made by fork its threads are not there.
    pool: ManuallyDrop<rayon::ThreadPool>,
    /// The process the pool's threads are in.
    process: u32,
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
        let probe = pool
            .install(|| Module::new(&engine, G::probe_module()))
            .expect("the probe module");
        let mut store = Store::new(&engine, ());
        let probe = Instance::new(&mut store, &probe, &[]).expect("the probe instance");
        let probe = probe
            .get_typed_func::<(), i32>(&mut store, contract::PROBE)
            .expect("the probe export");
        let host = G::host_from_probe(probe.call(&mut store, ()).expect("the probe runs"));
        Arc::new(Self {
            engine,
            thread_module,
            pool: ManuallyDrop::new(pool),
            process: std::process::id(),
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

    /// A translation from the cache's directory: none when the file is not there, and the host's
    /// error number (`u64::MAX` when it gave none) when it cannot be read or is refused.
    fn load(&self, file: &Path) -> Option<Result<Compiled, u64>> {
        let bytes = match std::fs::read(file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => {
                return Some(Err(error
                    .raw_os_error()
                    .map_or(u64::MAX, |number| number as u64)));
            }
        };
        Some(Self::parse(&self.engine, &bytes).ok_or(u64::MAX))
    }

    /// A translation from a file's bytes; none when they are not whole or wasmtime refuses them.
    fn parse(engine: &Engine, bytes: &[u8]) -> Option<Compiled> {
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
            let module = unsafe { Module::deserialize(engine, take(length)?) }.ok()?;
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
    fn store(&self, file: &Path, compiled: &Compiled) -> std::io::Result<()> {
        let mut bytes = CACHE_MAGIC.to_vec();
        bytes.extend((compiled.modules.len() as u64).to_le_bytes());
        for (module, pieces) in compiled.modules.iter().zip(&compiled.pieces) {
            bytes.extend((pieces.len() as u64).to_le_bytes());
            bytes.extend(pieces.iter().flat_map(|p| p.to_le_bytes()));
            let serialised = module.serialize().expect("a compiled module serialises");
            bytes.extend((serialised.len() as u64).to_le_bytes());
            bytes.extend(serialised);
        }
        // A name no other writer uses, in this process or another sharing the directory.
        static WRITES: AtomicU64 = AtomicU64::new(0);
        let write = WRITES.fetch_add(1, Ordering::Relaxed);
        let partial = file.with_extension(format!("partial-{}-{write}", std::process::id()));
        let written = std::fs::create_dir_all(file.parent().expect("a directory"))
            .and_then(|()| std::fs::write(&partial, &bytes))
            .and_then(|()| std::fs::rename(&partial, file));
        if written.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        written
    }

    /// The compiled translation of a blob from its starts, from the cache or made now, where it came
    /// from, and the slow paths of the cache's directory it took.
    fn compiled(
        &self,
        bytes: &[u8],
        hash: [u8; 32],
        mut starts: Vec<u64>,
        look_ahead: LookAhead,
        optimise: Optimise,
        offer: G::Offer,
    ) -> (Arc<Compiled>, Origin, Vec<CacheSlowPath>) {
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
            return (compiled.clone(), Origin::Memory, Vec::new());
        }
        let file = self.directory.as_deref().map(|d| self.file(d, &key));
        let mut slow = Vec::new();
        let started = Instant::now();
        match file.as_deref().and_then(|file| self.load(file)) {
            Some(Ok(compiled)) => {
                let compiled = Arc::new(compiled);
                let nanoseconds = started.elapsed().as_nanos() as u64;
                self.modules
                    .lock()
                    .expect("the cache")
                    .insert(key, compiled.clone());
                return (compiled, Origin::Disk(nanoseconds), slow);
            }
            Some(Err(error)) => slow.push(CacheSlowPath {
                reason: reason::TRANSLATION_REFUSED,
                detail: error,
                nanoseconds: started.elapsed().as_nanos() as u64,
            }),
            None => {}
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
        if let Some(file) = file.as_deref() {
            let started = Instant::now();
            if let Err(error) = self.store(file, &compiled) {
                slow.push(CacheSlowPath {
                    reason: reason::TRANSLATION_NOT_KEPT,
                    detail: error
                        .raw_os_error()
                        .map_or(u64::MAX, |number| number as u64),
                    nanoseconds: started.elapsed().as_nanos() as u64,
                });
            }
        }
        self.modules
            .lock()
            .expect("the cache")
            .insert(key, compiled.clone());
        let origin = Origin::Made {
            translating: translated,
            compiling: compiled_in,
        };
        (compiled, origin, slow)
    }
}

/// A slow path the cache's directory took, for the back end to count: why, its detail and the time
/// it cost. The run goes on either way.
struct CacheSlowPath {
    reason: &'static str,
    detail: u64,
    nanoseconds: u64,
}

impl<G: Guest> Drop for Compiler<G> {
    fn drop(&mut self) {
        // Ending the pool wakes its threads, which in a copy made by fork can wait for ever on a lock
        // a thread of the parent held at the fork; a copy leaves the pool as it is.
        if self.process == std::process::id() {
            // SAFETY: dropped once, here, and not used after.
            unsafe { ManuallyDrop::drop(&mut self.pool) };
        }
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
    /// A write being made one instruction at a time.
    stepping: Option<Stepping>,
    counters: BTreeMap<&'static str, (u64, u64)>,
    /// The (reason, detail) pairs already warned of.
    warned: HashSet<(&'static str, u64)>,
    /// Warnings made while no thread ran, given to the outside at the next run.
    unsent: Vec<Warning<'static>>,
    translated: Vec<Translated>,
    /// Brought back from a save: the clock hook's answers lack the kernel's offset.
    restored: bool,
}

/// A write made one instruction at a time: the lookup sends the instruction's address to its piece alone
/// and the next address to `load_code`, which ends the step.
struct Stepping {
    memory: MemoryId,
    /// The entries of the array of slots changed for the step, with what they held.
    slots: [(u64, u32); 2],
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
    pages: Pages,
    /// Ranges that show a block of shared pages, by start.
    shown: BTreeMap<u64, Shown>,
    /// Pages showing a block that the guest has written since their copies were last made.
    written: BTreeSet<u64>,
    /// Pages under the view lock: they show a block's page that one of `written` shows too.
    viewing: BTreeSet<u64>,
    ranges: Vec<CodeRange>,
    loads: Vec<Load>,
    next_slot: u32,
    free_blocks: Vec<u32>,
    next_block: u32,
    dropped: bool,
}

/// Write lock: the page shows a block of shared pages.
const LOCK_SHARED: u8 = 1;
/// Read and write lock: the page shows a block's page that the guest is writing through another page of
/// the same memory, whose copy alone holds the writes until the copies are made.
const LOCK_VIEW: u8 = 2;

/// What the back end keeps about one guest page.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Page {
    /// What the kernel last said the page allows.
    allows: Permissions,
    /// The back end's write locks on it, by reason bit.
    locks: u8,
    /// It may hold anything but zeros: given access or written since last zeroed.
    data: bool,
}

impl Page {
    const NOTHING: Page = Page {
        allows: Permissions::empty(),
        locks: 0,
        data: false,
    };
}

/// Pages one chunk of the records covers: 16 MiB of guest memory.
const CHUNK: usize = 4096;

/// Every page's record, in chunks that copies of a process share until one of them changes it, so a
/// fork copies what the process uses, not its whole address space. A chunk not there holds nothing.
#[derive(Clone)]
struct Pages(Vec<Option<Arc<[Page; CHUNK]>>>);

impl Pages {
    fn new(pages: u64) -> Self {
        Pages(vec![None; (pages as usize).div_ceil(CHUNK)])
    }

    fn get(&self, page: u64) -> Page {
        let page = page as usize;
        self.0
            .get(page / CHUNK)
            .and_then(Option::as_ref)
            .map_or(Page::NOTHING, |chunk| chunk[page % CHUNK])
    }

    /// Changes the records of `pages`; a chunk left holding nothing is dropped.
    fn update(&mut self, pages: Range<u64>, change: impl Fn(&mut Page)) {
        let (mut page, end) = (pages.start as usize, pages.end as usize);
        while page < end {
            let index = page / CHUNK;
            let (first, last) = (page % CHUNK, (end - index * CHUNK).min(CHUNK));
            page = index * CHUNK + last;
            let slot = &mut self.0[index];
            if slot.is_none() {
                let mut nothing = Page::NOTHING;
                change(&mut nothing);
                if nothing == Page::NOTHING {
                    continue;
                }
            }
            let chunk = Arc::make_mut(slot.get_or_insert_with(|| Arc::new([Page::NOTHING; CHUNK])));
            chunk[first..last].iter_mut().for_each(&change);
            if first == 0 && last == CHUNK && chunk.iter().all(|p| *p == Page::NOTHING) {
                *slot = None;
            }
        }
    }

    /// The pages of every chunk there, in order.
    fn chunks(&self) -> impl Iterator<Item = Range<u64>> + '_ {
        self.0.iter().enumerate().filter_map(|(index, chunk)| {
            chunk
                .as_ref()
                .map(|_| (index * CHUNK) as u64..((index + 1) * CHUNK) as u64)
        })
    }

    /// The runs of pages that may hold data, in order.
    fn data_runs(&self) -> Vec<Range<u64>> {
        let mut runs: Vec<Range<u64>> = Vec::new();
        for (index, chunk) in self.0.iter().enumerate() {
            let Some(chunk) = chunk else {
                continue;
            };
            for (offset, record) in chunk.iter().enumerate() {
                if !record.data {
                    continue;
                }
                let page = (index * CHUNK + offset) as u64;
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
    /// An access to a locked page (the page, and whether it is a write), to settle before the thread
    /// carries on.
    lock_fault: Option<(u64, bool)>,
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
            restored: false,
            stepping: None,
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

    /// What a page allows once the locks are taken off what the kernel said.
    fn effective(process: &Process<G>, page: u64) -> Permissions {
        let page = process.pages.get(page);
        let mut allows = page.allows;
        if page.locks != 0 {
            allows -= Permissions::WRITE;
        }
        if page.locks & LOCK_VIEW != 0 {
            allows -= Permissions::READ;
        }
        allows
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
                .pages
                .get(page)
                .allows
                .contains(Permissions::EXECUTE)
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
        let (compiled, origin, slow) = self.compiler.compiled(
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
        for slow in slow {
            self.slow_path(slow.reason, slow.detail, slow.nanoseconds);
        }
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

    /// Writes the registers the kernel set while the thread was running into its block.
    fn write_held(&mut self, thread: ThreadId) {
        if let Some(record) = self.threads.get_mut(&thread)
            && let Some(held) = record.held.take()
        {
            let record = &self.threads[&thread];
            self.write_block(record, &held);
        }
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
            pages: Pages::new(pages as u64),
            shown: BTreeMap::new(),
            written: BTreeSet::new(),
            viewing: BTreeSet::new(),
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
            let mut merged: Vec<Range<usize>> = Vec::new();
            for run in runs {
                let run = (run.start / host_page * host_page) as usize
                    ..run.end.next_multiple_of(host_page) as usize;
                match merged.last_mut() {
                    Some(last) if last.end >= run.start => last.end = last.end.max(run.end),
                    _ => merged.push(run),
                }
            }
            // SAFETY: runs of whole host pages, in order, inside the two processes' own mappings, the
            // new one still all zeros; no guest of either runs meanwhile.
            unsafe { memory::copy_pages(from, to, &merged) }
        };
        copy(
            source.base,
            into.base,
            &mut source
                .pages
                .data_runs()
                .into_iter()
                .map(|pages| pages.start * PAGE_SIZE..pages.end * PAGE_SIZE),
        );
        into.pages.clone_from(&source.pages);
        into.shown.clone_from(&source.shown);
        let layout = self.layout;
        if code {
            // The table is the same records', and its slots are the source's, so both are copied. Its
            // bytes are zero but where a page or the one after it allows something, so only those of
            // the records' chunks and the page before each.
            let at = u64::from(layout.permissions_at);
            let table = source
                .pages
                .chunks()
                .map(|pages| at + pages.start.saturating_sub(1)..at + pages.end);
            let slots = source.ranges.iter().map(|code| {
                let entry = |address: u64| u64::from(layout.slots_at) + 4 * (address / G::ALIGN);
                entry(code.range.start)..entry(code.range.end.min(layout.ceiling))
            });
            let mut runs: Vec<Range<u64>> = table.chain(slots).collect();
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

    /// Makes the copies of every page the guest wrote under a shared lock, and locks them again; the
    /// pages under the view lock then hold the same and are let go, and a step ends. Two pages written
    /// that show the same block's page (only in a step) each give the bytes they changed.
    fn flush(&mut self, memory: MemoryId) {
        if let Some(step) = self.stepping.take_if(|step| step.memory == memory) {
            for (address, slot) in step.slots.into_iter().rev() {
                self.set_slot(memory, address, slot);
            }
        }
        let process = self.processes.get_mut(&memory).expect("a memory");
        let pages = std::mem::take(&mut process.written);
        let viewing = std::mem::take(&mut process.viewing);
        let mut shown: BTreeMap<(SharedId, u64), Vec<u64>> = BTreeMap::new();
        for &page in &pages {
            if let Some(at) = self.shown_at(memory, page * PAGE_SIZE) {
                shown.entry(at).or_default().push(page);
            }
        }
        for ((block, offset), pages) in shown {
            let mut bytes = vec![0; PAGE_SIZE as usize];
            self.copy_out(memory, pages[0] * PAGE_SIZE, &mut bytes);
            if pages.len() > 1 {
                let before = &self.blocks[&block][offset as usize..(offset + PAGE_SIZE) as usize];
                bytes.copy_from_slice(before);
                let mut view = vec![0; PAGE_SIZE as usize];
                for &page in &pages {
                    self.copy_out(memory, page * PAGE_SIZE, &mut view);
                    for ((byte, new), old) in bytes.iter_mut().zip(&view).zip(before) {
                        if new != old {
                            *byte = *new;
                        }
                    }
                }
            }
            self.block_write(block, offset, &bytes);
            for page in pages {
                let process = self.processes.get_mut(&memory).expect("a memory");
                process
                    .pages
                    .update(page..page + 1, |p| p.locks |= LOCK_SHARED);
                self.write_table(memory, page..page + 1);
            }
        }
        for page in viewing {
            let process = self.processes.get_mut(&memory).expect("a memory");
            process
                .pages
                .update(page..page + 1, |p| p.locks &= !LOCK_VIEW);
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
        process
            .pages
            .update(pages.clone(), |p| p.locks &= !(LOCK_SHARED | LOCK_VIEW));
        for page in pages.clone() {
            process.written.remove(&page);
            process.viewing.remove(&page);
        }
        self.write_table(memory, pages);
    }

    /// The other pages of a memory that show the same page of a block as `page` does.
    fn other_views(&self, memory: MemoryId, page: u64) -> Vec<u64> {
        let Some((block, offset)) = self.shown_at(memory, page * PAGE_SIZE) else {
            return Vec::new();
        };
        self.process(memory)
            .shown
            .iter()
            .filter(|(start, shown)| {
                let end = shown.offset + (shown.end - *start);
                shown.block == block && (shown.offset..end).contains(&offset)
            })
            .map(|(start, shown)| (start + (offset - shown.offset)) / PAGE_SIZE)
            .filter(|other| *other != page)
            .collect()
    }

    /// A guest access to a locked page that the kernel allows. Reaching a page under the view lock
    /// makes the copies first. A write marks the page written and unlocks it until its copies are next
    /// made, and puts the view lock on the other pages of the memory that show the same block's page,
    /// so one page at a time holds what the guest wrote there. When a neighbouring page shows the same
    /// block's page, both are unlocked and the write is made one instruction at a time: true.
    fn settle(&mut self, memory: MemoryId, page: u64, write: bool) -> bool {
        if self.process(memory).pages.get(page).locks & LOCK_VIEW != 0 {
            self.slow_path(reason::SHARED_PAGE_WRITTEN_ELSEWHERE, page * PAGE_SIZE, 0);
            self.flush(memory);
        }
        if !write {
            return false;
        }
        self.slow_path(reason::FIRST_WRITE_TO_SHARED_PAGE, page * PAGE_SIZE, 0);
        let mut others = self.other_views(memory, page);
        let mut unlock = vec![page];
        let step = others.iter().any(|other| other.abs_diff(page) == 1);
        if step {
            self.slow_path(reason::WRITE_STEPPED, page * PAGE_SIZE, 0);
            self.flush(memory);
            others.retain(|other| {
                let neighbour = other.abs_diff(page) == 1;
                if neighbour {
                    unlock.push(*other);
                }
                !neighbour
            });
        }
        let process = self.processes.get_mut(&memory).expect("a memory");
        for &page in &unlock {
            process
                .pages
                .update(page..page + 1, |p| p.locks &= !LOCK_SHARED);
            process.written.insert(page);
        }
        for &other in &others {
            process
                .pages
                .update(other..other + 1, |p| p.locks |= LOCK_VIEW);
            process.viewing.insert(other);
        }
        for page in unlock.into_iter().chain(others) {
            self.write_table(memory, page..page + 1);
        }
        step
    }

    /// The piece of the one instruction at an address, of look-ahead none, made once per thread.
    fn exact_piece(
        &mut self,
        mut store: impl AsContextMut<Data = Data<G>>,
        thread: ThreadId,
        address: u64,
    ) -> Func {
        if let Some(piece) = self.threads[&thread].exact.get(&address) {
            return *piece;
        }
        let started = Instant::now();
        let memory = self.threads[&thread].memory;
        let index = self.range_at(memory, address);
        let range = self.process(memory).ranges[index].range.clone();
        let instance = self.threads[&thread]
            .instance
            .clone()
            .expect("a running thread's instances");
        let code = self.code(memory, &range);
        let one = LookAhead::Bounded(1);
        let compiled = self.compile(&code, vec![address - range.start], one, true, None);
        let load = Load {
            compiled,
            load_address: range.start,
            slot_base: contract::EXACT_SLOT as u32,
            live: false,
        };
        let record = &self.threads[&thread];
        self.instantiate_load(
            &mut store,
            self.process(memory),
            record,
            &instance,
            &load,
            load.slot_base,
        );
        let table_slot = contract::EXACT_SLOT as u64;
        let Some(Ref::Func(Some(piece))) = instance.table.get(&mut store, table_slot) else {
            panic!("the piece is in the exact slot");
        };
        self.threads
            .get_mut(&thread)
            .expect("a thread")
            .exact
            .insert(address, piece);
        let nanoseconds = started.elapsed().as_nanos() as u64;
        self.slow_path(Reason::TranslatedAtRunTime.name(), address, nanoseconds);
        piece
    }

    /// Makes the lookup run the thread's instruction alone from its piece in the exact slot, and send
    /// the address after it to `load_code`, where the step ends.
    fn step(&mut self, store: &mut Store<Data<G>>, thread: ThreadId) {
        let memory = self.threads[&thread].memory;
        let pc = self.read_block(&self.threads[&thread]).pc;
        let piece = self.exact_piece(&mut *store, thread, pc);
        let instance = self.threads[&thread]
            .instance
            .clone()
            .expect("a running thread's instances");
        let table_slot = contract::EXACT_SLOT as u64;
        instance
            .table
            .set(&mut *store, table_slot, Ref::Func(Some(piece)))
            .expect("the exact slot");
        let mut code = vec![0; G::MAX_LENGTH as usize];
        let length = self.layout.ceiling.saturating_sub(pc).min(G::MAX_LENGTH) as usize;
        self.copy_out(memory, pc, &mut code[..length]);
        let (_, length) = G::decode(&code[..length]).expect("the instruction that wrote");
        let next = pc + u64::from(length);
        let slots = [(pc, self.slot(memory, pc)), (next, self.slot(memory, next))];
        self.set_slot(memory, pc, contract::EXACT_SLOT as u32);
        self.set_slot(memory, next, 0);
        self.stepping = Some(Stepping { memory, slots });
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
    let record = process.pages.get(page);
    let write = access == 1;
    let (needs, locks) = if write {
        (Permissions::WRITE, record.locks)
    } else {
        (Permissions::READ, record.locks & LOCK_VIEW)
    };
    if locks != 0 && record.allows.contains(needs) {
        caller.data_mut().lock_fault = Some((page, write));
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
    let side = side(caller);
    let (memory, thread) = (caller.data().memory, caller.data().thread);
    // Pages under the view lock do not hold what the guest wrote through another page yet; a step ends
    // here.
    if !side.process(memory).viewing.is_empty() || side.stepping.is_some() {
        side.flush(memory);
    }
    // Code runs only from pages that allow it, as on Linux.
    let page = address / PAGE_SIZE;
    let runs = side
        .process(memory)
        .pages
        .get(page)
        .allows
        .contains(Permissions::EXECUTE);
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
        let piece = side.exact_piece(&mut *caller, thread, address);
        instance
            .table
            .set(
                &mut *caller,
                contract::EXACT_SLOT as u64,
                Ref::Func(Some(piece)),
            )
            .expect("the exact slot");
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
            .pages
            .get(address / PAGE_SIZE)
            .allows
            .contains(Permissions::READ)
    }
}

/// The guest's own code changed hook (ARM64 `ic ivau`): drops the pieces of the line at the address,
/// and of the same line wherever its block of shared pages is shown, as the instruction reaches every
/// CPU.
fn code_line_changed<G>(caller: &mut Caller<'_, Data<G>>, address: u64)
where
    G: Guest<Block = RegisterBlock>,
{
    let side = side(caller);
    let memory = caller.data().memory;
    let line = address & !63;
    let mut lines = vec![(memory, line)];
    if let Some((block, offset)) = side.shown_at(memory, line) {
        for (other, process) in &side.processes {
            for (start, shown) in &process.shown {
                let end = shown.offset + (shown.end - start);
                if shown.block == block && (shown.offset..end).contains(&offset) {
                    lines.push((*other, start + (offset - shown.offset)));
                }
            }
        }
    }
    lines.sort_unstable();
    lines.dedup();
    for (memory, line) in lines {
        if side.forget(memory, line..line + 64) {
            let process = side.processes.get_mut(&memory).expect("a memory");
            if let Some(range) = process.ranges.iter_mut().find(|r| r.range.contains(&line)) {
                range.changed = true;
            }
            side.slow_path(reason::CODE_CHANGED_AFTER_IT_RAN, line, 0);
        }
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
                let side = side(&caller);
                if side.restored {
                    side.slow_path(reason::COUNTER_WITHOUT_KERNEL_OFFSET, 0, 0);
                    side.send_warnings(caller.data().answer.expect("a run"));
                }
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
            // Registers the kernel set in answer to an exit handed on after the thread left.
            self.write_held(thread);
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
            self.write_held(thread);
            let gone = self.process(memory).dropped || self.threads[&thread].dropped;
            if gone {
                break;
            }
            if let Some((page, write)) = lock_fault {
                if self.settle(memory, page, write) {
                    self.step(&mut store, thread);
                }
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
        self.write_held(thread);
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
        side.restored = true;
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
        process.pages.update(pages, |p| p.data = true);
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
            process.pages.update(whole, |p| p.data = false);
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
        process.pages.update(pages.clone(), |p| {
            p.allows = permissions;
            p.data |= !permissions.is_empty();
        });
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
        process.pages.update(pages.clone(), |p| {
            p.locks |= LOCK_SHARED;
            p.data = true;
        });
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

#[cfg(test)]
mod tests {
    use super::{CHUNK, Compiler, Page, Pages, Permissions};

    /// A compiler copied into a fork of this process, where its pool's threads are not, is dropped
    /// there whatever those threads were doing at the fork, as a save's copy drops the back end it
    /// was brought back from.
    #[test]
    fn a_compiler_copied_by_fork_is_dropped_there() {
        let compiler = Compiler::<wcontainers_arm64::Arm64>::new(None);
        for round in 0..400u32 {
            // The pool's threads asleep, then woken, and the fork at a different moment of their
            // waking each round.
            std::thread::sleep(std::time::Duration::from_millis(2));
            for _ in 0..8 {
                compiler.pool.spawn(|| {
                    let _ = (0..2_000u64).fold(0u64, |a, b| a.wrapping_mul(31) ^ b);
                });
            }
            for _ in 0..round % 20 * 50 {
                std::hint::spin_loop();
            }
            // SAFETY: the copy only drops the compiler and leaves with `_exit`.
            let child = unsafe { libc::fork() };
            assert!(child >= 0, "fork");
            if child == 0 {
                drop(compiler);
                // SAFETY: ends the copy without running anything of the parent's.
                unsafe { libc::_exit(0) };
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let mut status = 0;
            // SAFETY: waits for the copy made above, and kills it if it never ends.
            unsafe {
                while libc::waitpid(child, &mut status, libc::WNOHANG) != child {
                    if std::time::Instant::now() > deadline {
                        libc::kill(child, libc::SIGKILL);
                        panic!("dropping the compiler in the copy did not finish in 20 s");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            assert!(libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0);
        }
    }

    /// Records change only where asked, copies share chunks until one side changes, a chunk left
    /// holding nothing is dropped, and the data runs cross chunk boundaries.
    #[test]
    fn page_records_are_kept_in_shared_chunks() {
        let chunk = CHUNK as u64;
        let mut pages = Pages::new(4 * chunk);
        pages.update(chunk - 2..chunk + 3, |p| {
            p.allows = Permissions::READ;
            p.data = true;
        });
        assert!(pages.get(chunk - 3) == Page::NOTHING);
        assert_eq!(pages.get(chunk + 2).allows, Permissions::READ);
        assert!(pages.get(chunk + 3) == Page::NOTHING);
        assert_eq!(pages.data_runs(), vec![chunk - 2..chunk + 3]);
        assert_eq!(
            pages.chunks().collect::<Vec<_>>(),
            vec![0..chunk, chunk..2 * chunk]
        );
        let copy = pages.clone();
        pages.update(chunk..chunk + 1, |p| p.locks = 1);
        assert_eq!(copy.get(chunk).locks, 0);
        assert_eq!(pages.get(chunk).locks, 1);
        // Taking a lock off pages that hold nothing makes no chunk.
        pages.update(3 * chunk..4 * chunk, |p| p.locks = 0);
        assert_eq!(pages.chunks().count(), 2);
        pages.update(0..2 * chunk, |p| *p = Page::NOTHING);
        assert_eq!(pages.chunks().count(), 0);
        assert!(pages.data_runs().is_empty());
        assert_eq!(copy.data_runs(), vec![chunk - 2..chunk + 3]);
        // Beyond the records, a page holds nothing.
        assert!(pages.get(5 * chunk) == Page::NOTHING);
    }
}
