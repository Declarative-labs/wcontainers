//! The WebAssembly engine interface run by wasmtime.

use core::cell::RefCell;
use core::convert::Infallible;
use std::sync::{Arc, OnceLock};

use thiserror::Error;
use wasmtime::{
    AsContextMut, Caller, Extern, Func, Global, GlobalType, Instance, Memory, MemoryTypeBuilder,
    Module, Ref, RefType, RootScope, SharedMemory, Store, Table, Tag, ThrownException, TypedFunc,
    Val, ValType, WasmParams, WasmResults, WasmRet, WasmTy,
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

/// What every engine value made from it shares: wasmtime's engine, and the modules compiled for it.
#[derive(Clone)]
pub struct Config {
    engine: wasmtime::Engine,
    copiers: Arc<OnceLock<Copiers>>,
}

impl Default for Config {
    /// The process's one wasmtime engine, with the proposals the translated modules use switched on, so
    /// that every default config can take another's shared memories and modules.
    fn default() -> Self {
        static DEFAULT: OnceLock<Config> = OnceLock::new();
        DEFAULT
            .get_or_init(|| {
                let mut config = wasmtime::Config::new();
                config
                    .wasm_tail_call(true)
                    .wasm_function_references(true)
                    .wasm_gc(true)
                    .wasm_exceptions(true)
                    .wasm_threads(true)
                    .wasm_memory64(true)
                    .wasm_multi_memory(true)
                    .shared_memory(true);
                Config {
                    engine: wasmtime::Engine::new(&config)
                        .expect("wasmtime supports these proposals on this host"),
                    copiers: Arc::default(),
                }
            })
            .clone()
    }
}

/// One thread's wasmtime store, whose data is the host context.
pub struct WasmtimeEngine<HostContext: 'static> {
    config: Config,
    /// Borrowed at run time only by the methods the interface gives `&self` but wasmtime needs `&mut` for.
    store: RefCell<Store<HostContext>>,
    shared_memories: Vec<SharedMemoryAddress>,
}

/// A memory of one engine value, unshared or shared.
#[derive(Clone)]
pub struct MemoryAddress(MemoryInstance);

#[derive(Clone)]
enum MemoryInstance {
    Unshared(Memory),
    Shared(SharedMemoryAddress),
}

/// A shared memory with the instance through which this engine value reads and writes its bytes.
#[derive(Clone)]
struct SharedMemoryAddress {
    memory: SharedMemory,
    /// The unshared memory the copier copies through.
    scratch: Memory,
    load: TypedFunc<(u64, u32), ()>,
    store: TypedFunc<(u64, u32), ()>,
}

/// The modules that copy between a shared memory and an unshared one, for each address type.
struct Copiers {
    narrow: Module,
    wide: Module,
}

/// The names the copier imports and exports under.
const COPIER_MODULE: &str = "copier";
const SHARED: &str = "shared";
const SCRATCH: &str = "scratch";
const LOAD: &str = "load";
const STORE: &str = "store";

impl Copiers {
    fn compile(engine: &wasmtime::Engine) -> Self {
        let compile = |address_type| {
            Module::from_binary(engine, &copier(address_type)).expect("the copier compiles")
        };
        Self {
            narrow: compile(AddressType::I32),
            wide: compile(AddressType::I64),
        }
    }
}

/// The pages a memory of the address type can hold at most.
fn most_pages(address_type: AddressType) -> u64 {
    match address_type {
        AddressType::I32 => (u64::from(u32::MAX) + 1) / PAGE_SIZE,
        AddressType::I64 => u64::MAX / PAGE_SIZE + 1,
    }
}

