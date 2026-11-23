//! The WebAssembly engine interface run by WasmEdge, every module compiled ahead of time by its LLVM compiler.

use core::cell::{Cell, RefCell};
use core::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use thiserror::Error;
use wasmedge_sdk::error::{CoreError, CoreExecutionError, InstanceError, WasmEdgeError};
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

/// What every engine value made from it shares: WasmEdge's configurations, and the helper modules compiled once.
#[derive(Clone)]
pub struct Config(Arc<Shared>);

struct Shared {
    /// For modules compiled ahead of time into native shared libraries and run as such.
    compiled: wasmedge_sys::Config,
    /// For the modules that only pass on imports, which hold no code.
    linking: wasmedge_sys::Config,
    helpers: OnceLock<Helpers>,
}

/// Modules that reach what the bindings' safe layer gives no direct access to.
struct Helpers {
    copier: Module,
    table_writer: Module,
}

impl Default for Config {
    /// Every proposal the translated modules use switched on.
    fn default() -> Self {
        Self(Arc::new(Shared {
            compiled: configuration(RunMode::Aot).expect("WasmEdge makes a configuration"),
            linking: configuration(RunMode::Interpreter).expect("WasmEdge makes a configuration"),
            helpers: OnceLock::new(),
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
    Ok(config)
}

impl Config {
    fn helpers(&self) -> &Helpers {
        self.0.helpers.get_or_init(|| Helpers {
            copier: compile(&self.0, &copier()).expect("the copier compiles"),
            table_writer: compile(&self.0, &table_writer()).expect("the table writer compiles"),
        })
    }
}

/// Compiles the module into a native shared library, loads it, and removes the file.
fn compile(shared: &Shared, bytes: &[u8]) -> Result<Module, Error> {
    static COMPILED: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "wcontainers-wasmedge-{}-{}.{}",
        std::process::id(),
        COMPILED.fetch_add(1, Ordering::Relaxed),
        std::env::consts::DLL_EXTENSION
    ));
    let compiled = Compiler::create(Some(&shared.compiled))
        .and_then(|compiler| compiler.compile_from_bytes(bytes, &path))
        .and_then(|()| Loader::create(Some(&shared.compiled))?.from_file(&path));
    // A loaded library stays mapped once its file is gone; a file a failed compilation left is not needed.
    let _ = std::fs::remove_file(&path);
    let module = compiled?;
    Validator::create(Some(&shared.compiled))?.validate(&module)?;
    Ok(Module(module))
}

/// A name no other module instance of the process has.
fn unique_name() -> String {
    static NAMED: AtomicU64 = AtomicU64::new(0);
    format!(
        "wcontainers-wasmedge-{}",
        NAMED.fetch_add(1, Ordering::Relaxed)
    )
}

/// The names helpers and host-made instances import and export under.
const COPIER: &str = "copier";
const TABLE_WRITER: &str = "table-writer";
const SHARED: &str = "shared";
const SCRATCH: &str = "scratch";
const TABLE: &str = "table";
const MEMORY: &str = "memory";
const GLOBAL: &str = "global";
const FUNCTION: &str = "function";
const LOAD: &str = "load";
const STORE: &str = "store";
const GROW: &str = "grow";
const SET: &str = "set";

/// The pages a 32-bit memory holds at most.
const MOST_PAGES: u64 = (1 << u32::BITS) / PAGE_SIZE;

/// A module that imports any shared 32-bit memory, defines an unshared scratch memory, and exports `load` and
/// `store`, which copy a length of bytes at an offset of the shared memory to and from the start of the scratch
/// memory, and `grow`, which grows the shared memory.
fn copier() -> Vec<u8> {
    use wasm_encoder::{
        CodeSection, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
        MemorySection, TypeSection, ValType,
    };
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32, ValType::I32], []);
    types.ty().function([ValType::I32], [ValType::I32]);
    let (copy_type, grow_type) = (0, 1);
    let mut imports = ImportSection::new();
    imports.import(
        COPIER,
        SHARED,
        wasm_encoder::MemoryType {
            minimum: u64::MIN,
            maximum: Some(MOST_PAGES),
            memory64: false,
            shared: true,
            page_size_log2: None,
        },
    );
    let (shared, scratch) = (0, 1);
    let mut functions = FunctionSection::new();
    functions
        .function(copy_type)
        .function(copy_type)
        .function(grow_type);
    let mut memories = MemorySection::new();
    memories.memory(wasm_encoder::MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    let mut exports = ExportSection::new();
    exports
        .export(LOAD, ExportKind::Func, 0)
        .export(STORE, ExportKind::Func, 1)
        .export(GROW, ExportKind::Func, 2)
        .export(SCRATCH, ExportKind::Memory, scratch);
    let (offset, length) = (0, 1);
    let mut load = Function::new([]);
    load.instructions()
        .i32_const(0)
        .local_get(offset)
        .local_get(length)
        .memory_copy(scratch, shared)
        .end();
    let mut store = Function::new([]);
    store
        .instructions()
        .local_get(offset)
        .i32_const(0)
        .local_get(length)
        .memory_copy(shared, scratch)
        .end();
    let mut grow = Function::new([]);
    grow.instructions().local_get(0).memory_grow(shared).end();
    let mut code = CodeSection::new();
    code.function(&load).function(&store).function(&grow);
    let mut module = wasm_encoder::Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&memories)
        .section(&exports)
        .section(&code);
    module.finish()
}

