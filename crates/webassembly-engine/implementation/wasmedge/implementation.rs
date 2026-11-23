//! The WebAssembly engine interface run by WasmEdge, every module compiled ahead of time by its LLVM compiler.

#![cfg(feature = "wasmedge")]

use core::cell::{Cell, RefCell};
use core::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use thiserror::Error;
use wasm_encoder::reencode::{Reencode, RoundtripReencoder};
use wasm_encoder::{
    AbstractHeapType, CodeSection, EntityType, ExportKind, ExportSection, FunctionSection,
    HeapType, ImportSection, InstructionSink, TypeSection,
};
use wasmedge_sdk::error::{CoreError, CoreExecutionError, WasmEdgeError};
use wasmedge_sdk::{
    CompilerOptimizationLevel, CompilerOutputFormat, FuncType, ImportObjectBuilder, RefType,
    RunMode, ValType, WasmVal, WasmValType,
};
use wasmedge_sys::instance::function::AsFunc;
use wasmedge_sys::{
    AsInstance, CallingFrame, Compiler, Executor, ImportModule, Loader, Store, SyncFn, Validator,
    WasmValue,
};
use wcontainers_webassembly_engine::function::{
    BindFunctionError, BoundFunction, FunctionStore, FunctionTypeBinder, InvokeError,
};
use wcontainers_webassembly_engine::global::{
    GlobalStore, Mutability, ReadGlobalError, Value, WriteGlobalError,
};
use wcontainers_webassembly_engine::memory::{
    GrowMemoryError, MemoryStore, MemoryType, PAGE_SIZE, ReadMemoryError, Sharing, WriteMemoryError,
};
use wcontainers_webassembly_engine::table::{
    GrowTableError, ReadTableError, TableStore, TableType, WriteTableError,
};
use wcontainers_webassembly_engine::{
    AddressType, ExternalAddress, Import, Limits, WebAssemblyEngine,
};

/// What WasmEdge answers when it fails.
pub type Error = Box<WasmEdgeError>;

type ReencodeError = wasm_encoder::reencode::Error;

/// What every engine value made from it shares: WasmEdge's configurations, and the helper modules compiled so far.
#[derive(Clone)]
pub struct Config(Arc<Shared>);

struct Shared {
    /// For modules compiled ahead of time into native shared libraries and run as such.
    compiled: wasmedge_sys::Config,
    /// For the modules that only declare or pass on items, which hold no code.
    declaring: wasmedge_sys::Config,
    /// Each helper module compiled so far, with the bytes it was compiled from.
    helpers: Mutex<Vec<(Vec<u8>, Module)>>,
}

impl Default for Config {
    /// Every proposal the translated modules use switched on.
    fn default() -> Self {
        Self(Arc::new(Shared {
            compiled: configuration(RunMode::Aot).expect("WasmEdge makes a configuration"),
            declaring: configuration(RunMode::Interpreter).expect("WasmEdge makes a configuration"),
            helpers: Mutex::default(),
        }))
    }
}

fn configuration(run_mode: RunMode) -> Result<wasmedge_sys::Config, Error> {
    let mut config = wasmedge_sys::Config::create()?;
    config.tail_call(true);
    config.function_references(true);
    config.gc(true);
    config.exception_handling(true);
    config.threads(true);
    config.multi_memories(true);
    config.set_run_mode(run_mode);
    // A native shared library either loads as native code or fails to load: WasmEdge never interprets it.
    config.set_aot_compiler_output_format(CompilerOutputFormat::Native);
    config.set_aot_optimization_level(CompilerOptimizationLevel::O3);
    // WasmEdge caps every memory at 4 GiB unless told otherwise; this is the most the bindings can ask for.
    config.set_max_memory_pages(u32::MAX);
    Ok(config)
}

impl Config {
    /// The helper module of the bytes, compiled the first time any engine value asks for it.
    fn helper(&self, bytes: Vec<u8>) -> Option<Module> {
        let mut helpers = self
            .0
            .helpers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some((_, module)) = helpers.iter().find(|(compiled, _)| *compiled == bytes) {
            return Some(module.clone());
        }
        let module = compile(&self.0, &bytes).ok()?;
        helpers.push((bytes, module.clone()));
        Some(module)
    }
}

/// A new folder only this user can write in, so nobody else can put a library in the place of a compiled one.
fn private_folder() -> std::io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(env!("CARGO_PKG_NAME"));
    #[cfg(unix)]
    builder.permissions(
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(mode_bits(
            rustix::fs::Mode::RWXU.bits(),
        )),
    );
    builder.tempdir()
}

/// The mode bits as the standard library takes them, from the width the system gives them.
#[cfg(unix)]
fn mode_bits(mode: impl Into<u32>) -> u32 {
    mode.into()
}

/// The name of the file a module is compiled into, before its extension.
const COMPILED_LIBRARY: &str = "compiled";

/// Compiles the module into a native shared library in a private folder, loads it, and removes the folder.
fn compile(shared: &Shared, bytes: &[u8]) -> Result<Module, CompileError> {
    let folder = private_folder().map_err(CompileError::Folder)?;
    let path = folder
        .path()
        .join(COMPILED_LIBRARY)
        .with_extension(std::env::consts::DLL_EXTENSION);
    // A loaded library stays mapped once its file is gone.
    let loaded = Compiler::create(Some(&shared.compiled))
        .and_then(|compiler| compiler.compile_from_bytes(bytes, &path))
        .and_then(|()| Loader::create(Some(&shared.compiled))?.from_file(&path))?;
    Validator::create(Some(&shared.compiled))?.validate(&loaded)?;
    Ok(Module {
        loaded,
        description: Arc::new(describe(bytes)?),
    })
}

/// Loads a module this crate wrote that holds no code, which needs no compiling.
fn declare(shared: &Shared, bytes: &[u8]) -> Result<Module, Error> {
    let loaded = Loader::create(Some(&shared.declaring))?.from_bytes(bytes)?;
    Validator::create(Some(&shared.declaring))?.validate(&loaded)?;
    let description = describe(bytes).expect("the modules this crate writes are readable");
    Ok(Module {
        loaded,
        description: Arc::new(description),
    })
}

/// A name no other module instance of the process has.
fn unique_name() -> String {
    static NAMED: AtomicU64 = AtomicU64::new(0);
    [
        env!("CARGO_PKG_NAME"),
        &NAMED.fetch_add(1, Ordering::Relaxed).to_string(),
    ]
    .concat()
}

/// Which engine value of the process made an address, so that another engine value refuses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity(u64);

impl Identity {
    fn new() -> Self {
        static MADE: AtomicU64 = AtomicU64::new(0);
        Self(MADE.fetch_add(1, Ordering::Relaxed))
    }
}

/// What an engine value does with an address another engine value made.
const FOREIGN_ADDRESS: &str = "an address is used only by the engine value that made it";

/// The module and item name under which a helper imports what it reaches.
const HELPED: &str = "helped";
/// The names of the items the modules of this crate export.
const COPIED_BYTES: &str = "copied_bytes";
const RETYPED_FUNCTION: &str = "retyped_function";
const HELD_FUNCTION: &str = "held_function";
const DEFINED_TABLE: &str = "defined_table";
const DEFINED_MEMORY: &str = "defined_memory";
const DEFINED_GLOBAL: &str = "defined_global";
const HOST_FUNCTION: &str = "host_function";
const COPY_OUT: &str = "copy_out";
const COPY_IN: &str = "copy_in";
const GROW_ITEM: &str = "grow_item";
const SET_ITEM: &str = "set_item";
const PASS_ON: &str = "pass_on";

/// The pages a 32-bit memory holds at most.
const MOST_PAGES: u64 = (1 << u32::BITS) / PAGE_SIZE;
/// The pages a 64-bit memory holds at most.
const MOST_PAGES_64: u64 = 1 << (u64::BITS - PAGE_SIZE.trailing_zeros());
/// The first byte the bindings' 32-bit offsets cannot reach.
const BINDINGS_REACH: u64 = 1 << u32::BITS;

/// The reason [`SlowPath::SharedMemoryCopied`] shows.
const SHARED_MEMORY_COPIED: &str =
    "bytes of a shared memory copied through a module, as the bindings lend no shared memory";
/// The reason [`SlowPath::SharedOwnerMemoryCopied`] shows.
const SHARED_OWNER_MEMORY_COPIED: &str = "unshared memory of an instance that exports a shared memory \
    written or grown through a module, as the bindings write no shared instance";
/// The reason [`SlowPath::BeyondBindingsReach`] shows.
const BEYOND_BINDINGS_REACH: &str =
    "bytes beyond the 32-bit offsets of the bindings copied through a module";
/// The reason [`SlowPath::TableWrittenByModule`] shows.
const TABLE_WRITTEN_BY_MODULE: &str =
    "table written or grown through a module, as the bindings write no table of an instance";
/// The reason [`SlowPath::GlobalSetByModule`] shows.
const GLOBAL_SET_BY_MODULE: &str = "global set through a module, as the bindings write no shared \
    instance and no typed reference";
/// The reason [`SlowPath::FunctionRetyped`] shows.
const FUNCTION_RETYPED: &str = "function reference read from a module's table to carry its own \
    type, as the bindings make references that carry only funcref";
/// The reason [`SlowPath::FunctionNotRetyped`] shows.
const FUNCTION_NOT_RETYPED: &str = "function reference written carrying only funcref, as no module \
    describes its type, so a guest's cast to its type fails";
/// The reason [`SlowPath::ReferenceHeld`] shows.
const REFERENCE_HELD: &str = "function reference read from a slot of a typed reference type stored in a \
    funcref table of a module and read back, as WasmEdge refuses the slot's type as a funcref parameter";
/// The reason [`SlowPath::ImportsForwarded`] shows.
const IMPORTS_FORWARDED: &str =
    "imports linked through a module that imports and exports them, as WasmEdge links by name only";
/// The reason [`SlowPath::FunctionPassedOn`] shows.
const FUNCTION_PASSED_ON: &str = "function no instance exports imported through a module that \
    calls it indirectly, as WasmEdge links by name only";

/// A place where this crate does something slower than the engine's direct route, because the bindings' safe layer
/// offers none, or does it less well than the standard asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlowPath {
    SharedMemoryCopied,
    SharedOwnerMemoryCopied,
    BeyondBindingsReach,
    TableWrittenByModule,
    GlobalSetByModule,
    FunctionRetyped,
    FunctionNotRetyped,
    ImportsForwarded,
    FunctionPassedOn,
    ReferenceHeld,
}

impl SlowPath {
    /// Every slow path, in the order of their counters.
    pub const ALL: &[SlowPath] = &[
        SlowPath::SharedMemoryCopied,
        SlowPath::SharedOwnerMemoryCopied,
        SlowPath::BeyondBindingsReach,
        SlowPath::TableWrittenByModule,
        SlowPath::GlobalSetByModule,
        SlowPath::FunctionRetyped,
        SlowPath::FunctionNotRetyped,
        SlowPath::ImportsForwarded,
        SlowPath::FunctionPassedOn,
        SlowPath::ReferenceHeld,
    ];

