//! Times the operations of a WebAssembly engine through the interface's traits alone.
//!
//! `cargo bench -p wcontainers-webassembly-engine-wasmedge`; the environment variable
//! `ENGINE_BENCHMARK_SCALE` multiplies every number of repetitions (default 1).

use std::hint::black_box;
use std::time::{Duration, Instant};

use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    Module, TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder};
use wcontainers_webassembly_engine::global::Value;
use wcontainers_webassembly_engine::memory::{MemoryType, Sharing};
use wcontainers_webassembly_engine::table::TableType;
use wcontainers_webassembly_engine::{
    AddressType, ExternalAddress, Import, Limits, WebAssemblyEngine,
};

/// A module that imports `env.hook: [] -> []` and exports `empty: [] -> []`, which does nothing,
/// and `repeat: [i32] -> []`, which calls the hook that many times.
fn bench_module() -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    types.ty().function([ValType::I32], []);
    let mut imports = ImportSection::new();
    imports.import("host", "upcall", EntityType::Function(0));
    let mut functions = FunctionSection::new();
    functions.function(0).function(1);
    let mut exports = ExportSection::new();
    exports
        .export("empty", ExportKind::Func, 1)
        .export("repeat", ExportKind::Func, 2);
    let mut empty = Function::new([]);
    empty.instructions().end();
    let mut repeat = Function::new([]);
    repeat
        .instructions()
        .block(wasm_encoder::BlockType::Empty)
        .local_get(0)
        .i32_eqz()
        .br_if(0)
        .loop_(wasm_encoder::BlockType::Empty)
        .call(0)
        .local_get(0)
        .i32_const(1)
        .i32_sub()
        .local_tee(0)
        .br_if(0)
        .end()
        .end()
        .end();
    let mut code = CodeSection::new();
    code.function(&empty).function(&repeat);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&exports)
        .section(&code);
    module.finish()
}

fn scale() -> u32 {
    std::env::var("ENGINE_BENCHMARK_SCALE")
        .ok()
        .and_then(|scale| scale.parse().ok())
        .unwrap_or(1)
}

/// Prints the time per repetition of `run`, repeated as many times as the scale asks.
fn report(name: &str, repetitions: u32, run: impl FnMut()) {
    time(name, repetitions * scale(), run);
}

/// Prints the time per repetition of `run`, the best of five rounds.
fn time(name: &str, repetitions: u32, mut run: impl FnMut()) {
    let best = (0..5)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..repetitions {
                run();
            }
            start.elapsed()
        })
        .min()
        .unwrap_or(Duration::ZERO);
    let each = best.as_secs_f64() * 1e9 / f64::from(repetitions);
    println!("{name}: {each:.1} ns ({repetitions} repetitions, best of 5 rounds)");
}

