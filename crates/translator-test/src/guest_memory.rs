//! The guest's memory as the runner holds it: its own, or shared between threads.
//!
//! A shared one, for the form that shares it (`AccessOrdering::shares_memory`), the runner reaches only through
//! WebAssembly's own loads and stores.

use std::sync::{Arc, Mutex};
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    MemArg, MemArgTarget, MemoryType, TypeSection, ValType,
};
use wasmtime::{
    AsContext, AsContextMut, Engine, Extern, Instance, Memory, MemoryTypeBuilder, Module,
    SharedMemory, Store, TypedFunc,
};
use wcontainers_translator::contract::{ENV, MEMORY, SHARED_MEMORY_MOST_PAGES, WASM_PAGE_BYTES};

// The module's exports, in the order of its functions, and where a function's values start, as the state memory's.
use crate::state_memory::{FIRST_VALUE_PARAMETER, LOAD_BYTE, LOAD_WORDS, STORE_BYTE, STORE_WORDS};

/// The guest's memory, as code of the runner's form imports it.
#[derive(Clone)]
pub enum GuestMemory {
    Own(Memory),
    Shared(SharedGuestMemory),
}

/// An access that runs past the end of the guest's memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutOfBounds;

impl GuestMemory {
    /// Returns how many bytes the memory has.
    pub fn data_size(&self, store: impl AsContext) -> u64 {
        match self {
            GuestMemory::Own(memory) => memory.data_size(store) as u64,
            GuestMemory::Shared(memory) => memory.memory.data_size() as u64,
        }
    }

    /// Returns how many WebAssembly pages the memory has.
    pub fn pages(&self, store: impl AsContext) -> u64 {
        match self {
            GuestMemory::Own(memory) => memory.size(store),
            GuestMemory::Shared(memory) => memory.memory.size(),
        }
    }

    /// Copies the bytes from `at` into `into`.
    pub fn read(&self, store: impl AsContext, at: u64, into: &mut [u8]) -> Result<(), OutOfBounds> {
        match self {
            GuestMemory::Own(memory) => memory
                .read(store, at as usize, into)
                .map_err(|_| OutOfBounds),
            GuestMemory::Shared(memory) => memory.read(at, into),
        }
    }

    /// Copies `bytes` to the memory from `at`.
    pub fn write(
        &self,
        store: impl AsContextMut,
        at: u64,
        bytes: &[u8],
    ) -> Result<(), OutOfBounds> {
        match self {
            GuestMemory::Own(memory) => memory
                .write(store, at as usize, bytes)
                .map_err(|_| OutOfBounds),
            GuestMemory::Shared(memory) => memory.write(at, bytes),
        }
    }

    /// Returns the memory as an import takes it.
    pub fn to_extern(&self) -> Extern {
        match self {
            GuestMemory::Own(memory) => Extern::Memory(*memory),
            GuestMemory::Shared(memory) => Extern::SharedMemory(memory.memory.clone()),
        }
    }
}

/// A shared guest memory and the module that reaches it, which every thread on the memory may use.
#[derive(Clone)]
pub struct SharedGuestMemory {
    memory: SharedMemory,
    access: Arc<Mutex<Access>>,
}

/// The words one call loads or stores, as a tuple wasmtime can pass.
type Words = (u64, u64, u64, u64, u64, u64, u64, u64);
/// How many words that is, and their bytes.
const WORDS: usize = size_of::<Words>() / size_of::<u64>();
const CHUNK: usize = size_of::<Words>();
/// What a store of words takes: the address, then the words.
type StoreWords = (u64, u64, u64, u64, u64, u64, u64, u64, u64);

/// The module's instance in a store of its own.
struct Access {
    store: Store<()>,
    load_words: TypedFunc<u64, Words>,
    store_words: TypedFunc<StoreWords, ()>,
    load_byte: TypedFunc<u64, u32>,
    store_byte: TypedFunc<(u64, u32), ()>,
}

/// The guest memory's index, the module's only memory.
const MEMORY_INDEX: u32 = u32::MIN;
/// The parameters of each function: an address in the guest memory, then the values.
const ADDRESS_PARAMETER: u32 = u32::MIN;