    pub fn reason(self) -> &'static str {
        match self {
            SlowPath::SharedMemoryCopied => SHARED_MEMORY_COPIED,
            SlowPath::SharedOwnerMemoryCopied => SHARED_OWNER_MEMORY_COPIED,
            SlowPath::BeyondBindingsReach => BEYOND_BINDINGS_REACH,
            SlowPath::TableWrittenByModule => TABLE_WRITTEN_BY_MODULE,
            SlowPath::GlobalSetByModule => GLOBAL_SET_BY_MODULE,
            SlowPath::FunctionRetyped => FUNCTION_RETYPED,
            SlowPath::FunctionNotRetyped => FUNCTION_NOT_RETYPED,
            SlowPath::ImportsForwarded => IMPORTS_FORWARDED,
            SlowPath::FunctionPassedOn => FUNCTION_PASSED_ON,
            SlowPath::ReferenceHeld => REFERENCE_HELD,
        }
    }
}

/// Whether each slow path has been warned of in this process.
static WARNED: [AtomicBool; SlowPath::ALL.len()] =
    [const { AtomicBool::new(false) }; SlowPath::ALL.len()];

/// How many times one engine value took each slow path.
struct SlowPaths([Cell<u64>; SlowPath::ALL.len()]);

impl SlowPaths {
    /// Counts one use of the path, and warns the first time the process takes it.
    fn took(&self, path: SlowPath) {
        let index = path as usize;
        let count = &self.0[index];
        if count.get() == 0 && !WARNED[index].swap(true, Ordering::Relaxed) {
            log::warn!("WasmEdge engine slow path: {}", path.reason());
        }
        count.set(count.get() + 1);
    }
}

/// What a module declares, read from its bytes: its types, and the type of each import and export.
#[derive(Default)]
struct Description {
    types: TypeSection,
    /// For the type at each index, its number of parameters when it is a function type.
    function_types: Vec<Option<u32>>,
    imports: Vec<(String, String, EntityType)>,
    exports: Vec<(String, EntityType)>,
}

impl Description {
    fn export(&self, name: &str) -> Option<EntityType> {
        self.exports
            .iter()
            .find(|(exported, _)| exported == name)
            .map(|(_, entity)| *entity)
    }

    fn exports_a_shared_memory(&self) -> bool {
        self.exports
            .iter()
            .any(|(_, entity)| matches!(entity, EntityType::Memory(memory) if memory.shared))
    }

    /// Whether references of the heap type refer to functions.
    fn refers_to_functions(&self, heap_type: HeapType) -> bool {
        match heap_type {
            HeapType::Abstract { ty, .. } => {
                matches!(ty, AbstractHeapType::Func | AbstractHeapType::NoFunc)
            }
            HeapType::Concrete(index) | HeapType::Exact(index) => usize::try_from(index)
                .ok()
                .and_then(|index| self.function_types.get(index))
                .is_some_and(Option::is_some),
        }
    }

    /// The number of parameters of the function type at the index.
    fn parameters(&self, function_type: u32) -> Option<u32> {
        *self
            .function_types
            .get(usize::try_from(function_type).ok()?)?
    }

    /// The index the types of a helper begin at, after the module's own.
    fn type_count(&self) -> u32 {
        u32::try_from(self.function_types.len()).expect("a module's type indices are u32")
    }

    /// The types a helper of an item of the reference type starts from, and the index its own types begin at.
    fn helper_types(&self, reference: Option<wasm_encoder::RefType>) -> (TypeSection, u32) {
        match reference.map(|reference| reference.heap_type) {
            Some(HeapType::Concrete(_) | HeapType::Exact(_)) => {
                (self.types.clone(), self.type_count())
            }
            _ => (TypeSection::new(), 0),
        }
    }
}

/// Reads what the module declares.
fn describe(bytes: &[u8]) -> Result<Description, ReencodeError> {
    use wasmparser::{CompositeInnerType, ExternalKind, Payload};
    let mut reencoder = RoundtripReencoder;
    let mut description = Description::default();
    let (mut functions, mut tables, mut memories, mut globals, mut tags) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::TypeSection(section) => {
                for group in section {
                    let group = group?;
                    description
                        .function_types
                        .extend(group.types().map(
                            |sub_type| match &sub_type.composite_type.inner {
                                CompositeInnerType::Func(function) => {
                                    u32::try_from(function.params().len()).ok()
                                }
                                _ => None,
                            },
                        ));
                    reencoder.parse_recursive_type_group(description.types.ty(), group)?;
                }
            }
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import?;
                    let entity = reencoder.entity_type(import.ty)?;
                    match entity {
                        EntityType::Function(_) | EntityType::FunctionExact(_) => {
                            functions.push(entity);
                        }
                        EntityType::Table(_) => tables.push(entity),
                        EntityType::Memory(_) => memories.push(entity),
                        EntityType::Global(_) => globals.push(entity),
                        EntityType::Tag(_) => tags.push(entity),
                    }
                    description.imports.push((
                        import.module.to_owned(),
                        import.name.to_owned(),
                        entity,
                    ));
                }
            }
            Payload::FunctionSection(section) => {
                for type_index in section {
                    functions.push(EntityType::Function(type_index?));
                }
            }
            Payload::TableSection(section) => {
                for table in section {
                    tables.push(EntityType::Table(reencoder.table_type(table?.ty)?));
                }
            }
            Payload::MemorySection(section) => {
                for memory in section {
                    memories.push(EntityType::Memory(reencoder.memory_type(memory?)?));
                }
            }
            Payload::GlobalSection(section) => {
                for global in section {
                    globals.push(EntityType::Global(reencoder.global_type(global?.ty)?));
                }
            }
            Payload::TagSection(section) => {
                for tag in section {
                    tags.push(EntityType::Tag(reencoder.tag_type(tag?)?));
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export?;
                    let space = match export.kind {
                        ExternalKind::Func | ExternalKind::FuncExact => &functions,
                        ExternalKind::Table => &tables,
                        ExternalKind::Memory => &memories,
                        ExternalKind::Global => &globals,
                        ExternalKind::Tag => &tags,
                    };
                    if let Some(entity) = usize::try_from(export.index)
                        .ok()
                        .and_then(|index| space.get(index))
                    {
                        description.exports.push((export.name.to_owned(), *entity));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(description)
}

fn export_kind(entity: EntityType) -> ExportKind {
    match entity {
        EntityType::Function(_) | EntityType::FunctionExact(_) => ExportKind::Func,
        EntityType::Table(_) => ExportKind::Table,
        EntityType::Memory(_) => ExportKind::Memory,
        EntityType::Global(_) => ExportKind::Global,
        EntityType::Tag(_) => ExportKind::Tag,
    }
}

/// The value type of a memory's or a table's addresses.
fn address_type(wide: bool) -> wasm_encoder::ValType {
    if wide {
        wasm_encoder::ValType::I64
    } else {
        wasm_encoder::ValType::I32
    }
}

fn address_zero<'sink, 'bytes>(
    sink: &'sink mut InstructionSink<'bytes>,
    wide: bool,
) -> &'sink mut InstructionSink<'bytes> {
    if wide {
        sink.i64_const(0)
    } else {
        sink.i32_const(0)
    }
}

/// The numeric value type, or nothing for a reference, whose exact type the bindings do not tell.
fn number_type(value_type: ValType) -> Option<wasm_encoder::ValType> {
    match value_type {
        ValType::I32 => Some(wasm_encoder::ValType::I32),
        ValType::I64 => Some(wasm_encoder::ValType::I64),
        ValType::F32 => Some(wasm_encoder::ValType::F32),
        ValType::F64 => Some(wasm_encoder::ValType::F64),
        ValType::V128 => Some(wasm_encoder::ValType::V128),
        ValType::FuncRef | ValType::ExternRef | ValType::UnsupportedRef => None,
    }
}

/// Pushes the function reference in the local as the reference type, trapping when the function is not of it. A
/// null becomes the type's own null: WasmEdge casts a reference by the type it carries, and the null it gives the
/// host carries `funcref`.
fn function_as(sink: &mut InstructionSink<'_>, local: u32, reference: wasm_encoder::RefType) {
    sink.local_get(local);
    if reference == wasm_encoder::RefType::FUNCREF {
        return;
    }
    if reference.nullable {
        sink.ref_is_null()
            .if_(wasm_encoder::BlockType::Result(wasm_encoder::ValType::Ref(
                reference,
            )))
            .ref_null(reference.heap_type)
            .else_()
            .local_get(local)
            .ref_cast_non_null(reference.heap_type)
            .end();
    } else {
        sink.ref_cast_non_null(reference.heap_type);
    }
}

/// Assembles a module from its sections, leaving out the empty ones.
fn assemble(
    types: &TypeSection,
    imports: &ImportSection,
    functions: &FunctionSection,
    memories: &wasm_encoder::MemorySection,
    exports: &ExportSection,
    code: &CodeSection,
) -> Vec<u8> {
    let mut module = wasm_encoder::Module::new();
    module.section(types).section(imports).section(functions);
    if !memories.is_empty() {
        module.section(memories);
    }
    module.section(exports).section(code);
    module.finish()
}

/// A module that imports a memory of the type and defines an unshared scratch memory of the same address type. It
/// exports `load` and `store`, which copy a length of bytes at an offset of the memory to and from the start of the
/// scratch memory, and `grow`, which grows the memory.
fn copier_module(memory: wasm_encoder::MemoryType) -> Vec<u8> {
    use wasm_encoder::Function;
    let wide = memory.memory64;
    let address = address_type(wide);
    let mut types = TypeSection::new();
    types.ty().function([address, address], []);
    types.ty().function([address], [address]);
    let (copy_type, grow_type) = (0, 1);
    let mut imports = ImportSection::new();
    let most = if wide { MOST_PAGES_64 } else { MOST_PAGES };
    imports.import(
        HELPED,
        HELPED,
        wasm_encoder::MemoryType {
            minimum: u64::MIN,
            maximum: memory.shared.then_some(most),
            memory64: wide,
            shared: memory.shared,
            page_size_log2: memory.page_size_log2,
        },
    );
    let (helped, scratch) = (0, 1);
    let mut functions = FunctionSection::new();
    functions
        .function(copy_type)
        .function(copy_type)
        .function(grow_type);
    let mut memories = wasm_encoder::MemorySection::new();
    memories.memory(wasm_encoder::MemoryType {
        minimum: 1,
        maximum: None,
        memory64: wide,
        shared: false,
        page_size_log2: None,
    });
    let mut exports = ExportSection::new();
    exports
        .export(COPY_OUT, ExportKind::Func, 0)
        .export(COPY_IN, ExportKind::Func, 1)
        .export(GROW_ITEM, ExportKind::Func, 2)
        .export(COPIED_BYTES, ExportKind::Memory, scratch);
    let (offset, length) = (0, 1);
    let mut load = Function::new([]);
    address_zero(&mut load.instructions(), wide)
        .local_get(offset)
        .local_get(length)
        .memory_copy(scratch, helped)
        .end();
    let mut store = Function::new([]);
    let mut sink = store.instructions();
    sink.local_get(offset);
    address_zero(&mut sink, wide)
        .local_get(length)
        .memory_copy(helped, scratch)
        .end();
    let mut grow = Function::new([]);
    grow.instructions().local_get(0).memory_grow(helped).end();
    let mut code = CodeSection::new();
    code.function(&load).function(&store).function(&grow);
    assemble(&types, &imports, &functions, &memories, &exports, &code)
}

/// A module that imports a table of the type, whose elements refer to functions, and exports `set`, which sets an
/// element to a function of the element type, and, when the elements may be null, `grow`, which grows it by nulls.
fn table_writer_module(
    mut types: TypeSection,
    first_type: u32,
    table: wasm_encoder::TableType,
) -> Vec<u8> {
    use wasm_encoder::Function;
    let index = address_type(table.table64);
    types
        .ty()
        .function([index, wasm_encoder::ValType::FUNCREF], []);
    types.ty().function([index], [index]);
    let (set_type, grow_type) = (first_type, first_type + 1);
    let mut imports = ImportSection::new();
    imports.import(
        HELPED,
        HELPED,
        wasm_encoder::TableType {
            minimum: u64::MIN,
            maximum: None,
            ..table
        },
    );
    let helped = 0;
    let mut functions = FunctionSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let mut set = Function::new([]);
    let mut sink = set.instructions();
    sink.local_get(0);
    function_as(&mut sink, 1, table.element_type);
    sink.table_set(helped).end();
    functions.function(set_type);
    exports.export(SET_ITEM, ExportKind::Func, 0);
    code.function(&set);
    if table.element_type.nullable {
        let mut grow = Function::new([]);
        grow.instructions()
            .ref_null(table.element_type.heap_type)
            .local_get(0)
            .table_grow(helped)
            .end();
        functions.function(grow_type);
        exports.export(GROW_ITEM, ExportKind::Func, 1);
        code.function(&grow);
    }
    let memories = wasm_encoder::MemorySection::new();
    assemble(&types, &imports, &functions, &memories, &exports, &code)
}

/// A module that imports a mutable global of the type and exports `set`, which sets it; a reference is passed as a
/// function reference and turned into the global's type.
fn global_setter_module(
    mut types: TypeSection,
    first_type: u32,
    global: wasm_encoder::GlobalType,
) -> Vec<u8> {
    use wasm_encoder::Function;
    let parameter = match global.val_type {
        wasm_encoder::ValType::Ref(_) => wasm_encoder::ValType::FUNCREF,
        other => other,
    };
    types.ty().function([parameter], []);
    let mut imports = ImportSection::new();
    imports.import(HELPED, HELPED, global);
    let mut functions = FunctionSection::new();
    functions.function(first_type);
    let mut exports = ExportSection::new();
    exports.export(SET_ITEM, ExportKind::Func, 0);
    let mut set = Function::new([]);
    let mut sink = set.instructions();
    if let wasm_encoder::ValType::Ref(reference) = global.val_type {
        function_as(&mut sink, 0, reference);
    } else {
        sink.local_get(0);
    }
    sink.global_set(0).end();
    let mut code = CodeSection::new();
    code.function(&set);
    let memories = wasm_encoder::MemorySection::new();
    assemble(&types, &imports, &functions, &memories, &exports, &code)
}

/// A module that imports a function of the type, given by its index among the module's types, and exports a table
/// whose one element refers to it. The element carries the function's own type, which WasmEdge keeps for a reference
/// read from a table and drops from one a call returns.
fn retyper_module(types: &TypeSection, function_type: u32) -> Vec<u8> {
    use wasm_encoder::{ConstExpr, ElementSection, Elements, TableSection};
    let mut imports = ImportSection::new();
    imports.import(HELPED, HELPED, EntityType::Function(function_type));
    let mut tables = TableSection::new();
    tables.table(one_element(wasm_encoder::RefType::FUNCREF));
    let mut exports = ExportSection::new();
    exports.export(RETYPED_FUNCTION, ExportKind::Table, 0);
    let (helped, retyped) = (0, 0);
    let mut elements = ElementSection::new();
    elements.active(
        Some(retyped),
        &ConstExpr::i32_const(0),
        Elements::Functions(std::borrow::Cow::Borrowed(&[helped])),
    );
    let mut module = wasm_encoder::Module::new();
    module
        .section(types)
        .section(&imports)
        .section(&tables)
        .section(&exports)
        .section(&elements);
    module.finish()
}

/// A module that exports a funcref table of one element and `set_item`, which stores a reference of the type there,
/// given among the types of the slot's module. Read back from that table, the reference is of the outer type
/// `funcref` and still carries the function's own type.
fn holder_module(
    mut types: TypeSection,
    first_type: u32,
    reference: wasm_encoder::RefType,
) -> Vec<u8> {
    use wasm_encoder::{Function, TableSection};
    types
        .ty()
        .function([wasm_encoder::ValType::Ref(reference)], []);
    let mut functions = FunctionSection::new();
    functions.function(first_type);
    let mut tables = TableSection::new();
    tables.table(one_element(wasm_encoder::RefType::FUNCREF));
    let mut exports = ExportSection::new();
    exports
        .export(SET_ITEM, ExportKind::Func, 0)
        .export(HELD_FUNCTION, ExportKind::Table, 0);
    let mut set = Function::new([]);
    set.instructions()
        .i32_const(0)
        .local_get(0)
        .table_set(0)
        .end();
    let mut code = CodeSection::new();
    code.function(&set);
    let mut module = wasm_encoder::Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&tables)
        .section(&exports)
        .section(&code);
    module.finish()
}

/// The type of a 32-bit table of one element.
fn one_element(element_type: wasm_encoder::RefType) -> wasm_encoder::TableType {
    wasm_encoder::TableType {
        element_type,
        table64: false,
        minimum: 1,
        maximum: Some(1),
        shared: false,
    }
}

/// A module whose one table element holds a function of the type, given by its index among the types, and which
/// exports `set_item`, which stores a function reference there and traps when the function is of another type, and
/// `pass_on`, of that type, which calls the stored function with its parameters.
fn passing_module(
    mut types: TypeSection,
    first_type: u32,
    function_type: u32,
    parameters: u32,
) -> Vec<u8> {
    use wasm_encoder::{Function, TableSection};
    let typed = wasm_encoder::RefType {
        nullable: true,
        heap_type: HeapType::Concrete(function_type),
    };
    types.ty().function([wasm_encoder::ValType::FUNCREF], []);
    let mut functions = FunctionSection::new();
    functions.function(first_type).function(function_type);
    let mut tables = TableSection::new();
    tables.table(one_element(typed));
    let mut exports = ExportSection::new();
    exports
        .export(SET_ITEM, ExportKind::Func, 0)
        .export(PASS_ON, ExportKind::Func, 1);
    let stored = 0;
    let mut set = Function::new([]);
    let mut sink = set.instructions();
    sink.i32_const(0);
    function_as(
        &mut sink,
        0,
        wasm_encoder::RefType {
            nullable: false,
            ..typed
        },
    );
    sink.table_set(stored).end();
    let mut pass_on = Function::new([]);
    let mut sink = pass_on.instructions();
    for parameter in 0..parameters {
        sink.local_get(parameter);
    }
    sink.i32_const(0).call_indirect(stored, function_type).end();
    let mut code = CodeSection::new();
    code.function(&set).function(&pass_on);
    let mut module = wasm_encoder::Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&tables)
        .section(&exports)
        .section(&code);
    module.finish()
}

