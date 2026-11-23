//! Checks of the WasmEdge engine that the interface's suite does not make.

use std::sync::Barrier;

use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder, InvokeError};
use wcontainers_webassembly_engine::memory::{MemoryStore, MemoryType, PAGE_SIZE, Sharing};
use wcontainers_webassembly_engine::table::{TableStore, TableType};
use wcontainers_webassembly_engine::{
    AddressType, ExternalAddress, Import, Limits, WebAssemblyEngine,
};
use wcontainers_webassembly_engine_wasmedge::{
    AllocateError, Config, FunctionAddress, Instance, InstantiateError, Module, WasmEdgeEngine,
};

type Engine = WasmEdgeEngine<i32>;

fn engine(config: Config) -> Engine {
    Engine::new(config, i32::default()).expect("the engine is made")
}

fn compile(engine: &Engine, text: &str) -> Module {
    let bytes = wat::parse_str(text).expect("the text is a module");
    engine.compile(&bytes).expect("the module compiles")
}

fn function(engine: &mut Engine, instance: &Instance, name: &str) -> FunctionAddress {
    match engine.find_export(instance, name) {
        Some(ExternalAddress::Function(function)) => function,
        _ => panic!("{name} is an exported function"),
    }
}

fn bind<Parameters, Results>(
    engine: &mut Engine,
    address: FunctionAddress,
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

fn import(item_name: &'static str, address: ExternalAddress<Engine>) -> Import<'static, Engine> {
    Import {
        module_name: "env",
        item_name,
        address,
    }
}

/// Imports a shared memory of one page and adds one to its first word, atomically, as many times as asked.
const COUNTS_IN_A_SHARED_MEMORY: &str = r#"(module
    (import "env" "memory" (memory 1 1 shared))
    (func (export "count") (param i32)
        (loop $again
            (drop (i32.atomic.rmw.add (i32.const 0) (i32.const 1)))
            (br_if $again (local.tee 0 (i32.sub (local.get 0) (i32.const 1)))))))"#;

#[test]
fn two_threads_share_one_module_and_one_shared_memory() {
    let config = Config::default();
    let mut first = engine(config.clone());
    let memory = first
        .allocate_memory(memory_type(AddressType::I32, 1, Sharing::Shared))
        .expect("the memory");
    let shared = first
        .to_shared_memory(&memory)
        .expect("the memory is shared");
    let module = compile(&first, COUNTS_IN_A_SHARED_MEMORY);
    let counts = 100_000;
    let threads = 2;
    let barrier = Barrier::new(threads);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut counting = engine(config.clone());
                let memory = counting.add_shared_memory(&shared);
                let instance = counting
                    .instantiate(
                        &module,
                        &[import("memory", ExternalAddress::Memory(memory))],
                    )
                    .expect("the module instantiates");
                let address = function(&mut counting, &instance, "count");
                let count = bind::<(i32,), ()>(&mut counting, address);
                barrier.wait();
                assert!(count.invoke(&mut counting, (counts,)).is_ok());
            });
        }
    });
    let mut word = [0; 4];
    assert_eq!(first.read_memory(&memory, 0, &mut word), Ok(()));
    assert_eq!(i32::from_le_bytes(word), counts * 2);
}

#[test]
fn reads_and_writes_more_than_a_page_of_every_kind_of_memory() {
    let mut engine = engine(Config::default());
    let pages = 3;
    let length = usize::try_from(PAGE_SIZE * 5 / 2).expect("the length fits");
    let bytes: Vec<u8> = (0..length).map(|index| (index % 251) as u8).collect();
    for sharing in [Sharing::Unshared, Sharing::Shared] {
        let memory = engine
            .allocate_memory(memory_type(AddressType::I32, pages, sharing))
            .expect("the memory");
        assert_eq!(engine.write_memory(&memory, 100, &bytes), Ok(()));
        let mut read = vec![0; length];
        assert_eq!(engine.read_memory(&memory, 100, &mut read), Ok(()));
        assert!(
            read == bytes,
            "the {sharing:?} memory holds what was written"
        );
        let end = pages * PAGE_SIZE;
        assert!(engine.read_memory(&memory, end - 1, &mut [0; 2]).is_err());
        assert_eq!(engine.read_memory(&memory, end, &mut []), Ok(()));
    }
}