/// A module that imports any 32-bit table of functions and exports `set`, which sets an element, and `grow`,
/// which grows it by null elements.
fn table_writer() -> Vec<u8> {
    use wasm_encoder::{
        CodeSection, ExportKind, ExportSection, Function, FunctionSection, HeapType, ImportSection,
        TypeSection, ValType,
    };
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32, ValType::FUNCREF], []);
    types.ty().function([ValType::I32], [ValType::I32]);
    let mut imports = ImportSection::new();
    imports.import(
        TABLE_WRITER,
        TABLE,
        wasm_encoder::TableType {
            element_type: wasm_encoder::RefType::FUNCREF,
            table64: false,
            minimum: u64::MIN,
            maximum: None,
            shared: false,
        },
    );
    let mut functions = FunctionSection::new();
    functions.function(0).function(1);
    let mut exports = ExportSection::new();
    exports
        .export(SET, ExportKind::Func, 0)
        .export(GROW, ExportKind::Func, 1);
    let table = 0;
    let mut set = Function::new([]);
    set.instructions()
        .local_get(0)
        .local_get(1)
        .table_set(table)
        .end();
    let mut grow = Function::new([]);
    grow.instructions()
        .ref_null(HeapType::FUNC)
        .local_get(0)
        .table_grow(table)
        .end();
    let mut code = CodeSection::new();
    code.function(&set).function(&grow);
    let mut module = wasm_encoder::Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&exports)
        .section(&code);
    module.finish()
}

/// A compiled module, loaded as native code.
#[derive(Clone)]
pub struct Module(Arc<wasmedge_sys::Module>);

/// Something that owns instances an address refers to.
enum Owner {
    Instance(wasmedge_sys::Instance),
    /// A memory, table or global the host made.
    Host(ImportModule<()>),
    Function(Box<dyn AsInstance>),
    /// A shared memory, or an instance that exports one, which other engine values may hold too.
    Shared(Arc<SharedOwner>),
}

enum SharedOwner {
    Host(ImportModule<()>),
    Instance(wasmedge_sys::Instance),
}

impl Owner {
    fn as_instance(&self) -> &dyn AsInstance {
        match self {
            Owner::Instance(instance) => instance,
            Owner::Host(module) => module,
            Owner::Function(module) => &**module,
            Owner::Shared(shared) => match &**shared {
                SharedOwner::Host(module) => module,
                SharedOwner::Instance(instance) => instance,
            },
        }
    }
}

/// Evaluates `$body` with `$instance` bound to the owner as its own type, or gives `$otherwise`.
macro_rules! readable {
    ($owner:expr, $instance:ident => $body:expr, $otherwise:expr) => {
        match $owner {
            Owner::Instance($instance) => $body,
            Owner::Host($instance) => $body,
            Owner::Shared(shared) => match &**shared {
                SharedOwner::Host($instance) => $body,
                SharedOwner::Instance($instance) => $body,
            },
            Owner::Function(_) => $otherwise,
        }
    };
}

/// Evaluates `$body` with `$instance` bound mutably to the owner as its own type.
macro_rules! writable {
    ($owner:expr, $instance:ident => $body:expr, $otherwise:expr) => {
        match $owner {
            Owner::Instance($instance) => $body,
            Owner::Host($instance) => $body,
            Owner::Shared(_) => panic!(
                "WasmEdge's bindings write an instance another engine value holds only through its shared memory"
            ),
            Owner::Function(_) => $otherwise,
        }
    };
}

/// Instances kept alive after their engine value is gone, as long as an address refers to one of them.
struct Graveyard(RefCell<Vec<Owner>>);

/// One thread's WasmEdge executor and the instances it made, with the host context.
pub struct WasmEdgeEngine<HostContext: 'static> {
    config: Config,
    /// Borrowed at run time only by the methods the interface gives `&self` that call into a helper.
    executor: RefCell<Executor>,
    /// The host context, always here except while a call lends it to the host functions.
    context: Option<HostContext>,
    /// Where host functions find the host context during a call.
    lent: Rc<Cell<Option<HostContext>>>,
    owners: Vec<Owner>,
    copiers: Vec<(Export, Copier)>,
    table_writers: Vec<(Export, TableWriter)>,
    /// A null reference to a function, which the bindings make only by reading an empty table.
    null_function: WasmValue,
    graveyard: Rc<Graveyard>,
}

impl<HostContext: 'static> Drop for WasmEdgeEngine<HostContext> {
    fn drop(&mut self) {
        self.graveyard.0.borrow_mut().append(&mut self.owners);
    }
}

/// An instance's export, by its owner and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Export {
    owner: usize,
    name: Rc<str>,
}

/// A function: WasmEdge's reference to it, where it is exported when that is known, and what keeps it alive.
#[derive(Clone)]
pub struct FunctionAddress {
    reference: WasmValue,
    origin: Option<Export>,
    _graveyard: Rc<Graveyard>,
}

/// A memory, with the copier this engine value reads and writes it through when it is shared.
#[derive(Clone)]
pub struct MemoryAddress {
    export: Export,
    copier: Option<Copier>,
}

/// The instance through which one engine value reaches a shared memory's bytes.
#[derive(Clone, Copy)]
struct Copier {
    instance: usize,
    load: WasmValue,
    store: WasmValue,
    grow: WasmValue,
}

