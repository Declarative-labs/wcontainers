//! Functions bound to Rust signatures: each call moves its values through the scratch slots and a small module.

use core::cell::{Cell, RefCell};
use core::marker::PhantomData;
use std::rc::Rc;

use js_sys::Function;
use js_sys::WebAssembly::Table;
use wasm_bindgen::{JsCast, JsValue};
use wasm_encoder::ValType;
use wcontainers_webassembly_engine::function::{
    BindFunctionError, BoundFunction, FunctionStore, FunctionTypeBinder, InvokeError,
};

use crate::adapter::{
    self, CALLEE, ENTRY, NAMESPACE, SCRATCH_MEMORY, from_slot, result_types, set_slot, slot,
    to_slot, value_type,
};
use crate::{BrowserEngine, glue};

/// A function of the engine bound to a Rust signature: the export of a caller module that imports it.
pub struct BrowserFunction<Parameters, Results> {
    caller: Function,
    signature: PhantomData<fn(Parameters) -> Results>,
}

/// A host function as its thread keeps it: it takes its arguments from the slots and leaves its result there.
type HostFunction = Rc<dyn Fn()>;

/// The host functions of this thread's engines, by the index their host modules pass, and the indices of removed ones.
struct HostFunctions {
    entries: Vec<Option<HostFunction>>,
    free: Vec<u32>,
}

/// The number no engine has, which `RUNNING` holds while no engine's code runs.
const NO_ENGINE: u64 = u64::MIN;

thread_local! {
    /// The number of the engine whose code runs on this thread.
    static RUNNING: Cell<u64> = const { Cell::new(NO_ENGINE) };
    static HOST_FUNCTIONS: RefCell<HostFunctions> =
        const { RefCell::new(HostFunctions { entries: Vec::new(), free: Vec::new() }) };
}

/// Makes the engine with the number the one whose code runs on this thread, returning the one it replaces.
#[inline(always)]
pub fn run(engine: u64) -> u64 {
    RUNNING.replace(engine)
}

/// Keeps the host function for this thread, returning the index its host module passes.
fn keep(function: HostFunction) -> u32 {
    HOST_FUNCTIONS.with_borrow_mut(|functions| match functions.free.pop() {
        Some(index) => {
            functions.entries[index as usize] = Some(function);
            index
        }
        None => {
            functions.entries.push(Some(function));
            u32::try_from(functions.entries.len() - 1)
                .expect("a thread has fewer host functions than a 32-bit index counts")
        }
    })
}

/// Removes the host functions of a dropped engine, dropping each after this thread's list is free again.
pub fn forget(indices: &[u32]) {
    let removed: Vec<Option<HostFunction>> = HOST_FUNCTIONS.with_borrow_mut(|functions| {
        functions.free.extend_from_slice(indices);
        indices
            .iter()
            .map(|index| functions.entries[*index as usize].take())
            .collect()
    });
    drop(removed);
}

/// Runs the host function with the index, which belongs to the engine with the number, whose arguments are in the
/// slots and which leaves its result there.
extern "C" fn run_host_function(engine: u64, index: u32) {
    assert!(
        RUNNING.get() == engine,
        "a host function runs only inside a call into its own engine's code"
    );
    let function = HOST_FUNCTIONS
        .with_borrow(|functions| functions.entries[index as usize].clone())
        .expect("a host function lives as long as its engine");
    function();
}

impl<HostContext> FunctionStore for BrowserEngine<HostContext> {
    type FunctionAddress = Function;
    type Trap = JsValue;
}

/// Returns the value types of a signature, or nothing when one of its types is not a value type.
fn function_types(
    parameters: &[Option<ValType>],
    results: Option<Vec<ValType>>,
) -> Option<(Vec<ValType>, Vec<ValType>)> {
    Some((parameters.iter().copied().collect::<Option<_>>()?, results?))
}

/// Returns the entry of a caller module of the function, which instantiates only when the function has the type.
fn bind_caller(
    function: &Function,
    parameters: &[ValType],
    results: &[ValType],
) -> Result<Function, BindFunctionError> {
    let module =
        glue::compile(&adapter::caller(parameters, results)).expect("a caller module is valid");
    let instance = glue::instantiate_with_two(
        &module,
        NAMESPACE,
        CALLEE,
        function,
        SCRATCH_MEMORY,
        &wasm_bindgen::memory(),
    )
    .map_err(|_| BindFunctionError)?;
    Ok(glue::instance_export(&instance, ENTRY).unchecked_into())
}