/// Defines a shared memory and exports it, with `load` and `store` of its words.
const DEFINES_A_SHARED_MEMORY: &str = r#"(module
    (memory (export "memory") 1 1 shared)
    (func (export "load") (param i32) (result i32) (i32.atomic.load (local.get 0)))
    (func (export "store") (param i32 i32) (i32.atomic.store (local.get 0) (local.get 1))))"#;

#[test]
fn a_shared_memory_a_module_exports_is_read_and_written() {
    let config = Config::default();
    let mut owner = engine(config.clone());
    let module = compile(&owner, DEFINES_A_SHARED_MEMORY);
    let instance = owner
        .instantiate(&module, &[])
        .expect("the module instantiates");
    let Some(ExternalAddress::Memory(memory)) = owner.find_export(&instance, "memory") else {
        panic!("the memory is exported");
    };
    assert_eq!(owner.write_memory(&memory, 8, &7_i32.to_le_bytes()), Ok(()));
    let address = function(&mut owner, &instance, "load");
    let load = bind::<(i32,), i32>(&mut owner, address);
    assert_eq!(load.invoke(&mut owner, (8,)).ok(), Some(7));
    let address = function(&mut owner, &instance, "store");
    let store = bind::<(i32, i32), ()>(&mut owner, address);
    assert!(store.invoke(&mut owner, (12, 9)).is_ok());
    let shared = owner
        .to_shared_memory(&memory)
        .expect("the memory is shared");
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut other = engine(config.clone());
            let added = other.add_shared_memory(&shared);
            let mut word = [0; 4];
            assert_eq!(other.read_memory(&added, 12, &mut word), Ok(()));
            assert_eq!(i32::from_le_bytes(word), 9);
        });
    });
}

#[test]
fn a_trap_after_a_host_function_returns_is_a_trap() {
    let mut engine = engine(Config::default());
    let module = compile(
        &engine,
        r#"(module
            (import "env" "hook" (func $hook (param i32 i64) (result i64)))
            (func (export "call") (result i64) (call $hook (i32.const 2) (i64.const 40)))
            (func (export "trap") (drop (call $hook (i32.const 1) (i64.const 1))) (unreachable)))"#,
    );
    let hook = <Engine as FunctionTypeBinder<(i32, i64), i64>>::bind_host_function(
        &mut engine,
        |calls: &mut i32, (left, right)| {
            *calls += 1;
            i64::from(left) + right
        },
    );
    let instance = engine
        .instantiate(&module, &[import("hook", ExternalAddress::Function(hook))])
        .expect("the module instantiates");
    let address = function(&mut engine, &instance, "call");
    let call = bind::<(), i64>(&mut engine, address);
    assert_eq!(call.invoke(&mut engine, ()).ok(), Some(42));
    let address = function(&mut engine, &instance, "trap");
    let trap = bind::<(), ()>(&mut engine, address);
    assert!(matches!(
        trap.invoke(&mut engine, ()),
        Err(InvokeError::Trap(_))
    ));
    assert_eq!(*engine.host_context(), 2);
}

