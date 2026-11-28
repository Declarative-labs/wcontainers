//! The state memory every thread of a space shares with the executor, reached only through WebAssembly's own accesses.
//!
//! The executor reaches it the way the translator's test runner does.

use std::sync::{Arc, Mutex, PoisonError};
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    MemArg, MemArgTarget, MemoryType, TypeSection, ValType,
};
use wasmtime::{Engine, Extern, Instance, Module, SharedMemory, Store, TypedFunc};
use wcontainers_translator::contract::{ENV, STATE, WASM_PAGE_BYTES};

/// The state memory and the module that reaches it, which any host thread may use.
#[derive(Clone)]
pub(crate) struct StateMemory {
    memory: SharedMemory,
    access: Arc<Mutex<Access>>,
}

/// The words one call loads or stores, as a tuple wasmtime can pass.
type Words = (u64, u64, u64, u64, u64, u64, u64, u64);
/// How many words that is, and their bytes.
const WORDS: usize = size_of::<Words>() / size_of::<u64>();
const CHUNK: usize = size_of::<Words>();
/// What a store of words takes: the offset, then the words.
type StoreWords = (u32, u64, u64, u64, u64, u64, u64, u64, u64);

/// The module's instance in a store of its own, which has no memory of its own to reserve.
struct Access {
    store: Store<()>,
    load_words: TypedFunc<u32, Words>,
    store_words: TypedFunc<StoreWords, ()>,
    load_byte: TypedFunc<u32, u32>,
    store_byte: TypedFunc<(u32, u32), ()>,
    fill: TypedFunc<(u32, u32, u32), ()>,
    store_u64: TypedFunc<(u32, u64), ()>,
}

/// The module's exports, in the order of its functions.
const LOAD_WORDS: &str = "load_words";
const STORE_WORDS: &str = "store_words";
const LOAD_BYTE: &str = "load_byte";
const STORE_BYTE: &str = "store_byte";
const FILL: &str = "fill";
const STORE_U32: &str = "store_u32";
const STORE_U64: &str = "store_u64";

/// The most pages a 32-bit memory has, so that the import takes a state memory of any size.
///
/// https://webassembly.github.io/spec/core/valid/types.html#memory-types
const MOST_PAGES: u64 = (u32::MAX as u64 + 1) / WASM_PAGE_BYTES;

/// The state memory's index, the module's only memory.
const STATE_INDEX: u32 = u32::MIN;
/// The parameters of each function: an offset in the state memory, then the values.
const OFFSET_PARAMETER: u32 = u32::MIN;
const FIRST_VALUE_PARAMETER: u32 = OFFSET_PARAMETER + 1;

impl StateMemory {
    /// Makes a state memory of `pages` WebAssembly pages, all zero; an error when the host refuses its memory.
    pub(crate) fn new(engine: &Engine, pages: u32) -> wasmtime::Result<StateMemory> {
        let memory = SharedMemory::new(engine, wasmtime::MemoryType::shared(pages, pages))?;
        let mut store = Store::new(engine, ());
        let instance = Instance::new(
            &mut store,
            &module(engine),
            &[Extern::SharedMemory(memory.clone())],
        )?;
        let access = Access {
            load_words: instance.get_typed_func(&mut store, LOAD_WORDS)?,
            store_words: instance.get_typed_func(&mut store, STORE_WORDS)?,
            load_byte: instance.get_typed_func(&mut store, LOAD_BYTE)?,
            store_byte: instance.get_typed_func(&mut store, STORE_BYTE)?,
            fill: instance.get_typed_func(&mut store, FILL)?,
            store_u64: instance.get_typed_func(&mut store, STORE_U64)?,
            store,
        };
        Ok(StateMemory {
            memory,
            access: Arc::new(Mutex::new(access)),
        })
    }

    /// Returns the memory itself, for a module that imports it.
    pub(crate) fn shared_memory(&self) -> SharedMemory {
        self.memory.clone()
    }

