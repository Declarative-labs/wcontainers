//! Import and export names a guest module may choose, among them every name a JavaScript object inherits: each is the
//! module's own, reaches no prototype or builtin, and leaves every other engine of the worker as it was.
#![cfg(target_arch = "wasm32")]

mod names;

use core::convert::Infallible;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_core::Stream;
use js_sys::Array;
use names::{CALLBACK, IMPORTS};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_test::wasm_bindgen_test as test;
use wasm_encoder::{
    CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, FunctionSection, GlobalSection,
    GlobalType, ImportSection, MemorySection, MemoryType, Module, RefType, TableSection, TableType,
    TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder};
use wcontainers_webassembly_engine::global::{GlobalStore, Value};
use wcontainers_webassembly_engine::memory::MemoryStore;
use wcontainers_webassembly_engine::table::TableStore;
use wcontainers_webassembly_engine::{ExternalAddress, Import, WebAssemblyEngine};
use wcontainers_webassembly_engine_browser::BrowserEngine;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

type Engine = BrowserEngine<i32>;
type Instance = <Engine as WebAssemblyEngine>::Instance;

const NO_VALUES: [ValType; 0] = [];

/// Returns a new engine whose host context counts the calls of its host functions.
fn new_engine() -> Engine {
    let Ok(engine) = BrowserEngine::new((), i32::default());
    engine
}

#[wasm_bindgen(inline_js = r#"
const builtins = [Object.prototype, Function.prototype].flatMap((prototype) => [
  prototype,
  ...Object.getOwnPropertyNames(prototype)
    .map((name) => Object.getOwnPropertyDescriptor(prototype, name).value)
    .filter((value) => (typeof value === "object" && value !== null) || typeof value === "function"),
]);
export function inheritedNames() {
  return [...new Set([Object.prototype, Function.prototype].flatMap((prototype) => Object.getOwnPropertyNames(prototype)))];
}
export function reachesABuiltin(name) {
  return builtins.some((builtin) => Object.hasOwn(builtin, name));
}
"#)]
extern "C" {
    /// Returns the names every object or every function inherits.
    #[wasm_bindgen(js_name = inheritedNames)]
    fn inherited_names() -> Array;
    /// Answers whether the name is a property of a prototype every object or function has, or of a value on one.
    #[wasm_bindgen(js_name = reachesABuiltin)]
    fn reaches_a_builtin(name: &str) -> bool;
}

/// The namespace's other import, which a test supplies beside the one it leaves out.
const OTHER: &str = "given_beside";
/// The name of the function every builtin function inherits, under which a namespace that is a builtin has a function.
const INHERITED_ITEM: &str = "toString";
/// The export that calls every import.
const RUN: &str = "call_every_import";
/// A name and the name it holds before its null character.
const WITH_NULL: &str = "item\0hidden";
const BEFORE_NULL: &str = "item";

/// Every name a JavaScript object or function inherits, and names a property key or a lookup could treat apart.
fn hostile_names() -> Vec<String> {
    let mut names: Vec<String> = inherited_names()
        .iter()
        .map(|name| name.as_string().expect("a property name is a string"))
        .collect();
    let long = "x".repeat(usize::from(u16::MAX));
    for name in [
        "then",
        "0",
        "1",
        "-1",
        "-0",
        "1e3",
        "4294967295",
        "NaN",
        "",
        "prototype",
        "__proto__",
        WITH_NULL,
        BEFORE_NULL,
        &long,
    ] {
        if !names.iter().any(|known| known == name) {
            names.push(name.into());
        }
    }
    names
}

/// A module that imports a function of no values under each pair of names and exports `RUN`, which calls them in turn,
/// and under each export name a function that returns that name's place among them.
fn module(imports: &[(&str, &str)], exports: &[&str]) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function(NO_VALUES, NO_VALUES);
    types.ty().function(NO_VALUES, [ValType::I32]);
    let mut import_section = ImportSection::new();
    for (module_name, item_name) in imports {
        import_section.import(module_name, item_name, EntityType::Function(0));
    }
    let imported = u32::try_from(imports.len()).expect("few imports");
    let mut functions = FunctionSection::new();
    let mut export_section = ExportSection::new();
    let mut code = CodeSection::new();
    functions.function(0);
    export_section.export(RUN, ExportKind::Func, imported);
    let mut run = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
    for import in 0..imported {
        run.instructions().call(import);
    }
    run.instructions().end();
    code.function(&run);
    for (place, name) in (imported + 1..).zip(exports) {
        functions.function(1);
        export_section.export(name, ExportKind::Func, place);
        let mut body = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
        body.instructions()
            .i32_const((place - imported).cast_signed())
            .end();
        code.function(&body);
    }
    let mut module = Module::new();
    module
        .section(&types)
        .section(&import_section)
        .section(&functions)
        .section(&export_section)
        .section(&code);
    module.finish()
}