#[test]
fn binds_every_value_type_and_several_results() {
    let mut engine = engine(Config::default());
    let module = compile(
        &engine,
        r#"(module
            (import "env" "sum" (func $sum (param i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
            (func (export "reverse") (param i32 i64 f32 f64 v128) (result v128 f64 f32 i64 i32)
                (local.get 4) (local.get 3) (local.get 2) (local.get 1) (local.get 0))
            (func (export "sum") (result i32)
                (call $sum (i32.const 1) (i32.const 2) (i32.const 3) (i32.const 4) (i32.const 5) (i32.const 6)
                    (i32.const 7) (i32.const 8) (i32.const 9) (i32.const 10) (i32.const 11) (i32.const 12)
                    (i32.const 13) (i32.const 14) (i32.const 15) (i32.const 16) (i32.const 17))))"#,
    );
    type Seventeen = (
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
        i32,
    );
    let sum = <Engine as FunctionTypeBinder<Seventeen, i32>>::bind_host_function(
        &mut engine,
        |_: &mut i32, (a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q): Seventeen| {
            [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q]
                .iter()
                .enumerate()
                .map(|(index, value)| value * i32::try_from(index + 1).expect("small"))
                .sum()
        },
    );
    let instance = engine
        .instantiate(&module, &[import("sum", ExternalAddress::Function(sum))])
        .expect("the module instantiates");
    let address = function(&mut engine, &instance, "reverse");
    let reverse =
        bind::<(i32, i64, f32, f64, i128), (i128, f64, f32, i64, i32)>(&mut engine, address);
    assert_eq!(
        reverse
            .invoke(&mut engine, (1, 2, 3.5, 4.25, i128::MIN))
            .ok(),
        Some((i128::MIN, 4.25, 3.5, 2, 1))
    );
    let address = function(&mut engine, &instance, "sum");
    let weighted = bind::<(), i32>(&mut engine, address);
    // The sum of the squares of one to seventeen.
    assert_eq!(weighted.invoke(&mut engine, ()).ok(), Some(1785));
}

/// Exports `answer`, which returns 42, and a table whose only element refers to a function it does not export.
const EXPORTS_A_TABLE: &str = r#"(module
    (table (export "table") 1 funcref)
    (func $hidden (result i32) (i32.const 7))
    (func (export "answer") (result i32) (i32.const 42))
    (elem (i32.const 0) func $hidden))"#;

/// Imports a function and exports a function that calls it.
const CALLS_AN_IMPORT: &str = r#"(module
    (import "env" "function" (func $function (result i32)))
    (func (export "call") (result i32) (call $function)))"#;

#[test]
fn a_function_read_from_a_table_is_imported_when_an_instance_exports_it() {
    let mut engine = engine(Config::default());
    let exporting = compile(&engine, EXPORTS_A_TABLE);
    let exporter = engine
        .instantiate(&exporting, &[])
        .expect("the module instantiates");
    let answer = function(&mut engine, &exporter, "answer");
    let table = engine
        .allocate_table(TableType {
            address_type: AddressType::I32,
            limits: Limits {
                minimum: 1,
                maximum: None,
            },
        })
        .expect("the table");
    assert_eq!(engine.write_table(&table, 0, Some(&answer)), Ok(()));
    let read = engine
        .read_table(&table, 0)
        .expect("the element is inside")
        .expect("the element refers to a function");
    let importing = compile(&engine, CALLS_AN_IMPORT);
    let importer = engine
        .instantiate(
            &importing,
            &[import("function", ExternalAddress::Function(read))],
        )
        .expect("the module instantiates");
    let address = function(&mut engine, &importer, "call");
    let call = bind::<(), i32>(&mut engine, address);
    assert_eq!(call.invoke(&mut engine, ()).ok(), Some(42));

    let Some(ExternalAddress::Table(exported_table)) = engine.find_export(&exporter, "table")
    else {
        panic!("the table is exported");
    };
    let hidden = engine
        .read_table(&exported_table, 0)
        .expect("the element is inside")
        .expect("the element refers to a function");
    let bound = bind::<(), i32>(&mut engine, hidden.clone());
    assert_eq!(bound.invoke(&mut engine, ()).ok(), Some(7));
    assert!(matches!(
        engine.instantiate(
            &importing,
            &[import("function", ExternalAddress::Function(hidden))]
        ),
        Err(InstantiateError::FunctionNotExported)
    ));
}

#[test]
fn a_64_bit_memory_a_module_exports_is_read_and_written() {
    let mut engine = engine(Config::default());
    let module = compile(
        &engine,
        r#"(module
            (memory (export "memory") i64 2)
            (func (export "load") (param i64) (result i64) (i64.load (local.get 0))))"#,
    );
    let instance = engine
        .instantiate(&module, &[])
        .expect("the module instantiates");
    let Some(ExternalAddress::Memory(memory)) = engine.find_export(&instance, "memory") else {
        panic!("the memory is exported");
    };
    assert_eq!(engine.count_memory_pages(&memory), 2);
    assert_eq!(
        engine.write_memory(&memory, 70_000, &(-5_i64).to_le_bytes()),
        Ok(())
    );
    let address = function(&mut engine, &instance, "load");
    let load = bind::<(i64,), i64>(&mut engine, address);
    assert_eq!(load.invoke(&mut engine, (70_000,)).ok(), Some(-5));
    assert!(matches!(
        engine.allocate_memory(memory_type(AddressType::I64, 1, Sharing::Unshared)),
        Err(AllocateError::AddressTypeOutsideBindings)
    ));
}
