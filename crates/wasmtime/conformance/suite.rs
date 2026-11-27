//! The tests every implementation of wcontainers-wasmtime runs, written against its trait alone.
#![cfg(target_os = "linux")]

use std::os::fd::{AsFd, OwnedFd};

use rustix::fs::{MemfdFlags, SealFlags};
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    MemArg, MemArgTarget, TypeSection, ValType,
};
use wasmtime::{Config, Engine, Instance, MemoryType, Store, TypedFunc};
use wcontainers_abi::{Access, SystemCallRequest};
use wcontainers_executor::{
    InterruptResponse, MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_volatile_bytes::VolatileBytes;
use wcontainers_wasmtime::{Errno, Wasmtime};

/// Tuning number: how many host pages each test's files and memories hold.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const PAGES: usize = 4;
/// Tuning number: the guard after each test's memories, in host pages.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const GUARD_PAGES: usize = 2;
/// A value the tests write and read back: every byte different, so that a byte out of place shows.
const WORD: u64 = u64::from_le_bytes(*b"the_word");
/// The name of each test's memory files, which only debugging shows.
const FILE_NAME: &str = env!("CARGO_PKG_NAME");
/// The import the test module takes its memory from, and the names of its two functions.
const MODULE: &str = "env";
const MEMORY: &str = "memory";
const LOAD: &str = "load";
const STORE: &str = "store";
/// The bytes of a WebAssembly page.
///
/// <https://webassembly.github.io/spec/core/exec/runtime.html#memory-instances>
const WASM_PAGE_BYTES: u64 = 1 << 16;
/// The reservation of the store test's engine: one WebAssembly page.
const STORE_RESERVATION_BYTES: u64 = WASM_PAGE_BYTES;
/// The WebAssembly pages of the store test's memory: all its reservation holds.
const WASM_PAGES: usize = (STORE_RESERVATION_BYTES / WASM_PAGE_BYTES) as usize;
/// A byte, by which the mappings the tests refuse are not whole host pages.
const MISALIGNMENT: u64 = size_of::<u8>() as u64;
/// The test module's one memory, the first.
const MEMORY_INDEX: u32 = u32::MIN;
/// The test module's types and functions: the load first, then the store.
const LOAD_TYPE: u32 = u32::MIN;
const STORE_TYPE: u32 = LOAD_TYPE + 1;
const LOAD_FUNCTION: u32 = LOAD_TYPE;
const STORE_FUNCTION: u32 = STORE_TYPE;
/// The parameters of the test module's functions: the address, then the value stored.
const ADDRESS_PARAMETER: u32 = u32::MIN;
const VALUE_PARAMETER: u32 = ADDRESS_PARAMETER + 1;
/// The guard after the store test's memory, which its engine is configured with: one WebAssembly page.
const STORE_GUARD_BYTES: u64 = WASM_PAGE_BYTES;

fn page() -> usize {
    rustix::param::page_size()
}

/// Returns a memory file of the length that cannot shrink.
fn sealed_file(length: usize) -> OwnedFd {
    let file = rustix::fs::memfd_create(FILE_NAME, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
        .expect("a memory file");
    rustix::fs::ftruncate(&file, length as u64).expect("the file's length");
    rustix::fs::fcntl_add_seals(&file, SealFlags::SHRINK).expect("the file's seal");
    file
}

fn read_word<W: Wasmtime>(wasmtime: &W, memory: &W::HostMemory, at: u64) -> u64 {
    let bytes = wasmtime
        .host_memory_bytes(memory, at..at + size_of::<u64>() as u64)
        .expect("offsets inside the memory");
    let mut word = [u8::MIN; size_of::<u64>()];
    assert_eq!(bytes.read(&mut word), word.len(), "the bytes read");
    u64::from_le_bytes(word)
}

fn write_word<W: Wasmtime>(wasmtime: &W, memory: &W::HostMemory, at: u64, value: u64) {
    let bytes = wasmtime
        .host_memory_bytes(memory, at..at + size_of::<u64>() as u64)
        .expect("offsets inside the memory");
    assert_eq!(
        bytes.write(&value.to_le_bytes()),
        size_of::<u64>(),
        "the bytes written"
    );
}

/// Checks that a host memory shows its file's bytes, both ways, and lends nothing past its length.
pub fn host_memory_shows_its_file<W: Wasmtime>(wasmtime: W) {
    let length = PAGES * page();
    let file = sealed_file(length);
    let at = page() as u64;
    rustix::io::pwrite(&file, &WORD.to_le_bytes(), at).expect("a write to the file");
    let memory = wasmtime
        .new_host_memory(file.as_fd(), length, GUARD_PAGES * page())
        .expect("a host memory");
    assert_eq!(read_word(&wasmtime, &memory, at), WORD, "the file's bytes");
    write_word(&wasmtime, &memory, at, !WORD);
    let mut word = [u8::MIN; size_of::<u64>()];
    rustix::io::pread(&file, &mut word, at).expect("a read of the file");
    assert_eq!(
        u64::from_le_bytes(word),
        !WORD,
        "the file after a write to the memory"
    );
    assert!(
        wasmtime
            .host_memory_bytes(&memory, 0..length as u64 + 1)
            .is_none(),
        "bytes past the length were lent"
    );
}

/// Checks that a file mapped into two memories shows each one's writes in the other.
pub fn map_file_shares_pages_between_memories<W: Wasmtime>(wasmtime: W) {
    let length = PAGES * page();
    let (first_file, second_file) = (sealed_file(length), sealed_file(length));
    let guard = GUARD_PAGES * page();
    let first = wasmtime
        .new_host_memory(first_file.as_fd(), length, guard)
        .expect("a host memory");
    let second = wasmtime
        .new_host_memory(second_file.as_fd(), length, guard)
        .expect("a host memory");
    let page = page() as u64;
    assert_eq!(
        wasmtime.map_file(&second, 2 * page..2 * page + page, first_file.as_fd(), page),
        Ok(())
    );
    write_word(&wasmtime, &first, page, WORD);
    assert_eq!(
        read_word(&wasmtime, &second, 2 * page),
        WORD,
        "the shared page"
    );
    write_word(&wasmtime, &second, 2 * page, !WORD);
    assert_eq!(
        read_word(&wasmtime, &first, page),
        !WORD,
        "the page after a write through the other"
    );
    assert_eq!(
        read_word(&wasmtime, &second, page),
        0,
        "a page the mapping left alone"
    );
}

/// Checks that every mapping that could leave a page faulting is refused.
pub fn mappings_that_could_fault_are_refused<W: Wasmtime>(wasmtime: W) {
    let length = PAGES * page();
    let guard = GUARD_PAGES * page();
    let unsealed = rustix::fs::memfd_create(FILE_NAME, MemfdFlags::CLOEXEC).expect("a memory file");
    rustix::fs::ftruncate(&unsealed, length as u64).expect("the file's length");
    assert!(
        matches!(
            wasmtime.new_host_memory(unsealed.as_fd(), length, guard),
            Err(Errno::INVAL)
        ),
        "a file that may shrink was mapped"
    );
    let short = sealed_file(length - page());
    assert!(
        matches!(
            wasmtime.new_host_memory(short.as_fd(), length, guard),
            Err(Errno::INVAL)
        ),
        "a file shorter than the memory was mapped"
    );
    let file = sealed_file(length);
    assert!(
        matches!(
            wasmtime.new_host_memory(file.as_fd(), length + 1, guard),
            Err(Errno::INVAL)
        ),
        "a length of part of a page was taken"
    );
    let memory = wasmtime
        .new_host_memory(file.as_fd(), length, guard)
        .expect("a host memory");
    let page = page() as u64;
    let length = length as u64;
    for (addresses, offset) in [
        (length - page..length + page, u64::MIN),
        (MISALIGNMENT..page, u64::MIN),
        (u64::MIN..page, MISALIGNMENT),
        (0..page, length),
    ] {
        assert_eq!(
            wasmtime.map_file(&memory, addresses.clone(), file.as_fd(), offset),
            Err(Errno::INVAL),
            "{addresses:#x?} mapped from {offset:#x}"
        );
    }
}

/// Checks that a store's memory over a host memory is that memory: guest loads and stores meet the bytes lent.
pub fn store_memory_is_the_host_memory<W: Wasmtime>(wasmtime: W) {
    let mut config = Config::new();
    config
        .wasm_memory64(true)
        .memory_may_move(false)
        .memory_reservation(STORE_RESERVATION_BYTES)
        .memory_guard_size(STORE_GUARD_BYTES);
    wasmtime.with_host_memory(&mut config);
    let engine = Engine::new(&config).expect("an engine");
    let wasm_pages = WASM_PAGES;
    let length = usize::try_from(WASM_PAGE_BYTES).expect("a page fits") * wasm_pages;
    let file = sealed_file(length);
    let memory = wasmtime
        .new_host_memory(file.as_fd(), length, STORE_GUARD_BYTES as usize)
        .expect("a host memory");
    let mut store = Store::new(&engine, ());
    let too_big = MemoryType::new64(wasm_pages as u64 + 1, Some(wasm_pages as u64 + 1));
    assert!(
        wasmtime.new_memory(&mut store, too_big, &memory).is_err(),
        "a memory larger than its host memory was made"
    );
    let ty = MemoryType::new64(wasm_pages as u64, Some(wasm_pages as u64));
    let store_memory = wasmtime
        .new_memory(&mut store, ty, &memory)
        .expect("a store memory");
    let module = wasmtime::Module::new(&engine, module_bytes(wasm_pages as u64)).expect("a module");
    let instance = Instance::new(&mut store, &module, &[store_memory.into()]).expect("an instance");
    let load: TypedFunc<u64, u64> = instance.get_typed_func(&mut store, LOAD).expect("load");
    let store_word: TypedFunc<(u64, u64), ()> =
        instance.get_typed_func(&mut store, STORE).expect("store");
    let at = page() as u64;
    write_word(&wasmtime, &memory, at, WORD);
    assert_eq!(
        load.call(&mut store, at).expect("a load"),
        WORD,
        "the word a guest loaded"
    );
    store_word.call(&mut store, (at, !WORD)).expect("a store");
    assert_eq!(
        read_word(&wasmtime, &memory, at),
        !WORD,
        "the word a guest stored"
    );
}

/// Checks that the handler is lent inside the call alone, and not twice at once.
pub fn trap_handler_is_lent_inside_the_call_only<W: Wasmtime>(wasmtime: W) {
    assert!(
        wasmtime.with_trap_handler(|_| ()).is_none(),
        "a handler was lent outside a call"
    );
    let mut handler = Counting::default();
    let lent = wasmtime.call_with_trap_handler(&mut handler, || {
        wasmtime.with_trap_handler(|handler| {
            handler.handle_interrupt();
            wasmtime.with_trap_handler(|_| ()).is_none()
        })
    });
    assert_eq!(
        lent,
        Some(true),
        "the handler was lent once, and not again inside"
    );
    assert!(
        wasmtime.with_trap_handler(|_| ()).is_none(),
        "a handler was lent after its call"
    );
    assert_eq!(
        handler.interrupts, 1,
        "the interrupts the handler was given"
    );
}

/// A handler that counts interrupts and ends everything else.
#[derive(Default)]
struct Counting {
    interrupts: usize,
}

impl TrapHandler for Counting {
    fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, _: u64, _: Access) -> MemoryFaultResponse {
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.interrupts += 1;
        InterruptResponse::Continue
    }
}

/// Returns a module importing a 64-bit memory of the pages that exports a load and a store of a word at an address.
fn module_bytes(pages: u64) -> Vec<u8> {
    let word = MemArg {
        offset: u64::MIN,
        align: size_of::<u64>().trailing_zeros(),
        target: MemArgTarget::Memory(MEMORY_INDEX),
        ordering: None,
    };
    let mut types = TypeSection::new();
    types.ty().function([ValType::I64], [ValType::I64]);
    types.ty().function([ValType::I64, ValType::I64], []);
    let mut imports = ImportSection::new();
    imports.import(
        MODULE,
        MEMORY,
        EntityType::Memory(wasm_encoder::MemoryType {
            minimum: pages,
            maximum: Some(pages),
            memory64: true,
            shared: false,
            page_size_log2: None,
        }),
    );
    let mut functions = FunctionSection::new();
    functions.function(LOAD_TYPE);
    functions.function(STORE_TYPE);
    let mut exports = ExportSection::new();
    exports.export(LOAD, ExportKind::Func, LOAD_FUNCTION);
    exports.export(STORE, ExportKind::Func, STORE_FUNCTION);
    let mut code = CodeSection::new();
    let mut load = Function::new([]);
    load.instructions()
        .local_get(ADDRESS_PARAMETER)
        .i64_load(word)
        .end();
    code.function(&load);
    let mut store = Function::new([]);
    store
        .instructions()
        .local_get(ADDRESS_PARAMETER)
        .local_get(VALUE_PARAMETER)
        .i64_store(word)
        .end();
    code.function(&store);
    let mut module = wasm_encoder::Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&exports)
        .section(&code);
    module.finish()
}

/// Runs each suite function above as its own test on the implementation, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($wasmtime:expr) => {
        #[test]
        fn host_memory_shows_its_file() {
            $crate::host_memory_shows_its_file($wasmtime);
        }

        #[test]
        fn map_file_shares_pages_between_memories() {
            $crate::map_file_shares_pages_between_memories($wasmtime);
        }

        #[test]
        fn mappings_that_could_fault_are_refused() {
            $crate::mappings_that_could_fault_are_refused($wasmtime);
        }

        #[test]
        fn store_memory_is_the_host_memory() {
            $crate::store_memory_is_the_host_memory($wasmtime);
        }

        #[test]
        fn trap_handler_is_lent_inside_the_call_only() {
            $crate::trap_handler_is_lent_inside_the_call_only($wasmtime);
        }
    };
}
