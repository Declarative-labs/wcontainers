//! A module and a shared memory made by an engine in one worker, posted to another worker that uses them.
#![cfg(target_arch = "wasm32")]

use js_sys::Promise;
use js_sys::WebAssembly::{Memory, Module};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, FunctionSection, ImportSection, MemArg,
    MemArgTarget, MemoryType, TypeSection, ValType,
};
use wcontainers_webassembly_engine::memory::{
    MemoryStore, MemoryType as EngineMemoryType, Sharing,
};
use wcontainers_webassembly_engine::{AddressType, Import, Limits, WebAssemblyEngine};
use wcontainers_webassembly_engine_browser::{
    BrowserEngine, module_to_post, receive_module, receive_shared_memory, shared_memory_to_post,
};

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

#[wasm_bindgen(inline_js = r#"
export function storeInAnotherWorker(module, memory) {
  const source = "onmessage = ({ data: [module, memory] }) => {"
    + " new WebAssembly.Instance(module, { env: { memory } }).exports.store(); postMessage(true); };";
  const worker = new Worker(URL.createObjectURL(new Blob([source], { type: "text/javascript" })));
  return new Promise((resolve, reject) => {
    worker.onmessage = () => { worker.terminate(); resolve(); };
    worker.onerror = (error) => reject(error.message);
    worker.postMessage([module, memory]);
  });
}
"#)]
extern "C" {
    /// Posts the module and the memory to a new worker, which instantiates the module with the memory and calls its
    /// export `store`.
    #[wasm_bindgen(js_name = storeInAnotherWorker)]
    fn store_in_another_worker(module: &Module, memory: &Memory) -> Promise;
}

/// The value `store` writes at the start of the memory.
const STORED: i32 = i32::MAX;

/// A module that imports a shared memory of one page as `env.memory` and exports `store`, which writes `STORED`.
fn storing_module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([] as [ValType; 0], []);
    let mut imports = ImportSection::new();
    let memory = MemoryType {
        minimum: 1,
        maximum: Some(1),
        memory64: false,
        shared: true,
        page_size_log2: None,
    };
    imports.import("env", "memory", EntityType::Memory(memory));
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export("store", ExportKind::Func, 0);
    let mut body = wasm_encoder::Function::new_with_locals_types([] as [ValType; 0]);
    let argument = MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    };
    body.instructions()
        .i32_const(0)
        .i32_const(STORED)
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

fn new_engine() -> BrowserEngine<i32> {
    let Ok(engine) = BrowserEngine::new((), i32::default());
    engine
}

#[wasm_bindgen_test]
async fn another_worker_uses_the_module_and_the_shared_memory() {
    let mut engine = new_engine();
    let module = engine
        .compile(&storing_module())
        .expect("the module compiles");
    let limits = Limits {
        minimum: 1,
        maximum: Some(1),
    };
    let memory = engine
        .allocate_memory(EngineMemoryType {
            address_type: AddressType::I32,
            limits,
            sharing: Sharing::Shared,
        })
        .expect("the memory");
    let shared = engine
        .to_shared_memory(&memory)
        .expect("the memory is shared");
    let posted_module = module_to_post(&module).expect("this worker knows the module");
    let posted_memory = shared_memory_to_post(&shared).expect("this worker knows the memory");
    JsFuture::from(store_in_another_worker(&posted_module, &posted_memory))
        .await
        .expect("the other worker stored");
    let mut read = [u8::MIN; size_of::<i32>()];
    engine
        .read_memory(&memory, 0, &mut read)
        .expect("the bytes lie inside the memory");
    assert_eq!(read, STORED.to_le_bytes());

    let mut receiving = new_engine();
    let received = receive_shared_memory(&posted_memory).expect("the memory is shared");
    let added = receiving.add_shared_memory(&received);
    let imports = [Import {
        module_name: "env",
        item_name: "memory",
        address: wcontainers_webassembly_engine::ExternalAddress::Memory(added.clone()),
    }];
    assert!(
        receiving
            .instantiate(&receive_module(&posted_module), &imports)
            .is_ok()
    );
    let mut read = [u8::MIN; size_of::<i32>()];
    receiving
        .read_memory(&added, 0, &mut read)
        .expect("the bytes lie inside the memory");
    assert_eq!(read, STORED.to_le_bytes());
}
