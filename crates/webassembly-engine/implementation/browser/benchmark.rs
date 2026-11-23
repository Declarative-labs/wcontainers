//! The figures of an engine, taken through the interface's traits alone, here for the browser's engine in a worker.
#![cfg(target_arch = "wasm32")]

use core::hint::black_box;

use js_sys::WebAssembly::Memory;
use js_sys::{Function, Reflect, Uint8Array};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::{console_log, wasm_bindgen_test};
use wasm_encoder::{
    BlockType, CodeSection, EntityType, ExportKind, ExportSection, FunctionSection, ImportSection,
    Module, TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder};
use wcontainers_webassembly_engine::memory::{MemoryType as EngineMemoryType, PAGE_SIZE, Sharing};
use wcontainers_webassembly_engine::table::TableType;
use wcontainers_webassembly_engine::{
    AddressType, ExternalAddress, Import, Limits, WebAssemblyEngine,
};
use wcontainers_webassembly_engine_browser::BrowserEngine;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

const NAMESPACE: &str = "env";
const HOOK: &str = "hook";
const EMPTY: &str = "empty";
const LOOP: &str = "loop";

const NO_VALUES: [ValType; 0] = [];

/// How many times each operation runs for one figure.
const CALLS: u32 = 1_000_000;
const COMPILES: u32 = 1_000;
const COPIES: u32 = 100_000;
const SMALL: usize = 64;
const LARGE: usize = 64 * 1024;

/// A module that imports `hook`, and exports `empty`, which does nothing, and `loop`, which calls `hook` as many
/// times as its argument says.
fn calling_module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function(NO_VALUES, NO_VALUES);
    types.ty().function([ValType::I32], []);
    let mut imports = ImportSection::new();
    imports.import(NAMESPACE, HOOK, EntityType::Function(0));
    let mut functions = FunctionSection::new();
    functions.function(0).function(1);
    let mut exports = ExportSection::new();
    exports
        .export(EMPTY, ExportKind::Func, 1)
        .export(LOOP, ExportKind::Func, 2);
    let mut code = CodeSection::new();
    let mut empty = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
    empty.instructions().end();
    code.function(&empty);
    let mut repeat = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
    repeat
        .instructions()
        .block(BlockType::Empty)
        .loop_(BlockType::Empty)
        .local_get(0)
        .i32_eqz()
        .br_if(1)
        .call(0)
        .local_get(0)
        .i32_const(1)
        .i32_sub()
        .local_set(0)
        .br(0)
        .end()
        .end()
        .end();
    code.function(&repeat);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&exports)
        .section(&code);
    module.finish()
}

fn now() -> f64 {
    let performance = Reflect::get(&js_sys::global(), &JsValue::from_str("performance"))
        .expect("a performance clock");
    let now: Function = Reflect::get(&performance, &JsValue::from_str("now"))
        .expect("now")
        .unchecked_into();
    now.call0(&performance)
        .expect("the time")
        .as_f64()
        .expect("a number")
}

/// Runs the operation the number of times and returns the nanoseconds each run took.
fn time(times: u32, mut operation: impl FnMut()) -> f64 {
    let start = now();
    for _ in 0..times {
        operation();
    }
    (now() - start) * 1e6 / f64::from(times)
}

/// Keeps a value the optimizer could otherwise find unused.
fn consume<T>(value: T) {
    black_box(value);
}

fn report(name: &str, nanoseconds: f64) {
    console_log!("figure {name}: {nanoseconds:.1} ns");
}

fn function<E: WebAssemblyEngine>(
    engine: &mut E,
    instance: &E::Instance,
    name: &str,
) -> E::FunctionAddress {
    match engine.find_export(instance, name) {
        Some(ExternalAddress::Function(function)) => function,
        _ => panic!("the export is a function"),
    }
}