/// A table of functions, with the instance that writes and grows it, which a table of other references lacks.
#[derive(Clone)]
pub struct TableAddress {
    export: Export,
    writer: Option<TableWriter>,
}

#[derive(Clone, Copy)]
struct TableWriter {
    set: WasmValue,
    grow: WasmValue,
}

#[derive(Clone)]
pub struct GlobalAddress(Export);

/// The bindings give no access to tags, so no address of one exists.
#[derive(Clone, Copy, Debug)]
pub enum TagAddress {}

/// An instance of a module, made by one engine value.
pub struct Instance(usize);

/// A shared memory and what owns it, which any engine value can add.
#[derive(Clone)]
pub struct SharedMemory {
    owner: Arc<SharedOwner>,
    name: Arc<str>,
}

/// Why `instantiate` failed.
#[derive(Debug, Error)]
pub enum InstantiateError {
    #[error("no address was given for the import {module_name}.{item_name}")]
    MissingImport {
        module_name: String,
        item_name: String,
    },
    #[error("the imported function is exported by no instance, so WasmEdge cannot link it by name")]
    FunctionNotExported,
    #[error("the import's type holds a reference type WasmEdge's bindings cannot describe")]
    TypeOutsideBindings,
    #[error("WasmEdge refused the instantiation: {0}")]
    Engine(Error),
}

/// Why a memory or a table could not be allocated.
#[derive(Debug, Error)]
pub enum AllocateError {
    #[error("WasmEdge's bindings make only 32-bit memories and tables")]
    AddressTypeOutsideBindings,
    #[error("the limits do not fit the address type")]
    LimitsOutsideAddressType,
    #[error("WasmEdge refused it: {0}")]
    Engine(Error),
}

impl From<Error> for AllocateError {
    fn from(error: Error) -> Self {
        Self::Engine(error)
    }
}

impl From<Error> for InstantiateError {
    fn from(error: Error) -> Self {
        Self::Engine(error)
    }
}

/// An item an instantiation links, by the names the module imports it under.
struct Link<'name> {
    module_name: &'name str,
    item_name: &'name str,
    item: Item,
}

enum Item {
    Function(FunctionAddress),
    Memory(Export),
    Table(Export),
    Global(Export),
}

/// What a module that passes on an import imports.
struct Forwarded {
    owner: String,
    name: Rc<str>,
    item_name: String,
    kind: Forward,
}

enum Forward {
    Function(FuncType),
    Memory(wasmedge_sdk::MemoryType),
    Table(wasmedge_sdk::TableType),
    Global(wasmedge_sdk::GlobalType),
}

fn encoder_value_type(value_type: ValType) -> Result<wasm_encoder::ValType, InstantiateError> {
    Ok(match value_type {
        ValType::I32 => wasm_encoder::ValType::I32,
        ValType::I64 => wasm_encoder::ValType::I64,
        ValType::F32 => wasm_encoder::ValType::F32,
        ValType::F64 => wasm_encoder::ValType::F64,
        ValType::V128 => wasm_encoder::ValType::V128,
        ValType::FuncRef => wasm_encoder::ValType::FUNCREF,
        ValType::ExternRef => wasm_encoder::ValType::EXTERNREF,
        ValType::UnsupportedRef => return Err(InstantiateError::TypeOutsideBindings),
    })
}

fn encoder_reference_type(reference_type: RefType) -> wasm_encoder::RefType {
    match reference_type {
        RefType::FuncRef => wasm_encoder::RefType::FUNCREF,
        RefType::ExternRef => wasm_encoder::RefType::EXTERNREF,
    }
}

/// A module that imports each item from the instance that exports it and exports it under the name another
/// module imports it under.
fn forwarder<'item>(
    items: impl Iterator<Item = &'item Forwarded>,
) -> Result<Vec<u8>, InstantiateError> {
    use wasm_encoder::{EntityType, ExportKind, ExportSection, ImportSection, TypeSection};
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut exports = ExportSection::new();
    let (mut functions, mut memories, mut tables, mut globals) = (0, 0, 0, 0);
    for item in items {
        let (entity, kind, index) = match &item.kind {
            Forward::Function(function_type) => {
                let parameters = function_type
                    .args()
                    .iter()
                    .map(|value_type| encoder_value_type(*value_type))
                    .collect::<Result<Vec<_>, _>>()?;
                let results = function_type
                    .returns()
                    .iter()
                    .map(|value_type| encoder_value_type(*value_type))
                    .collect::<Result<Vec<_>, _>>()?;
                let type_index = types.len();
                types.ty().function(parameters, results);
                functions += 1;
                (
                    EntityType::Function(type_index),
                    ExportKind::Func,
                    functions - 1,
                )
            }
            Forward::Memory(memory_type) => {
                memories += 1;
                let memory = wasm_encoder::MemoryType {
                    minimum: u64::MIN,
                    maximum: memory_type.maximum().map(u64::from),
                    memory64: false,
                    shared: memory_type.shared(),
                    page_size_log2: None,
                };
                (EntityType::Memory(memory), ExportKind::Memory, memories - 1)
            }
            Forward::Table(table_type) => {
                tables += 1;
                let table = wasm_encoder::TableType {
                    element_type: encoder_reference_type(table_type.elem_ty()),
                    table64: false,
                    minimum: u64::MIN,
                    maximum: table_type.maximum().map(u64::from),
                    shared: false,
                };
                (EntityType::Table(table), ExportKind::Table, tables - 1)
            }
            Forward::Global(global_type) => {
                globals += 1;
                let global = wasm_encoder::GlobalType {
                    val_type: encoder_value_type(global_type.value_ty())?,
                    mutable: global_type.mutability() == wasmedge_sdk::Mutability::Var,
                    shared: false,
                };
                (EntityType::Global(global), ExportKind::Global, globals - 1)
            }
        };
        imports.import(&item.owner, &item.name, entity);
        exports.export(&item.item_name, kind, index);
    }
    let mut module = wasm_encoder::Module::new();
    module.section(&types).section(&imports).section(&exports);
    Ok(module.finish())
}