/// A module that defines and exports one memory or one table.
fn defining_module(entity: EntityType) -> Vec<u8> {
    let mut module = wasm_encoder::Module::new();
    let mut exports = ExportSection::new();
    match entity {
        EntityType::Memory(memory) => {
            let mut memories = wasm_encoder::MemorySection::new();
            memories.memory(memory);
            exports.export(DEFINED_MEMORY, ExportKind::Memory, 0);
            module.section(&memories);
        }
        EntityType::Table(table) => {
            let mut tables = wasm_encoder::TableSection::new();
            tables.table(table);
            exports.export(DEFINED_TABLE, ExportKind::Table, 0);
            module.section(&tables);
        }
        EntityType::Function(_)
        | EntityType::FunctionExact(_)
        | EntityType::Global(_)
        | EntityType::Tag(_) => {}
    }
    module.section(&exports);
    module.finish()
}

/// A module that imports each item from the instance that exports it, with the type the importing module declares,
/// and exports it under the name the importing module imports it under.
fn forwarder(types: &TypeSection, items: &[Forwarded<'_>]) -> Vec<u8> {
    let mut imports = ImportSection::new();
    let mut exports = ExportSection::new();
    let mut exported: Vec<&str> = Vec::new();
    let (mut functions, mut tables, mut memories, mut globals, mut tags) = (0, 0, 0, 0, 0);
    for item in items {
        let kind = export_kind(item.entity);
        let count = match kind {
            ExportKind::Func => &mut functions,
            ExportKind::Table => &mut tables,
            ExportKind::Memory => &mut memories,
            ExportKind::Global => &mut globals,
            ExportKind::Tag => &mut tags,
        };
        let index = *count;
        *count += 1;
        imports.import(&item.owner, &item.name, item.entity);
        if !exported.contains(&item.item_name) {
            exports.export(item.item_name, kind, index);
            exported.push(item.item_name);
        }
    }
    let mut module = wasm_encoder::Module::new();
    module.section(types).section(&imports).section(&exports);
    module.finish()
}

/// A compiled module, loaded as native code, with what it declares.
#[derive(Clone)]
pub struct Module {
    loaded: Arc<wasmedge_sys::Module>,
    description: Arc<Description>,
}

/// Something that owns instances an address refers to.
enum Owner {
    Instance(wasmedge_sys::Instance, Arc<Description>),
    /// A global the host made.
    Global(ImportModule<()>),
    Function(Box<dyn AsInstance>),
    /// An instance that exports a shared memory, which other engine values may hold too.
    Shared(Arc<SharedOwner>),
}

struct SharedOwner {
    instance: wasmedge_sys::Instance,
    description: Arc<Description>,
}

impl Owner {
    fn as_instance(&self) -> &dyn AsInstance {
        match self {
            Owner::Instance(instance, _) => instance,
            Owner::Global(module) => module,
            Owner::Function(module) => &**module,
            Owner::Shared(shared) => &shared.instance,
        }
    }
}

/// Evaluates `$body` with `$instance` bound to the owner as its own type, or gives `$otherwise`.
macro_rules! readable {
    ($owner:expr, $instance:ident => $body:expr, $otherwise:expr) => {
        match $owner {
            Owner::Instance($instance, _) => $body,
            Owner::Global($instance) => $body,
            Owner::Shared(shared) => {
                let $instance = &shared.instance;
                $body
            }
            Owner::Function(_) => $otherwise,
        }
    };
}

/// Evaluates `$body` with `$instance` bound mutably to the owner as its own type, or gives `$otherwise` when the
/// bindings cannot write the owner: a shared owner is written through helpers.
macro_rules! writable {
    ($owner:expr, $instance:ident => $body:expr, $otherwise:expr) => {
        match $owner {
            Owner::Instance($instance, _) => $body,
            Owner::Global($instance) => $body,
            Owner::Shared(_) | Owner::Function(_) => $otherwise,
        }
    };
}

/// Instances kept alive after their engine value is gone, as long as an address refers to one of them.
struct Graveyard(RefCell<Vec<Owner>>);

/// One thread's WasmEdge executor and the instances it made, with the host context.
pub struct WasmEdgeEngine<HostContext: 'static> {
    identity: Identity,
    config: Config,
    /// Borrowed at run time only by the methods the interface gives `&self` that call into a helper.
    executor: RefCell<Executor>,
    /// The host context, always here except while a call lends it to the host functions. It is boxed so that
    /// lending it moves a pointer, whatever its size.
    context: Option<Box<HostContext>>,
    /// Where host functions find the host context during a call.
    lent: Rc<Cell<Option<Box<HostContext>>>>,
    owners: Vec<Owner>,
    /// The helper instance linked to each item that has one.
    helped: Vec<(Export, usize)>,
    /// Each holder made so far, by the helper module it is an instance of.
    holders: Vec<(Arc<wasmedge_sys::Module>, Holder)>,
    /// A null reference to a function, which the bindings make only by reading an empty table.
    null_function: WasmValue,
    graveyard: Rc<Graveyard>,
    slow_paths: SlowPaths,
}

