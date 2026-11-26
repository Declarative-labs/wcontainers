//! Checks of the wasmtime engine that the interface's suite does not make.

use std::sync::Barrier;

use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    Module, TagKind, TagSection, TagType, TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder, InvokeError};
use wcontainers_webassembly_engine::memory::{
    MemoryStore, MemoryType, PAGE_SIZE, ReadMemoryError, Sharing, WriteMemoryError,
};
use wcontainers_webassembly_engine::{
    AddressType, ExternalAddress, Import, Limits, WebAssemblyEngine,
};
use wcontainers_webassembly_engine_wasmtime::{Config, WasmtimeEngine};

type Engine = WasmtimeEngine<i32>;

fn engine(config: Config) -> Engine {
    Engine::new(config, i32::default()).unwrap_or_else(|never| match never {})
}

fn function(engine: &mut Engine, instance: &wasmtime::Instance, name: &str) -> wasmtime::Func {
    match engine.find_export(instance, name) {
        Some(ExternalAddress::Function(function)) => function,
        _ => panic!("{name} is an exported function"),
    }
}

fn bind<Parameters, Results>(
    engine: &mut Engine,
    address: wasmtime::Func,
) -> <Engine as FunctionTypeBinder<Parameters, Results>>::Function
where
    Engine: FunctionTypeBinder<Parameters, Results>,
{
    <Engine as FunctionTypeBinder<Parameters, Results>>::bind_engine_function(engine, address)
        .expect("the function has the function type")
}

fn memory_type(address_type: AddressType, pages: u64, sharing: Sharing) -> MemoryType {
    MemoryType {
        address_type,
        limits: Limits {
            minimum: pages,
            maximum: Some(pages),
        },
        sharing,
    }
}

/// A module that imports a shared memory of `pages` pages as env.memory and exports
/// `fill(offset: i32, value: i32, length: i32)`, which runs `memory.fill`.
fn fills_a_shared_memory(pages: u64) -> Vec<u8> {
    let mut types = TypeSection::new();
    types
        .ty()
        .function([ValType::I32, ValType::I32, ValType::I32], []);
    let mut imports = ImportSection::new();
    imports.import(
        "env",
        "memory",
        wasm_encoder::MemoryType {
            minimum: pages,
            maximum: Some(pages),
            memory64: false,
            shared: true,
            page_size_log2: None,
        },
    );
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export("fill", ExportKind::Func, 0);
    let mut fill = Function::new([]);
    fill.instructions()
        .local_get(0)
        .local_get(1)
        .local_get(2)
        .memory_fill(0)
        .end();
    let mut code = CodeSection::new();
    code.function(&fill);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&exports)
        .section(&code);
    module.finish()
}

#[test]
fn two_threads_share_one_module_and_one_shared_memory() {
    const THREADS: u8 = 4;
    const ROUNDS: u8 = 200;
    let region = usize::try_from(PAGE_SIZE).expect("a page fits") / usize::from(THREADS);
    let config = Config::default();
    let mut main = engine(config.clone());
    let memory = main
        .allocate_memory(memory_type(AddressType::I32, 1, Sharing::Shared))
        .expect("the memory");
    let shared = main
        .to_shared_memory(&memory)
        .expect("the memory is shared");
    let module = main
        .compile(&fills_a_shared_memory(1))
        .expect("the module compiles");
    let barrier = Barrier::new(THREADS.into());
    std::thread::scope(|scope| {
        for thread in 0..THREADS {
            let (config, shared, module, barrier) = (config.clone(), &shared, &module, &barrier);
            scope.spawn(move || {
                let mut engine = engine(config);
                let memory = engine.add_shared_memory(shared);
                let imports = [Import {
                    module_name: "env",
                    item_name: "memory",
                    address: ExternalAddress::Memory(memory.clone()),
                }];
                let instance = engine.instantiate(module, &imports).expect("instantiates");
                let address = function(&mut engine, &instance, "fill");
                let fill = bind::<(i32, i32, i32), ()>(&mut engine, address);
                let start = usize::from(thread) * region;
                let half = region / 2;
                barrier.wait();
                for round in 0..ROUNDS {
                    let value = thread.wrapping_mul(ROUNDS).wrapping_add(round);
                    let offset = i32::try_from(start).expect("fits");
                    let length = i32::try_from(half).expect("fits");
                    fill.invoke(&mut engine, (offset, value.into(), length))
                        .expect("fills");
                    let bytes = vec![value; region - half];
                    engine
                        .write_memory(&memory, (start + half) as u64, &bytes)
                        .expect("writes");
                    let mut read = vec![0; region];
                    engine
                        .read_memory(&memory, start as u64, &mut read)
                        .expect("reads");
                    assert!(read.iter().all(|byte| *byte == value));
                }
            });
        }
    });
    let mut read = vec![0; usize::try_from(PAGE_SIZE).expect("a page fits")];
    main.read_memory(&memory, 0, &mut read).expect("reads");
    for thread in 0..THREADS {
        let last = thread.wrapping_mul(ROUNDS).wrapping_add(ROUNDS - 1);
        let start = usize::from(thread) * region;
        assert!(read[start..start + region].iter().all(|byte| *byte == last));
    }
}