/// Times every operation the status of an engine track reports.
pub fn measure<T>(config: T::Config)
where
    T: WebAssemblyEngine<HostContext = u64>
        + FunctionTypeBinder<(), ()>
        + FunctionTypeBinder<(i32,), ()>,
{
    let mut engine = T::new(config, 0).ok().expect("the engine is made");
    let bytes = bench_module();
    report("compile the benchmark module", 200, || {
        black_box(engine.compile(black_box(&bytes)).ok().expect("compiles"));
    });
    let module = engine.compile(&bytes).ok().expect("compiles");
    let hook = <T as FunctionTypeBinder<(), ()>>::bind_host_function(
        &mut engine,
        |count: &mut u64, ()| *count += 1,
    );
    let imports = [Import {
        module_name: "host",
        item_name: "upcall",
        address: ExternalAddress::Function(hook),
    }];
    // Not scaled: instances live as long as the engine value, and an engine may cap how many one holds
    // (wasmtime's store holds at most 10000).
    time("instantiate it", 1_000, || {
        black_box(
            engine
                .instantiate(&module, &imports)
                .ok()
                .expect("instantiates"),
        );
    });
    let instance = engine
        .instantiate(&module, &imports)
        .ok()
        .expect("instantiates");
    let Some(ExternalAddress::Function(empty)) = engine.find_export(&instance, "empty") else {
        panic!("empty is exported");
    };
    let Some(ExternalAddress::Function(repeat)) = engine.find_export(&instance, "repeat") else {
        panic!("repeat is exported");
    };
    let empty = <T as FunctionTypeBinder<(), ()>>::bind_engine_function(&mut engine, empty.clone())
        .expect("binds");
    let repeat = <T as FunctionTypeBinder<(i32,), ()>>::bind_engine_function(&mut engine, repeat)
        .expect("binds");
    report("an empty call into the module", 10_000_000, || {
        empty.invoke(&mut engine, ()).ok().expect("returns");
    });
    let calls_per_repetition = 1_000;
    let host_calls = 10_000;
    report(
        "1000 calls of an empty host function from the module",
        host_calls,
        || {
            repeat
                .invoke(&mut engine, (black_box(calls_per_repetition),))
                .ok()
                .expect("returns");
        },
    );
    assert!(*engine.host_context() > 0);
    for sharing in [Sharing::Unshared, Sharing::Shared] {
        let memory = engine
            .allocate_memory(MemoryType {
                address_type: AddressType::I32,
                limits: Limits {
                    minimum: 2,
                    maximum: Some(2),
                },
                sharing,
            })
            .ok()
            .expect("the memory");
        for (size, repetitions) in [(64, 10_000_000), (64 * 1024, 100_000)] {
            let mut bytes = vec![0x5a_u8; size];
            report(
                &format!("write {size} bytes of a {sharing:?} memory"),
                repetitions,
                || {
                    engine
                        .write_memory(&memory, black_box(64), black_box(&bytes))
                        .expect("writes");
                },
            );
            report(
                &format!("read {size} bytes of a {sharing:?} memory"),
                repetitions,
                || {
                    engine
                        .read_memory(&memory, black_box(64), black_box(&mut bytes))
                        .expect("reads");
                },
            );
        }
    }
    let table = engine
        .allocate_table(TableType {
            address_type: AddressType::I32,
            limits: Limits {
                minimum: 1,
                maximum: Some(1),
            },
        })
        .ok()
        .expect("the table");
    let Some(ExternalAddress::Function(function)) = engine.find_export(&instance, "empty") else {
        panic!("empty is exported");
    };
    report("write a function into a table", 10_000_000, || {
        engine
            .write_table(&table, black_box(0), Some(&function))
            .expect("writes");
    });
}

/// Times the operations that reach an item through a helper module when the engine's own API cannot: a global and
/// an unshared memory of an instance that exports a shared memory, and a table of typed function references.
pub fn measure_helpers<T>(config: T::Config)
where
    T: WebAssemblyEngine<HostContext = u64>,
{
    let mut engine = T::new(config, 0).ok().expect("the engine is made");
    let bytes = wat::parse_str(
        r#"(module
            (type $answers (func (result i32)))
            (memory (export "shared") 1 1 shared)
            (memory (export "unshared") 1)
            (global (export "variable") (mut i32) (i32.const 0))
            (table (export "typed") 1 (ref null $answers))
            (func (export "forty_two") (type $answers) (i32.const 3)))"#,
    )
    .expect("the text is a module");
    let module = engine.compile(&bytes).ok().expect("compiles");
    let instance = engine.instantiate(&module, &[]).ok().expect("instantiates");
    let Some(ExternalAddress::Global(global)) = engine.find_export(&instance, "variable") else {
        panic!("global is exported");
    };
    report("write a global beside a shared memory", 1_000_000, || {
        engine
            .write_global(&global, black_box(Value::I32(1)))
            .expect("writes");
    });
    let Some(ExternalAddress::Memory(memory)) = engine.find_export(&instance, "unshared") else {
        panic!("unshared is exported");
    };
    let mut bytes = vec![0x5a_u8; 64];
    report(
        "write 64 bytes of an unshared memory beside a shared one",
        1_000_000,
        || {
            engine
                .write_memory(&memory, black_box(64), black_box(&bytes))
                .expect("writes");
        },
    );
    report(
        "read 64 bytes of an unshared memory beside a shared one",
        1_000_000,
        || {
            engine
                .read_memory(&memory, black_box(64), black_box(&mut bytes))
                .expect("reads");
        },
    );
    let Some(ExternalAddress::Table(table)) = engine.find_export(&instance, "typed") else {
        panic!("typed is exported");
    };
    let Some(ExternalAddress::Function(function)) = engine.find_export(&instance, "forty_two")
    else {
        panic!("answer is exported");
    };
    report("write a function into a typed table", 1_000_000, || {
        engine
            .write_table(&table, black_box(0), Some(&function))
            .expect("writes");
    });
}

fn main() {
    measure::<wcontainers_webassembly_engine_wasmedge::WasmEdgeEngine<u64>>(Default::default());
    measure_helpers::<wcontainers_webassembly_engine_wasmedge::WasmEdgeEngine<u64>>(
        Default::default(),
    );
}