    /// Copies the bytes from `at` into `into`; they must lie inside the memory.
    pub(crate) fn read(&self, at: usize, into: &mut [u8]) {
        let mut access = self.access.lock().unwrap_or_else(PoisonError::into_inner);
        let Access {
            store,
            load_words,
            load_byte,
            ..
        } = &mut *access;
        let (chunks, rest) = into.as_chunks_mut::<CHUNK>();
        let mut offset = at;
        for chunk in chunks {
            let (a, b, c, d, e, f, g, h) = load_words
                .call(&mut *store, address(offset))
                .expect("bytes inside the state memory");
            let (words, _) = chunk.as_chunks_mut::<{ size_of::<u64>() }>();
            for (bytes, word) in words.iter_mut().zip([a, b, c, d, e, f, g, h]) {
                *bytes = word.to_le_bytes();
            }
            offset += CHUNK;
        }
        for byte in rest {
            let value = load_byte
                .call(&mut *store, address(offset))
                .expect("a byte inside the state memory");
            *byte = u8::try_from(value).expect("a byte load gives a byte");
            offset += 1;
        }
    }

    /// Copies `bytes` to the memory from `at`; they must fit inside it.
    pub(crate) fn write(&self, at: usize, bytes: &[u8]) {
        let mut access = self.access.lock().unwrap_or_else(PoisonError::into_inner);
        let Access {
            store,
            store_words,
            store_byte,
            ..
        } = &mut *access;
        let (chunks, rest) = bytes.as_chunks::<CHUNK>();
        let mut offset = at;
        for chunk in chunks {
            let (words, _) = chunk.as_chunks::<{ size_of::<u64>() }>();
            let words: [u64; WORDS] = std::array::from_fn(|i| u64::from_le_bytes(words[i]));
            let [a, b, c, d, e, f, g, h] = words;
            store_words
                .call(&mut *store, (address(offset), a, b, c, d, e, f, g, h))
                .expect("bytes inside the state memory");
            offset += CHUNK;
        }
        for &byte in rest {
            store_byte
                .call(&mut *store, (address(offset), u32::from(byte)))
                .expect("a byte inside the state memory");
            offset += 1;
        }
    }

    /// Sets `length` bytes from `at` to `byte`; they must fit inside the memory.
    pub(crate) fn fill(&self, at: usize, length: usize, byte: u8) {
        let mut access = self.access.lock().unwrap_or_else(PoisonError::into_inner);
        let Access { store, fill, .. } = &mut *access;
        fill.call(&mut *store, (address(at), address(length), u32::from(byte)))
            .expect("bytes inside the state memory");
    }

