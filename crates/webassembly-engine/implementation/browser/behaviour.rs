//! What this engine does beyond the interface's suite: host functions live and die with their engine, an engine's
//! code runs inside another's host function, a host function's panic goes on, and floats keep their bits.
#![cfg(target_arch = "wasm32")]

use core::hint::black_box;
use core::sync::atomic::{AtomicUsize, Ordering};

use wasm_bindgen_test::wasm_bindgen_test as test;
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, FunctionSection, ImportSection, Module,
    TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder, InvokeError};
use wcontainers_webassembly_engine::global::{GlobalStore, Mutability, Value};
use wcontainers_webassembly_engine::{ExternalAddress, Import, WebAssemblyEngine};
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
            assert_eq!(bits(engine.read_global(&global).expect("read")), u64::from(nan));
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