impl<HostContext: 'static> Drop for WasmEdgeEngine<HostContext> {
    fn drop(&mut self) {
        self.graveyard.0.borrow_mut().append(&mut self.owners);
    }
}

impl<HostContext: 'static> WasmEdgeEngine<HostContext> {
    /// How many times this engine value took the slow path.
    pub fn slow_path_count(&self, path: SlowPath) -> u64 {
        self.slow_paths.0[path as usize].get()
    }
}

/// An instance's export, by the engine value that holds its owner, the owner and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Export {
    engine: Identity,
    owner: usize,
    name: Rc<str>,
}

/// A function: WasmEdge's reference to it, where it is exported when that is known, and what keeps it alive.
#[derive(Clone)]
pub struct FunctionAddress {
    engine: Identity,
    reference: WasmValue,
    /// Where the function is exported, for a reference the bindings made, which carries only `funcref`. Missing for
    /// a reference read from a table or a global, which carries the function's own type.
    origin: Option<Export>,
    /// The reference that carries the function's own type, once a retyper gave it.
    retyped: Cell<Option<WasmValue>>,
    _graveyard: Rc<Graveyard>,
}

/// A memory, and how this engine value reaches its bytes.
#[derive(Clone)]
pub struct MemoryAddress {
    export: Export,
    /// The memory is unshared, so its bytes are read directly.
    direct_reads: bool,
    /// Also, its owner is this engine value's alone, so its bytes are written and grown directly.
    direct_writes: bool,
    /// Reaches the bytes otherwise, and those beyond the bindings' reach.
    copier: Option<Copier>,
}

/// The helper instance through which one engine value reaches a memory's bytes.
#[derive(Clone, Copy)]
struct Copier {
    instance: usize,
    /// It takes 64-bit offsets.
    wide: bool,
    load: WasmValue,
    store: WasmValue,
    grow: WasmValue,
}

impl MemoryAddress {
    /// Why a write or a growth of the memory goes through its copier.
    fn copied_writes(&self) -> SlowPath {
        if !self.direct_reads {
            SlowPath::SharedMemoryCopied
        } else if !self.direct_writes {
            SlowPath::SharedOwnerMemoryCopied
        } else {
            SlowPath::BeyondBindingsReach
        }
    }
}

impl Copier {
    fn address(&self, value: u64) -> Option<WasmValue> {
        if self.wide {
            Some(WasmValue::from_i64(value.cast_signed()))
        } else {
            u32::try_from(value)
                .ok()
                .map(|value| WasmValue::from_i32(value.cast_signed()))
        }
    }
}

/// A table, with the helper instance that writes and grows it when its elements refer to functions.
#[derive(Clone)]
pub struct TableAddress {
    export: Export,
    writer: Option<TableWriter>,
    /// Missing when the elements are of the type `funcref`.
    holder: Option<Holder>,
}

/// The helper instance that holds a reference read from a slot of a typed reference type, so that it is read back
/// of the outer type `funcref`.
#[derive(Clone, Copy)]
struct Holder {
    instance: usize,
    set: WasmValue,
}

#[derive(Clone, Copy)]
struct TableWriter {
    /// It takes 64-bit indices.
    wide: bool,
    set: WasmValue,
    /// Missing when the elements may not be null.
    grow: Option<WasmValue>,
}

/// A global, its type, and the helper that sets it when the bindings cannot.
#[derive(Clone)]
pub struct GlobalAddress {
    export: Export,
    slot: Slot,
    mutable: bool,
    setter: Option<WasmValue>,
    /// Present when the global holds references of a type other than `funcref`.
    holder: Option<Holder>,
}

/// What a global holds, as far as the interface's values tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    I32,
    I64,
    F32,
    F64,
    V128,
    Function {
        nullable: bool,
    },
    /// A reference to something other than a function.
    Other,
}

impl Slot {
    fn of(value_type: wasm_encoder::ValType, description: &Description) -> Self {
        match value_type {
            wasm_encoder::ValType::I32 => Self::I32,
            wasm_encoder::ValType::I64 => Self::I64,
            wasm_encoder::ValType::F32 => Self::F32,
            wasm_encoder::ValType::F64 => Self::F64,
            wasm_encoder::ValType::V128 => Self::V128,
            wasm_encoder::ValType::Ref(reference)
                if description.refers_to_functions(reference.heap_type) =>
            {
                Self::Function {
                    nullable: reference.nullable,
                }
            }
            wasm_encoder::ValType::Ref(_) => Self::Other,
        }
    }

    fn of_value(value: &Value<FunctionAddress>) -> Self {
        match value {
            Value::I32(_) => Self::I32,
            Value::I64(_) => Self::I64,
            Value::F32(_) => Self::F32,
            Value::F64(_) => Self::F64,
            Value::V128(_) => Self::V128,
            Value::FunctionReference(_) => Self::Function { nullable: true },
        }
    }

    fn takes(self, value: &Value<FunctionAddress>) -> bool {
        match (self, value) {
            (Self::Function { nullable }, Value::FunctionReference(function)) => {
                nullable || function.is_some()
            }
            (slot, value) => slot != Self::Other && slot == Self::of_value(value),
        }
    }
}

/// A tag, by where it is exported.
#[derive(Clone)]
pub struct TagAddress {
    export: Export,
}

/// An instance of a module, made by one engine value.
pub struct Instance {
    engine: Identity,
    owner: usize,
}

/// A shared memory and what owns it, which any engine value can add.
#[derive(Clone)]
pub struct SharedMemory {
    owner: Arc<SharedOwner>,
    name: Arc<str>,
}

/// Why `compile` failed.
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("no private folder for the compiled library could be made: {0}")]
    Folder(std::io::Error),
    #[error("WasmEdge refused the module: {0}")]
    Engine(#[from] Error),
    #[error("the module's declarations could not be read: {0}")]
    Unreadable(#[from] ReencodeError),
}

/// Why `instantiate` failed.
#[derive(Debug, Error)]
pub enum InstantiateError {
    #[error("no address was given for the import {module_name}.{item_name}")]
    MissingImport {
        module_name: String,
        item_name: String,
    },
    #[error(
        "the address given for the import {module_name}.{item_name} was made by another engine value"
    )]
    ForeignAddress {
        module_name: String,
        item_name: String,
    },
    #[error("the imported function, which no instance exports, is of another type than the import")]
    FunctionTypeMismatch,
    #[error(
        "WasmEdge refused the helper module that passes on an imported function no instance exports"
    )]
    HelperRefused,
    #[error("WasmEdge refused the instantiation: {0}")]
    Engine(#[from] Error),
}

/// Why a memory or a table could not be allocated.
#[derive(Debug, Error)]
pub enum AllocateError {
    #[error("the limits do not fit the address type")]
    LimitsOutsideAddressType,
    #[error("WasmEdge refused the helper module that reaches what was allocated")]
    HelperRefused,
    #[error("WasmEdge refused it: {0}")]
    Engine(#[from] Error),
    #[error("WasmEdge refused to instantiate it: {0}")]
    Instantiate(#[from] InstantiateError),
}

/// An item an instantiation links, by the names the module imports it under and the export it is.
struct Link<'name> {
    module_name: &'name str,
    item_name: &'name str,
    export: Export,
}

/// What a module that passes on an import imports, and the name it exports it under.
struct Forwarded<'name> {
    owner: String,
    name: Rc<str>,
    item_name: &'name str,
    entity: EntityType,
}

/// Whether `length` bytes at `offset` lie inside `size` bytes.
fn inside(size: u64, offset: u64, length: usize) -> bool {
    u64::try_from(length)
        .ok()
        .and_then(|length| offset.checked_add(length))
        .is_some_and(|end| end <= size)
}

/// Whether `length` bytes at `offset` end within the bindings' reach.
fn within_reach(offset: u64, length: usize) -> bool {
    inside(BINDINGS_REACH, offset, length)
}

/// The size of the memory in bytes.
fn memory_size(memory: &wasmedge_sys::Memory) -> u64 {
    u64::from(memory.size()) * PAGE_SIZE
}

/// Copies the bytes at the offset of an unshared memory, within the bindings' reach, into the given bytes.
fn read_unshared(
    memory: &wasmedge_sys::Memory,
    offset: u64,
    into: &mut [u8],
) -> Result<(), ReadMemoryError> {
    if !inside(memory_size(memory), offset, into.len()) {
        return Err(ReadMemoryError);
    }
    if into.is_empty() {
        return Ok(());
    }
    let offset = usize::try_from(offset).map_err(|_| ReadMemoryError)?;
    let bytes = memory
        .slice::<u8>(offset, into.len())
        .ok_or(ReadMemoryError)?;
    into.copy_from_slice(bytes);
    Ok(())
}

fn write_unshared(
    memory: &mut wasmedge_sys::Memory,
    offset: u64,
    bytes: &[u8],
) -> Result<(), WriteMemoryError> {
    if !inside(memory_size(memory), offset, bytes.len()) {
        return Err(WriteMemoryError);
    }
    if bytes.is_empty() {
        return Ok(());
    }
    let offset = u32::try_from(offset).map_err(|_| WriteMemoryError)?;
    memory.set_data(bytes, offset).map_err(|_| WriteMemoryError)
}

