use wcontainers_webassembly_engine::WebAssemblyEngine;

wcontainers_webassembly_engine_conformance::tests!(
    wcontainers_webassembly_engine_wasmtime::WasmtimeEngine::<i32>::new(
        Default::default(),
        i32::default()
    )
    .unwrap_or_else(|never| match never {})
);