/// A module that imports any shared memory of the address type, defines an unshared scratch memory,
/// and exports `load` and `store`, which copy a length of bytes at an offset of the shared memory
/// to and from the start of the scratch memory.
fn copier(address_type: AddressType) -> Vec<u8> {
    use wasm_encoder::{
        CodeSection, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
        MemorySection, TypeSection,
    };
    let memory64 = address_type == AddressType::I64;
    let mut types = TypeSection::new();
    types
        .ty()
        .function([wasm_encoder::ValType::I64, wasm_encoder::ValType::I32], []);
    let mut imports = ImportSection::new();
    imports.import(
        COPIER_MODULE,
        SHARED,
        wasm_encoder::MemoryType {
            minimum: u64::MIN,
            maximum: Some(most_pages(address_type)),
            memory64,
            shared: true,
            page_size_log2: None,
        },
    );
    let (shared, scratch) = (0, 1);
    let copy_type = 0;
    let mut functions = FunctionSection::new();
    functions.function(copy_type).function(copy_type);
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
        .export(SCRATCH, ExportKind::Memory, scratch);
    let (offset, length) = (0, 1);
    let mut load = Function::new([]);
    load.instructions().i32_const(0).local_get(offset);
    let mut store = Function::new([]);
    store.instructions().local_get(offset);
    if !memory64 {
        load.instructions().i32_wrap_i64();
        store.instructions().i32_wrap_i64();
    }
    load.instructions()
        .local_get(length)
        .memory_copy(scratch, shared)
        .end();
    store
        .instructions()
        .i32_const(0)
        .local_get(length)
        .memory_copy(shared, scratch)
        .end();
    let mut code = CodeSection::new();
    code.function(&load).function(&store);
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

impl<HostContext: 'static> WasmtimeEngine<HostContext> {
    /// The address of the shared memory, through a copier instance made the first time it is added.
    fn add_shared(&mut self, memory: SharedMemory) -> MemoryAddress {
        // The base of a shared memory never moves, so it tells one memory from another.
        let base = memory.data().as_ptr();
        if let Some(added) = self
            .shared_memories
            .iter()
            .find(|added| core::ptr::eq(added.memory.data().as_ptr(), base))
        {
            return MemoryAddress(MemoryInstance::Shared(added.clone()));
        }
        let copiers = self
            .config
            .copiers
            .get_or_init(|| Copiers::compile(&self.config.engine));
        let module = if memory.ty().is_64() {
            &copiers.wide
        } else {
            &copiers.narrow
        };
        let store = self.store.get_mut();
        let instance = Instance::new(&mut *store, module, &[memory.clone().into()])
            .expect("a shared memory of this wasmtime engine imports into the copier");
        let added = SharedMemoryAddress {
            memory,
            scratch: instance
                .get_memory(&mut *store, SCRATCH)
                .expect("the copier exports its scratch memory"),
            load: instance
                .get_typed_func(&mut *store, LOAD)
                .expect("the copier exports load"),
            store: instance
                .get_typed_func(&mut *store, STORE)
                .expect("the copier exports store"),
        };
        self.shared_memories.push(added.clone());
        MemoryAddress(MemoryInstance::Shared(added))
    }
}

/// Whether `length` bytes at `offset` lie inside a memory of `size` bytes.
fn inside(size: usize, offset: u64, length: usize) -> bool {
    u64::try_from(length)
        .ok()
        .and_then(|length| offset.checked_add(length))
        .zip(u64::try_from(size).ok())
        .is_some_and(|(end, size)| end <= size)
}

impl SharedMemoryAddress {
    /// Grows the scratch memory to hold `length` bytes when it can, and returns the bytes one copy moves.
    fn make_room<T>(&self, store: &mut Store<T>, length: usize) -> Option<usize> {
        let wanted = u64::try_from(length)
            .ok()?
            .div_ceil(PAGE_SIZE)
            .min(most_pages(AddressType::I32));
        let pages = self.scratch.size(&*store);
        if pages < wanted {
            self.scratch.grow(&mut *store, wanted - pages).ok()?;
        }
        Some(
            self.scratch
                .data_size(&*store)
                .min(usize::try_from(u32::MAX).unwrap_or(usize::MAX)),
        )
    }

