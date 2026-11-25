//! The state memory the guest thread shares with the runner, reached only through WebAssembly's own copies and stores.

use std::sync::{Arc, Mutex};
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    MemArg, MemArgTarget, MemorySection, MemoryType, TypeSection, ValType,
};
use wasmtime::{Engine, Extern, Instance, Memory, Module, SharedMemory, Store, TypedFunc};
use wcontainers_translator::contract::{ENV, STATE, WASM_PAGE_BYTES};

/// The state memory and the module that reaches it, which another host thread may use as well.
#[derive(Clone)]
pub struct StateMemory {
    memory: SharedMemory,
    access: Arc<Mutex<Access>>,
}

/// The module's instance in a store of its own, with its private memory for the bytes copied in and out.
struct Access {
    store: Store<()>,
    scratch: Memory,
    copy_out: TypedFunc<(u32, u32), ()>,
    copy_in: TypedFunc<(u32, u32), ()>,
    store_u32: TypedFunc<(u32, u32), ()>,
    store_u64: TypedFunc<(u32, u64), ()>,
}

/// The module's exports.
const COPY_OUT: &str = "copy_out";
const COPY_IN: &str = "copy_in";
const STORE_U32: &str = "store_u32";
const STORE_U64: &str = "store_u64";
const SCRATCH: &str = "scratch";

/// The most pages a 32-bit memory has, so that the import takes a state memory of any size.
///
/// https://webassembly.github.io/spec/core/valid/types.html#memory-types
const MOST_PAGES: u64 = (u32::MAX as u64 + 1) / WASM_PAGE_BYTES;

/// The import's index among the memories, which come before the module's own.
const STATE_INDEX: u32 = u32::MIN;
const SCRATCH_INDEX: u32 = STATE_INDEX + 1;
/// Where the scratch memory's copy starts.
const SCRATCH_AT: u32 = u32::MIN;
/// The module's two function types.
const TWO_WORDS: u32 = u32::MIN;
const WORD_AND_WIDE: u32 = TWO_WORDS + 1;
/// The parameters of each function: an offset in the state memory, then a length or a value.
const OFFSET_PARAMETER: u32 = u32::MIN;
const LENGTH_OR_VALUE_PARAMETER: u32 = OFFSET_PARAMETER + 1;

impl StateMemory {
    /// Makes a state memory of `pages` WebAssembly pages, all zero.
    pub fn new(engine: &Engine, pages: u32) -> StateMemory {
        let memory = SharedMemory::new(engine, wasmtime::MemoryType::shared(pages, pages))
            .expect("state memory");
        let mut store = Store::new(engine, ());
        let instance = Instance::new(
            &mut store,
            &module(engine),
            &[Extern::SharedMemory(memory.clone())],
        )
        .expect("the state memory's module");
        let access = Access {
            scratch: instance
                .get_memory(&mut store, SCRATCH)
                .expect("the scratch memory"),
            copy_out: instance.get_typed_func(&mut store, COPY_OUT).unwrap(),
            copy_in: instance.get_typed_func(&mut store, COPY_IN).unwrap(),
            store_u32: instance.get_typed_func(&mut store, STORE_U32).unwrap(),
            store_u64: instance.get_typed_func(&mut store, STORE_U64).unwrap(),
            store,
        };
        StateMemory {
            memory,
            access: Arc::new(Mutex::new(access)),
        }
    }

    /// Returns the memory itself, for a module that imports it.
    pub fn shared_memory(&self) -> SharedMemory {
        self.memory.clone()
    }

    /// Returns how many bytes the memory has.
    pub fn data_size(&self) -> usize {
        self.memory.data_size()
    }

    /// Copies the bytes from `at` into `into`; they must lie inside the memory.
    pub fn read(&self, at: usize, into: &mut [u8]) {
        let mut access = self.access.lock().unwrap();
        access.make_room(into.len());
        let Access {
            store,
            scratch,
            copy_out,
            ..
        } = &mut *access;
        copy_out
            .call(&mut *store, (address(at), address(into.len())))
            .expect("bytes inside the state memory");
        scratch
            .read(&*store, SCRATCH_AT as usize, into)
            .expect("bytes inside the scratch memory");
    }

    /// Copies `bytes` to the memory from `at`; they must fit inside it.
    pub fn write(&self, at: usize, bytes: &[u8]) {
        let mut access = self.access.lock().unwrap();
        access.make_room(bytes.len());
        let Access {
            store,
            scratch,
            copy_in,
            ..
        } = &mut *access;
        scratch
            .write(&mut *store, SCRATCH_AT as usize, bytes)
            .expect("bytes inside the scratch memory");
        copy_in
            .call(&mut *store, (address(at), address(bytes.len())))
            .expect("bytes inside the state memory");
    }

    /// Writes back the bytes of `after` that differ from `before`, so that a byte another thread wrote meanwhile is kept.
    pub fn write_changed(&self, at: usize, before: &[u8], after: &[u8]) {
        let mut offset = usize::MIN;
        while offset < after.len() {
            if before[offset] == after[offset] {
                offset += 1;
                continue;
            }
            let end = (offset..after.len())
                .find(|&i| before[i] == after[i])
                .unwrap_or(after.len());
            self.write(at + offset, &after[offset..end]);
            offset = end;
        }
    }

