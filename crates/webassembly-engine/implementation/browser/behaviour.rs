//! What this engine does beyond the interface's suite: host functions live and die with their engine, an engine's
//! code runs inside another's host function, a host function's panic goes on, floats keep their bits, and tables,
//! globals and memories of the types the browser cannot describe are told apart.
#![cfg(target_arch = "wasm32")]

use core::hint::black_box;
use core::sync::atomic::{AtomicUsize, Ordering};

use wasm_bindgen_test::wasm_bindgen_test as test;
use wasm_encoder::{
    AbstractHeapType, CodeSection, ConstExpr, EntityType, ExportKind, ExportSection,
    FunctionSection, GlobalSection, GlobalType, HeapType, ImportSection, Module, RefType,
    TableSection, TableType, TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder, InvokeError};
use wcontainers_webassembly_engine::global::{
    GlobalStore, Mutability, ReadGlobalError, Value, WriteGlobalError,
};
use wcontainers_webassembly_engine::memory::{MemoryStore, MemoryType, PAGE_SIZE, Sharing};
use wcontainers_webassembly_engine::table::{ReadTableError, TableStore, WriteTableError};
use wcontainers_webassembly_engine::{
    AddressType, ExternalAddress, Import, Limits, WebAssemblyEngine,
};
use wcontainers_webassembly_engine_browser::{BrowserEngine, BrowserFunction};

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

const NAMESPACE: &str = "env";
const HOOK: &str = "hook";
const RUN: &str = "run";
const NO_VALUES: [ValType; 0] = [];

/// A module that imports `hook` and exports `run`, which calls it.
fn calling_module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function(NO_VALUES, NO_VALUES);
    let mut imports = ImportSection::new();
    imports.import(NAMESPACE, HOOK, EntityType::Function(0));
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export(RUN, ExportKind::Func, 1);
    let mut body = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
    body.instructions().call(0).end();
    let mut code = CodeSection::new();
    code.function(&body);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&exports)
        .section(&code);
    module.finish()
}

fn new_engine<HostContext: 'static>(context: HostContext) -> BrowserEngine<HostContext> {
    let Ok(engine) = BrowserEngine::new((), context);
    engine
}

/// Instantiates the calling module with the hook and binds its `run`.
fn run_calling<HostContext: 'static>(
    engine: &mut BrowserEngine<HostContext>,
    hook: js_sys::Function,
) -> BrowserFunction<(), ()> {
    let module = engine
        .compile(&calling_module())
        .expect("the module compiles");
    let imports = [Import {
        module_name: NAMESPACE,
        item_name: HOOK,
        address: ExternalAddress::Function(hook),
    }];
    let instance = engine
        .instantiate(&module, &imports)
        .expect("the module instantiates");
    let Some(ExternalAddress::Function(run)) = engine.find_export(&instance, RUN) else {
        panic!("the module exports run");
    };
    FunctionTypeBinder::<(), ()>::bind_engine_function(engine, run).expect("run has no values")
}

static DROPPED: AtomicUsize = AtomicUsize::new(usize::MIN);

/// Counts its drop in `DROPPED`.
struct DropCounter;