    /// Writes back the bytes of `after` that differ from `before`, so that a byte another thread wrote meanwhile is kept.
    pub(crate) fn write_changed(&self, at: usize, before: &[u8], after: &[u8]) {
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

    /// Stores a u64 at `at`, which must be 8 byte aligned, as one sequentially consistent atomic store.
    pub(crate) fn store_u64(&self, at: usize, value: u64) {
        let mut access = self.access.lock().unwrap_or_else(PoisonError::into_inner);
        let Access {
            store, store_u64, ..
        } = &mut *access;
        store_u64
            .call(&mut *store, (address(at), value))
            .expect("an aligned word inside the state memory");
    }
}

/// Returns `at` as an offset in the state memory, a 32-bit memory.
fn address(at: usize) -> u32 {
    u32::try_from(at).expect("an offset inside the state memory")
}

/// Returns the module compiled for an engine, once for each engine.
fn module(engine: &Engine) -> Module {
    static MADE: Mutex<Vec<(Engine, Module)>> = Mutex::new(Vec::new());
    let kept = |made: &[(Engine, Module)]| {
        made.iter()
            .find(|(e, _)| Engine::same(e, engine))
            .map(|(_, module)| module.clone())
    };
    if let Some(module) = kept(&MADE.lock().unwrap_or_else(PoisonError::into_inner)) {
        return module;
    }
    // Compiled with no lock held: the compiler's threads may run other work of this thread's pool meanwhile.
    let module = Module::new(engine, module_bytes()).expect("the state memory's module");
    let mut made = MADE.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(module) = kept(&made) {
        return module;
    }
    made.push((engine.clone(), module.clone()));
    module
}

/// Returns the module, whose functions load and store the imported state memory's words and bytes.
fn module_bytes() -> Vec<u8> {
    let at = |offset: usize, align: usize| MemArg {
        offset: offset as u64,
        align: align.trailing_zeros(),
        target: MemArgTarget::Memory(STATE_INDEX),
        ordering: None,
    };
    let words = [ValType::I64; WORDS];
    let word_offsets = (0..WORDS).map(|i| i * size_of::<u64>());
    // Each function: its parameters, its results and its body after the parameters are pushed.
    let mut functions: Vec<(Vec<ValType>, Vec<ValType>, Function)> = Vec::new();
    let mut f = Function::new([]);
    let mut sink = f.instructions();
    for offset in word_offsets.clone() {
        sink.local_get(OFFSET_PARAMETER)
            .i64_load(at(offset, size_of::<u8>()));
    }
    sink.end();
    functions.push((vec![ValType::I32], words.to_vec(), f));
    let mut f = Function::new([]);
    let mut sink = f.instructions();
    for (value, offset) in (FIRST_VALUE_PARAMETER..).zip(word_offsets) {
        sink.local_get(OFFSET_PARAMETER)
            .local_get(value)
            .i64_store(at(offset, size_of::<u8>()));
    }
    sink.end();
    let mut store_words_parameters = vec![ValType::I32];
    store_words_parameters.extend(words);
    functions.push((store_words_parameters, Vec::new(), f));
    let mut f = Function::new([]);
    f.instructions()
        .local_get(OFFSET_PARAMETER)
        .i32_load8_u(at(usize::MIN, size_of::<u8>()))
        .end();
    functions.push((vec![ValType::I32], vec![ValType::I32], f));
    let mut f = Function::new([]);
    f.instructions()
        .local_get(OFFSET_PARAMETER)
        .local_get(FIRST_VALUE_PARAMETER)
        .i32_store8(at(usize::MIN, size_of::<u8>()))
        .end();
    functions.push((vec![ValType::I32, ValType::I32], Vec::new(), f));
    // fill(at, length, byte): memory.fill takes the place, the byte and the length.
    let mut f = Function::new([]);
    f.instructions()
        .local_get(OFFSET_PARAMETER)
        .local_get(FIRST_VALUE_PARAMETER + 1)
        .local_get(FIRST_VALUE_PARAMETER)
        .memory_fill(STATE_INDEX)
        .end();
    functions.push((
        vec![ValType::I32, ValType::I32, ValType::I32],
        Vec::new(),
        f,
    ));
    let mut f = Function::new([]);
    f.instructions()
        .local_get(OFFSET_PARAMETER)
        .local_get(FIRST_VALUE_PARAMETER)
        .i32_atomic_store(at(usize::MIN, size_of::<u32>()))
        .end();
    functions.push((vec![ValType::I32, ValType::I32], Vec::new(), f));
    let mut f = Function::new([]);
    f.instructions()
        .local_get(OFFSET_PARAMETER)
        .local_get(FIRST_VALUE_PARAMETER)
        .i64_atomic_store(at(usize::MIN, size_of::<u64>()))
        .end();
    functions.push((vec![ValType::I32, ValType::I64], Vec::new(), f));
    let names = [
        LOAD_WORDS,
        STORE_WORDS,
        LOAD_BYTE,
        STORE_BYTE,
        FILL,
        STORE_U32,
        STORE_U64,
    ];
    assert_eq!(names.len(), functions.len());

    let mut module = wasm_encoder::Module::new();
    let mut types = TypeSection::new();
    for (parameters, results, _) in &functions {
        types
            .ty()
            .function(parameters.iter().copied(), results.iter().copied());
    }
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
    let mut declared = FunctionSection::new();
    let mut exports = ExportSection::new();
    for (index, name) in (u32::MIN..).zip(names) {
        declared.function(index);
        exports.export(name, ExportKind::Func, index);
    }
    module.section(&declared);
    module.section(&exports);
    let mut code = CodeSection::new();
    for (_, _, f) in &functions {
        code.function(f);
    }
    module.section(&code);
    module.finish()
}