    /// Stores a u32 at `at`, which must be 4 byte aligned, as one sequentially consistent atomic store.
    pub fn store_u32(&self, at: usize, value: u32) {
        let mut access = self.access.lock().unwrap();
        let Access {
            store, store_u32, ..
        } = &mut *access;
        store_u32
            .call(&mut *store, (address(at), value))
            .expect("an aligned word inside the state memory");
    }

    /// Stores a u64 at `at`, which must be 8 byte aligned, as one sequentially consistent atomic store.
    pub fn store_u64(&self, at: usize, value: u64) {
        let mut access = self.access.lock().unwrap();
        let Access {
            store, store_u64, ..
        } = &mut *access;
        store_u64
            .call(&mut *store, (address(at), value))
            .expect("an aligned word inside the state memory");
    }
}

impl Access {
    /// Grows the scratch memory to hold at least `length` bytes.
    fn make_room(&mut self, length: usize) {
        let have = self.scratch.data_size(&self.store);
        if have < length {
            let pages = (length - have).div_ceil(WASM_PAGE_BYTES as usize) as u64;
            self.scratch
                .grow(&mut self.store, pages)
                .expect("a scratch memory that grows");
        }
    }
}

/// Returns `at` as an offset in the state memory, a 32-bit memory.
fn address(at: usize) -> u32 {
    u32::try_from(at).unwrap()
}

/// Returns the module compiled for an engine, once for each engine.
fn module(engine: &Engine) -> Module {
    static MADE: Mutex<Vec<(Engine, Module)>> = Mutex::new(Vec::new());
    let mut made = MADE.lock().unwrap();
    if let Some((_, module)) = made.iter().find(|(e, _)| Engine::same(e, engine)) {
        return module.clone();
    }
    let module = Module::new(engine, module_bytes()).expect("the state memory's module");
    made.push((engine.clone(), module.clone()));
    module
}

/// Returns the module, which copies between the imported state memory and a scratch memory of its own and stores words.
fn module_bytes() -> Vec<u8> {
    let mut module = wasm_encoder::Module::new();
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32, ValType::I32], []);
    types.ty().function([ValType::I32, ValType::I64], []);
    module.section(&types);
    let mut imports = ImportSection::new();
    imports.import(
        ENV,
        STATE,
        EntityType::Memory(MemoryType {
            minimum: u64::MIN,
            maximum: Some(MOST_PAGES),
            memory64: false,
            shared: true,
            page_size_log2: None,
        }),
    );
    module.section(&imports);
    let mut functions = FunctionSection::new();
    for ty in [TWO_WORDS, TWO_WORDS, TWO_WORDS, WORD_AND_WIDE] {
        functions.function(ty);
    }
    module.section(&functions);
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: u64::MIN,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memories);
    let mut exports = ExportSection::new();
    for (index, name) in [COPY_OUT, COPY_IN, STORE_U32, STORE_U64]
        .into_iter()
        .enumerate()
    {
        exports.export(name, ExportKind::Func, index as u32);
    }
    exports.export(SCRATCH, ExportKind::Memory, SCRATCH_INDEX);
    module.section(&exports);
    let mut code = CodeSection::new();
    // copy_out(at, length): the state memory's bytes from `at` to the scratch memory's start.
    let mut f = Function::new([]);
    f.instructions()
        .i32_const(SCRATCH_AT.cast_signed())
        .local_get(OFFSET_PARAMETER)
        .local_get(LENGTH_OR_VALUE_PARAMETER)
        .memory_copy(SCRATCH_INDEX, STATE_INDEX)
        .end();
    code.function(&f);
    // copy_in(at, length): the scratch memory's first bytes to the state memory from `at`.
    let mut f = Function::new([]);
    f.instructions()
        .local_get(OFFSET_PARAMETER)
        .i32_const(SCRATCH_AT.cast_signed())
        .local_get(LENGTH_OR_VALUE_PARAMETER)
        .memory_copy(STATE_INDEX, SCRATCH_INDEX)
        .end();
    code.function(&f);
    // store_u32(at, value) and store_u64(at, value).
    let aligned = |bytes: usize| MemArg {
        offset: u64::MIN,
        align: bytes.trailing_zeros(),
        target: MemArgTarget::Memory(STATE_INDEX),
        ordering: None,
    };
    let mut f = Function::new([]);
    f.instructions()
        .local_get(OFFSET_PARAMETER)
        .local_get(LENGTH_OR_VALUE_PARAMETER)
        .i32_atomic_store(aligned(size_of::<u32>()))
        .end();
    code.function(&f);
    let mut f = Function::new([]);
    f.instructions()
        .local_get(OFFSET_PARAMETER)
        .local_get(LENGTH_OR_VALUE_PARAMETER)
        .i64_atomic_store(aligned(size_of::<u64>()))
        .end();
    code.function(&f);
    module.section(&code);
    module.finish()
}