impl SharedGuestMemory {
    /// Makes a shared guest memory of `bytes`, rounded up to whole WebAssembly pages, all zero.
    pub fn new(engine: &Engine, bytes: u64) -> SharedGuestMemory {
        let pages = bytes.div_ceil(WASM_PAGE_BYTES);
        assert!(
            pages <= SHARED_MEMORY_MOST_PAGES,
            "a shared guest memory of {pages} pages, more than code declares"
        );
        let ty = MemoryTypeBuilder::new()
            .memory64(true)
            .shared(true)
            .min(pages)
            .max(Some(pages))
            .build()
            .expect("a shared guest memory's type");
        let memory = SharedMemory::new(engine, ty).expect("a shared guest memory");
        let mut store = Store::new(engine, ());
        let instance = Instance::new(
            &mut store,
            &module(engine),
            &[Extern::SharedMemory(memory.clone())],
        )
        .expect("the guest memory's module");
        let access = Access {
            load_words: instance.get_typed_func(&mut store, LOAD_WORDS).unwrap(),
            store_words: instance.get_typed_func(&mut store, STORE_WORDS).unwrap(),
            load_byte: instance.get_typed_func(&mut store, LOAD_BYTE).unwrap(),
            store_byte: instance.get_typed_func(&mut store, STORE_BYTE).unwrap(),
            store,
        };
        SharedGuestMemory {
            memory,
            access: Arc::new(Mutex::new(access)),
        }
    }

    /// Fails unless `length` bytes from `at` lie inside the memory.
    fn check(&self, at: u64, length: usize) -> Result<(), OutOfBounds> {
        let end = at.checked_add(length as u64).ok_or(OutOfBounds)?;
        if end > self.memory.data_size() as u64 {
            return Err(OutOfBounds);
        }
        Ok(())
    }

    /// Copies the bytes from `at` into `into`.
    pub fn read(&self, at: u64, into: &mut [u8]) -> Result<(), OutOfBounds> {
        self.check(at, into.len())?;
        let mut access = self.access.lock().unwrap();
        let Access {
            store,
            load_words,
            load_byte,
            ..
        } = &mut *access;
        let (chunks, rest) = into.as_chunks_mut::<CHUNK>();
        let mut address = at;
        for chunk in chunks {
            let (a, b, c, d, e, f, g, h) = load_words
                .call(&mut *store, address)
                .expect("bytes inside the guest memory");
            let (words, _) = chunk.as_chunks_mut::<{ size_of::<u64>() }>();
            for (bytes, word) in words.iter_mut().zip([a, b, c, d, e, f, g, h]) {
                *bytes = word.to_le_bytes();
            }
            address += CHUNK as u64;
        }
        for byte in rest {
            let value = load_byte
                .call(&mut *store, address)
                .expect("a byte inside the guest memory");
            *byte = u8::try_from(value).unwrap();
            address += 1;
        }
        Ok(())
    }

    /// Copies `bytes` to the memory from `at`.
    pub fn write(&self, at: u64, bytes: &[u8]) -> Result<(), OutOfBounds> {
        self.check(at, bytes.len())?;
        let mut access = self.access.lock().unwrap();
        let Access {
            store,
            store_words,
            store_byte,
            ..
        } = &mut *access;
        let (chunks, rest) = bytes.as_chunks::<CHUNK>();
        let mut address = at;
        for chunk in chunks {
            let (words, _) = chunk.as_chunks::<{ size_of::<u64>() }>();
            let words: [u64; WORDS] = std::array::from_fn(|i| u64::from_le_bytes(words[i]));
            let [a, b, c, d, e, f, g, h] = words;
            store_words
                .call(&mut *store, (address, a, b, c, d, e, f, g, h))
                .expect("bytes inside the guest memory");
            address += CHUNK as u64;
        }
        for &byte in rest {
            store_byte
                .call(&mut *store, (address, u32::from(byte)))
                .expect("a byte inside the guest memory");
            address += 1;
        }
        Ok(())
    }
}

/// Returns the module compiled for an engine, once for each engine.
fn module(engine: &Engine) -> Module {
    static MADE: Mutex<Vec<(Engine, Module)>> = Mutex::new(Vec::new());
    let mut made = MADE.lock().unwrap();
    if let Some((_, module)) = made.iter().find(|(e, _)| Engine::same(e, engine)) {
        return module.clone();
    }
    let module = Module::new(engine, module_bytes()).expect("the guest memory's module");
    made.push((engine.clone(), module.clone()));
    module
}