/// Takes the figures of compiling, instantiating and calling.
fn calls<E>(mut engine: E)
where
    E: WebAssemblyEngine<HostContext = i32>
        + FunctionTypeBinder<(), ()>
        + FunctionTypeBinder<(i32,), ()>,
{
    let bytes = calling_module();
    report(
        "compile a small module",
        time(COMPILES, || consume(black_box(engine.compile(&bytes)))),
    );
    let module = engine.compile(&bytes).ok().expect("the module compiles");
    for (name, touches) in [
        ("an empty host function", false),
        ("a host function that touches the context", true),
    ] {
        let hook = if touches {
            FunctionTypeBinder::<(), ()>::bind_host_function(
                &mut engine,
                |context: &mut i32, ()| *context += 1,
            )
        } else {
            FunctionTypeBinder::<(), ()>::bind_host_function(&mut engine, |_: &mut i32, ()| {})
        };
        let imports = [Import {
            module_name: NAMESPACE,
            item_name: HOOK,
            address: ExternalAddress::Function(hook),
        }];
        if !touches {
            report(
                "instantiate it",
                time(COMPILES, || {
                    consume(black_box(engine.instantiate(&module, &imports)))
                }),
            );
        }
        let instance = engine
            .instantiate(&module, &imports)
            .ok()
            .expect("the module instantiates");
        let address = function(&mut engine, &instance, EMPTY);
        let empty =
            FunctionTypeBinder::<(), ()>::bind_engine_function(&mut engine, address.clone())
                .expect("bound");
        if !touches {
            report(
                "an empty call into a module",
                time(CALLS, || consume(black_box(empty.invoke(&mut engine, ())))),
            );
            report(
                "bind a function",
                time(COMPILES, || {
                    consume(black_box(
                        FunctionTypeBinder::<(), ()>::bind_engine_function(
                            &mut engine,
                            address.clone(),
                        ),
                    ))
                }),
            );
        }
        let address = function(&mut engine, &instance, LOOP);
        let repeat = FunctionTypeBinder::<(i32,), ()>::bind_engine_function(&mut engine, address)
            .expect("bound");
        let start = now();
        repeat
            .invoke(&mut engine, (CALLS.cast_signed(),))
            .ok()
            .expect("the loop runs");
        report(
            &format!("{name} called from a module"),
            (now() - start) * 1e6 / f64::from(CALLS),
        );
    }
}

/// Takes the figures of copying into and out of an unshared and a shared memory.
fn copies<E>(mut engine: E)
where
    E: WebAssemblyEngine,
{
    let mut buffer = vec![u8::MAX; LARGE];
    for sharing in [Sharing::Unshared, Sharing::Shared] {
        let pages = (LARGE as u64).div_ceil(PAGE_SIZE);
        let memory = engine
            .allocate_memory(EngineMemoryType {
                address_type: AddressType::I32,
                limits: Limits {
                    minimum: pages,
                    maximum: Some(pages),
                },
                sharing,
            })
            .ok()
            .expect("the memory");
        for size in [SMALL, LARGE] {
            let label = format!("{size} bytes of a {sharing:?} memory");
            let bytes = &mut buffer[..size];
            report(
                &format!("read {label}"),
                time(COPIES, || {
                    consume(black_box(engine.read_memory(&memory, 0, bytes)))
                }),
            );
            report(
                &format!("write {label}"),
                time(COPIES, || {
                    consume(black_box(engine.write_memory(&memory, 0, bytes)))
                }),
            );
        }
    }
}

/// Takes the figure of writing a reference to a function into a table.
fn tables<E: WebAssemblyEngine<HostContext = i32> + FunctionTypeBinder<(), ()>>(mut engine: E) {
    let function =
        FunctionTypeBinder::<(), ()>::bind_host_function(&mut engine, |_: &mut i32, ()| {});
    let limits = Limits {
        minimum: 1,
        maximum: Some(1),
    };
    let table = engine
        .allocate_table(TableType {
            address_type: AddressType::I32,
            limits,
        })
        .ok()
        .expect("the table");
    report(
        "write a function into a table",
        time(COPIES, || {
            consume(black_box(engine.write_table(&table, 0, Some(&function))))
        }),
    );
}

