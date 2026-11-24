//! The interface's suite, run in a browser's dedicated worker.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test as test;
use wcontainers_webassembly_engine::WebAssemblyEngine;
use wcontainers_webassembly_engine_browser::BrowserEngine;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

/// Returns a new engine whose host context is zero.
fn engine() -> BrowserEngine<i32> {
    let Ok(engine) = BrowserEngine::new((), i32::default());
    engine
}

wcontainers_webassembly_engine_conformance::tests!(engine());