impl<HostContext: 'static> WasmEdgeEngine<HostContext> {
    fn add_owner(&mut self, owner: Owner) -> usize {
        self.owners.push(owner);
        self.owners.len() - 1
    }

    /// The owner's export under the name.
    fn export(&self, owner: usize, name: Rc<str>) -> Export {
        Export {
            engine: self.identity,
            owner,
            name,
        }
    }

    /// Panics on an address another engine value made: its owner index means nothing here.
    fn check(&self, engine: Identity) {
        assert!(engine == self.identity, "{FOREIGN_ADDRESS}");
    }

    fn owner(&self, export: &Export) -> &Owner {
        self.check(export.engine);
        &self.owners[export.owner]
    }

    fn owner_mut(&mut self, export: &Export) -> &mut Owner {
        self.check(export.engine);
        &mut self.owners[export.owner]
    }

    fn description(&self, owner: usize) -> Option<Arc<Description>> {
        match &self.owners[owner] {
            Owner::Instance(_, description) => Some(Arc::clone(description)),
            Owner::Shared(shared) => Some(Arc::clone(&shared.description)),
            Owner::Global(_) | Owner::Function(_) => None,
        }
    }

    fn function_address(&self, reference: WasmValue, origin: Option<Export>) -> FunctionAddress {
        FunctionAddress {
            engine: self.identity,
            reference,
            origin,
            retyped: Cell::default(),
            _graveyard: Rc::clone(&self.graveyard),
        }
    }

    /// Runs a call with the host context lent to the host functions.
    fn lend<R>(&mut self, call: impl FnOnce(&mut Executor) -> R) -> R {
        self.lent.set(self.context.take());
        let result = call(self.executor.get_mut());
        self.context = self.lent.take();
        result
    }

    /// Finds where a function is exported, among every instance of this engine value.
    fn origin(&self, function: &FunctionAddress) -> Option<Export> {
        self.check(function.engine);
        if let Some(origin) = &function.origin {
            return Some(origin.clone());
        }
        // A reference read from a table or a global may carry another type beside the function; making it anew
        // from the function gives every reference to one function the same bits.
        let wanted = WasmValue::from_func_ref(&*function.reference.func_ref()?)
            .as_raw()
            .Value;
        self.owners.iter().enumerate().find_map(|(owner, found)| {
            let instance = found.as_instance();
            instance.func_names()?.into_iter().find_map(|name| {
                let exported = instance.get_func(&name).ok()?;
                (WasmValue::from_func_ref(&*exported).as_raw().Value == wanted)
                    .then(|| self.export(owner, name.into()))
            })
        })
    }

    /// Instantiates the module in a store of its own, which holds the owners of what it imports and, under each
    /// name it imports from, a module that passes those imports on.
    fn link(&mut self, module: &Module, links: &[Link<'_>]) -> Result<usize, InstantiateError> {
        let mut store = Store::create()?;
        let mut registered = Vec::new();
        let mut forwarders: Vec<(&str, Vec<Forwarded<'_>>)> = Vec::new();
        for link in links {
            self.check(link.export.engine);
            let owner = link.export.owner;
            if !registered.contains(&owner) {
                self.executor
                    .get_mut()
                    .register_import_module(&mut store, self.owners[owner].as_instance())?;
                registered.push(owner);
            }
            let Some((_, _, entity)) =
                module
                    .description
                    .imports
                    .iter()
                    .find(|(module_name, item_name, _)| {
                        module_name == link.module_name && item_name == link.item_name
                    })
            else {
                continue;
            };
            let forwarded = Forwarded {
                owner: self.owners[owner].as_instance().name().unwrap_or_default(),
                name: Rc::clone(&link.export.name),
                item_name: link.item_name,
                entity: *entity,
            };
            match forwarders
                .iter_mut()
                .find(|(module_name, _)| *module_name == link.module_name)
            {
                Some((_, items)) => items.push(forwarded),
                None => forwarders.push((link.module_name, vec![forwarded])),
            }
        }
        for (module_name, items) in &forwarders {
            let forwarding = declare(&self.config.0, &forwarder(&module.description.types, items))?;
            let instance = self.executor.get_mut().register_named_module(
                &mut store,
                &forwarding.loaded,
                module_name,
            )?;
            self.add_owner(Owner::Instance(instance, forwarding.description));
        }
        let instance = self.lend(|executor| {
            executor.register_named_module(&mut store, &module.loaded, unique_name())
        })?;
        let description = Arc::clone(&module.description);
        Ok(self.add_owner(if description.exports_a_shared_memory() {
            Owner::Shared(Arc::new(SharedOwner {
                instance,
                description,
            }))
        } else {
            Owner::Instance(instance, description)
        }))
    }

    /// Instantiates a module this crate wrote that defines one item.
    fn define(&mut self, entity: EntityType) -> Result<usize, AllocateError> {
        let module = declare(&self.config.0, &defining_module(entity))?;
        Ok(self.link(&module, &[])?)
    }

    /// The function the instance exports under the name.
    fn exported_function(&self, owner: usize, name: &str) -> Option<WasmValue> {
        let function = self.owners[owner].as_instance().get_func(name).ok()?;
        Some(WasmValue::from_func_ref(&*function))
    }

    /// The instance of the compiled helper module of the bytes for the item, linked the first time it is asked for.
    fn helper_instance(&mut self, export: &Export, bytes: Vec<u8>) -> Option<usize> {
        self.linked_helper(export, |config| config.helper(bytes))
    }

    /// The instance of the helper module for the item, made and linked the first time it is asked for.
    fn linked_helper(
        &mut self,
        export: &Export,
        module: impl FnOnce(&Config) -> Option<Module>,
    ) -> Option<usize> {
        if let Some((_, instance)) = self.helped.iter().find(|(found, _)| found == export) {
            return Some(*instance);
        }
        let module = module(&self.config)?;
        let link = Link {
            module_name: HELPED,
            item_name: HELPED,
            export: export.clone(),
        };
        let instance = self.link(&module, &[link]).ok()?;
        self.helped.push((export.clone(), instance));
        Some(instance)
    }

    /// The address of the memory, or nothing when a copier it needs cannot be made.
    fn memory_address(
        &mut self,
        export: Export,
        memory_type: wasm_encoder::MemoryType,
    ) -> Option<MemoryAddress> {
        let alone = !matches!(self.owner(&export), Owner::Shared(_));
        let direct_reads = !memory_type.shared;
        let direct_writes = direct_reads && alone;
        let copier = if direct_writes && !memory_type.memory64 {
            None
        } else {
            let instance = self.helper_instance(&export, copier_module(memory_type))?;
            Some(Copier {
                instance,
                wide: memory_type.memory64,
                load: self.exported_function(instance, COPY_OUT)?,
                store: self.exported_function(instance, COPY_IN)?,
                grow: self.exported_function(instance, GROW_ITEM)?,
            })
        };
        Some(MemoryAddress {
            export,
            direct_reads,
            direct_writes,
            copier,
        })
    }

    /// The address of the table, or nothing when the writer of a table of functions cannot be made.
    fn table_address(
        &mut self,
        export: Export,
        table_type: wasm_encoder::TableType,
        description: &Description,
    ) -> Option<TableAddress> {
        if !description.refers_to_functions(table_type.element_type.heap_type) {
            return Some(TableAddress {
                export,
                writer: None,
                holder: None,
            });
        }
        let holder = self.holder(table_type.element_type, description)?;
        let (types, first_type) = description.helper_types(Some(table_type.element_type));
        let instance =
            self.helper_instance(&export, table_writer_module(types, first_type, table_type))?;
        let writer = TableWriter {
            wide: table_type.table64,
            set: self.exported_function(instance, SET_ITEM)?,
            grow: self.exported_function(instance, GROW_ITEM),
        };
        Some(TableAddress {
            export,
            writer: Some(writer),
            holder,
        })
    }

    /// The address of the global, or nothing when a setter it needs cannot be made. A global needs one when its
    /// owner is shared, which the bindings cannot write, or when it holds typed function references, which a
    /// function's reference must be turned into.
    fn global_address(
        &mut self,
        export: Export,
        global_type: wasm_encoder::GlobalType,
        description: &Description,
    ) -> Option<GlobalAddress> {
        let slot = Slot::of(global_type.val_type, description);
        let typed = matches!(global_type.val_type, wasm_encoder::ValType::Ref(reference) if reference != wasm_encoder::RefType::FUNCREF);
        let shared_owner = matches!(self.owner(&export), Owner::Shared(_));
        let setter = if global_type.mutable && slot != Slot::Other && (shared_owner || typed) {
            let reference = match global_type.val_type {
                wasm_encoder::ValType::Ref(reference) => Some(reference),
                _ => None,
            };
            let (types, first_type) = description.helper_types(reference);
            let instance = self.helper_instance(
                &export,
                global_setter_module(types, first_type, global_type),
            )?;
            Some(self.exported_function(instance, SET_ITEM)?)
        } else {
            None
        };
        let holder = match (slot, global_type.val_type) {
            (Slot::Function { .. }, wasm_encoder::ValType::Ref(reference)) => {
                self.holder(reference, description)?
            }
            _ => None,
        };
        Some(GlobalAddress {
            export,
            slot,
            mutable: global_type.mutable,
            setter,
            holder,
        })
    }

    /// The reference to the function that carries the function's own type, which every reference the host writes
    /// into a table or a global must carry: WasmEdge casts a reference by the type it carries, and a reference the
    /// bindings make carries only `funcref`. Such a reference is read from the table of a helper that imports the
    /// function from where it is exported, with the type its module declares, or for a host function the type its
    /// values have.
    fn typed_reference(&mut self, function: &FunctionAddress) -> WasmValue {
        self.check(function.engine);
        let Some(origin) = &function.origin else {
            return function.reference;
        };
        if let Some(retyped) = function.retyped.get() {
            return retyped;
        }
        if let Some(retyped) = self.retyped(origin, function) {
            function.retyped.set(Some(retyped));
            self.slow_paths.took(SlowPath::FunctionRetyped);
            retyped
        } else {
            self.slow_paths.took(SlowPath::FunctionNotRetyped);
            function.reference
        }
    }

    /// The reference read from the table of the function's retyper, made the first time it is asked for.
    fn retyped(&mut self, origin: &Export, function: &FunctionAddress) -> Option<WasmValue> {
        let instance = match self.helped.iter().find(|(found, _)| found == origin) {
            Some((_, instance)) => *instance,
            None => {
                let bytes = self.retyper_bytes(origin, function)?;
                self.linked_helper(origin, |config| declare(&config.0, &bytes).ok())?
            }
        };
        let Owner::Instance(retyper, _) = &self.owners[instance] else {
            return None;
        };
        retyper.get_table(RETYPED_FUNCTION).ok()?.get_data(0).ok()
    }

    /// The holder of references read from slots of the reference type, made the first time any slot of a module
    /// with these types asks for it; nothing for `funcref`, whose references need none. Answers `None` when the
    /// holder cannot be made.
    fn holder(
        &mut self,
        reference: wasm_encoder::RefType,
        description: &Description,
    ) -> Option<Option<Holder>> {
        if reference == wasm_encoder::RefType::FUNCREF {
            return Some(None);
        }
        let (types, first_type) = description.helper_types(Some(reference));
        let module = self
            .config
            .helper(holder_module(types, first_type, reference))?;
        if let Some((_, holder)) = self
            .holders
            .iter()
            .find(|(loaded, _)| Arc::ptr_eq(loaded, &module.loaded))
        {
            return Some(Some(*holder));
        }
        let instance = self.link(&module, &[]).ok()?;
        let holder = Holder {
            instance,
            set: self.exported_function(instance, SET_ITEM)?,
        };
        self.holders.push((Arc::clone(&module.loaded), holder));
        Some(Some(holder))
    }

    /// The reference read from a slot, as the host passes it on: of the outer type `funcref`, carrying the
    /// function's own type. The outer type WasmEdge gives a reference read from a typed slot names a type index of
    /// the slot's module, and WasmEdge refuses such a value as a `funcref` parameter; so the slot's holder, whose
    /// parameter is of the slot's own type, stores it in a funcref table, from which it is read back.
    fn passable(&self, reference: WasmValue, holder: Option<Holder>) -> WasmValue {
        let Some(holder) = holder else {
            return reference;
        };
        self.slow_paths.took(SlowPath::ReferenceHeld);
        Self::call_helper(&mut self.executor.borrow_mut(), holder.set, [reference])
            .expect("a holder takes every reference its slot holds");
        let Owner::Instance(instance, _) = &self.owners[holder.instance] else {
            panic!("a holder is an instance of this engine value alone");
        };
        instance
            .get_table(HELD_FUNCTION)
            .and_then(|table| table.get_data(0))
            .expect("a holder's table holds what it stored")
    }

    /// An export that passes calls on to a function no instance exports, with the type the importing module declares
    /// for it: WasmEdge links only by name, so a helper holds the function in its table and calls it indirectly.
    fn pass_on(
        &mut self,
        function: &FunctionAddress,
        entity: EntityType,
        importing: &Description,
    ) -> Result<Export, InstantiateError> {
        let (EntityType::Function(function_type) | EntityType::FunctionExact(function_type)) =
            entity
        else {
            return Err(InstantiateError::FunctionTypeMismatch);
        };
        let parameters = importing
            .parameters(function_type)
            .ok_or(InstantiateError::FunctionTypeMismatch)?;
        let bytes = passing_module(
            importing.types.clone(),
            importing.type_count(),
            function_type,
            parameters,
        );
        let module = self
            .config
            .helper(bytes)
            .ok_or(InstantiateError::HelperRefused)?;
        let owner = self.link(&module, &[])?;
        let set = self
            .exported_function(owner, SET_ITEM)
            .ok_or(InstantiateError::HelperRefused)?;
        // A trap here is a function of another type than the import's.
        Self::call_helper(self.executor.get_mut(), set, [function.reference])
            .map_err(|_| InstantiateError::FunctionTypeMismatch)?;
        self.slow_paths.took(SlowPath::FunctionPassedOn);
        Ok(self.export(owner, PASS_ON.into()))
    }

    /// The retyper module for the function exported there.
    fn retyper_bytes(&self, origin: &Export, function: &FunctionAddress) -> Option<Vec<u8>> {
        if let Some(description) = self.description(origin.owner) {
            let (EntityType::Function(type_index) | EntityType::FunctionExact(type_index)) =
                description.export(&origin.name)?
            else {
                return None;
            };
            return Some(retyper_module(&description.types, type_index));
        }
        let function_type = function.reference.func_ref()?.ty()?;
        let numbers = |value_types: &[ValType]| -> Option<Vec<wasm_encoder::ValType>> {
            value_types
                .iter()
                .map(|value_type| number_type(*value_type))
                .collect()
        };
        let mut types = TypeSection::new();
        types.ty().function(
            numbers(function_type.args())?,
            numbers(function_type.returns())?,
        );
        Some(retyper_module(&types, u32::MIN))
    }

    /// Makes an instance of the host that holds one item.
    fn host_module(add: impl FnOnce(&mut ImportModule<()>)) -> Result<ImportModule<()>, Error> {
        let mut module = ImportModule::create(unique_name(), Box::new(()))?;
        add(&mut module);
        Ok(module)
    }

    fn add_host_function<Data: 'static>(
        &mut self,
        function_type: FuncType,
        data: Data,
        function: SyncFn<Data>,
    ) -> FunctionAddress {
        let mut builder = ImportObjectBuilder::new(&unique_name(), data)
            .expect("WasmEdge makes a module for the host function");
        builder
            .with_func_by_type(HOST_FUNCTION, function_type, function)
            .expect("WasmEdge makes the host function");
        let module = builder.build();
        let reference = WasmValue::from_func_ref(
            &*module
                .get_func(HOST_FUNCTION)
                .expect("the host function's module holds it"),
        );
        let owner = self.add_owner(Owner::Function(Box::new(module)));
        let origin = self.export(owner, HOST_FUNCTION.into());
        self.function_address(reference, Some(origin))
    }

    /// Calls a function with the values and returns what it returns.
    fn call_helper(
        executor: &mut Executor,
        function: WasmValue,
        values: impl IntoIterator<Item = WasmValue>,
    ) -> Result<Vec<WasmValue>, Error> {
        let function = function.func_ref().expect("a function called is not null");
        executor.call_func_ref(&mut Callee(&function), values)
    }
}

