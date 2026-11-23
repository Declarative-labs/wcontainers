//! The functions of glue.js, each one call of the WebAssembly JavaScript API.

use js_sys::Function;
use js_sys::WebAssembly::{Global, Instance, Memory, Module, Table};
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(module = "/glue.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    pub fn compile(bytes: &[u8]) -> Result<Module, JsValue>;
    #[wasm_bindgen(js_name = newExportTypes)]
    pub fn new_export_types() -> JsValue;
    #[wasm_bindgen(js_name = keepExportType)]
    pub fn keep_export_type(types: &JsValue, name: &str, kept: &JsValue);
    #[wasm_bindgen(js_name = registerModule)]
    pub fn register_module(id: f64, module: &Module, types: &JsValue);
    #[wasm_bindgen(js_name = moduleToPost)]
    pub fn module_to_post(id: f64) -> JsValue;
    #[wasm_bindgen(js_name = receiveModule)]
    pub fn receive_module(id: f64, posted: &JsValue) -> bool;
    #[wasm_bindgen(js_name = forgetModule)]
    pub fn forget_module(id: f64);
    #[wasm_bindgen(js_name = registerSharedMemory)]
    pub fn register_shared_memory(id: f64, memory: &Memory);
    #[wasm_bindgen(js_name = registeredSharedMemory)]
    pub fn registered_shared_memory(id: f64) -> Option<Memory>;
    #[wasm_bindgen(js_name = forgetSharedMemory)]
    pub fn forget_shared_memory(id: f64);

    #[wasm_bindgen(js_name = newImports)]
    pub fn new_imports() -> JsValue;
    #[wasm_bindgen(js_name = addImport)]
    pub fn add_import(imports: &JsValue, module_name: &str, item_name: &str, value: &JsValue);
    #[wasm_bindgen(catch)]
    pub fn instantiate(module: &Module, imports: &JsValue) -> Result<Instance, JsValue>;
    #[wasm_bindgen(js_name = instantiateRegistered, catch)]
    pub fn instantiate_registered(id: f64, imports: &JsValue) -> Result<Instance, JsValue>;
    #[wasm_bindgen(js_name = instanceExports)]
    pub fn instance_export(instance: &Instance, name: &str) -> JsValue;
    #[wasm_bindgen(js_name = exportType)]
    pub fn export_type(instance: &Instance, name: &str) -> JsValue;
    #[wasm_bindgen(js_name = isFunction)]
    pub fn is_function(value: &JsValue) -> bool;
    #[wasm_bindgen(js_name = isTable)]
    pub fn is_table(value: &JsValue) -> bool;
    #[wasm_bindgen(js_name = isMemory)]
    pub fn is_memory(value: &JsValue) -> bool;
    #[wasm_bindgen(js_name = isGlobal)]
    pub fn is_global(value: &JsValue) -> bool;

    pub fn call(callee: &Function) -> bool;
    #[wasm_bindgen(js_name = takeThrown)]
    pub fn take_thrown() -> JsValue;
    #[wasm_bindgen(js_name = isException)]
    pub fn is_exception(error: &JsValue) -> bool;
    #[wasm_bindgen(js_name = instantiateWithTwo, catch)]
    pub fn instantiate_with_two(
        module: &Module,
        namespace: &str,
        name: &str,
        value: &JsValue,
        other_name: &str,
        other_value: &JsValue,
    ) -> Result<Instance, JsValue>;

    #[wasm_bindgen(js_name = newMemory, catch)]
    pub fn new_memory(
        initial: f64,
        maximum: Option<f64>,
        shared: bool,
        address64: bool,
    ) -> Result<Memory, JsValue>;
    #[wasm_bindgen(js_name = memoryBytes)]
    pub fn memory_bytes(memory: &Memory) -> f64;
    #[wasm_bindgen(js_name = growMemory)]
    pub fn grow_memory(memory: &Memory, pages: f64, address64: bool) -> bool;
    #[wasm_bindgen(js_name = isShared)]
    pub fn is_shared(memory: &Memory) -> bool;

    #[wasm_bindgen(js_name = newTable, catch)]
    pub fn new_table(initial: f64, maximum: Option<f64>, address64: bool)
    -> Result<Table, JsValue>;
    #[wasm_bindgen(js_name = tableLength)]
    pub fn table_length(table: &Table) -> f64;
    #[wasm_bindgen(js_name = isTable64)]
    pub fn is_table64(table: &Table) -> bool;
    #[wasm_bindgen(js_name = growTable)]
    pub fn grow_table(table: &Table, elements: f64, address64: bool) -> bool;
    #[wasm_bindgen(js_name = tableElement)]
    pub fn table_element(table: &Table, index: f64, address64: bool) -> JsValue;
    #[wasm_bindgen(js_name = setTableElement)]
    pub fn set_table_element(table: &Table, index: f64, value: &JsValue, address64: bool) -> bool;

    #[wasm_bindgen(js_name = newI32Global, catch)]
    pub fn new_i32_global(mutable: bool, value: i32) -> Result<Global, JsValue>;
    #[wasm_bindgen(js_name = newI64Global, catch)]
    pub fn new_i64_global(mutable: bool, value: i64) -> Result<Global, JsValue>;
    #[wasm_bindgen(js_name = newFunctionGlobal, catch)]
    pub fn new_function_global(mutable: bool, value: &JsValue) -> Result<Global, JsValue>;
    #[wasm_bindgen(js_name = numberValue)]
    pub fn number_value(global: &Global) -> f64;
    #[wasm_bindgen(js_name = bigIntValue)]
    pub fn big_int_value(global: &Global) -> i64;
    #[wasm_bindgen(js_name = referenceValue)]
    pub fn reference_value(global: &Global) -> JsValue;
    #[wasm_bindgen(js_name = setNumberValue)]
    pub fn set_number_value(global: &Global, value: f64);
    #[wasm_bindgen(js_name = setBigIntValue)]
    pub fn set_big_int_value(global: &Global, value: i64);
    #[wasm_bindgen(js_name = setReferenceValue)]
    pub fn set_reference_value(global: &Global, value: &JsValue) -> bool;

}