/// Whether `length` bytes at `offset` lie inside `size` bytes.
fn inside(size: u64, offset: u64, length: usize) -> bool {
    u64::try_from(length)
        .ok()
        .and_then(|length| offset.checked_add(length))
        .is_some_and(|end| end <= size)
}

/// The size of the memory in bytes.
fn memory_size(memory: &wasmedge_sys::Memory) -> u64 {
    u64::from(memory.size()) * PAGE_SIZE
}

/// Copies the bytes at the offset of an unshared memory into the given bytes.
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
    // The bindings take 32-bit offsets and lengths.
    reachable(offset, into.len());
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
    reachable(offset, bytes.len());
    let offset = u32::try_from(offset).map_err(|_| WriteMemoryError)?;
    memory.set_data(bytes, offset).map_err(|_| WriteMemoryError)
}

/// Stops when bytes that lie inside a memory end beyond the first 4 GiB, which the bindings cannot reach.
fn reachable(offset: u64, length: usize) {
    let end = u64::try_from(length)
        .ok()
        .and_then(|length| offset.checked_add(length));
    assert!(
        end.is_some_and(|end| end <= 1 << u32::BITS),
        "WasmEdge's bindings reach only the first 4 GiB of a memory"
    );
}

impl<HostContext: 'static> WasmEdgeEngine<HostContext> {
    fn add_owner(&mut self, owner: Owner) -> usize {
        self.owners.push(owner);
        self.owners.len() - 1
    }

    fn function_address(&self, reference: WasmValue, origin: Option<Export>) -> FunctionAddress {
        FunctionAddress {
            reference,
            origin,
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
                (WasmValue::from_func_ref(&*exported).as_raw().Value == wanted).then(|| Export {
                    owner,
                    name: name.into(),
                })
            })
        })
    }

    /// What a module that passes the item on imports, and the owner it imports it from.
    fn forwarded(&self, link: &Link<'_>) -> Result<(usize, Forwarded), InstantiateError> {
        let (export, kind) = match &link.item {
            Item::Function(function) => {
                let export = self
                    .origin(function)
                    .ok_or(InstantiateError::FunctionNotExported)?;
                let function_type = function
                    .reference
                    .func_ref()
                    .and_then(|function| function.ty())
                    .ok_or(InstantiateError::FunctionNotExported)?;
                (export, Forward::Function(function_type))
            }
            Item::Memory(export) => {
                let memory_type = readable!(
                    &self.owners[export.owner],
                    instance => instance.get_memory_ref(&*export.name).and_then(|memory| memory.ty()),
                    Err(not_found(InstanceError::NotFoundMem(export.name.to_string())))
                )?;
                (export.clone(), Forward::Memory(memory_type))
            }
            Item::Table(export) => {
                let table_type = readable!(
                    &self.owners[export.owner],
                    instance => instance.get_table(&*export.name).and_then(|table| table.ty()),
                    Err(not_found(InstanceError::NotFoundTable(export.name.to_string())))
                )?;
                (export.clone(), Forward::Table(table_type))
            }
            Item::Global(export) => {
                let global_type = readable!(
                    &self.owners[export.owner],
                    instance => instance.get_global(&*export.name).and_then(|global| global.ty()),
                    Err(not_found(InstanceError::NotFoundGlobal(export.name.to_string())))
                )?;
                (export.clone(), Forward::Global(global_type))
            }
        };
        let forwarded = Forwarded {
            owner: self.owners[export.owner]
                .as_instance()
                .name()
                .unwrap_or_default(),
            name: export.name,
            item_name: link.item_name.to_owned(),
            kind,
        };
        Ok((export.owner, forwarded))
    }

    /// Instantiates the module in a store of its own, which holds the owners of what it imports and, under each
    /// name it imports from, a module that passes those imports on.
    fn link(
        &mut self,
        module: &wasmedge_sys::Module,
        links: &[Link<'_>],
    ) -> Result<usize, InstantiateError> {
        let mut store = Store::create()?;
        let mut registered = Vec::new();
        let mut module_names: Vec<&str> = Vec::new();
        let mut forwarded = Vec::with_capacity(links.len());
        for link in links {
            let (owner, item) = self.forwarded(link)?;
            if !registered.contains(&owner) {
                self.executor
                    .get_mut()
                    .register_import_module(&mut store, self.owners[owner].as_instance())?;
                registered.push(owner);
            }
            if !module_names.contains(&link.module_name) {
                module_names.push(link.module_name);
            }
            forwarded.push((link.module_name, item));
        }
        let loader = Loader::create(Some(&self.config.0.linking))?;
        let validator = Validator::create(Some(&self.config.0.linking))?;
        for module_name in module_names {
            let items = forwarded
                .iter()
                .filter(|(name, _)| *name == module_name)
                .map(|(_, item)| item);
            let forwarding = loader.from_bytes(forwarder(items)?)?;
            validator.validate(&forwarding)?;
            let instance = self.executor.get_mut().register_named_module(
                &mut store,
                &forwarding,
                module_name,
            )?;
            self.add_owner(Owner::Instance(instance));
        }
        let instance = self
            .lend(|executor| executor.register_named_module(&mut store, module, unique_name()))?;
        let shares = instance.mem_names().is_some_and(|names| {
            names.iter().any(|name| {
                instance
                    .get_memory_ref(name)
                    .and_then(|memory| memory.ty())
                    .is_ok_and(|memory_type| memory_type.shared())
            })
        });
        Ok(self.add_owner(if shares {
            Owner::Shared(Arc::new(SharedOwner::Instance(instance)))
        } else {
            Owner::Instance(instance)
        }))
    }

    /// The function the instance exports under the name.
    fn exported_function(&self, owner: usize, name: &str) -> WasmValue {
        let function = self.owners[owner]
            .as_instance()
            .get_func(name)
            .expect("the helper exports the function");
        WasmValue::from_func_ref(&*function)
    }

    /// The copier of the shared memory, made the first time it is asked for.
    fn copier(&mut self, export: &Export) -> Copier {
        if let Some((_, copier)) = self.copiers.iter().find(|(found, _)| found == export) {
            return *copier;
        }
        let config = self.config.clone();
        let link = Link {
            module_name: COPIER,
            item_name: SHARED,
            item: Item::Memory(export.clone()),
        };
        let instance = self
            .link(&config.helpers().copier.0, &[link])
            .expect("a shared 32-bit memory links into the copier");
        let copier = Copier {
            instance,
            load: self.exported_function(instance, LOAD),
            store: self.exported_function(instance, STORE),
            grow: self.exported_function(instance, GROW),
        };
        self.copiers.push((export.clone(), copier));
        copier
    }

    /// The writer of the table, made the first time it is asked for, or nothing when it holds no functions.
    fn table_writer(&mut self, export: &Export) -> Option<TableWriter> {
        if let Some((_, writer)) = self.table_writers.iter().find(|(found, _)| found == export) {
            return Some(*writer);
        }
        let config = self.config.clone();
        let link = Link {
            module_name: TABLE_WRITER,
            item_name: TABLE,
            item: Item::Table(export.clone()),
        };
        let instance = self.link(&config.helpers().table_writer.0, &[link]).ok()?;
        let writer = TableWriter {
            set: self.exported_function(instance, SET),
            grow: self.exported_function(instance, GROW),
        };
        self.table_writers.push((export.clone(), writer));
        Some(writer)
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
            .with_func_by_type(FUNCTION, function_type, function)
            .expect("WasmEdge makes the host function");
        let module = builder.build();
        let reference = WasmValue::from_func_ref(
            &*module
                .get_func(FUNCTION)
                .expect("the host function's module holds it"),
        );
        let owner = self.add_owner(Owner::Function(Box::new(module)));
        self.function_address(
            reference,
            Some(Export {
                owner,
                name: FUNCTION.into(),
            }),
        )
    }

    /// Calls a helper's function with the values and returns what it returns.
    fn call_helper(
        executor: &mut Executor,
        function: WasmValue,
        values: impl IntoIterator<Item = WasmValue>,
    ) -> Result<Vec<WasmValue>, Error> {
        let function = function
            .func_ref()
            .expect("a helper's function is not null");
        executor.call_func_ref(&mut Callee(&function), values)
    }
}