/// A borrowed function in the form the bindings call.
struct Callee<'function>(&'function wasmedge_sys::Function);

impl AsRef<wasmedge_sys::Function> for Callee<'_> {
    fn as_ref(&self) -> &wasmedge_sys::Function {
        self.0
    }
}

impl<HostContext: 'static> WebAssemblyEngine for WasmEdgeEngine<HostContext> {
    type Config = Config;
    type Module = Module;
    type Instance = Instance;
    type TagAddress = TagAddress;
    type HostContext = HostContext;

    type NewError = Error;
    fn new(config: Config, host_context: HostContext) -> Result<Self, Error> {
        let executor = Executor::create(Some(&config.0.compiled), None)?;
        let null_function = wasmedge_sys::Table::create(wasmedge_sdk::TableType::new(
            RefType::FuncRef,
            1,
            Some(1),
        ))?
        .get_data(0)?;
        Ok(Self {
            identity: Identity::new(),
            config,
            executor: RefCell::new(executor),
            context: Some(Box::new(host_context)),
            lent: Rc::default(),
            owners: Vec::new(),
            helped: Vec::new(),
            holders: Vec::new(),
            null_function,
            graveyard: Rc::new(Graveyard(RefCell::default())),
            slow_paths: SlowPaths(Default::default()),
        })
    }

    type CompileError = CompileError;
    fn compile(&self, bytes: &[u8]) -> Result<Module, CompileError> {
        compile(&self.config.0, bytes)
    }

    type InstantiateError = InstantiateError;
    fn instantiate(
        &mut self,
        module: &Module,
        imports: &[Import<'_, Self>],
    ) -> Result<Instance, InstantiateError> {
        let mut links = Vec::with_capacity(module.description.imports.len());
        for (module_name, item_name, entity) in &module.description.imports {
            let given = imports
                .iter()
                .find(|given| given.module_name == module_name && given.item_name == item_name)
                .ok_or_else(|| InstantiateError::MissingImport {
                    module_name: module_name.clone(),
                    item_name: item_name.clone(),
                })?;
            let engine = match &given.address {
                ExternalAddress::Function(function) => function.engine,
                ExternalAddress::Memory(memory) => memory.export.engine,
                ExternalAddress::Table(table) => table.export.engine,
                ExternalAddress::Global(global) => global.export.engine,
                ExternalAddress::Tag(tag) => tag.export.engine,
            };
            if engine != self.identity {
                return Err(InstantiateError::ForeignAddress {
                    module_name: module_name.clone(),
                    item_name: item_name.clone(),
                });
            }
            let export = match &given.address {
                ExternalAddress::Function(function) => match self.origin(function) {
                    Some(export) => export,
                    None => self.pass_on(function, *entity, &module.description)?,
                },
                ExternalAddress::Memory(memory) => memory.export.clone(),
                ExternalAddress::Table(table) => table.export.clone(),
                ExternalAddress::Global(global) => global.export.clone(),
                ExternalAddress::Tag(tag) => tag.export.clone(),
            };
            links.push(Link {
                module_name,
                item_name,
                export,
            });
        }
        if !links.is_empty() {
            self.slow_paths.took(SlowPath::ImportsForwarded);
        }
        let owner = self.link(module, &links)?;
        Ok(Instance {
            engine: self.identity,
            owner,
        })
    }

    fn find_export(&mut self, instance: &Instance, name: &str) -> Option<ExternalAddress<Self>> {
        self.check(instance.engine);
        let owner = instance.owner;
        let export = self.export(owner, name.into());
        if let Ok(function) = self.owner(&export).as_instance().get_func(name) {
            let reference = WasmValue::from_func_ref(&*function);
            return Some(ExternalAddress::Function(
                self.function_address(reference, Some(export)),
            ));
        }
        let description = self.description(owner)?;
        match description.export(name)? {
            EntityType::Memory(memory_type) => self
                .memory_address(export, memory_type)
                .map(ExternalAddress::Memory),
            EntityType::Table(table_type) => self
                .table_address(export, table_type, &description)
                .map(ExternalAddress::Table),
            EntityType::Global(global_type) => self
                .global_address(export, global_type, &description)
                .map(ExternalAddress::Global),
            EntityType::Tag(_) => Some(ExternalAddress::Tag(TagAddress { export })),
            EntityType::Function(_) | EntityType::FunctionExact(_) => None,
        }
    }

    fn host_context(&mut self) -> &mut HostContext {
        self.context
            .as_deref_mut()
            .expect("the host context is lent only during a call")
    }
}

impl<HostContext: 'static> FunctionStore for WasmEdgeEngine<HostContext> {
    type FunctionAddress = FunctionAddress;
    type Trap = Error;
}

/// A function bound to a Rust signature.
pub struct Function<Parameters, Results> {
    address: FunctionAddress,
    types: PhantomData<fn(Parameters) -> Results>,
}

/// A value WasmEdge passed, in the form a Rust value is made from.
pub struct Received(WasmValue);

impl From<Received> for i32 {
    fn from(received: Received) -> Self {
        received.0.to_i32()
    }
}

impl From<Received> for i64 {
    fn from(received: Received) -> Self {
        received.0.to_i64()
    }
}

impl From<Received> for f32 {
    fn from(received: Received) -> Self {
        received.0.to_f32()
    }
}

impl From<Received> for f64 {
    fn from(received: Received) -> Self {
        received.0.to_f64()
    }
}

impl From<Received> for i128 {
    fn from(received: Received) -> Self {
        received.0.to_v128()
    }
}

/// The data of a host function's module: the function, and where it finds the host context.
struct HostFunction<HostContext, F> {
    function: F,
    context: Rc<Cell<Option<Box<HostContext>>>>,
}

fn next(values: &mut std::vec::IntoIter<WasmValue>) -> Received {
    Received(
        values
            .next()
            .expect("WasmEdge passes as many values as the function type has"),
    )
}

/// Fixes the element type of an array of value types, which an empty one does not tell.
fn value_types<const COUNT: usize>(value_types: [ValType; COUNT]) -> [ValType; COUNT] {
    value_types
}

/// Fixes the element type of an array of values, which an empty one does not tell.
fn values<const COUNT: usize>(values: [WasmValue; COUNT]) -> [WasmValue; COUNT] {
    values
}

/// Tells an exception that left the invocation from a trap.
#[cold]
fn uncaught(error: Error) -> InvokeError<Error> {
    match *error {
        WasmEdgeError::Core(CoreError::UnknownError(code))
            if code == wasmedge_sys::ffi::WasmEdge_ErrCode_UncaughtException =>
        {
            InvokeError::Exception
        }
        _ => InvokeError::Trap(error),
    }
}

macro_rules! result_type {
    (tuple $($result:ident)*) => { ($($result,)*) };
    (bare $result:ident) => { $result };
}

