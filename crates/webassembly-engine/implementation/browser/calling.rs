//! A module that calls a host function, for the tests of host functions.

use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, FunctionSection, ImportSection, Module,
    TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::FunctionTypeBinder;
use wcontainers_webassembly_engine::{ExternalAddress, Import, WebAssemblyEngine};
use wcontainers_webassembly_engine_browser::{BrowserEngine, BrowserFunction};

use crate::names::{CALLBACK, IMPORTS};

const RUN_CALLBACK: &str = "run_callback";
pub const NO_VALUES: [ValType; 0] = [];

/// A module that imports `callback` and exports `run_callback`, which calls it.
fn calling_module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function(NO_VALUES, NO_VALUES);
    let mut imports = ImportSection::new();
    imports.import(IMPORTS, CALLBACK, EntityType::Function(0));
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export(RUN_CALLBACK, ExportKind::Func, 1);
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

pub fn new_engine<HostContext: 'static>(context: HostContext) -> BrowserEngine<HostContext> {
    let Ok(engine) = BrowserEngine::new((), context);
    engine
}

/// Instantiates the calling module with the host function and binds its `run_callback`.
pub fn run_calling<HostContext: 'static>(
    engine: &mut BrowserEngine<HostContext>,
    hook: js_sys::Function,
) -> BrowserFunction<(), ()> {
    let module = engine
        .compile(&calling_module())
        .expect("the module compiles");
    let imports = [Import {
        module_name: IMPORTS,
        item_name: CALLBACK,
        address: ExternalAddress::Function(hook),
    }];
    let instance = engine
        .instantiate(&module, &imports)
        .expect("the module instantiates");
    let Some(ExternalAddress::Function(run)) = engine.find_export(&instance, RUN_CALLBACK) else {
        panic!("the module exports run");
    };
    FunctionTypeBinder::<(), ()>::bind_engine_function(engine, run).expect("run has no values")
}
