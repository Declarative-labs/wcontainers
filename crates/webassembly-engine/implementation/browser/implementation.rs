//! The WebAssembly engine interface run by the browser's own engine, through the WebAssembly JavaScript API.

mod adapter;
mod function;
mod glue;
mod store;

use core::cell::Cell;
use core::convert::Infallible;
use core::sync::atomic::{AtomicU64, Ordering};
use std::rc::Rc;
use std::sync::Arc;

use js_sys::WebAssembly::{Instance, Memory, Module};
use wasm_bindgen::{JsCast, JsValue};
use wcontainers_webassembly_engine::{ExternalAddress, Import, WebAssemblyEngine};

use function::{HostFunction, Running};

pub use function::BrowserFunction;
pub use store::{BrowserGlobal, BrowserMemory};

/// An engine of one thread: the host context and the host functions, which it lends to the thread while its code runs.
pub struct BrowserEngine<HostContext> {
    context: Option<Box<HostContext>>,
    lent: Rc<Cell<Option<Box<HostContext>>>>,
    /// Tells this engine's host modules from those of other engines.
    number: u64,
    host_functions: Vec<HostFunction>,
}

/// A compiled module, which instantiates in any engine of a worker it was compiled in or posted to.
pub struct BrowserModule {
    registration: Registration,
}

/// A shared memory, which any engine of a worker it was made in or posted to can add.
#[derive(Clone)]
pub struct BrowserSharedMemory {
    registration: Arc<Registration>,
    memory64: bool,
}

/// What a module or a shared memory is known by in the registries of glue.js, which it leaves when dropped.
struct Registration {
    id: f64,
    forget: fn(f64),
}

impl Drop for Registration {
    fn drop(&mut self) {
        (self.forget)(self.id);
    }
}

/// The last number given to an engine or a registration; in our own memory, so shared by every thread that shares it.
static NUMBERED: AtomicU64 = AtomicU64::new(u64::MIN);

/// Returns a number no engine or registration of any thread that shares our memory has had.
fn new_number() -> u64 {
    NUMBERED.fetch_add(1, Ordering::Relaxed) + 1
}

/// Returns a registration whose id no other has had, which a JavaScript number holds exactly.
fn new_id(forget: fn(f64)) -> Registration {
    let number = new_number();
    assert!(
        number < 1 << f64::MANTISSA_DIGITS,
        "a JavaScript number holds every registration id exactly"
    );
    Registration {
        id: number as f64,
        forget,
    }
}

/// Returns the object to post to another worker, where `receive_module` makes the module known, or nothing when
/// this worker does not know the module.
pub fn module_to_post(module: &BrowserModule) -> Option<Module> {
    glue::registered_module(module.registration.id)
}

/// Makes a module another worker posted known to this worker's engines.
pub fn receive_module(posted: &Module) -> BrowserModule {
    let registration = new_id(glue::forget_module);
    glue::register_module(registration.id, posted);
    BrowserModule { registration }
}

/// Returns the object to post to another worker, where `receive_shared_memory` makes the memory known, or nothing
/// when this worker does not know the memory.
pub fn shared_memory_to_post(memory: &BrowserSharedMemory) -> Option<Memory> {
    glue::registered_shared_memory(memory.registration.id)
}

/// Makes a shared memory another worker posted known to this worker's engines, or answers nothing when the memory is
/// not shared.
pub fn receive_shared_memory(posted: &Memory) -> Option<BrowserSharedMemory> {
    glue::is_shared(posted).then(|| {
        let registration = new_id(glue::forget_shared_memory);
        glue::register_shared_memory(registration.id, posted);
        BrowserSharedMemory {
            registration: Arc::new(registration),
            memory64: store::memory64(&store::memory_of(posted.clone(), None)),
        }
    })
}

/// Lends the host context and the host functions to the thread for a call into the engine's code, returning the host
/// functions of the call it is made inside, which `take_back` gives back.
fn lend<HostContext>(engine: &mut BrowserEngine<HostContext>) -> Running {
    engine.lent.set(engine.context.take());
    function::run(Running::new(
        engine.number,
        core::mem::take(&mut engine.host_functions),
    ))
}

/// Takes the host context and the host functions back after a call into the engine's code; a host function that
/// panicked kept them, and its panic goes on from here, as it would have if the call could unwind.
fn take_back<HostContext>(engine: &mut BrowserEngine<HostContext>, outer: Running) {
    let running = function::run(outer);
    engine.context = engine.lent.take();
    assert!(
        running.is(engine.number) && engine.context.is_some(),
        "a host function panicked in a call into the engine's code"
    );
    engine.host_functions = running.into_functions();
}

impl<HostContext> WebAssemblyEngine for BrowserEngine<HostContext> {
    type Config = ();
    type Module = BrowserModule;
    type Instance = Instance;
    type TagAddress = JsValue;
    type HostContext = HostContext;

    type NewError = Infallible;
    fn new(_: (), host_context: HostContext) -> Result<Self, Infallible> {
        Ok(Self {
            context: Some(Box::new(host_context)),
            lent: Rc::default(),
            number: new_number(),
            host_functions: Vec::new(),
        })
    }

    type CompileError = JsValue;
    fn compile(&self, bytes: &[u8]) -> Result<BrowserModule, JsValue> {
        let module = glue::compile(bytes)?;
        let registration = new_id(glue::forget_module);
        glue::register_module(registration.id, &module);
        Ok(BrowserModule { registration })
    }

    type InstantiateError = JsValue;
    fn instantiate(
        &mut self,
        module: &BrowserModule,
        imports: &[Import<'_, Self>],
    ) -> Result<Instance, JsValue> {
        let module = glue::registered_module(module.registration.id).ok_or_else(|| {
            js_sys::Error::new(
                "the module was compiled in another worker and not posted to this one",
            )
        })?;
        let object = glue::new_imports();
        for import in imports {
            let value: &JsValue = match &import.address {
                ExternalAddress::Function(function) => function,
                ExternalAddress::Table(table) => table,
                ExternalAddress::Memory(memory) => store::memory_object(memory),
                ExternalAddress::Global(global) => store::global_object(global),
                ExternalAddress::Tag(tag) => tag,
            };
            glue::add_import(&object, import.module_name, import.item_name, value);
        }
        let outer = lend(self);
        let instance = glue::instantiate(&module, &object);
        take_back(self, outer);
        instance
    }

    fn find_export(&mut self, instance: &Instance, name: &str) -> Option<ExternalAddress<Self>> {
        let value = glue::instance_export(instance, name);
        Some(if value.is_undefined() {
            return None;
        } else if glue::is_function(&value) {
            ExternalAddress::Function(value.unchecked_into())
        } else if glue::is_table(&value) {
            ExternalAddress::Table(value.unchecked_into())
        } else if glue::is_memory(&value) {
            ExternalAddress::Memory(store::memory_of(value.unchecked_into(), None))
        } else if glue::is_global(&value) {
            ExternalAddress::Global(store::global_of(value.unchecked_into()))
        } else {
            ExternalAddress::Tag(value)
        })
    }

    fn host_context(&mut self) -> &mut HostContext {
        self.context
            .as_mut()
            .expect("the engine holds its host context whenever none of its code runs")
    }
}
