//! The WebAssembly engine interface run by the browser's own engine, through the WebAssembly JavaScript API.

mod adapter;
mod declared;
mod function;
mod glue;
mod store;

use core::cell::Cell;
use core::convert::Infallible;
use core::sync::atomic::{AtomicU64, Ordering};
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, ThreadId};

use js_sys::WebAssembly::{Instance, Memory};
use wasm_bindgen::{JsCast, JsValue};
use wcontainers_webassembly_engine::{ExternalAddress, Import, WebAssemblyEngine};

pub use function::BrowserFunction;
pub use store::{BrowserGlobal, BrowserMemory, BrowserTable};

/// An engine of one thread, with its host context and the host functions it keeps on the thread.
pub struct BrowserEngine<HostContext> {
    /// The host context once `host_context` has taken it from `lent`, until the engine's code runs again.
    home: Option<Box<HostContext>>,
    /// The host context where the engine's host functions take it from.
    lent: Rc<Cell<Option<Box<HostContext>>>>,
    /// Tells this engine's host modules from those of other engines.
    number: u64,
    /// The indices of this engine's host functions in its thread's list.
    host_functions: Vec<u32>,
}

impl<HostContext> Drop for BrowserEngine<HostContext> {
    fn drop(&mut self) {
        function::forget(&self.host_functions);
    }
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

/// What a module or a shared memory is known by in the registry of glue.js of the thread that made it.
#[derive(Clone, Copy)]
struct Registered {
    id: f64,
    forget: fn(f64),
    thread: ThreadId,
}

/// A module's or a shared memory's place in a registry of glue.js, which it leaves when dropped.
struct Registration {
    registered: Registered,
}

/// Registrations dropped on a thread other than their own, which their own thread forgets at its next registration.
static DROPPED_ELSEWHERE: Mutex<Vec<Registered>> = Mutex::new(Vec::new());

impl Drop for Registration {
    fn drop(&mut self) {
        let registered = self.registered;
        if thread::current().id() == registered.thread {
            (registered.forget)(registered.id);
        } else {
            DROPPED_ELSEWHERE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(registered);
        }
    }
}

/// Forgets the registrations of this thread that were dropped on another.
fn forget_dropped_elsewhere() {
    let this = thread::current().id();
    DROPPED_ELSEWHERE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|registered| {
            let own = registered.thread == this;
            if own {
                (registered.forget)(registered.id);
            }
            !own
        });
}

/// The last number given to an engine or a registration; in our own memory, so shared by every thread that shares it.
static NUMBERED: AtomicU64 = AtomicU64::new(u64::MIN);

/// Returns a number no engine or registration of any thread that shares our memory has had.
fn new_number() -> u64 {
    NUMBERED.fetch_add(1, Ordering::Relaxed) + 1
}

/// Returns a registration whose id no other has had, which a JavaScript number holds exactly.
fn new_id(forget: fn(f64)) -> Registration {
    forget_dropped_elsewhere();
    let number = new_number();
    assert!(
        number < 1 << f64::MANTISSA_DIGITS,
        "a JavaScript number holds every registration id exactly"
    );
    Registration {
        registered: Registered {
            id: number as f64,
            forget,
            thread: thread::current().id(),
        },
    }
}

/// Returns the value to post to another worker, where `receive_module` makes the module known, or nothing when
/// this worker does not know the module.
pub fn module_to_post(module: &BrowserModule) -> Option<JsValue> {
    Some(glue::module_to_post(module.registration.registered.id))
        .filter(|posted| !posted.is_undefined())
}

/// Makes a module another worker posted known to this worker's engines, or answers nothing when what was posted is
/// not what `module_to_post` returned.
pub fn receive_module(posted: &JsValue) -> Option<BrowserModule> {
    let registration = new_id(glue::forget_module);
    glue::receive_module(registration.registered.id, posted)
        .then_some(BrowserModule { registration })
}

/// Returns the object to post to another worker, where `receive_shared_memory` makes the memory known, or nothing
/// when this worker does not know the memory.
pub fn shared_memory_to_post(memory: &BrowserSharedMemory) -> Option<Memory> {
    glue::registered_shared_memory(memory.registration.registered.id)
}