impl Drop for DropCounter {
    fn drop(&mut self) {
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn drops_host_functions_with_their_engine() {
    {
        let mut engine = new_engine(i32::default());
        let counter = DropCounter;
        let hook = FunctionTypeBinder::<(), ()>::bind_host_function(
            &mut engine,
            move |_: &mut i32, ()| {
                black_box(&counter);
            },
        );
        let run = run_calling(&mut engine, hook);
        assert_eq!(run.invoke(&mut engine, ()), Ok(()));
        assert_eq!(DROPPED.load(Ordering::Relaxed), usize::MIN);
    }
    assert_eq!(DROPPED.load(Ordering::Relaxed), 1);
}

#[test]
fn a_host_function_of_another_engine_traps() {
    let mut owner = new_engine(i32::default());
    let hook =
        FunctionTypeBinder::<(), ()>::bind_host_function(&mut owner, |context: &mut i32, ()| {
            *context += 1
        });
    let mut other = new_engine(i32::default());
    let run = run_calling(&mut other, hook);
    assert!(matches!(
        run.invoke(&mut other, ()),
        Err(InvokeError::Trap(_))
    ));
    assert_eq!(*owner.host_context(), i32::default());
    assert_eq!(*other.host_context(), i32::default());
}

/// A host context holding a second engine and a function bound in it.
struct Outer {
    inner: BrowserEngine<i32>,
    run: BrowserFunction<(), ()>,
}

#[test]
fn binds_and_calls_into_another_engine_inside_a_host_function() {
    let mut inner = new_engine(i32::default());
    let counting =
        FunctionTypeBinder::<(), ()>::bind_host_function(&mut inner, |count: &mut i32, ()| {
            *count += 1;
        });
    let run = run_calling(&mut inner, counting);
    let mut outer = new_engine(Outer { inner, run });
    let hook =
        FunctionTypeBinder::<(), ()>::bind_host_function(&mut outer, |outer: &mut Outer, ()| {
            let bound = FunctionTypeBinder::<(), ()>::bind_host_function(
                &mut outer.inner,
                |count: &mut i32, ()| *count += 1,
            );
            black_box(bound);
            assert_eq!(outer.run.invoke(&mut outer.inner, ()), Ok(()));
        });
    let run = run_calling(&mut outer, hook);
    for _ in 0..2 {
        assert_eq!(run.invoke(&mut outer, ()), Ok(()));
    }
    assert_eq!(*outer.host_context().inner.host_context(), 2);
}

#[test]
#[should_panic(expected = "a host function panicked in a call into the engine's code")]
fn a_panic_in_a_host_function_goes_on_from_the_call() {
    let mut engine = new_engine(i32::default());
    let hook = FunctionTypeBinder::<(), ()>::bind_host_function(&mut engine, |_: &mut i32, ()| {
        panic!("the host function fails");
    });
    let run = run_calling(&mut engine, hook);
    assert!(run.invoke(&mut engine, ()).is_err());
}

/// A quiet NaN and a signalling NaN of each width, each with a payload a JavaScript number would not keep.
const F32_NANS: [u32; 2] = [0x7fa0_0001, 0xff80_0001];
const F64_NANS: [u64; 2] = [0x7ff4_0000_0000_0001, 0xfff8_0000_0000_00ff];

fn bits(value: Value<js_sys::Function>) -> u64 {
    match value {
        Value::F32(value) => value.to_bits().into(),
        Value::F64(value) => value.to_bits(),
        _ => panic!("a float"),
    }
}

#[test]
fn keeps_the_bits_of_a_nan_in_a_float_global() {
    let mut engine = new_engine(i32::default());
    for mutability in [Mutability::Constant, Mutability::Variable] {
        for nan in F32_NANS {
            let global = engine
                .allocate_global(mutability, Value::F32(f32::from_bits(nan)))
                .expect("the global");
            assert_eq!(
                bits(engine.read_global(&global).expect("read")),
                u64::from(nan)
            );
        }
        for nan in F64_NANS {
            let global = engine
                .allocate_global(mutability, Value::F64(f64::from_bits(nan)))
                .expect("the global");
            assert_eq!(bits(engine.read_global(&global).expect("read")), nan);
        }
    }
    let narrow = engine
        .allocate_global(Mutability::Variable, Value::F32(f32::default()))
        .expect("the global");
    let wide = engine
        .allocate_global(Mutability::Variable, Value::F64(f64::default()))
        .expect("the global");
    for (narrow_nan, wide_nan) in F32_NANS.into_iter().zip(F64_NANS) {
        engine
            .write_global(&narrow, Value::F32(f32::from_bits(narrow_nan)))
            .expect("written");
        engine
            .write_global(&wide, Value::F64(f64::from_bits(wide_nan)))
            .expect("written");
        assert_eq!(
            bits(engine.read_global(&narrow).expect("read")),
            u64::from(narrow_nan)
        );
        assert_eq!(bits(engine.read_global(&wide).expect("read")), wide_nan);
    }
}

#[test]
fn lends_the_host_context_back_and_forth() {
    let mut engine = new_engine(i32::default());
    let hook =
        FunctionTypeBinder::<(), ()>::bind_host_function(&mut engine, |count: &mut i32, ()| {
            *count += 1;
        });
    let run = run_calling(&mut engine, hook);
    assert_eq!(run.invoke(&mut engine, ()), Ok(()));
    *engine.host_context() += 10;
    assert_eq!(run.invoke(&mut engine, ()), Ok(()));
    assert_eq!(run.invoke(&mut engine, ()), Ok(()));
    assert_eq!(*engine.host_context(), 13);
}

const FUNCTION: &str = "f";
const TABLE64: &str = "table64";
const EXTERNAL_TABLE: &str = "externref_table";
const EXTERNAL_GLOBAL: &str = "externref_global";

/// A module exporting a function, a 64-bit `funcref` table, an `externref` table and a mutable `externref` global.
fn references_module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function(NO_VALUES, NO_VALUES);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut tables = TableSection::new();
    for (element_type, table64) in [(RefType::FUNCREF, true), (RefType::EXTERNREF, false)] {
        tables.table(TableType {
            element_type,
            table64,
            minimum: 1,
            maximum: Some(4),
            shared: false,
        });
    }
    let mut globals = GlobalSection::new();
    globals.global(
        GlobalType {
            val_type: ValType::Ref(RefType::EXTERNREF),
            mutable: true,
            shared: false,
        },
        &ConstExpr::ref_null(HeapType::Abstract {
            shared: false,
            ty: AbstractHeapType::Extern,
        }),
    );
    let mut exports = ExportSection::new();
    exports.export(FUNCTION, ExportKind::Func, 0);
    exports.export(TABLE64, ExportKind::Table, 0);
    exports.export(EXTERNAL_TABLE, ExportKind::Table, 1);
    exports.export(EXTERNAL_GLOBAL, ExportKind::Global, 0);
    let mut body = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
    body.instructions().end();
    let mut code = CodeSection::new();
    code.function(&body);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&tables)
        .section(&globals)
        .section(&exports)
        .section(&code);
    module.finish()
}

