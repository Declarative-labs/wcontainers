use wcontainers_webassembly_engine::WebAssemblyEngine;

wcontainers_webassembly_engine_conformance::tests!(
    wcontainers_webassembly_engine_wasmedge::WasmEdgeEngine::<i32>::new(
        Default::default(),
        i32::default()
    )
    .expect("WasmEdge makes an executor")
);