    fn read<T>(&self, store: &mut Store<T>, offset: u64, into: &mut [u8]) -> Option<()> {
        if into.is_empty() {
            return inside(self.memory.data_size(), offset, 0).then_some(());
        }
        if !inside(self.memory.data_size(), offset, into.len()) {
            return None;
        }
        let room = self.make_room(store, into.len())?;
        let mut position = offset;
        for chunk in into.chunks_mut(room) {
            let length = u32::try_from(chunk.len()).ok()?;
            self.load.call(&mut *store, (position, length)).ok()?;
            self.scratch.read(&*store, 0, chunk).ok()?;
            position += u64::from(length);
        }
        Some(())
    }

    fn write<T>(&self, store: &mut Store<T>, offset: u64, bytes: &[u8]) -> Option<()> {
        if bytes.is_empty() {
            return inside(self.memory.data_size(), offset, 0).then_some(());
        }
        if !inside(self.memory.data_size(), offset, bytes.len()) {
            return None;
        }
        let room = self.make_room(store, bytes.len())?;
        let mut position = offset;
        for chunk in bytes.chunks(room) {
            let length = u32::try_from(chunk.len()).ok()?;
            self.scratch.write(&mut *store, 0, chunk).ok()?;
            self.store.call(&mut *store, (position, length)).ok()?;
            position += u64::from(length);
        }
        Some(())
    }
}

/// Why `instantiate` failed.
#[derive(Debug, Error)]
pub enum InstantiateError {
    #[error("no address was given for the import {module_name}.{item_name}")]
    MissingImport {
        module_name: String,
        item_name: String,
    },
    #[error("wasmtime refused the instantiation: {0}")]
    Engine(wasmtime::Error),
}

/// Why `allocate_table` failed.
#[derive(Debug, Error)]
pub enum AllocateTableError {
    #[error("the limits do not fit the table's address type")]
    LimitsOutsideAddressType,
    #[error("wasmtime refused the table: {0}")]
    Engine(wasmtime::Error),
}

