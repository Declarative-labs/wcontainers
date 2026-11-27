#![cfg(target_os = "linux")]

wcontainers_wasmtime_conformance::tests!(wcontainers_wasmtime_direct::DirectWasmtime::default());