/// Makes a shared memory another worker posted known to this worker's engines, or answers nothing when the memory is
/// not shared.
pub fn receive_shared_memory(posted: &Memory) -> Option<BrowserSharedMemory> {
    glue::is_shared(posted).then(|| {
        let registration = new_id(glue::forget_shared_memory);
        glue::register_shared_memory(registration.registered.id, posted);
        BrowserSharedMemory {
            registration: Arc::new(registration),
            memory64: store::memory64(&store::memory_of(posted.clone(), None)),
        }
    })
}

impl<HostContext> BrowserEngine<HostContext> {
    /// Runs the engine's code: its host functions run only inside, and find the host context where they take it from.
    #[inline(always)]
    fn run<Returned>(&mut self, code: impl FnOnce() -> Returned) -> Returned {
        if self.home.is_some() {
            self.lent.set(self.home.take());
        }
        let outer = function::run(self.number);
        let returned = code();
        function::run(outer);
        returned
    }

    /// Checks, after the engine's code did not return, that its host functions gave the host context back: a host
    /// function that panicked kept it, and its panic goes on from here, as it would if the call could unwind.
    #[cold]
    fn check_context_returned(&self) {
        let context = self.lent.take();
        assert!(
            context.is_some(),
            "a host function panicked in a call into the engine's code"
        );
        self.lent.set(context);
    }
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
            home: None,
            lent: Rc::new(Cell::new(Some(Box::new(host_context)))),
            number: new_number(),
            host_functions: Vec::new(),
        })
    }

    type CompileError = JsValue;
    fn compile(&self, bytes: &[u8]) -> Result<BrowserModule, JsValue> {
        let module = glue::compile(bytes)?;
        let export_types = declared::export_types(bytes).map_err(|error| {
            js_sys::Error::new(&format!(
                "the types of the module's exports cannot be read: {error}"
            ))
        })?;
        let registration = new_id(glue::forget_module);
        glue::register_module(registration.registered.id, &module, &export_types);
        Ok(BrowserModule { registration })
    }

    type InstantiateError = JsValue;
    fn instantiate(
        &mut self,
        module: &BrowserModule,
        imports: &[Import<'_, Self>],
    ) -> Result<Instance, JsValue> {
        let object = glue::new_imports();
        for import in imports {
            let value: &JsValue = match &import.address {
                ExternalAddress::Function(function) => function,
                ExternalAddress::Table(table) => store::table_object(table),
                ExternalAddress::Memory(memory) => store::memory_object(memory),
                ExternalAddress::Global(global) => store::global_object(global),
                ExternalAddress::Tag(tag) => tag,
            };
            glue::add_import(&object, import.module_name, import.item_name, value);
        }
        let id = module.registration.registered.id;
        let instance = self.run(|| glue::instantiate_registered(id, &object));
        if instance.is_err() {
            self.check_context_returned();
        }
        instance
    }

    fn find_export(&mut self, instance: &Instance, name: &str) -> Option<ExternalAddress<Self>> {
        let value = glue::instance_export(instance, name);
        Some(if value.is_undefined() {
            return None;
        } else if glue::is_function(&value) {
            ExternalAddress::Function(value.unchecked_into())
        } else if glue::is_table(&value) {
            let functions = declared::table_refers_to_functions(&glue::export_type(instance, name));
            ExternalAddress::Table(store::table_of(value.unchecked_into(), functions))
        } else if glue::is_memory(&value) {
            ExternalAddress::Memory(store::memory_of(value.unchecked_into(), None))
        } else if glue::is_global(&value) {
            let kind = declared::exported_global_kind(&glue::export_type(instance, name));
            ExternalAddress::Global(store::global_of(value.unchecked_into(), kind))
        } else {
            ExternalAddress::Tag(value)
        })
    }

    fn host_context(&mut self) -> &mut HostContext {
        if self.home.is_none() {
            self.home = self.lent.take();
        }
        self.home
            .as_mut()
            .expect("the host context is where the engine's host functions take it from whenever none of its code runs")
    }
}