fn not_found(error: InstanceError) -> Error {
    Box::new(WasmEdgeError::Instance(error))
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
            config,
            executor: RefCell::new(executor),
            context: Some(host_context),
            lent: Rc::default(),
            owners: Vec::new(),
            copiers: Vec::new(),
            table_writers: Vec::new(),
            null_function,
            graveyard: Rc::new(Graveyard(RefCell::default())),
        })
    }

    type CompileError = Error;
    fn compile(&self, bytes: &[u8]) -> Result<Module, Error> {
        compile(&self.config.0, bytes)
    }

    type InstantiateError = InstantiateError;
    fn instantiate(
        &mut self,
        module: &Module,
        imports: &[Import<'_, Self>],
    ) -> Result<Instance, InstantiateError> {
        let links = module
            .0
            .imports()
            .iter()
            .map(|wanted| {
                let (module_name, item_name) = (wanted.module_name(), wanted.name());
                let given = imports
                    .iter()
                    .find(|given| given.module_name == module_name && given.item_name == item_name)
                    .ok_or_else(|| InstantiateError::MissingImport {
                        module_name: module_name.to_string(),
                        item_name: item_name.to_string(),
                    })?;
                let item = match &given.address {
                    ExternalAddress::Function(function) => Item::Function(function.clone()),
                    ExternalAddress::Memory(memory) => Item::Memory(memory.export.clone()),
                    ExternalAddress::Table(table) => Item::Table(table.export.clone()),
                    ExternalAddress::Global(global) => Item::Global(global.0.clone()),
                    ExternalAddress::Tag(tag) => match *tag {},
                };
                Ok(Link {
                    module_name: given.module_name,
                    item_name: given.item_name,
                    item,
                })
            })
            .collect::<Result<Vec<_>, InstantiateError>>()?;
        self.link(&module.0, &links).map(Instance)
    }

    fn find_export(&mut self, instance: &Instance, name: &str) -> Option<ExternalAddress<Self>> {
        let owner = instance.0;
        let export = Export {
            owner,
            name: name.into(),
        };
        let found = &self.owners[owner];
        if let Ok(function) = found.as_instance().get_func(name) {
            let reference = WasmValue::from_func_ref(&*function);
            return Some(ExternalAddress::Function(
                self.function_address(reference, Some(export)),
            ));
        }
        let memory_sharing = readable!(
            found,
            instance => instance.get_memory_ref(name).ok().map(|memory| memory.ty().is_ok_and(|memory_type| memory_type.shared())),
            None
        );
        if let Some(shared) = memory_sharing {
            let copier = shared.then(|| self.copier(&export));
            return Some(ExternalAddress::Memory(MemoryAddress { export, copier }));
        }
        let table = readable!(found, instance => instance.get_table(name).is_ok(), false);
        if table {
            let writer = self.table_writer(&export);
            return Some(ExternalAddress::Table(TableAddress { export, writer }));
        }
        let global = readable!(found, instance => instance.get_global(name).is_ok(), false);
        global.then(|| ExternalAddress::Global(GlobalAddress(export)))
    }

    fn host_context(&mut self) -> &mut HostContext {
        self.context
            .as_mut()
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
    context: Rc<Cell<Option<HostContext>>>,
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
                    let results = (data.function)(&mut context, parameters);
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
                let callee = self
                    .address
                    .reference
                    .func_ref()
                    .expect("a bound function is not null");
                let ($($argument,)*) = parameters;
                let returned = engine
                    .lend(|executor| {
                        executor.call_func_ref(
                            &mut Callee(&callee),
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
        if memory_type.address_type == AddressType::I64 {
            return Err(AllocateError::AddressTypeOutsideBindings);
        }
        let Limits { minimum, maximum } = memory_type.limits;
        let narrow = |limit: u64| {
            u32::try_from(limit)
                .ok()
                .filter(|_| limit <= MOST_PAGES)
                .ok_or(AllocateError::LimitsOutsideAddressType)
        };
        let shared = memory_type.sharing == Sharing::Shared;
        let memory_type_of_wasmedge = wasmedge_sdk::MemoryType::new(
            narrow(minimum)?,
            maximum.map(narrow).transpose()?,
            shared,
        )?;
        let memory = wasmedge_sys::Memory::create(&memory_type_of_wasmedge)?;
        let module = Self::host_module(|module| module.add_memory(MEMORY, memory))?;
        if shared {
            let owner = self.add_owner(Owner::Shared(Arc::new(SharedOwner::Host(module))));
            let export = Export {
                owner,
                name: MEMORY.into(),
            };
            let copier = Some(self.copier(&export));
            Ok(MemoryAddress { export, copier })
        } else {
            let owner = self.add_owner(Owner::Host(module));
            Ok(MemoryAddress {
                export: Export {
                    owner,
                    name: MEMORY.into(),
                },
                copier: None,
            })
        }
    }

    fn count_memory_pages(&self, memory: &MemoryAddress) -> u64 {
        let export = &memory.export;
        readable!(
            &self.owners[export.owner],
            instance => instance.get_memory_ref(&*export.name).ok().map(|memory| u64::from(memory.size())),
            None
        )
        .expect("a memory address refers to a memory")
    }

    fn grow_memory(&mut self, memory: &MemoryAddress, pages: u64) -> Result<(), GrowMemoryError> {
        let pages = u32::try_from(pages).map_err(|_| GrowMemoryError)?;
        if let Some(copier) = &memory.copier {
            let grown = Self::call_helper(
                self.executor.get_mut(),
                copier.grow,
                [WasmValue::from_i32(pages.cast_signed())],
            )
            .map_err(|_| GrowMemoryError)?;
            return match grown.first().map(WasmValue::to_i32) {
                Some(previous) if previous >= 0 => Ok(()),
                _ => Err(GrowMemoryError),
            };
        }
        let export = &memory.export;
        writable!(
            &mut self.owners[export.owner],
            instance => instance.get_memory_mut(&*export.name).ok().map(|mut memory| memory.grow(pages)),
            None
        )
        .ok_or(GrowMemoryError)?
        .map_err(|_| GrowMemoryError)
    }

    fn read_memory(
        &self,
        memory: &MemoryAddress,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), ReadMemoryError> {
        let export = &memory.export;
        let Some(copier) = &memory.copier else {
            return readable!(
                &self.owners[export.owner],
                instance => instance.get_memory_ref(&*export.name).ok().map(|memory| read_unshared(&memory, offset, into)),
                None
            )
            .unwrap_or(Err(ReadMemoryError));
        };
        let size = readable!(
            &self.owners[export.owner],
            instance => instance.get_memory_ref(&*export.name).ok().map(|memory| memory_size(&memory)),
            None
        )
        .ok_or(ReadMemoryError)?;
        if !inside(size, offset, into.len()) {
            return Err(ReadMemoryError);
        }
        let scratch = self.scratch_size(copier).ok_or(ReadMemoryError)?;
        let mut position = u32::try_from(offset).map_err(|_| ReadMemoryError)?;
        let mut executor = self.executor.borrow_mut();
        for chunk in into.chunks_mut(scratch) {
            let length = u32::try_from(chunk.len()).map_err(|_| ReadMemoryError)?;
            Self::call_helper(
                &mut executor,
                copier.load,
                [
                    WasmValue::from_i32(position.cast_signed()),
                    WasmValue::from_i32(length.cast_signed()),
                ],
            )
            .map_err(|_| ReadMemoryError)?;
            let Owner::Instance(helper) = &self.owners[copier.instance] else {
                return Err(ReadMemoryError);
            };
            let scratch_memory = helper
                .get_memory_ref(SCRATCH)
                .map_err(|_| ReadMemoryError)?;
            read_unshared(&scratch_memory, u64::MIN, chunk)?;
            position = position.wrapping_add(length);
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
        let Some(copier) = &memory.copier else {
            return writable!(
                &mut self.owners[export.owner],
                instance => instance.get_memory_mut(&*export.name).ok().map(|mut memory| write_unshared(&mut memory, offset, bytes)),
                None
            )
            .unwrap_or(Err(WriteMemoryError));
        };
        let size = readable!(
            &self.owners[export.owner],
            instance => instance.get_memory_ref(&*export.name).ok().map(|memory| memory_size(&memory)),
            None
        )
        .ok_or(WriteMemoryError)?;
        if !inside(size, offset, bytes.len()) {
            return Err(WriteMemoryError);
        }
        let scratch = self.scratch_size(copier).ok_or(WriteMemoryError)?;
        let mut position = u32::try_from(offset).map_err(|_| WriteMemoryError)?;
        for chunk in bytes.chunks(scratch) {
            let length = u32::try_from(chunk.len()).map_err(|_| WriteMemoryError)?;
            let Owner::Instance(helper) = &mut self.owners[copier.instance] else {
                return Err(WriteMemoryError);
            };
            write_unshared(
                &mut *helper
                    .get_memory_mut(SCRATCH)
                    .map_err(|_| WriteMemoryError)?,
                u64::MIN,
                chunk,
            )?;
            Self::call_helper(
                self.executor.get_mut(),
                copier.store,
                [
                    WasmValue::from_i32(position.cast_signed()),
                    WasmValue::from_i32(length.cast_signed()),
                ],
            )
            .map_err(|_| WriteMemoryError)?;
            position = position.wrapping_add(length);
        }
        Ok(())
    }

    type SharedMemory = SharedMemory;

    fn to_shared_memory(&self, memory: &MemoryAddress) -> Option<SharedMemory> {
        memory.copier?;
        match &self.owners[memory.export.owner] {
            Owner::Shared(owner) => Some(SharedMemory {
                owner: Arc::clone(owner),
                name: Arc::from(&*memory.export.name),
            }),
            _ => None,
        }
    }

    fn add_shared_memory(&mut self, memory: &SharedMemory) -> MemoryAddress {
        let added = self.owners.iter().position(
            |owner| matches!(owner, Owner::Shared(found) if Arc::ptr_eq(found, &memory.owner)),
        );
        let owner =
            added.unwrap_or_else(|| self.add_owner(Owner::Shared(Arc::clone(&memory.owner))));
        let export = Export {
            owner,
            name: Rc::from(&*memory.name),
        };
        let copier = Some(self.copier(&export));
        MemoryAddress { export, copier }
    }
}

impl<HostContext: 'static> WasmEdgeEngine<HostContext> {
    /// The bytes one copy through the copier's scratch memory moves.
    fn scratch_size(&self, copier: &Copier) -> Option<usize> {
        let Owner::Instance(helper) = &self.owners[copier.instance] else {
            return None;
        };
        let scratch = helper.get_memory_ref(SCRATCH).ok()?;
        usize::try_from(memory_size(&scratch)).ok()
    }
}

impl<HostContext: 'static> TableStore for WasmEdgeEngine<HostContext> {
    type TableAddress = TableAddress;

    type AllocateTableError = AllocateError;
    fn allocate_table(&mut self, table_type: TableType) -> Result<TableAddress, AllocateError> {
        if table_type.address_type == AddressType::I64 {
            return Err(AllocateError::AddressTypeOutsideBindings);
        }
        let Limits { minimum, maximum } = table_type.limits;
        let narrow =
            |limit: u64| u32::try_from(limit).map_err(|_| AllocateError::LimitsOutsideAddressType);
        let table = wasmedge_sys::Table::create(wasmedge_sdk::TableType::new(
            RefType::FuncRef,
            narrow(minimum)?,
            maximum.map(narrow).transpose()?,
        ))?;
        let module = Self::host_module(|module| module.add_table(TABLE, table))?;
        let owner = self.add_owner(Owner::Host(module));
        let export = Export {
            owner,
            name: TABLE.into(),
        };
        let writer = self.table_writer(&export);
        Ok(TableAddress { export, writer })
    }

    fn count_table_elements(&self, table: &TableAddress) -> u64 {
        let export = &table.export;
        readable!(
            &self.owners[export.owner],
            instance => instance.get_table(&*export.name).ok().map(|table| u64::try_from(table.capacity()).unwrap_or(u64::MAX)),
            None
        )
        .expect("a table address refers to a table")
    }

    fn grow_table(&mut self, table: &TableAddress, elements: u64) -> Result<(), GrowTableError> {
        let writer = table.writer.ok_or(GrowTableError)?;
        let elements = u32::try_from(elements).map_err(|_| GrowTableError)?;
        let grown = Self::call_helper(
            self.executor.get_mut(),
            writer.grow,
            [WasmValue::from_i32(elements.cast_signed())],
        )
        .map_err(|_| GrowTableError)?;
        match grown.first().map(WasmValue::to_i32) {
            Some(previous) if previous >= 0 => Ok(()),
            _ => Err(GrowTableError),
        }
    }

    fn read_table(
        &self,
        table: &TableAddress,
        index: u64,
    ) -> Result<Option<FunctionAddress>, ReadTableError> {
        let export = &table.export;
        let element = readable!(
            &self.owners[export.owner],
            instance => instance.get_table(&*export.name).ok().map(|table| {
                let index = u32::try_from(index).ok().filter(|index| usize::try_from(*index).is_ok_and(|index| index < table.capacity()));
                index.map(|index| table.get_data(index))
            }),
            None
        )
        .flatten()
        .ok_or(ReadTableError::IndexOutsideTable)?
        .map_err(|_| ReadTableError::IndexOutsideTable)?;
        if element.ty() != ValType::FuncRef {
            return Err(ReadTableError::ElementNotFunction);
        }
        Ok((!element.is_null_ref()).then(|| self.function_address(element, None)))
    }

    fn write_table(
        &mut self,
        table: &TableAddress,
        index: u64,
        function: Option<&FunctionAddress>,
    ) -> Result<(), WriteTableError> {
        let outside = |engine: &Self| index >= engine.count_table_elements(table);
        let Some(writer) = table.writer else {
            return Err(if outside(self) {
                WriteTableError::IndexOutsideTable
            } else {
                WriteTableError::ReferenceTypeMismatch
            });
        };
        let Ok(narrow) = u32::try_from(index) else {
            return Err(WriteTableError::IndexOutsideTable);
        };
        let reference = function.map_or(self.null_function, |function| function.reference);
        Self::call_helper(
            self.executor.get_mut(),
            writer.set,
            [WasmValue::from_i32(narrow.cast_signed()), reference],
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
    fn to_wasm_value(&self, value: Value<FunctionAddress>) -> WasmValue {
        match value {
            Value::I32(value) => WasmValue::from_i32(value),
            Value::I64(value) => WasmValue::from_i64(value),
            Value::F32(value) => WasmValue::from_f32(value),
            Value::F64(value) => WasmValue::from_f64(value),
            Value::V128(value) => WasmValue::from_v128(value.cast_signed()),
            Value::FunctionReference(function) => {
                function.map_or(self.null_function, |function| function.reference)
            }
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
        let mutability = match mutability {
            Mutability::Constant => wasmedge_sdk::Mutability::Const,
            Mutability::Variable => wasmedge_sdk::Mutability::Var,
        };
        let global_type = wasmedge_sdk::GlobalType::new(value_type(&value), mutability);
        let global = wasmedge_sys::Global::create(&global_type, self.to_wasm_value(value))?;
        let module = Self::host_module(|module| module.add_global(GLOBAL, global))?;
        let owner = self.add_owner(Owner::Host(module));
        Ok(GlobalAddress(Export {
            owner,
            name: GLOBAL.into(),
        }))
    }

    fn read_global(
        &self,
        global: &GlobalAddress,
    ) -> Result<Value<FunctionAddress>, ReadGlobalError> {
        let export = &global.0;
        let value = readable!(
            &self.owners[export.owner],
            instance => instance.get_global(&*export.name).ok().map(|global| global.get_value()),
            None
        )
        .ok_or(ReadGlobalError)?;
        Ok(match value.ty() {
            ValType::I32 => Value::I32(value.to_i32()),
            ValType::I64 => Value::I64(value.to_i64()),
            ValType::F32 => Value::F32(value.to_f32()),
            ValType::F64 => Value::F64(value.to_f64()),
            ValType::V128 => Value::V128(value.to_v128().cast_unsigned()),
            ValType::FuncRef => Value::FunctionReference(
                (!value.is_null_ref()).then(|| self.function_address(value, None)),
            ),
            ValType::ExternRef | ValType::UnsupportedRef => return Err(ReadGlobalError),
        })
    }

    fn write_global(
        &mut self,
        global: &GlobalAddress,
        value: Value<FunctionAddress>,
    ) -> Result<(), WriteGlobalError> {
        let wanted = value_type(&value);
        let value = self.to_wasm_value(value);
        let export = &global.0;
        writable!(
            &mut self.owners[export.owner],
            instance => instance.get_global_mut(&*export.name).ok().map(|mut global| {
                let global_type = global.ty().map_err(|_| WriteGlobalError::ValueTypeMismatch)?;
                if global_type.mutability() == wasmedge_sdk::Mutability::Const {
                    return Err(WriteGlobalError::Constant);
                }
                if global_type.value_ty() != wanted {
                    return Err(WriteGlobalError::ValueTypeMismatch);
                }
                global.set_value(value).map_err(|_| WriteGlobalError::ValueTypeMismatch)
            }),
            None
        )
        .unwrap_or(Err(WriteGlobalError::ValueTypeMismatch))
    }
}