impl<HostContext: 'static> WebAssemblyEngine for WasmtimeEngine<HostContext> {
    type Config = Config;
    type Module = Module;
    type Instance = Instance;
    type TagAddress = Tag;
    type HostContext = HostContext;

    type NewError = Infallible;
    fn new(config: Config, host_context: HostContext) -> Result<Self, Infallible> {
        Ok(Self {
            store: RefCell::new(Store::new(&config.engine, host_context)),
            config,
            shared_memories: Vec::new(),
        })
    }

    type CompileError = wasmtime::Error;
    fn compile(&self, bytes: &[u8]) -> Result<Module, wasmtime::Error> {
        Module::from_binary(&self.config.engine, bytes)
    }

    type InstantiateError = InstantiateError;
    fn instantiate(
        &mut self,
        module: &Module,
        imports: &[Import<'_, Self>],
    ) -> Result<Instance, InstantiateError> {
        let externs = module
            .imports()
            .map(|wanted| {
                imports
                    .iter()
                    .find(|given| {
                        given.module_name == wanted.module() && given.item_name == wanted.name()
                    })
                    .map(|given| to_extern(&given.address))
                    .ok_or_else(|| InstantiateError::MissingImport {
                        module_name: wanted.module().to_owned(),
                        item_name: wanted.name().to_owned(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Instance::new(self.store.get_mut(), module, &externs).map_err(InstantiateError::Engine)
    }

    fn find_export(&mut self, instance: &Instance, name: &str) -> Option<ExternalAddress<Self>> {
        Some(match instance.get_export(self.store.get_mut(), name)? {
            Extern::Func(function) => ExternalAddress::Function(function),
            Extern::Global(global) => ExternalAddress::Global(global),
            Extern::Table(table) => ExternalAddress::Table(table),
            Extern::Memory(memory) => {
                ExternalAddress::Memory(MemoryAddress(MemoryInstance::Unshared(memory)))
            }
            Extern::SharedMemory(memory) => ExternalAddress::Memory(self.add_shared(memory)),
            Extern::Tag(tag) => ExternalAddress::Tag(tag),
        })
    }

    fn host_context(&mut self) -> &mut HostContext {
        self.store.get_mut().data_mut()
    }
}

fn to_extern<HostContext: 'static>(
    address: &ExternalAddress<WasmtimeEngine<HostContext>>,
) -> Extern {
    match address {
        ExternalAddress::Function(function) => (*function).into(),
        ExternalAddress::Table(table) => (*table).into(),
        ExternalAddress::Memory(MemoryAddress(MemoryInstance::Unshared(memory))) => {
            (*memory).into()
        }
        ExternalAddress::Memory(MemoryAddress(MemoryInstance::Shared(shared))) => {
            shared.memory.clone().into()
        }
        ExternalAddress::Global(global) => (*global).into(),
        ExternalAddress::Tag(tag) => (*tag).into(),
    }
}

impl<HostContext: 'static> FunctionStore for WasmtimeEngine<HostContext> {
    type FunctionAddress = Func;
    type Trap = wasmtime::Error;
}

/// A function bound to a Rust signature: wasmtime's typed function.
pub struct Function<Parameters, Results>(TypedFunc<Parameters, Results>);

impl<HostContext: 'static, Parameters: WasmParams, Results: WasmResults>
    BoundFunction<WasmtimeEngine<HostContext>, Parameters, Results>
    for Function<Parameters, Results>
{
    fn invoke(
        &self,
        engine: &mut WasmtimeEngine<HostContext>,
        parameters: Parameters,
    ) -> Result<Results, InvokeError<wasmtime::Error>> {
        let store = engine.store.get_mut();
        self.0
            .call(&mut *store, parameters)
            .map_err(|error| uncaught(store, error))
    }
}

/// Tells an exception that left the invocation from a trap, and clears the exception from the store.
#[cold]
fn uncaught<T>(store: &mut Store<T>, error: wasmtime::Error) -> InvokeError<wasmtime::Error> {
    if error.is::<ThrownException>() {
        let mut scope = RootScope::new(store);
        // The scope unroots the exception when it ends.
        scope.as_context_mut().take_pending_exception();
        InvokeError::Exception
    } else {
        InvokeError::Trap(error)
    }
}

/// Binds every function type of wasmtime's typed calls: one implementation per number of parameters.
macro_rules! bind_function_types {
    () => {
        bind_function_types!(@one);
    };
    ($first:ident $first_argument:ident $($parameter:ident $argument:ident)*) => {
        bind_function_types!(@one $first $first_argument $($parameter $argument)*);
        bind_function_types!($($parameter $argument)*);
    };
    (@one $($parameter:ident $argument:ident)*) => {
        impl<HostContext: 'static, $($parameter: WasmTy,)* Results: WasmResults + WasmRet>
            FunctionTypeBinder<($($parameter,)*), Results> for WasmtimeEngine<HostContext>
        {
            type Function = Function<($($parameter,)*), Results>;

            fn bind_engine_function(
                &mut self,
                address: Func,
            ) -> Result<Self::Function, BindFunctionError> {
                address
                    .typed(&*self.store.get_mut())
                    .map(Function)
                    .map_err(|_| BindFunctionError)
            }

            fn bind_host_function(
                &mut self,
                function: impl Fn(&mut HostContext, ($($parameter,)*)) -> Results
                    + Send
                    + Sync
                    + 'static,
            ) -> Func {
                Func::wrap(
                    self.store.get_mut(),
                    move |mut caller: Caller<'_, HostContext>, $($argument: $parameter),*| {
                        function(caller.data_mut(), ($($argument,)*))
                    },
                )
            }
        }
    };
}

bind_function_types!(
    A1 a1 A2 a2 A3 a3 A4 a4 A5 a5 A6 a6 A7 a7 A8 a8 A9 a9 A10 a10 A11 a11 A12 a12 A13 a13 A14 a14
    A15 a15 A16 a16 A17 a17
);

impl<HostContext: 'static> MemoryStore for WasmtimeEngine<HostContext> {
    type MemoryAddress = MemoryAddress;

    type AllocateMemoryError = wasmtime::Error;
    fn allocate_memory(
        &mut self,
        memory_type: MemoryType,
    ) -> Result<MemoryAddress, wasmtime::Error> {
        let Limits { minimum, maximum } = memory_type.limits;
        let memory_type_of_wasmtime = MemoryTypeBuilder::new()
            .min(minimum)
            .max(maximum)
            .memory64(memory_type.address_type == AddressType::I64)
            .shared(memory_type.sharing == Sharing::Shared)
            .build()?;
        Ok(match memory_type.sharing {
            Sharing::Unshared => MemoryAddress(MemoryInstance::Unshared(Memory::new(
                self.store.get_mut(),
                memory_type_of_wasmtime,
            )?)),
            Sharing::Shared => {
                let memory = SharedMemory::new(&self.config.engine, memory_type_of_wasmtime)?;
                self.add_shared(memory)
            }
        })
    }

    fn count_memory_pages(&self, memory: &MemoryAddress) -> u64 {
        match &memory.0 {
            MemoryInstance::Unshared(memory) => memory.size(&*self.store.borrow()),
            MemoryInstance::Shared(shared) => shared.memory.size(),
        }
    }

    fn grow_memory(&mut self, memory: &MemoryAddress, pages: u64) -> Result<(), GrowMemoryError> {
        match &memory.0 {
            MemoryInstance::Unshared(memory) => memory.grow(self.store.get_mut(), pages),
            MemoryInstance::Shared(shared) => shared.memory.grow(pages),
        }
        .map(drop)
        .map_err(|_| GrowMemoryError)
    }

    fn read_memory(
        &self,
        memory: &MemoryAddress,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), ReadMemoryError> {
        match &memory.0 {
            MemoryInstance::Unshared(memory) => memory
                .read(
                    &*self.store.borrow(),
                    usize::try_from(offset).map_err(|_| ReadMemoryError)?,
                    into,
                )
                .map_err(|_| ReadMemoryError),
            MemoryInstance::Shared(shared) => shared
                .read(&mut self.store.borrow_mut(), offset, into)
                .ok_or(ReadMemoryError),
        }
    }

    fn write_memory(
        &mut self,
        memory: &MemoryAddress,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), WriteMemoryError> {
        match &memory.0 {
            MemoryInstance::Unshared(memory) => memory
                .write(
                    self.store.get_mut(),
                    usize::try_from(offset).map_err(|_| WriteMemoryError)?,
                    bytes,
                )
                .map_err(|_| WriteMemoryError),
            MemoryInstance::Shared(shared) => shared
                .write(self.store.get_mut(), offset, bytes)
                .ok_or(WriteMemoryError),
        }
    }

    type SharedMemory = SharedMemory;

    fn to_shared_memory(&self, memory: &MemoryAddress) -> Option<SharedMemory> {
        match &memory.0 {
            MemoryInstance::Unshared(_) => None,
            MemoryInstance::Shared(shared) => Some(shared.memory.clone()),
        }
    }

    fn add_shared_memory(&mut self, memory: &SharedMemory) -> MemoryAddress {
        self.add_shared(memory.clone())
    }
}

/// Narrows a limit of a table to the 32-bit address type.
fn narrow(limit: u64) -> Result<u32, AllocateTableError> {
    u32::try_from(limit).map_err(|_| AllocateTableError::LimitsOutsideAddressType)
}

impl<HostContext: 'static> TableStore for WasmtimeEngine<HostContext> {
    type TableAddress = Table;

    type AllocateTableError = AllocateTableError;
    fn allocate_table(&mut self, table_type: TableType) -> Result<Table, AllocateTableError> {
        let Limits { minimum, maximum } = table_type.limits;
        let table_type_of_wasmtime = match table_type.address_type {
            AddressType::I32 => wasmtime::TableType::new(
                RefType::FUNCREF,
                narrow(minimum)?,
                maximum.map(narrow).transpose()?,
            ),
            AddressType::I64 => wasmtime::TableType::new64(RefType::FUNCREF, minimum, maximum),
        };
        Table::new(
            self.store.get_mut(),
            table_type_of_wasmtime,
            Ref::Func(None),
        )
        .map_err(AllocateTableError::Engine)
    }

    fn count_table_elements(&self, table: &Table) -> u64 {
        table.size(&*self.store.borrow())
    }

    fn grow_table(&mut self, table: &Table, elements: u64) -> Result<(), GrowTableError> {
        table
            .grow(self.store.get_mut(), elements, Ref::Func(None))
            .map(drop)
            .map_err(|_| GrowTableError)
    }

    fn read_table(&self, table: &Table, index: u64) -> Result<Option<Func>, ReadTableError> {
        let mut store = self.store.borrow_mut();
        let mut scope = RootScope::new(&mut *store);
        match table.get(&mut scope, index) {
            None => Err(ReadTableError::IndexOutsideTable),
            Some(Ref::Func(function)) => Ok(function),
            Some(_) => Err(ReadTableError::ElementNotFunction),
        }
    }

    fn write_table(
        &mut self,
        table: &Table,
        index: u64,
        function: Option<&Func>,
    ) -> Result<(), WriteTableError> {
        let store = self.store.get_mut();
        if index >= table.size(&*store) {
            return Err(WriteTableError::IndexOutsideTable);
        }
        table
            .set(store, index, Ref::Func(function.cloned()))
            .map_err(|_| WriteTableError::ReferenceTypeMismatch)
    }
}