/// Instantiates the module with a host function, which counts its calls in the host context, under each pair of names.
fn instantiate(
    engine: &mut Engine,
    bytes: &[u8],
    supplied: &[(&str, &str)],
) -> Result<Instance, <Engine as WebAssemblyEngine>::InstantiateError> {
    let module = engine.compile(bytes).expect("the module compiles");
    let imports: Vec<_> = supplied
        .iter()
        .map(|(module_name, item_name)| Import {
            module_name,
            item_name,
            address: ExternalAddress::Function(FunctionTypeBinder::<(), ()>::bind_host_function(
                engine,
                |calls: &mut i32, ()| *calls += 1,
            )),
        })
        .collect();
    engine.instantiate(&module, &imports)
}

/// Runs the instance's `RUN` and returns how many host function calls it made.
fn run(engine: &mut Engine, instance: &Instance) -> i32 {
    let Some(ExternalAddress::Function(run)) = engine.find_export(instance, RUN) else {
        panic!("the module exports run");
    };
    let before = *engine.host_context();
    FunctionTypeBinder::<(), ()>::bind_engine_function(engine, run)
        .expect("run has no values")
        .invoke(engine, ())
        .expect("run returns");
    *engine.host_context() - before
}

/// Checks that a module missing the import under `CALLBACK` does not instantiate, in a new engine.
fn assert_callback_planted_nowhere() {
    assert!(!reaches_a_builtin(CALLBACK), "no builtin has `{CALLBACK}`");
    let mut engine = new_engine();
    let wanting = module(&[(IMPORTS, OTHER), (IMPORTS, CALLBACK)], &[]);
    assert!(instantiate(&mut engine, &wanting, &[(IMPORTS, OTHER)]).is_err());
}

#[test]
fn supplies_imports_under_any_name_and_plants_them_nowhere() {
    for name in hostile_names() {
        for (module_name, item_name) in [(name.as_str(), CALLBACK), (IMPORTS, name.as_str())] {
            let mut engine = new_engine();
            let pair = [(module_name, item_name)];
            let instance = instantiate(&mut engine, &module(&pair, &[]), &pair)
                .unwrap_or_else(|error| panic!("{name:?} instantiates: {error:?}"));
            assert_eq!(run(&mut engine, &instance), 1, "{name:?}");
            assert_callback_planted_nowhere();
        }
    }
}

#[test]
fn refuses_a_missing_import_under_any_name() {
    for name in hostile_names() {
        let mut engine = new_engine();
        let missing_item = module(&[(IMPORTS, OTHER), (IMPORTS, name.as_str())], &[]);
        assert!(
            instantiate(&mut engine, &missing_item, &[(IMPORTS, OTHER)]).is_err(),
            "the item {name:?} was never supplied"
        );
        let missing_namespace = module(&[(IMPORTS, OTHER), (name.as_str(), INHERITED_ITEM)], &[]);
        assert!(
            instantiate(&mut engine, &missing_namespace, &[(IMPORTS, OTHER)]).is_err(),
            "the namespace {name:?} was never supplied"
        );
    }
}

#[test]
fn tells_a_name_from_the_part_before_its_null_character() {
    let mut engine = new_engine();
    let wanting = module(&[(IMPORTS, WITH_NULL)], &[WITH_NULL]);
    assert!(instantiate(&mut engine, &wanting, &[(IMPORTS, BEFORE_NULL)]).is_err());
    let instance = instantiate(&mut engine, &wanting, &[(IMPORTS, WITH_NULL)])
        .expect("the import is supplied");
    assert!(engine.find_export(&instance, BEFORE_NULL).is_none());
    assert!(engine.find_export(&instance, WITH_NULL).is_some());
}

#[test]
fn finds_no_export_a_module_does_not_have() {
    let mut engine = new_engine();
    let instance = instantiate(&mut engine, &module(&[], &[]), &[]).expect("nothing to import");
    for name in hostile_names() {
        assert!(engine.find_export(&instance, &name).is_none(), "{name:?}");
    }
}