#[test]
fn reads_and_writes_more_than_a_page_of_every_kind_of_memory() {
    for address_type in [AddressType::I32, AddressType::I64] {
        for sharing in [Sharing::Unshared, Sharing::Shared] {
            let mut engine = engine(Config::default());
            let memory = engine
                .allocate_memory(memory_type(address_type, 3, sharing))
                .expect("the memory");
            let bytes: Vec<u8> = (0..2 * PAGE_SIZE + 1).map(|index| index as u8).collect();
            let offset = PAGE_SIZE - 1;
            engine
                .write_memory(&memory, offset, &bytes)
                .expect("writes");
            let mut read = vec![0; bytes.len()];
            engine
                .read_memory(&memory, offset, &mut read)
                .expect("reads");
            assert_eq!(read, bytes);
            let end = 3 * PAGE_SIZE;
            assert_eq!(engine.read_memory(&memory, end, &mut []), Ok(()));
            assert_eq!(
                engine.read_memory(&memory, end - 1, &mut [0; 2]),
                Err(ReadMemoryError)
            );
            assert_eq!(
                engine.write_memory(&memory, u64::MAX, &[0]),
                Err(WriteMemoryError)
            );
        }
    }
}

#[test]
fn a_shared_memory_a_module_exports_is_read_and_written() {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    let mut module = Module::new();
    let mut memories = wasm_encoder::MemorySection::new();
    memories.memory(wasm_encoder::MemoryType {
        minimum: 1,
        maximum: Some(1),
        memory64: false,
        shared: true,
        page_size_log2: None,
    });
    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    module.section(&memories).section(&exports);
    let mut engine = engine(Config::default());
    let module = engine.compile(&module.finish()).expect("compiles");
    let instance = engine.instantiate(&module, &[]).expect("instantiates");
    let Some(ExternalAddress::Memory(first)) = engine.find_export(&instance, "memory") else {
        panic!("a memory");
    };
    let Some(ExternalAddress::Memory(again)) = engine.find_export(&instance, "memory") else {
        panic!("a memory");
    };
    assert!(engine.to_shared_memory(&first).is_some());
    engine.write_memory(&first, 7, b"shared").expect("writes");
    let mut read = [0; 6];
    engine.read_memory(&again, 7, &mut read).expect("reads");
    assert_eq!(&read, b"shared");
}

#[test]
fn a_trap_after_a_host_function_returns_is_a_trap() {
    let mut types = TypeSection::new();
    types.ty().function([], []);
    let mut imports = ImportSection::new();
    imports.import("env", "hook", EntityType::Function(0));
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export("run", ExportKind::Func, 1);
    let mut run = Function::new([]);
    run.instructions().call(0).unreachable().end();
    let mut code = CodeSection::new();
    code.function(&run);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&exports)
        .section(&code);
    let mut engine = engine(Config::default());
    let module = engine.compile(&module.finish()).expect("compiles");
    let hook = engine.bind_host_function(|context: &mut i32, ()| *context += 1);
    let imports = [Import {
        module_name: "env",
        item_name: "hook",
        address: ExternalAddress::Function(hook),
    }];
    let instance = engine.instantiate(&module, &imports).expect("instantiates");
    let address = function(&mut engine, &instance, "run");
    let run = bind::<(), ()>(&mut engine, address);
    for count in 1..=2 {
        assert!(matches!(
            run.invoke(&mut engine, ()),
            Err(InvokeError::Trap(_))
        ));
        assert_eq!(*engine.host_context(), count);
    }
}

#[test]
fn an_uncaught_exception_is_an_exception() {
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], []);
    types.ty().function([], []);
    let mut functions = FunctionSection::new();
    functions.function(1);
    let mut tags = TagSection::new();
    tags.tag(TagType {
        kind: TagKind::Exception,
        func_type_idx: 0,
    });
    let mut exports = ExportSection::new();
    exports
        .export("throw", ExportKind::Func, 0)
        .export("tag", ExportKind::Tag, 0);
    let mut throw = Function::new([]);
    throw.instructions().i32_const(7).throw(0).end();
    let mut code = CodeSection::new();
    code.function(&throw);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&tags)
        .section(&exports)
        .section(&code);
    let mut engine = engine(Config::default());
    let module = engine.compile(&module.finish()).expect("compiles");
    let instance = engine.instantiate(&module, &[]).expect("instantiates");
    assert!(matches!(
        engine.find_export(&instance, "tag"),
        Some(ExternalAddress::Tag(_))
    ));
    let address = function(&mut engine, &instance, "throw");
    let throw = bind::<(), ()>(&mut engine, address);
    for _ in 0..2 {
        assert!(matches!(
            throw.invoke(&mut engine, ()),
            Err(InvokeError::Exception)
        ));
    }
}