fn to_val(value: Value<Func>) -> Val {
    match value {
        Value::I32(value) => Val::I32(value),
        Value::I64(value) => Val::I64(value),
        Value::F32(value) => Val::F32(value.to_bits()),
        Value::F64(value) => Val::F64(value.to_bits()),
        Value::V128(value) => Val::V128(value.into()),
        Value::FunctionReference(function) => Val::FuncRef(function),
    }
}

fn value_type(value: &Value<Func>) -> ValType {
    match value {
        Value::I32(_) => ValType::I32,
        Value::I64(_) => ValType::I64,
        Value::F32(_) => ValType::F32,
        Value::F64(_) => ValType::F64,
        Value::V128(_) => ValType::V128,
        Value::FunctionReference(_) => ValType::FUNCREF,
    }
}

impl<HostContext: 'static> GlobalStore for WasmtimeEngine<HostContext> {
    type GlobalAddress = Global;

    type AllocateGlobalError = wasmtime::Error;
    fn allocate_global(
        &mut self,
        mutability: Mutability,
        value: Value<Func>,
    ) -> Result<Global, wasmtime::Error> {
        let mutability = match mutability {
            Mutability::Constant => wasmtime::Mutability::Const,
            Mutability::Variable => wasmtime::Mutability::Var,
        };
        let global_type = GlobalType::new(value_type(&value), mutability);
        Global::new(self.store.get_mut(), global_type, to_val(value))
    }

    fn read_global(&self, global: &Global) -> Result<Value<Func>, ReadGlobalError> {
        let mut store = self.store.borrow_mut();
        let mut scope = RootScope::new(&mut *store);
        match global.get(&mut scope) {
            Val::I32(value) => Ok(Value::I32(value)),
            Val::I64(value) => Ok(Value::I64(value)),
            Val::F32(bits) => Ok(Value::F32(f32::from_bits(bits))),
            Val::F64(bits) => Ok(Value::F64(f64::from_bits(bits))),
            Val::V128(value) => Ok(Value::V128(value.as_u128())),
            Val::FuncRef(function) => Ok(Value::FunctionReference(function)),
            _ => Err(ReadGlobalError),
        }
    }

    fn write_global(
        &mut self,
        global: &Global,
        value: Value<Func>,
    ) -> Result<(), WriteGlobalError> {
        let store = self.store.get_mut();
        if global.ty(&*store).mutability() == wasmtime::Mutability::Const {
            return Err(WriteGlobalError::Constant);
        }
        global
            .set(store, to_val(value))
            .map_err(|_| WriteGlobalError::ValueTypeMismatch)
    }
}
