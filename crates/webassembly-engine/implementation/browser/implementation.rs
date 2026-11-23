//! The WebAssembly engine interface run by the browser's own engine, through the WebAssembly JavaScript API.

mod adapter;
mod function;
mod glue;
mod store;

use core::cell::Cell;
use core::convert::Infallible;
use std::rc::Rc;
use std::sync::Arc;

use js_sys::WebAssembly::{Instance, Memory, Module};
use wasm_bindgen::{JsCast, JsValue};
use wcontainers_webassembly_engine::{ExternalAddress, Import, WebAssemblyEngine};

pub use function::BrowserFunction;
pub use store::{BrowserGlobal, BrowserMemory};

/// An engine of one thread: the host context, which it lends to its host functions while its code runs.
pub struct BrowserEngine<HostContext> {
    context: Option<Box<HostContext>>,
    lent: Rc<Cell<Option<Box<HostContext>>>>,
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

thread_local! {
    /// Telling this worker's registrations from those of other workers, whose objects may be posted to it.
    static WORKER: u64 = (js_sys::Math::random() * (1u64 << (f64::MANTISSA_DIGITS - u32::BITS)) as f64) as u64;
    static REGISTERED: Cell<u32> = const { Cell::new(u32::MIN) };
}

/// Returns a registration id no other worker's registration has, which a JavaScript number holds exactly.
fn new_id(forget: fn(f64)) -> Registration {
    let count = REGISTERED.with(|registered| registered.replace(registered.get().wrapping_add(1)));
    let id = (WORKER.with(|worker| *worker) << u32::BITS | u64::from(count)) as f64;
    Registration { id, forget }
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

/// Lends the host context to the host functions for a call into the engine's code.
fn lend<HostContext>(engine: &mut BrowserEngine<HostContext>) {
    engine.lent.set(engine.context.take());
}

/// Takes the host context back from the host functions after a call into the engine's code.
fn take_back<HostContext>(engine: &mut BrowserEngine<HostContext>) {
    engine.context = engine.lent.take();
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
        lend(self);
        let instance = glue::instantiate(&module, &object);
        take_back(self);
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