#[wasm_bindgen(inline_js = r#"
export function newMemory(shared) {
  return new WebAssembly.Memory({ initial: 1, maximum: 1, shared });
}
export function copyOut(guest, offset, own, address, length) {
  new Uint8Array(own.buffer, address, length).set(new Uint8Array(guest.buffer, offset, length));
}
export function copyIn(guest, offset, own, address, length) {
  new Uint8Array(guest.buffer, offset, length).set(new Uint8Array(own.buffer, address, length));
}
"#)]
extern "C" {
    /// Returns a memory of one page, shared or not.
    #[wasm_bindgen(js_name = newMemory)]
    fn new_memory(shared: bool) -> Memory;
    /// Copies from the guest memory into our own through typed-array views, as hand-written glue would.
    #[wasm_bindgen(js_name = copyOut)]
    fn copy_out(guest: &Memory, offset: u32, own: &JsValue, address: u32, length: u32);
    /// Copies from our own memory into the guest memory through typed-array views.
    #[wasm_bindgen(js_name = copyIn)]
    fn copy_in(guest: &Memory, offset: u32, own: &JsValue, address: u32, length: u32);
}

/// Takes the figures of copying through typed-array views made by hand-written glue, which the engine does not use.
fn glue_views() {
    let own = wasm_bindgen::memory();
    let mut buffer = vec![u8::MAX; LARGE];
    for shared in [false, true] {
        let memory = new_memory(shared);
        for size in [SMALL, LARGE] {
            let label = format!("{size} bytes of a memory shared {shared}");
            let address = buffer.as_mut_ptr() as usize as u32;
            report(
                &format!("glue views: read {label}"),
                time(COPIES, || copy_out(&memory, 0, &own, address, size as u32)),
            );
            report(
                &format!("glue views: write {label}"),
                time(COPIES, || copy_in(&memory, 0, &own, address, size as u32)),
            );
        }
    }
    black_box(&mut buffer);
}

/// Takes the figures through js-sys's generic objects, which the engine does not use, to compare its route with.
fn generic_objects() {
    let mut engine = engine();
    let module = engine
        .compile(&calling_module())
        .expect("the module compiles");
    let hook = FunctionTypeBinder::<(), ()>::bind_host_function(&mut engine, |_: &mut i32, ()| {});
    let imports = [Import {
        module_name: NAMESPACE,
        item_name: HOOK,
        address: ExternalAddress::Function(hook),
    }];
    let instance = engine
        .instantiate(&module, &imports)
        .expect("the module instantiates");
    let empty = function(&mut engine, &instance, EMPTY);
    report(
        "js-sys: an empty call into a module",
        time(CALLS, || {
            consume(black_box(empty.call0(&JsValue::UNDEFINED)))
        }),
    );
    let descriptor = js_sys::Object::new();
    Reflect::set(
        &descriptor,
        &JsValue::from_str("initial"),
        &JsValue::from(1),
    )
    .expect("a descriptor");
    let memory = Memory::new(&descriptor).expect("a memory");
    let mut buffer = vec![u8::MIN; LARGE];
    for size in [SMALL, LARGE] {
        let bytes = &mut buffer[..size];
        report(
            &format!("js-sys: read {size} bytes of an unshared memory"),
            time(COPIES, || {
                Uint8Array::new_with_byte_offset_and_length(&memory.buffer(), 0, size as u32)
                    .copy_to(bytes);
            }),
        );
    }
}

fn engine() -> BrowserEngine<i32> {
    let Ok(engine) = BrowserEngine::new((), i32::default());
    engine
}

#[wasm_bindgen_test]
fn figures() {
    let global = js_sys::global();
    let property = |name: &str| Reflect::get(&global, &JsValue::from_str(name)).ok();
    console_log!(
        "cross-origin isolated: {:?}",
        property("crossOriginIsolated")
    );
    console_log!("global scope: {:?}", global.constructor().name());
    console_log!(
        "user agent: {:?}",
        property("navigator").and_then(|navigator| Reflect::get(
            &navigator,
            &JsValue::from_str("userAgent")
        )
        .ok())
    );
    calls(engine());
    copies(engine());
    tables(engine());
    generic_objects();
    glue_views();
}