/// Returns the module, whose functions load and store the imported guest memory's words and bytes.
fn module_bytes() -> Vec<u8> {
    let at = |offset: usize| MemArg {
        offset: offset as u64,
        align: u32::MIN,
        target: MemArgTarget::Memory(MEMORY_INDEX),
        ordering: None,
    };
    let words = [ValType::I64; WORDS];
    let word_offsets = (0..WORDS).map(|i| i * size_of::<u64>());
    // Each function: its parameters, its results and its body.
    let mut functions: Vec<(Vec<ValType>, Vec<ValType>, Function)> = Vec::new();
    let mut f = Function::new([]);
    let mut sink = f.instructions();
    for offset in word_offsets.clone() {
        sink.local_get(ADDRESS_PARAMETER).i64_load(at(offset));
    }
    sink.end();
    functions.push((vec![ValType::I64], words.to_vec(), f));
    let mut f = Function::new([]);
    let mut sink = f.instructions();
    for (value, offset) in (FIRST_VALUE_PARAMETER..).zip(word_offsets) {
        sink.local_get(ADDRESS_PARAMETER)
            .local_get(value)
            .i64_store(at(offset));
    }
    sink.end();
    let mut store_words_parameters = vec![ValType::I64];
    store_words_parameters.extend(words);
    functions.push((store_words_parameters, Vec::new(), f));
    let mut f = Function::new([]);
    f.instructions()
        .local_get(ADDRESS_PARAMETER)
        .i32_load8_u(at(usize::MIN))
        .end();
    functions.push((vec![ValType::I64], vec![ValType::I32], f));
    let mut f = Function::new([]);
    f.instructions()
        .local_get(ADDRESS_PARAMETER)
        .local_get(FIRST_VALUE_PARAMETER)
        .i32_store8(at(usize::MIN))
        .end();
    functions.push((vec![ValType::I64, ValType::I32], Vec::new(), f));
    let names = [LOAD_WORDS, STORE_WORDS, LOAD_BYTE, STORE_BYTE];
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
        MEMORY,
        EntityType::Memory(MemoryType {
            minimum: u64::MIN,
            maximum: Some(SHARED_MEMORY_MOST_PAGES),
            memory64: true,
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

/// The section of a module that holds its imports.
///
/// https://webassembly.github.io/spec/core/binary/modules.html#import-section
const IMPORT_SECTION: u8 = 2;
/// The bytes before a module's first section: the magic number and the version.
///
/// https://webassembly.github.io/spec/core/binary/modules.html#binary-module
const PREAMBLE_BYTES: usize = 8;
/// The guest memory's import in code that does not share it: names, memory kind, limits with the 64 bit flag, minimum 0.
///
/// https://webassembly.github.io/spec/core/binary/types.html#limits
const OWN_MEMORY_IMPORT: &[u8] = b"\x03env\x06memory\x02\x04\x00";
/// The same import shared: the 64 bit, shared and maximum flags, the minimum of 0, then the maximum.
const SHARED_MEMORY_IMPORT: &[u8] = b"\x03env\x06memory\x02\x07\x00";

/// Returns a code module with its import of the guest memory made shared, as the form that shares it declares it.
///
/// Its plain accesses then run on a shared memory: the control of the parallel tests, which shows what code without
/// ordered accesses does there.
pub fn import_shared(module: &[u8]) -> Vec<u8> {
    let mut out = module[..PREAMBLE_BYTES].to_vec();
    let mut at = PREAMBLE_BYTES;
    let mut found = false;
    while at < module.len() {
        let id = module[at];
        let (size, length) = read_leb(&module[at + 1..]);
        let start = at + 1 + length;
        let contents = &module[start..start + size as usize];
        at = start + size as usize;
        out.push(id);
        if id != IMPORT_SECTION {
            write_leb(&mut out, size);
            out.extend_from_slice(contents);
            continue;
        }
        let place = contents
            .windows(OWN_MEMORY_IMPORT.len())
            .position(|w| w == OWN_MEMORY_IMPORT)
            .expect("the guest memory's import, unshared");
        let mut changed = contents[..place].to_vec();
        changed.extend_from_slice(SHARED_MEMORY_IMPORT);
        write_leb(&mut changed, SHARED_MEMORY_MOST_PAGES);
        changed.extend_from_slice(&contents[place + OWN_MEMORY_IMPORT.len()..]);
        write_leb(&mut out, changed.len() as u64);
        out.extend(changed);
        found = true;
    }
    assert!(found, "a code module with an import section");
    out
}

/// Bits of a value each byte of an unsigned LEB128 number holds, and the flag of every byte but the last.
///
/// https://webassembly.github.io/spec/core/binary/values.html#integers
const LEB_BITS: u32 = 7;
/// https://webassembly.github.io/spec/core/binary/values.html#integers
const LEB_MORE: u8 = 0x80;

/// Returns the unsigned LEB128 number at the start of `bytes` and how many bytes it takes.
fn read_leb(bytes: &[u8]) -> (u64, usize) {
    let mut value = u64::MIN;
    for (i, &byte) in bytes.iter().enumerate() {
        value |= u64::from(byte & !LEB_MORE) << (LEB_BITS * i as u32);
        if byte & LEB_MORE == 0 {
            return (value, i + 1);
        }
    }
    panic!("a number that runs past the module's end")
}

/// Appends `value` as an unsigned LEB128 number.
fn write_leb(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value as u8) & !LEB_MORE;
        value >>= LEB_BITS;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | LEB_MORE);
    }
}