macro_rules! pack {
    (tuple $values:ident $($result:ident)*) => { ($($result::from(next($values)),)*) };
    (bare $values:ident $result:ident) => { $result::from(next($values)) };
}

macro_rules! unpack {
    (tuple $results:ident $($value:ident)*) => {{
        let ($($value,)*) = $results;
        vec![$($value.to_wasm_value()),*]
    }};
    (bare $results:ident $value:ident) => { vec![$results.to_wasm_value()] };
}

/// Binds one function type: its parameters, and its results as a tuple or as one bare value.
macro_rules! bind_function_type {
    ([$($parameter:ident $argument:ident)*] $shape:ident [$($result:ident $value:ident)*]) => {
        impl<HostContext: 'static, $($parameter,)* $($result,)*>
            FunctionTypeBinder<($($parameter,)*), result_type!($shape $($result)*)>
            for WasmEdgeEngine<HostContext>
        where
            $($parameter: WasmValType + WasmVal + From<Received>,)*
            $($result: WasmValType + WasmVal + From<Received>,)*
        {
            type Function = Function<($($parameter,)*), result_type!($shape $($result)*)>;

            fn bind_engine_function(
                &mut self,
                address: FunctionAddress,
            ) -> Result<Self::Function, BindFunctionError> {
                self.check(address.engine);
                let function_type = address
                    .reference
                    .func_ref()
                    .and_then(|function| function.ty())
                    .ok_or(BindFunctionError)?;
                if function_type.args() == value_types([$($parameter::WASM_TYPE),*])
                    && function_type.returns() == value_types([$($result::WASM_TYPE),*])
                {
                    Ok(Function { address, types: PhantomData })
                } else {
                    Err(BindFunctionError)
                }
            }

            fn bind_host_function(
                &mut self,
                function: impl Fn(&mut HostContext, ($($parameter,)*)) -> result_type!($shape $($result)*)
                    + Send
                    + Sync
                    + 'static,
            ) -> FunctionAddress {
                fn host<HostContext, F, $($parameter,)* $($result,)*>(
                    data: &mut HostFunction<HostContext, F>,
                    _: &mut wasmedge_sys::Instance,
                    _: &mut CallingFrame,
                    values: Vec<WasmValue>,
                ) -> Result<Vec<WasmValue>, CoreError>
                where
                    F: Fn(&mut HostContext, ($($parameter,)*)) -> result_type!($shape $($result)*),
                    $($parameter: From<Received>,)*
                    $($result: WasmVal,)*
                {
                    let _values = &mut values.into_iter();
                    let parameters = ($($parameter::from(next(_values)),)*);
                    let mut context = data
                        .context
                        .take()
                        .ok_or(CoreError::Execution(CoreExecutionError::HostFuncFailed))?;
                    let results = (data.function)(&mut *context, parameters);
                    data.context.set(Some(context));
                    Ok(unpack!($shape results $($value)*))
                }
                let function_type = FuncType::new(
                    vec![$($parameter::WASM_TYPE),*],
                    vec![$($result::WASM_TYPE),*],
                );
                let data = HostFunction { function, context: Rc::clone(&self.lent) };
                self.add_host_function(function_type, data, host::<HostContext, _, $($parameter,)* $($result,)*>)
            }
        }

        impl<HostContext: 'static, $($parameter,)* $($result,)*>
            BoundFunction<WasmEdgeEngine<HostContext>, ($($parameter,)*), result_type!($shape $($result)*)>
            for Function<($($parameter,)*), result_type!($shape $($result)*)>
        where
            $($parameter: WasmValType + WasmVal + From<Received>,)*
            $($result: WasmValType + WasmVal + From<Received>,)*
        {
            fn invoke(
                &self,
                engine: &mut WasmEdgeEngine<HostContext>,
                parameters: ($($parameter,)*),
            ) -> Result<result_type!($shape $($result)*), InvokeError<Error>> {
                engine.check(self.address.engine);
                let function = self.address.reference;
                let ($($argument,)*) = parameters;
                let returned = engine
                    .lend(|executor| {
                        WasmEdgeEngine::<HostContext>::call_helper(
                            executor,
                            function,
                            values([$($argument.to_wasm_value()),*]),
                        )
                    })
                    .map_err(uncaught)?;
                let _values = &mut returned.into_iter();
                Ok(pack!($shape _values $($result)*))
            }
        }
    };
}

/// Binds every shape of results for one list of parameters.
macro_rules! bind_results {
    ($parameters:tt) => {
        bind_function_type!($parameters bare [R1 r1]);
        bind_function_type!($parameters tuple []);
        bind_function_type!($parameters tuple [R1 r1]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9 R10 r10]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9 R10 r10 R11 r11]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9 R10 r10 R11 r11 R12 r12]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9 R10 r10 R11 r11 R12 r12 R13 r13]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9 R10 r10 R11 r11 R12 r12 R13 r13 R14 r14]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9 R10 r10 R11 r11 R12 r12 R13 r13 R14 r14 R15 r15]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9 R10 r10 R11 r11 R12 r12 R13 r13 R14 r14 R15 r15 R16 r16]);
        bind_function_type!($parameters tuple [R1 r1 R2 r2 R3 r3 R4 r4 R5 r5 R6 r6 R7 r7 R8 r8 R9 r9 R10 r10 R11 r11 R12 r12 R13 r13 R14 r14 R15 r15 R16 r16 R17 r17]);
    };
}

/// Binds every function type for each number of parameters up to the longest list given.
macro_rules! bind_parameters {
    () => {
        bind_results!([]);
    };
    ($first:ident $first_argument:ident $($parameter:ident $argument:ident)*) => {
        bind_results!([$first $first_argument $($parameter $argument)*]);
        bind_parameters!($($parameter $argument)*);
    };
}

bind_parameters!(
    A1 a1 A2 a2 A3 a3 A4 a4 A5 a5 A6 a6 A7 a7 A8 a8 A9 a9 A10 a10 A11 a11 A12 a12 A13 a13 A14 a14
    A15 a15 A16 a16 A17 a17
);

impl<HostContext: 'static> MemoryStore for WasmEdgeEngine<HostContext> {
    type MemoryAddress = MemoryAddress;

    type AllocateMemoryError = AllocateError;
    fn allocate_memory(&mut self, memory_type: MemoryType) -> Result<MemoryAddress, AllocateError> {
        let memory64 = memory_type.address_type == AddressType::I64;
        let most = if memory64 { MOST_PAGES_64 } else { MOST_PAGES };
        let Limits { minimum, maximum } = memory_type.limits;
        if minimum > most || maximum.is_some_and(|maximum| maximum > most) {
            return Err(AllocateError::LimitsOutsideAddressType);
        }
        let declared = wasm_encoder::MemoryType {
            minimum,
            maximum,
            memory64,
            shared: memory_type.sharing == Sharing::Shared,
            page_size_log2: None,
        };
        let owner = self.define(EntityType::Memory(declared))?;
        let export = self.export(owner, DEFINED_MEMORY.into());
        self.memory_address(export, declared)
            .ok_or(AllocateError::HelperRefused)
    }

    fn count_memory_pages(&self, memory: &MemoryAddress) -> u64 {
        self.pages(&memory.export)
            .expect("a memory address refers to a memory")
    }

    fn grow_memory(&mut self, memory: &MemoryAddress, pages: u64) -> Result<(), GrowMemoryError> {
        let export = &memory.export;
        self.check(export.engine);
        if memory.direct_writes {
            let pages = u32::try_from(pages).map_err(|_| GrowMemoryError)?;
            return writable!(
                self.owner_mut(export),
                instance => instance.get_memory_mut(&*export.name).ok().map(|mut memory| memory.grow(pages)),
                None
            )
            .ok_or(GrowMemoryError)?
            .map_err(|_| GrowMemoryError);
        }
        let copier = memory.copier.as_ref().ok_or(GrowMemoryError)?;
        self.slow_paths.took(memory.copied_writes());
        let pages = copier.address(pages).ok_or(GrowMemoryError)?;
        let grown = Self::call_helper(self.executor.get_mut(), copier.grow, [pages])
            .map_err(|_| GrowMemoryError)?;
        let failed = grown.first().is_none_or(|previous| {
            if copier.wide {
                previous.to_i64() == -1
            } else {
                previous.to_i32() == -1
            }
        });
        if failed { Err(GrowMemoryError) } else { Ok(()) }
    }

    fn read_memory(
        &self,
        memory: &MemoryAddress,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), ReadMemoryError> {
        let export = &memory.export;
        if memory.direct_reads && within_reach(offset, into.len()) {
            return readable!(
                self.owner(export),
                instance => instance.get_memory_ref(&*export.name).ok().map(|memory| read_unshared(&memory, offset, into)),
                None
            )
            .unwrap_or(Err(ReadMemoryError));
        }
        let copier = memory.copier.as_ref().ok_or(ReadMemoryError)?;
        self.slow_paths.took(if memory.direct_reads {
            SlowPath::BeyondBindingsReach
        } else {
            SlowPath::SharedMemoryCopied
        });
        let size = self.pages(export).ok_or(ReadMemoryError)? * PAGE_SIZE;
        if !inside(size, offset, into.len()) {
            return Err(ReadMemoryError);
        }
        let scratch = self.scratch_size(copier).ok_or(ReadMemoryError)?;
        let mut position = offset;
        let mut executor = self.executor.borrow_mut();
        for chunk in into.chunks_mut(scratch) {
            let length = u64::try_from(chunk.len()).map_err(|_| ReadMemoryError)?;
            let span = [copier.address(position), copier.address(length)];
            let [Some(at), Some(length_value)] = span else {
                return Err(ReadMemoryError);
            };
            Self::call_helper(&mut executor, copier.load, [at, length_value])
                .map_err(|_| ReadMemoryError)?;
            let Owner::Instance(helper, _) = &self.owners[copier.instance] else {
                return Err(ReadMemoryError);
            };
            let scratch_memory = helper
                .get_memory_ref(COPIED_BYTES)
                .map_err(|_| ReadMemoryError)?;
            read_unshared(&scratch_memory, u64::MIN, chunk)?;
            position += length;
        }
        Ok(())
    }

    fn write_memory(
        &mut self,
        memory: &MemoryAddress,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), WriteMemoryError> {
        let export = &memory.export;
        if memory.direct_writes && within_reach(offset, bytes.len()) {
            return writable!(
                self.owner_mut(export),
                instance => instance.get_memory_mut(&*export.name).ok().map(|mut memory| write_unshared(&mut memory, offset, bytes)),
                None
            )
            .unwrap_or(Err(WriteMemoryError));
        }
        let copier = memory.copier.as_ref().ok_or(WriteMemoryError)?;
        self.slow_paths.took(memory.copied_writes());
        let size = self.pages(export).ok_or(WriteMemoryError)? * PAGE_SIZE;
        if !inside(size, offset, bytes.len()) {
            return Err(WriteMemoryError);
        }
        let scratch = self.scratch_size(copier).ok_or(WriteMemoryError)?;
        let mut position = offset;
        for chunk in bytes.chunks(scratch) {
            let length = u64::try_from(chunk.len()).map_err(|_| WriteMemoryError)?;
            let span = [copier.address(position), copier.address(length)];
            let [Some(at), Some(length_value)] = span else {
                return Err(WriteMemoryError);
            };
            let Owner::Instance(helper, _) = &mut self.owners[copier.instance] else {
                return Err(WriteMemoryError);
            };
            write_unshared(
                &mut *helper
                    .get_memory_mut(COPIED_BYTES)
                    .map_err(|_| WriteMemoryError)?,
                u64::MIN,
                chunk,
            )?;
            Self::call_helper(self.executor.get_mut(), copier.store, [at, length_value])
                .map_err(|_| WriteMemoryError)?;
            position += length;
        }
        Ok(())
    }

    type SharedMemory = SharedMemory;

    fn to_shared_memory(&self, memory: &MemoryAddress) -> Option<SharedMemory> {
        if memory.direct_reads {
            return None;
        }
        match self.owner(&memory.export) {
            Owner::Shared(owner) => Some(SharedMemory {
                owner: Arc::clone(owner),
                name: Arc::from(&*memory.export.name),
            }),
            Owner::Instance(..) | Owner::Global(_) | Owner::Function(_) => None,
        }
    }

    fn add_shared_memory(&mut self, memory: &SharedMemory) -> MemoryAddress {
        let added = self.owners.iter().position(
            |owner| matches!(owner, Owner::Shared(found) if Arc::ptr_eq(found, &memory.owner)),
        );
        let owner =
            added.unwrap_or_else(|| self.add_owner(Owner::Shared(Arc::clone(&memory.owner))));
        let Some(EntityType::Memory(memory_type)) = memory.owner.description.export(&memory.name)
        else {
            panic!("a shared memory is a memory its owner exports");
        };
        let export = self.export(owner, Rc::from(&*memory.name));
        self.memory_address(export, memory_type)
            .expect("WasmEdge links a copier to any shared memory")
    }
}

