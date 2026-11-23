//! A panic in a host function goes on from the call into the engine's code; alone, since a panic aborts on this
//! target and leaves the thread panicking for every later test.
#![cfg(target_arch = "wasm32")]

mod calling;
mod names;

use calling::{new_engine, run_calling};
use wasm_bindgen_test::wasm_bindgen_test as test;
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder};

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

#[test]
#[should_panic(expected = "a host function panicked in a call into the engine's code")]
fn a_panic_in_a_host_function_goes_on_from_the_call() {
    let mut engine = new_engine(i32::default());
    let hook = FunctionTypeBinder::<(), ()>::bind_host_function(&mut engine, |_: &mut i32, ()| {
        panic!("the host function fails");
    });
    let run = run_calling(&mut engine, hook);
    assert!(run.invoke(&mut engine, ()).is_err());
}
