//! A module and a shared memory made by an engine in one worker, posted to another worker whose engine uses them.
#![cfg(target_arch = "wasm32")]

mod names;

use js_sys::Promise;
use js_sys::WebAssembly::{Memory, Module};
use names::{CALLBACK, IMPORTS};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, FunctionSection, ImportSection, MemArg,
    MemArgTarget, MemoryType, TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder};
use wcontainers_webassembly_engine::memory::{
    MemoryStore, MemoryType as EngineMemoryType, Sharing,
};
use wcontainers_webassembly_engine::{
    AddressType, ExternalAddress, Import, Limits, WebAssemblyEngine,
};
use wcontainers_webassembly_engine_browser::{
    BrowserEngine, BrowserMemory, module_to_post, receive_module, receive_shared_memory,
    shared_memory_to_post,
};

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

#[wasm_bindgen(inline_js = r#"
export function storeInAnotherWorker(module, memory) {
  // The files the test runner serves beside the worker that runs the tests: our code and its bindings.
  const bindings = JSON.stringify(new URL("wasm-bindgen-test.js", self.location.href).href);
  const code = JSON.stringify(new URL("wasm-bindgen-test_bg.wasm", self.location.href).href);
  const source = `import init, { storeInThisWorker } from ${bindings};
onmessage = async ({ data: [module, memory] }) => {
  try {
    await init({ module_or_path: ${code} });
    postMessage({ read: storeInThisWorker(module, memory) });
  } catch (error) {
    postMessage({ error: String(error) });
  }
};`;
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  const worker = new Worker(url, { type: "module" });
  return new Promise((resolve, reject) => {
    worker.onmessage = ({ data }) => {
      if (data === null || typeof data !== "object" || !("read" in data || "error" in data)) return;
      worker.terminate();
      if ("error" in data) reject(data.error); else resolve(data.read);
    };
    worker.onerror = (error) => reject(error.message);
    worker.postMessage([module, memory]);
  });
}
"#)]
extern "C" {
    /// Posts the module and the memory to a new worker that runs our code, which calls `storeInThisWorker` there and
    /// answers what it returned.
    #[wasm_bindgen(js_name = storeInAnotherWorker)]
    fn store_in_another_worker(module: &Module, memory: &Memory) -> Promise;
}

const SHARED_MEMORY: &str = "shared_memory";
const KEEP: &str = "keep";

/// The value the host function gives `keep`, which writes it at the start of the memory.
const STORED: i32 = i32::MAX;

/// A module that imports a shared memory of one page and `callback`, and exports `keep`, which writes what `callback`
/// returns at the start of the memory.
fn storing_module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([] as [ValType; 0], []);
    types.ty().function([] as [ValType; 0], [ValType::I32]);
    let mut imports = ImportSection::new();
    let memory = MemoryType {
        minimum: 1,
        maximum: Some(1),
        memory64: false,
        shared: true,
        page_size_log2: None,
    };
    imports.import(IMPORTS, SHARED_MEMORY, EntityType::Memory(memory));
    imports.import(IMPORTS, CALLBACK, EntityType::Function(1));
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export(KEEP, ExportKind::Func, 1);
    let mut body = wasm_encoder::Function::new_with_locals_types([] as [ValType; 0]);
    let argument = MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    };
    body.instructions()
        .i32_const(0)
        .call(0)
        .i32_store(argument)
        .end();
    let mut code = CodeSection::new();
    code.function(&body);
    let mut module = wasm_encoder::Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&exports)
        .section(&code);
    module.finish()
}

/// Returns a new engine whose host context counts the calls of its host function.
fn new_engine() -> BrowserEngine<i32> {
    let Ok(engine) = BrowserEngine::new((), i32::default());
    engine
}

/// Instantiates the storing module with the memory and a host function that gives `STORED`, runs `keep`, and returns
/// the value at the start of the memory.
fn store(engine: &mut BrowserEngine<i32>, module: &Module, memory: &BrowserMemory) -> i32 {
    let hook = FunctionTypeBinder::<(), i32>::bind_host_function(engine, |calls: &mut i32, ()| {
        *calls += 1;
        STORED
    });
    let imports = [
        Import {
            module_name: IMPORTS,
            item_name: SHARED_MEMORY,
            address: ExternalAddress::Memory(memory.clone()),
        },
        Import {
            module_name: IMPORTS,
            item_name: CALLBACK,
            address: ExternalAddress::Function(hook),
        },
    ];
    let instance = engine
        .instantiate(&receive_module(module), &imports)
        .expect("the module instantiates");
    let Some(ExternalAddress::Function(function)) = engine.find_export(&instance, KEEP) else {
        panic!("the module exports store");
    };
    FunctionTypeBinder::<(), ()>::bind_engine_function(engine, function)
        .expect("store has no values")
        .invoke(engine, ())
        .expect("store runs");
    assert_eq!(*engine.host_context(), 1);
    let mut read = [u8::MIN; size_of::<i32>()];
    engine
        .read_memory(memory, 0, &mut read)
        .expect("the bytes lie inside the memory");
    i32::from_le_bytes(read)
}

/// Runs in the other worker: makes the posted module and memory known to a new engine there and stores with them.
#[wasm_bindgen(js_name = storeInThisWorker)]
pub fn store_in_this_worker(module: Module, memory: Memory) -> i32 {
    let mut engine = new_engine();
    let shared = receive_shared_memory(&memory).expect("the memory is shared");
    let added = engine.add_shared_memory(&shared);
    store(&mut engine, &module, &added)
}

#[wasm_bindgen_test]
async fn an_engine_in_another_worker_uses_the_module_and_the_shared_memory() {
    let mut engine = new_engine();
    let module = engine
        .compile(&storing_module())
        .expect("the module compiles");
    let memory = engine
        .allocate_memory(EngineMemoryType {
            address_type: AddressType::I32,
            limits: Limits {
                minimum: 1,
                maximum: Some(1),
            },
            sharing: Sharing::Shared,
        })
        .expect("the memory");
    let shared = engine
        .to_shared_memory(&memory)
        .expect("the memory is shared");
    let posted_module = module_to_post(&module).expect("this worker knows the module");
    let posted_memory = shared_memory_to_post(&shared).expect("this worker knows the memory");
    let read_there = JsFuture::from(store_in_another_worker(&posted_module, &posted_memory))
        .await
        .expect("the other worker stored");
    assert_eq!(read_there.as_f64(), Some(f64::from(STORED)));
    let mut read = [u8::MIN; size_of::<i32>()];
    engine
        .read_memory(&memory, 0, &mut read)
        .expect("the bytes lie inside the memory");
    assert_eq!(read, STORED.to_le_bytes());

    let mut receiving = new_engine();
    let received = receive_shared_memory(&posted_memory).expect("the memory is shared");
    let added = receiving.add_shared_memory(&received);
    assert_eq!(store(&mut receiving, &posted_module, &added), STORED);
}