#[test]
fn tells_the_tables_and_globals_of_a_module_apart() {
    let mut engine = new_engine(i32::default());
    let module = engine
        .compile(&references_module())
        .expect("the module compiles");
    let instance = engine
        .instantiate(&module, &[])
        .expect("the module instantiates");
    let Some(ExternalAddress::Function(function)) = engine.find_export(&instance, FUNCTION) else {
        panic!("the module exports a function");
    };
    let Some(ExternalAddress::Table(table64)) = engine.find_export(&instance, TABLE64) else {
        panic!("the module exports a 64-bit table");
    };
    assert_eq!(engine.read_table(&table64, 0), Ok(None));
    assert_eq!(engine.write_table(&table64, 0, Some(&function)), Ok(()));
    assert_eq!(engine.read_table(&table64, 0), Ok(Some(function.clone())));
    assert_eq!(
        engine.read_table(&table64, 1),
        Err(ReadTableError::IndexOutsideTable)
    );
    assert!(engine.grow_table(&table64, 1).is_ok());
    assert_eq!(engine.count_table_elements(&table64), 2);
    assert_eq!(engine.write_table(&table64, 1, Some(&function)), Ok(()));
    assert_eq!(
        engine.write_table(&table64, 2, None),
        Err(WriteTableError::IndexOutsideTable)
    );

    let Some(ExternalAddress::Table(external)) = engine.find_export(&instance, EXTERNAL_TABLE)
    else {
        panic!("the module exports an externref table");
    };
    assert_eq!(
        engine.write_table(&external, 0, Some(&function)),
        Err(WriteTableError::ReferenceTypeMismatch)
    );
    assert_eq!(
        engine.read_table(&external, 0),
        Err(ReadTableError::ElementNotFunction)
    );
    assert_eq!(
        engine.read_table(&external, 1),
        Err(ReadTableError::IndexOutsideTable)
    );
    assert!(engine.grow_table(&external, 1).is_ok());
    assert_eq!(engine.count_table_elements(&external), 2);

    let Some(ExternalAddress::Global(global)) = engine.find_export(&instance, EXTERNAL_GLOBAL)
    else {
        panic!("the module exports an externref global");
    };
    assert_eq!(engine.read_global(&global), Err(ReadGlobalError));
    assert_eq!(
        engine.write_global(&global, Value::FunctionReference(Some(function))),
        Err(WriteGlobalError::ValueTypeMismatch)
    );
}

#[test]
fn reads_and_writes_a_shared_64_bit_memory() {
    let mut engine = new_engine(i32::default());
    let memory = engine
        .allocate_memory(MemoryType {
            address_type: AddressType::I64,
            limits: Limits {
                minimum: 1,
                maximum: Some(2),
            },
            sharing: Sharing::Shared,
        })
        .expect("the memory");
    let shared = engine
        .to_shared_memory(&memory)
        .expect("the memory is shared");
    let added = engine.add_shared_memory(&shared);
    let bytes = [1, 2, 3, 4];
    assert_eq!(engine.write_memory(&memory, PAGE_SIZE - 4, &bytes), Ok(()));
    let mut read = [0; 4];
    assert_eq!(engine.read_memory(&added, PAGE_SIZE - 4, &mut read), Ok(()));
    assert_eq!(read, bytes);
    assert!(
        engine
            .read_memory(&added, PAGE_SIZE - 3, &mut read)
            .is_err()
    );
    assert!(engine.grow_memory(&memory, 1).is_ok());
    assert_eq!(engine.count_memory_pages(&added), 2);
    assert!(engine.grow_memory(&memory, 1).is_err());
}