#[test]
fn finds_exports_under_any_name() {
    let names = hostile_names();
    let exports: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut engine = new_engine();
    let instance =
        instantiate(&mut engine, &module(&[], &exports), &[]).expect("nothing to import");
    for (place, name) in (1..).zip(&names) {
        let Some(ExternalAddress::Function(function)) = engine.find_export(&instance, name) else {
            panic!("the module exports a function under {name:?}");
        };
        let returned = FunctionTypeBinder::<(), i32>::bind_engine_function(&mut engine, function)
            .expect("the function returns an i32")
            .invoke(&mut engine, ());
        assert_eq!(returned, Ok(place), "{name:?}");
    }
}

const TABLE: &str = "constructor";
const MEMORY: &str = "valueOf";
const GLOBAL: &str = "__proto__";
const FUNCTION: &str = "hasOwnProperty";

/// A module that exports a table, a memory, a global and a function under names every object inherits.
fn kinds_module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function(NO_VALUES, NO_VALUES);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: RefType::FUNCREF,
        table64: false,
        minimum: 1,
        maximum: None,
        shared: false,
    });
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    let mut globals = GlobalSection::new();
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(i32::MAX),
    );
    let mut exports = ExportSection::new();
    exports
        .export(TABLE, ExportKind::Table, 0)
        .export(MEMORY, ExportKind::Memory, 0)
        .export(GLOBAL, ExportKind::Global, 0)
        .export(FUNCTION, ExportKind::Func, 0);
    let mut body = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
    body.instructions().end();
    let mut code = CodeSection::new();
    code.function(&body);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&tables)
        .section(&memories)
        .section(&globals)
        .section(&exports)
        .section(&code);
    module.finish()
}

#[test]
fn tells_the_kind_of_an_export_under_an_inherited_name() {
    let mut engine = new_engine();
    let instance = instantiate(&mut engine, &kinds_module(), &[]).expect("nothing to import");
    let Some(ExternalAddress::Table(table)) = engine.find_export(&instance, TABLE) else {
        panic!("a table under {TABLE}");
    };
    assert_eq!(
        engine
            .read_table(&table, 0)
            .map(|element| element.is_none()),
        Ok(true)
    );
    let Some(ExternalAddress::Memory(memory)) = engine.find_export(&instance, MEMORY) else {
        panic!("a memory under {MEMORY}");
    };
    assert_eq!(engine.count_memory_pages(&memory), 1);
    let Some(ExternalAddress::Global(global)) = engine.find_export(&instance, GLOBAL) else {
        panic!("a global under {GLOBAL}");
    };
    assert!(matches!(
        engine.read_global(&global),
        Ok(Value::I32(i32::MAX))
    ));
    assert!(matches!(
        engine.find_export(&instance, FUNCTION),
        Some(ExternalAddress::Function(_))
    ));
}

/// A stream that gives the bytes in one chunk.
struct OneChunk(Option<Vec<u8>>);

impl Stream for OneChunk {
    type Item = Result<Vec<u8>, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.0.take().map(Ok))
    }
}

/// The name a JavaScript promise looks up on whatever it is resolved with.
const THEN: &str = "then";

#[test]
async fn compiles_from_a_stream_a_module_whose_import_and_export_are_named_then() {
    let mut engine = new_engine();
    let pair = [(THEN, THEN)];
    let module = engine
        .compile_streaming(OneChunk(Some(module(&pair, &[THEN]))))
        .await
        .expect("the module compiles");
    let hook =
        FunctionTypeBinder::<(), ()>::bind_host_function(&mut engine, |calls: &mut i32, ()| {
            *calls += 1;
        });
    let imports = [Import {
        module_name: THEN,
        item_name: THEN,
        address: ExternalAddress::Function(hook),
    }];
    let instance = engine
        .instantiate(&module, &imports)
        .expect("the import is supplied");
    assert_eq!(run(&mut engine, &instance), 1);
    let Some(ExternalAddress::Function(then)) = engine.find_export(&instance, THEN) else {
        panic!("the module exports a function under {THEN}");
    };
    let returned = FunctionTypeBinder::<(), i32>::bind_engine_function(&mut engine, then)
        .expect("the function returns an i32")
        .invoke(&mut engine, ());
    assert_eq!(returned, Ok(1));
    assert_callback_planted_nowhere();
}