/// Returns the entry of a host module that runs the host function, which lives as long as the engine from now on.
fn bind_host<HostContext>(
    engine: &mut BrowserEngine<HostContext>,
    parameters: &[ValType],
    results: &[ValType],
    function: HostFunction,
) -> Function {
    let index = keep(function);
    engine.host_functions.push(index);
    let module = glue::compile(&adapter::host(parameters, results, engine.number, index))
        .expect("a host module is valid");
    let table: Table = wasm_bindgen::function_table().unchecked_into();
    let runner = glue::table_element(
        &table,
        f64::from(run_host_function as extern "C" fn(u64, u32) as usize as u32),
        false,
    );
    let instance = glue::instantiate_with_two(
        &module,
        NAMESPACE,
        CALLEE,
        &runner,
        SCRATCH_MEMORY,
        &wasm_bindgen::memory(),
    )
    .expect("a host module instantiates with the function that runs host functions");
    glue::instance_export(&instance, ENTRY).unchecked_into()
}

/// Returns the result a call left in the slots, or what it threw.
#[inline(always)]
fn finish<HostContext, Results: 'static>(
    engine: &BrowserEngine<HostContext>,
    returned: bool,
) -> Result<Results, InvokeError<JsValue>> {
    if returned {
        Ok(from_slot(slot(0)))
    } else {
        engine.check_context_returned();
        let thrown = glue::take_thrown();
        Err(if glue::is_exception(&thrown) {
            InvokeError::Exception
        } else {
            InvokeError::Trap(thrown)
        })
    }
}

macro_rules! binders {
    ($(($($parameter:ident $argument:ident $index:tt)*))*) => {$(
        impl<HostContext: 'static, $($parameter: 'static,)* Results: 'static>
            FunctionTypeBinder<($($parameter,)*), Results> for BrowserEngine<HostContext>
        {
            type Function = BrowserFunction<($($parameter,)*), Results>;

            fn bind_engine_function(&mut self, address: Function) -> Result<Self::Function, BindFunctionError> {
                let (parameters, results) =
                    function_types(&[$(value_type::<$parameter>()),*], result_types::<Results>())
                        .ok_or(BindFunctionError)?;
                Ok(BrowserFunction {
                    caller: bind_caller(&address, &parameters, &results)?,
                    signature: PhantomData,
                })
            }

            fn bind_host_function(
                &mut self,
                function: impl Fn(&mut HostContext, ($($parameter,)*)) -> Results + Send + Sync + 'static,
            ) -> Function {
                let (parameters, results) =
                    function_types(&[$(value_type::<$parameter>()),*], result_types::<Results>())
                        .expect("a host function takes and returns value types");
                let context = self.lent.clone();
                bind_host(self, &parameters, &results, Rc::new(move || {
                    let arguments = ($(from_slot::<$parameter>(slot($index)),)*);
                    let mut lent = context.take().expect("a host function runs inside a call into its engine's code");
                    let returned = function(&mut *lent, arguments);
                    context.set(Some(lent));
                    set_slot(0, to_slot(returned));
                }))
            }
        }

        impl<HostContext, $($parameter: 'static,)* Results: 'static>
            BoundFunction<BrowserEngine<HostContext>, ($($parameter,)*), Results>
            for BrowserFunction<($($parameter,)*), Results>
        {
            #[inline]
            fn invoke(
                &self,
                engine: &mut BrowserEngine<HostContext>,
                ($($argument,)*): ($($parameter,)*),
            ) -> Result<Results, InvokeError<JsValue>> {
                $(set_slot($index, to_slot($argument));)*
                let returned = engine.run(|| glue::call(&self.caller));
                finish(engine, returned)
            }
        }
    )*};
}

binders! {
    ()
    (A a 0)
    (A a 0 B b 1)
    (A a 0 B b 1 C c 2)
    (A a 0 B b 1 C c 2 D d 3)
    (A a 0 B b 1 C c 2 D d 3 E e 4)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7 I i 8)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7 I i 8 J j 9)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7 I i 8 J j 9 K k 10)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7 I i 8 J j 9 K k 10 L l 11)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7 I i 8 J j 9 K k 10 L l 11 M m 12)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7 I i 8 J j 9 K k 10 L l 11 M m 12 N n 13)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7 I i 8 J j 9 K k 10 L l 11 M m 12 N n 13 O o 14)
    (A a 0 B b 1 C c 2 D d 3 E e 4 F f 5 G g 6 H h 7 I i 8 J j 9 K k 10 L l 11 M m 12 N n 13 O o 14 P p 15)
}