impl<HostContext: 'static> WasmEdgeEngine<HostContext> {
    /// The pages the memory holds.
    fn pages(&self, export: &Export) -> Option<u64> {
        readable!(
            self.owner(export),
            instance => instance.get_memory_ref(&*export.name).ok().map(|memory| u64::from(memory.size())),
            None
        )
    }

    /// The bytes one copy through the copier's scratch memory moves.
    fn scratch_size(&self, copier: &Copier) -> Option<usize> {
        let Owner::Instance(helper, _) = &self.owners[copier.instance] else {
            return None;
        };
        let scratch = helper.get_memory_ref(COPIED_BYTES).ok()?;
        usize::try_from(memory_size(&scratch)).ok()
    }
}

impl<HostContext: 'static> TableStore for WasmEdgeEngine<HostContext> {
    type TableAddress = TableAddress;

    type AllocateTableError = AllocateError;
    fn allocate_table(&mut self, table_type: TableType) -> Result<TableAddress, AllocateError> {
        let table64 = table_type.address_type == AddressType::I64;
        let Limits { minimum, maximum } = table_type.limits;
        let most = if table64 {
            u64::MAX
        } else {
            u64::from(u32::MAX)
        };
        if minimum > most || maximum.is_some_and(|maximum| maximum > most) {
            return Err(AllocateError::LimitsOutsideAddressType);
        }
        let declared = wasm_encoder::TableType {
            element_type: wasm_encoder::RefType::FUNCREF,
            table64,
            minimum,
            maximum,
            shared: false,
        };
        let owner = self.define(EntityType::Table(declared))?;
        let export = self.export(owner, DEFINED_TABLE.into());
        self.table_address(export, declared, &Description::default())
            .ok_or(AllocateError::HelperRefused)
    }

    fn count_table_elements(&self, table: &TableAddress) -> u64 {
        let export = &table.export;
        readable!(
            self.owner(export),
            instance => instance.get_table(&*export.name).ok().map(|table| u64::try_from(table.capacity()).unwrap_or(u64::MAX)),
            None
        )
        .expect("a table address refers to a table")
    }

    fn grow_table(&mut self, table: &TableAddress, elements: u64) -> Result<(), GrowTableError> {
        self.check(table.export.engine);
        let writer = table.writer.ok_or(GrowTableError)?;
        let grow = writer.grow.ok_or(GrowTableError)?;
        self.slow_paths.took(SlowPath::TableWrittenByModule);
        let elements = if writer.wide {
            WasmValue::from_i64(elements.cast_signed())
        } else {
            WasmValue::from_i32(
                u32::try_from(elements)
                    .map_err(|_| GrowTableError)?
                    .cast_signed(),
            )
        };
        let grown = Self::call_helper(self.executor.get_mut(), grow, [elements])
            .map_err(|_| GrowTableError)?;
        let failed = grown.first().is_none_or(|previous| {
            if writer.wide {
                previous.to_i64() == -1
            } else {
                previous.to_i32() == -1
            }
        });
        if failed { Err(GrowTableError) } else { Ok(()) }
    }

    fn read_table(
        &self,
        table: &TableAddress,
        index: u64,
    ) -> Result<Option<FunctionAddress>, ReadTableError> {
        let export = &table.export;
        let element = readable!(
            self.owner(export),
            instance => instance.get_table(&*export.name).ok().map(|table| {
                let index = u32::try_from(index).ok().filter(|index| usize::try_from(*index).is_ok_and(|index| index < table.capacity()));
                index.map(|index| table.get_data(index))
            }),
            None
        )
        .flatten()
        .ok_or(ReadTableError::IndexOutsideTable)?
        .map_err(|_| ReadTableError::IndexOutsideTable)?;
        // Only a table whose elements refer to functions has a writer.
        if table.writer.is_none() {
            return Err(ReadTableError::ElementNotFunction);
        }
        Ok((!element.is_null_ref())
            .then(|| self.function_address(self.passable(element, table.holder), None)))
    }

    fn write_table(
        &mut self,
        table: &TableAddress,
        index: u64,
        function: Option<&FunctionAddress>,
    ) -> Result<(), WriteTableError> {
        self.check(table.export.engine);
        let outside = |engine: &Self| index >= engine.count_table_elements(table);
        let Some(writer) = table.writer else {
            return Err(if outside(self) {
                WriteTableError::IndexOutsideTable
            } else {
                WriteTableError::ReferenceTypeMismatch
            });
        };
        let index_value = if writer.wide {
            WasmValue::from_i64(index.cast_signed())
        } else {
            let Ok(narrow) = u32::try_from(index) else {
                return Err(WriteTableError::IndexOutsideTable);
            };
            WasmValue::from_i32(narrow.cast_signed())
        };
        let reference = match function {
            None => self.null_function,
            Some(function) => self.typed_reference(function),
        };
        self.slow_paths.took(SlowPath::TableWrittenByModule);
        Self::call_helper(
            self.executor.get_mut(),
            writer.set,
            [index_value, reference],
        )
        .map(drop)
        .map_err(|_| {
            if outside(self) {
                WriteTableError::IndexOutsideTable
            } else {
                WriteTableError::ReferenceTypeMismatch
            }
        })
    }
}

impl<HostContext: 'static> WasmEdgeEngine<HostContext> {
    /// The value as WasmEdge holds it, a function reference carrying the function's own type.
    fn wasm_value(&mut self, value: Value<FunctionAddress>) -> WasmValue {
        match value {
            Value::I32(value) => WasmValue::from_i32(value),
            Value::I64(value) => WasmValue::from_i64(value),
            Value::F32(value) => WasmValue::from_f32(value),
            Value::F64(value) => WasmValue::from_f64(value),
            Value::V128(value) => WasmValue::from_v128(value.cast_signed()),
            Value::FunctionReference(None) => self.null_function,
            Value::FunctionReference(Some(function)) => self.typed_reference(&function),
        }
    }
}

fn value_type(value: &Value<FunctionAddress>) -> ValType {
    match value {
        Value::I32(_) => ValType::I32,
        Value::I64(_) => ValType::I64,
        Value::F32(_) => ValType::F32,
        Value::F64(_) => ValType::F64,
        Value::V128(_) => ValType::V128,
        Value::FunctionReference(_) => ValType::FuncRef,
    }
}

impl<HostContext: 'static> GlobalStore for WasmEdgeEngine<HostContext> {
    type GlobalAddress = GlobalAddress;

    type AllocateGlobalError = Error;
    fn allocate_global(
        &mut self,
        mutability: Mutability,
        value: Value<FunctionAddress>,
    ) -> Result<GlobalAddress, Error> {
        let slot = Slot::of_value(&value);
        let mutable = mutability == Mutability::Variable;
        let mutability = if mutable {
            wasmedge_sdk::Mutability::Var
        } else {
            wasmedge_sdk::Mutability::Const
        };
        let global_type = wasmedge_sdk::GlobalType::new(value_type(&value), mutability);
        let global = wasmedge_sys::Global::create(&global_type, self.wasm_value(value))?;
        let module = Self::host_module(|module| module.add_global(DEFINED_GLOBAL, global))?;
        let owner = self.add_owner(Owner::Global(module));
        Ok(GlobalAddress {
            export: self.export(owner, DEFINED_GLOBAL.into()),
            slot,
            mutable,
            setter: None,
            holder: None,
        })
    }

    fn read_global(
        &self,
        global: &GlobalAddress,
    ) -> Result<Value<FunctionAddress>, ReadGlobalError> {
        let export = &global.export;
        let value = readable!(
            self.owner(export),
            instance => instance.get_global(&*export.name).ok().map(|global| global.get_value()),
            None
        )
        .ok_or(ReadGlobalError)?;
        Ok(match global.slot {
            Slot::I32 => Value::I32(value.to_i32()),
            Slot::I64 => Value::I64(value.to_i64()),
            Slot::F32 => Value::F32(value.to_f32()),
            Slot::F64 => Value::F64(value.to_f64()),
            Slot::V128 => Value::V128(value.to_v128().cast_unsigned()),
            Slot::Function { .. } => Value::FunctionReference(
                (!value.is_null_ref())
                    .then(|| self.function_address(self.passable(value, global.holder), None)),
            ),
            Slot::Other => return Err(ReadGlobalError),
        })
    }

    fn write_global(
        &mut self,
        global: &GlobalAddress,
        value: Value<FunctionAddress>,
    ) -> Result<(), WriteGlobalError> {
        self.check(global.export.engine);
        if !global.mutable {
            return Err(WriteGlobalError::Constant);
        }
        if !global.slot.takes(&value) {
            return Err(WriteGlobalError::ValueTypeMismatch);
        }
        let value = self.wasm_value(value);
        if let Some(setter) = global.setter {
            self.slow_paths.took(SlowPath::GlobalSetByModule);
            // A trap here is a function of another type than the global's.
            return Self::call_helper(self.executor.get_mut(), setter, [value])
                .map(drop)
                .map_err(|_| WriteGlobalError::ValueTypeMismatch);
        }
        let export = &global.export;
        writable!(
            self.owner_mut(export),
            instance => instance.get_global_mut(&*export.name).ok().map(|mut global| {
                global.set_value(value).map_err(|_| WriteGlobalError::ValueTypeMismatch)
            }),
            None
        )
        .unwrap_or(Err(WriteGlobalError::ValueTypeMismatch))
    }
}