/// Checks, for one number of `i64` parameters, a module function and a host function that add them up,
/// each weighted by its position so that the order of the arguments counts.
macro_rules! adds_up {
    ($name:ident, $($argument:ident)*) => {
        #[test]
        fn $name() {
            let count = u32::try_from(count!($($argument)*)).expect("fits");
            let mut types = TypeSection::new();
            let parameters = vec![ValType::I64; usize::try_from(count).expect("fits")];
            types.ty().function(parameters, [ValType::I64]);
            let mut functions = FunctionSection::new();
            functions.function(0);
            let mut exports = ExportSection::new();
            exports.export("sum", ExportKind::Func, 0);
            let mut sum = Function::new([]);
            sum.instructions().i64_const(0);
            for index in 0..count {
                sum.instructions()
                    .local_get(index)
                    .i64_const(weight(index))
                    .i64_mul()
                    .i64_add();
            }
            sum.instructions().end();
            let mut code = CodeSection::new();
            code.function(&sum);
            let mut module = Module::new();
            module.section(&types).section(&functions).section(&exports).section(&code);
            let mut engine = engine(Config::default());
            let module = engine.compile(&module.finish()).expect("compiles");
            let instance = engine.instantiate(&module, &[]).expect("instantiates");
            let address = function(&mut engine, &instance, "sum");
            let [$($argument,)*]: [i64; count!($($argument)*)] =
                core::array::from_fn(|index| 1 << index);
            let expected = weighted(&[$($argument,)*]);
            let sum = bind::<($(stand_for!($argument),)*), i64>(&mut engine, address);
            assert_eq!(sum.invoke(&mut engine, ($($argument,)*)).ok(), Some(expected));
            let host = engine.bind_host_function(
                |context: &mut i32, ($($argument,)*): ($(stand_for!($argument),)*)| {
                    *context += 1;
                    weighted(&[$($argument,)*])
                },
            );
            let host = bind::<($(stand_for!($argument),)*), i64>(&mut engine, host);
            assert_eq!(host.invoke(&mut engine, ($($argument,)*)).ok(), Some(expected));
            assert_eq!(*engine.host_context(), 1);
        }
    };
}

macro_rules! count {
    ($($argument:ident)*) => {
        <[&str]>::len(&[$(stringify!($argument)),*])
    };
}

fn weight(index: u32) -> i64 {
    i64::from(index) + 1
}

fn weighted(arguments: &[i64]) -> i64 {
    (0..)
        .zip(arguments)
        .map(|(index, argument)| weight(index) * argument)
        .sum()
}

macro_rules! stand_for {
    ($argument:ident) => {
        i64
    };
}

adds_up!(adds_up_no_parameters,);
adds_up!(adds_up_1_parameter, a1);
adds_up!(adds_up_2_parameters, a1 a2);
adds_up!(adds_up_3_parameters, a1 a2 a3);
adds_up!(adds_up_4_parameters, a1 a2 a3 a4);
adds_up!(adds_up_5_parameters, a1 a2 a3 a4 a5);
adds_up!(adds_up_6_parameters, a1 a2 a3 a4 a5 a6);
adds_up!(adds_up_7_parameters, a1 a2 a3 a4 a5 a6 a7);
adds_up!(adds_up_8_parameters, a1 a2 a3 a4 a5 a6 a7 a8);
adds_up!(adds_up_9_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9);
adds_up!(adds_up_10_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9 a10);
adds_up!(adds_up_11_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11);
adds_up!(adds_up_12_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11 a12);
adds_up!(adds_up_13_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11 a12 a13);
adds_up!(adds_up_14_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11 a12 a13 a14);
adds_up!(adds_up_15_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11 a12 a13 a14 a15);
adds_up!(adds_up_16_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11 a12 a13 a14 a15 a16);
adds_up!(adds_up_17_parameters, a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11 a12 a13 a14 a15 a16 a17);

#[test]
fn binds_every_value_type_and_several_results() {
    let mut types = TypeSection::new();
    let all = [
        ValType::I32,
        ValType::I64,
        ValType::F32,
        ValType::F64,
        ValType::V128,
    ];
    types.ty().function(all, all);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export("same", ExportKind::Func, 0);
    let mut same = Function::new([]);
    for index in 0..5 {
        same.instructions().local_get(index);
    }
    same.instructions().end();
    let mut code = CodeSection::new();
    code.function(&same);
    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&exports)
        .section(&code);
    let mut engine = engine(Config::default());
    let module = engine.compile(&module.finish()).expect("compiles");
    let instance = engine.instantiate(&module, &[]).expect("instantiates");
    let address = function(&mut engine, &instance, "same");
    type All = (i32, i64, f32, f64, wasmtime::V128);
    let values: All = (i32::MIN, i64::MAX, 1.5, -2.25, u128::MAX.into());
    let same = bind::<All, All>(&mut engine, address);
    let answer = same.invoke(&mut engine, values).expect("returns");
    assert_eq!(
        (answer.0, answer.1, answer.2, answer.3, answer.4.as_u128()),
        (values.0, values.1, values.2, values.3, u128::MAX)
    );
}
