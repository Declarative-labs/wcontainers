//! Checks of the WasmEdge engine that the interface's suite does not make.

use std::sync::Barrier;

use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder, InvokeError};
use wcontainers_webassembly_engine::global::{GlobalStore, Mutability, Value, WriteGlobalError};
use wcontainers_webassembly_engine::memory::{
    GrowMemoryError, MemoryStore, MemoryType, PAGE_SIZE, Sharing,
};
use wcontainers_webassembly_engine::table::{
    GrowTableError, TableStore, TableType, WriteTableError,
};
use wcontainers_webassembly_engine::{
    AddressType, ExternalAddress, Import, Limits, WebAssemblyEngine,
};
use wcontainers_webassembly_engine_wasmedge::{
    AllocateError, Config, FunctionAddress, GlobalAddress, Instance, InstantiateError,
    MemoryAddress, Module, SlowPath, TableAddress, WasmEdgeEngine,
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
        module_name: "host",
        item_name,
        address,
    }
}

/// Imports a shared memory of one page and adds one to its first word, atomically, as many times as asked.
const COUNTS_IN_A_SHARED_MEMORY: &str = r#"(module
    (import "host" "heap" (memory 1 1 shared))
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
                    .instantiate(&module, &[import("heap", ExternalAddress::Memory(memory))])
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
    (memory (export "heap") 1 1 shared)
    (func (export "fetch") (param i32) (result i32) (i32.atomic.load (local.get 0)))
    (func (export "put") (param i32 i32) (i32.atomic.store (local.get 0) (local.get 1))))"#;

#[test]
fn a_shared_memory_a_module_exports_is_read_and_written() {
    let config = Config::default();
    let mut owner = engine(config.clone());
    let module = compile(&owner, DEFINES_A_SHARED_MEMORY);
    let instance = owner
        .instantiate(&module, &[])
        .expect("the module instantiates");
    let Some(ExternalAddress::Memory(memory)) = owner.find_export(&instance, "heap") else {
        panic!("the memory is exported");
    };
    assert_eq!(owner.write_memory(&memory, 8, &7_i32.to_le_bytes()), Ok(()));
    let address = function(&mut owner, &instance, "fetch");
    let load = bind::<(i32,), i32>(&mut owner, address);
    assert_eq!(load.invoke(&mut owner, (8,)).ok(), Some(7));
    let address = function(&mut owner, &instance, "put");
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
            (import "host" "upcall" (func $hook (param i32 i64) (result i64)))
            (func (export "caller") (result i64) (call $hook (i32.const 2) (i64.const 40)))
            (func (export "fault") (drop (call $hook (i32.const 1) (i64.const 1))) (unreachable)))"#,
    );
    let hook = <Engine as FunctionTypeBinder<(i32, i64), i64>>::bind_host_function(
        &mut engine,
        |calls: &mut i32, (left, right)| {
            *calls += 1;
            i64::from(left) + right
        },
    );
    let instance = engine
        .instantiate(
            &module,
            &[import("upcall", ExternalAddress::Function(hook))],
        )
        .expect("the module instantiates");
    let address = function(&mut engine, &instance, "caller");
    let call = bind::<(), i64>(&mut engine, address);
    assert_eq!(call.invoke(&mut engine, ()).ok(), Some(42));
    let address = function(&mut engine, &instance, "fault");
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
            (import "host" "sum" (func $sum (param i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
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
    (table (export "elements") 1 funcref)
    (func $hidden (result i32) (i32.const 7))
    (func (export "forty_two") (result i32) (i32.const 42))
    (elem (i32.const 0) func $hidden))"#;

/// Imports a function and exports a function that calls it.
const CALLS_AN_IMPORT: &str = r#"(module
    (import "host" "routine" (func $function (result i32)))
    (func (export "caller") (result i32) (call $function)))"#;

#[test]
fn a_function_read_from_a_table_is_imported() {
    let mut engine = engine(Config::default());
    let exporting = compile(&engine, EXPORTS_A_TABLE);
    let exporter = engine
        .instantiate(&exporting, &[])
        .expect("the module instantiates");
    let answer = function(&mut engine, &exporter, "forty_two");
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
            &[import("routine", ExternalAddress::Function(read))],
        )
        .expect("the module instantiates");
    let address = function(&mut engine, &importer, "caller");
    let call = bind::<(), i32>(&mut engine, address);
    assert_eq!(call.invoke(&mut engine, ()).ok(), Some(42));

    let Some(ExternalAddress::Table(exported_table)) = engine.find_export(&exporter, "elements")
    else {
        panic!("the table is exported");
    };
    let hidden = engine
        .read_table(&exported_table, 0)
        .expect("the element is inside")
        .expect("the element refers to a function");
    let bound = bind::<(), i32>(&mut engine, hidden.clone());
    assert_eq!(bound.invoke(&mut engine, ()).ok(), Some(7));
    let passed_on = engine.slow_path_count(SlowPath::FunctionPassedOn);
    let importer = engine
        .instantiate(
            &importing,
            &[import("routine", ExternalAddress::Function(hidden.clone()))],
        )
        .expect("a function no instance exports is imported");
    assert_eq!(
        engine.slow_path_count(SlowPath::FunctionPassedOn),
        passed_on + 1
    );
    let address = function(&mut engine, &importer, "caller");
    let call = bind::<(), i32>(&mut engine, address);
    assert_eq!(call.invoke(&mut engine, ()).ok(), Some(7));
    let of_another_type = compile(
        &engine,
        r#"(module (import "host" "routine" (func (result i64))))"#,
    );
    assert!(matches!(
        engine.instantiate(
            &of_another_type,
            &[import("routine", ExternalAddress::Function(hidden))]
        ),
        Err(InstantiateError::FunctionTypeMismatch)
    ));
}

/// Tests a function reference against a function type and calls it as one, from a table a host writes into, which
/// is either plain or typed, and from a plain global.
const CASTS: &str = r#"(module
    (type $t (func (result i32)))
    (table (export "plain") 2 funcref)
    (table (export "typed") 2 (ref null $t))
    (global (export "cell") (mut funcref) (ref.null func))
    (func (export "test_plain") (param i32) (result i32) (ref.test (ref $t) (table.get 0 (local.get 0))))
    (func (export "cast_plain") (param i32) (result i32)
        (call_ref $t (ref.cast (ref $t) (table.get 0 (local.get 0)))))
    (func (export "call_typed") (param i32) (result i32)
        (call_ref $t (ref.as_non_null (table.get 1 (local.get 0)))))
    (func (export "cast_cell") (result i32) (call_ref $t (ref.cast (ref $t) (global.get 0)))))"#;

#[test]
fn every_function_the_host_writes_carries_its_own_type() {
    let mut engine = engine(Config::default());
    let exporting = compile(
        &engine,
        r#"(module (func (export "seven") (result i32) (i32.const 7)))"#,
    );
    let exporter = engine
        .instantiate(&exporting, &[])
        .expect("the module instantiates");
    let seven = function(&mut engine, &exporter, "seven");
    let nine = <Engine as FunctionTypeBinder<(), i32>>::bind_host_function(&mut engine, |_, ()| 9);
    let module = compile(&engine, CASTS);
    let instance = engine
        .instantiate(&module, &[])
        .expect("the module instantiates");
    let plain = table(&mut engine, &instance, "plain");
    let typed = table(&mut engine, &instance, "typed");
    let retyped = engine.slow_path_count(SlowPath::FunctionRetyped);
    for (index, written) in [(0, &seven), (1, &nine)] {
        assert_eq!(engine.write_table(&plain, index, Some(written)), Ok(()));
        assert_eq!(engine.write_table(&typed, index, Some(written)), Ok(()));
    }
    assert_eq!(
        engine.slow_path_count(SlowPath::FunctionRetyped),
        retyped + 2,
        "each address is retyped once"
    );
    assert_eq!(engine.slow_path_count(SlowPath::FunctionNotRetyped), 0);
    for (index, expected) in [(0, 7), (1, 9)] {
        for name in ["test_plain", "cast_plain", "call_typed"] {
            let address = function(&mut engine, &instance, name);
            let bound = bind::<(i32,), i32>(&mut engine, address);
            let expected = if name == "test_plain" { 1 } else { expected };
            assert_eq!(
                bound.invoke(&mut engine, (index,)).ok(),
                Some(expected),
                "{name} {index}"
            );
        }
    }
    // A reference read back from a typed table is written on as it is, into either table.
    let read = engine
        .read_table(&typed, 1)
        .ok()
        .flatten()
        .expect("the typed table holds a function");
    assert_eq!(engine.write_table(&typed, 0, Some(&read)), Ok(()));
    assert_eq!(engine.write_table(&plain, 0, Some(&read)), Ok(()));
    for name in ["call_typed", "cast_plain"] {
        let address = function(&mut engine, &instance, name);
        let bound = bind::<(i32,), i32>(&mut engine, address);
        assert_eq!(bound.invoke(&mut engine, (0,)).ok(), Some(9), "{name}");
    }
    let Some(ExternalAddress::Global(cell)) = engine.find_export(&instance, "cell") else {
        panic!("cell is an exported global");
    };
    let cast_cell = function(&mut engine, &instance, "cast_cell");
    let cast_cell = bind::<(), i32>(&mut engine, cast_cell);
    for (written, expected) in [(&seven, 7), (&nine, 9)] {
        assert_eq!(
            engine.write_global(&cell, Value::FunctionReference(Some(written.clone()))),
            Ok(())
        );
        assert_eq!(cast_cell.invoke(&mut engine, ()).ok(), Some(expected));
    }
    // A reference read back from a global keeps its type when it is written on.
    let Ok(Value::FunctionReference(Some(read))) = engine.read_global(&cell) else {
        panic!("the global holds a function");
    };
    assert_eq!(engine.write_table(&plain, 0, Some(&read)), Ok(()));
    let address = function(&mut engine, &instance, "cast_plain");
    let cast_plain = bind::<(i32,), i32>(&mut engine, address);
    assert_eq!(cast_plain.invoke(&mut engine, (0,)).ok(), Some(9));
    let allocated = engine
        .allocate_global(Mutability::Variable, Value::FunctionReference(Some(seven)))
        .expect("the global is allocated");
    let importing = compile(
        &engine,
        r#"(module
            (type $t (func (result i32)))
            (import "host" "cell" (global $cell (mut funcref)))
            (func (export "cast_cell") (result i32) (call_ref $t (ref.cast (ref $t) (global.get $cell)))))"#,
    );
    let importer = engine
        .instantiate(
            &importing,
            &[import("cell", ExternalAddress::Global(allocated))],
        )
        .expect("the module instantiates");
    let address = function(&mut engine, &importer, "cast_cell");
    let cast = bind::<(), i32>(&mut engine, address);
    assert_eq!(cast.invoke(&mut engine, ()).ok(), Some(7));
}

#[test]
fn slow_paths_are_counted() {
    for (index, path) in SlowPath::ALL.iter().enumerate() {
        assert_eq!(*path as usize, index);
        assert!(!path.reason().is_empty());
    }
    let mut engine = engine(Config::default());
    let shared = engine
        .allocate_memory(memory_type(AddressType::I32, 1, Sharing::Shared))
        .expect("the memory is allocated");
    assert_eq!(engine.slow_path_count(SlowPath::SharedMemoryCopied), 0);
    assert_eq!(engine.write_memory(&shared, 0, b"abc"), Ok(()));
    let mut read = [0; 3];
    assert_eq!(engine.read_memory(&shared, 0, &mut read), Ok(()));
    assert_eq!(engine.slow_path_count(SlowPath::SharedMemoryCopied), 2);
    let unshared = engine
        .allocate_memory(memory_type(AddressType::I32, 1, Sharing::Unshared))
        .expect("the memory is allocated");
    assert_eq!(engine.write_memory(&unshared, 0, b"abc"), Ok(()));
    assert_eq!(engine.read_memory(&unshared, 0, &mut read), Ok(()));
    assert_eq!(engine.slow_path_count(SlowPath::SharedMemoryCopied), 2);
    assert_eq!(engine.slow_path_count(SlowPath::BeyondBindingsReach), 0);
}

#[test]
fn a_64_bit_memory_a_module_exports_is_read_and_written() {
    let mut engine = engine(Config::default());
    let module = compile(
        &engine,
        r#"(module
            (memory (export "heap") i64 2)
            (func (export "fetch") (param i64) (result i64) (i64.load (local.get 0))))"#,
    );
    let instance = engine
        .instantiate(&module, &[])
        .expect("the module instantiates");
    let Some(ExternalAddress::Memory(memory)) = engine.find_export(&instance, "heap") else {
        panic!("the memory is exported");
    };
    assert_eq!(engine.count_memory_pages(&memory), 2);
    assert_eq!(
        engine.write_memory(&memory, 70_000, &(-5_i64).to_le_bytes()),
        Ok(())
    );
    let address = function(&mut engine, &instance, "fetch");
    let load = bind::<(i64,), i64>(&mut engine, address);
    assert_eq!(load.invoke(&mut engine, (70_000,)).ok(), Some(-5));
    assert!(matches!(
        engine.allocate_memory(memory_type(AddressType::I64, 1 << 49, Sharing::Unshared)),
        Err(AllocateError::LimitsOutsideAddressType)
    ));
}

fn memory(engine: &mut Engine, instance: &Instance, name: &str) -> MemoryAddress {
    match engine.find_export(instance, name) {
        Some(ExternalAddress::Memory(memory)) => memory,
        _ => panic!("{name} is an exported memory"),
    }
}

fn table(engine: &mut Engine, instance: &Instance, name: &str) -> TableAddress {
    match engine.find_export(instance, name) {
        Some(ExternalAddress::Table(table)) => table,
        _ => panic!("{name} is an exported table"),
    }
}

fn global(engine: &mut Engine, instance: &Instance, name: &str) -> GlobalAddress {
    match engine.find_export(instance, name) {
        Some(ExternalAddress::Global(global)) => global,
        _ => panic!("{name} is an exported global"),
    }
}

fn instantiate(engine: &mut Engine, text: &str, imports: &[Import<'_, Engine>]) -> Instance {
    let module = compile(engine, text);
    engine
        .instantiate(&module, imports)
        .expect("the module instantiates")
}

/// Exports a shared memory, an unshared one, and a mutable global, which a translated threaded module has.
const SHARES_A_MEMORY_BESIDE_OTHER_ITEMS: &str = r#"(module
    (memory (export "shared") 1 1 shared)
    (memory (export "unshared") 1 2)
    (global (export "variable") (mut i32) (i32.const 1))
    (global (export "constant") i64 (i64.const 2))
    (func (export "read_global") (result i32) (global.get 0))
    (func (export "fetch") (param i32) (result i32) (i32.load 1 (local.get 0))))"#;

#[test]
fn the_items_beside_an_exported_shared_memory_are_written() {
    let mut engine = engine(Config::default());
    let instance = instantiate(&mut engine, SHARES_A_MEMORY_BESIDE_OTHER_ITEMS, &[]);
    let written = global(&mut engine, &instance, "variable");
    assert_eq!(engine.write_global(&written, Value::I32(5)), Ok(()));
    assert!(matches!(engine.read_global(&written), Ok(Value::I32(5))));
    let address = function(&mut engine, &instance, "read_global");
    let read_global = bind::<(), i32>(&mut engine, address);
    assert_eq!(read_global.invoke(&mut engine, ()).ok(), Some(5));
    assert_eq!(
        engine.write_global(&written, Value::I64(5)),
        Err(WriteGlobalError::ValueTypeMismatch)
    );
    let constant = global(&mut engine, &instance, "constant");
    assert_eq!(
        engine.write_global(&constant, Value::I64(3)),
        Err(WriteGlobalError::Constant)
    );
    let unshared = memory(&mut engine, &instance, "unshared");
    assert_eq!(engine.write_memory(&unshared, 3, b"abcd"), Ok(()));
    let mut read = [0; 4];
    assert_eq!(engine.read_memory(&unshared, 3, &mut read), Ok(()));
    assert_eq!(&read, b"abcd");
    let address = function(&mut engine, &instance, "fetch");
    let load = bind::<(i32,), i32>(&mut engine, address);
    assert_eq!(
        load.invoke(&mut engine, (3,)).ok(),
        Some(i32::from_le_bytes(*b"abcd"))
    );
    assert_eq!(engine.grow_memory(&unshared, 1), Ok(()));
    assert_eq!(engine.count_memory_pages(&unshared), 2);
    assert_eq!(engine.grow_memory(&unshared, 1), Err(GrowMemoryError));
    assert!(
        engine
            .write_memory(&unshared, 2 * PAGE_SIZE - 1, b"ab")
            .is_err()
    );
    let shared = memory(&mut engine, &instance, "shared");
    assert!(engine.to_shared_memory(&shared).is_some());
    assert!(engine.to_shared_memory(&unshared).is_none());
}

#[test]
fn a_shared_64_bit_memory_is_found_shared_and_imported() {
    let config = Config::default();
    let mut owner = engine(config.clone());
    let instance = instantiate(
        &mut owner,
        r#"(module (memory (export "heap") i64 1 2 shared) (data (i64.const 5) "xyz"))"#,
        &[],
    );
    let exported = memory(&mut owner, &instance, "heap");
    let mut read = [0; 3];
    assert_eq!(owner.read_memory(&exported, 5, &mut read), Ok(()));
    assert_eq!(&read, b"xyz");
    assert_eq!(owner.write_memory(&exported, 9, b"uvw"), Ok(()));
    let shared = owner
        .to_shared_memory(&exported)
        .expect("the memory is shared");
    let module = compile(
        &owner,
        r#"(module (import "host" "heap" (memory i64 1 2 shared))
            (func (export "fetch") (param i64) (result i32) (i32.atomic.load (local.get 0))))"#,
    );
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut other = engine(config.clone());
            let added = other.add_shared_memory(&shared);
            assert_eq!(other.grow_memory(&added, 1), Ok(()));
            assert_eq!(other.count_memory_pages(&added), 2);
            let instance = other
                .instantiate(&module, &[import("heap", ExternalAddress::Memory(added))])
                .expect("the module instantiates");
            let address = function(&mut other, &instance, "fetch");
            let load = bind::<(i64,), i32>(&mut other, address);
            assert_eq!(
                load.invoke(&mut other, (8,)).ok(),
                Some(i32::from_le_bytes(*b"\0uvw"))
            );
        });
    });
    assert_eq!(owner.count_memory_pages(&exported), 2);
}

#[test]
fn a_64_bit_memory_is_imported_and_allocated() {
    let mut engine = engine(Config::default());
    let exporter = instantiate(
        &mut engine,
        r#"(module (memory (export "heap") i64 1))"#,
        &[],
    );
    let exported = memory(&mut engine, &exporter, "heap");
    let sizes = r#"(module (import "host" "heap" (memory i64 1))
        (func (export "size") (result i64) (memory.size))
        (func (export "fetch") (param i64) (result i64) (i64.load (local.get 0))))"#;
    let importer = instantiate(
        &mut engine,
        sizes,
        &[import("heap", ExternalAddress::Memory(exported))],
    );
    let address = function(&mut engine, &importer, "size");
    let size = bind::<(), i64>(&mut engine, address);
    assert_eq!(size.invoke(&mut engine, ()).ok(), Some(1));
    for sharing in [Sharing::Unshared, Sharing::Shared] {
        let allocated = engine
            .allocate_memory(MemoryType {
                address_type: AddressType::I64,
                limits: Limits {
                    minimum: 1,
                    maximum: Some(3),
                },
                sharing,
            })
            .expect("the memory");
        assert_eq!(
            engine.write_memory(&allocated, 16, &7_i64.to_le_bytes()),
            Ok(())
        );
        assert_eq!(engine.grow_memory(&allocated, 2), Ok(()));
        assert_eq!(engine.count_memory_pages(&allocated), 3);
        assert_eq!(engine.grow_memory(&allocated, 1), Err(GrowMemoryError));
        if sharing == Sharing::Unshared {
            let importer = instantiate(
                &mut engine,
                sizes,
                &[import("heap", ExternalAddress::Memory(allocated))],
            );
            let address = function(&mut engine, &importer, "fetch");
            let load = bind::<(i64,), i64>(&mut engine, address);
            assert_eq!(load.invoke(&mut engine, (16,)).ok(), Some(7));
        } else {
            assert!(engine.to_shared_memory(&allocated).is_some());
        }
    }
}

#[test]
fn bytes_beyond_4_gib_of_a_64_bit_memory_are_read_and_written() {
    let mut engine = engine(Config::default());
    let pages = (1 << u32::BITS) / PAGE_SIZE + 1;
    let instance = instantiate(
        &mut engine,
        &format!(
            r#"(module (memory (export "heap") i64 {pages})
                (func (export "fetch") (param i64) (result i64) (i64.load (local.get 0))))"#
        ),
        &[],
    );
    let exported = memory(&mut engine, &instance, "heap");
    let far = 1 << u32::BITS;
    let bytes = 0x0123_4567_89ab_cdef_i64.to_le_bytes();
    // Across the first 4 GiB and beyond them.
    for offset in [far - 4, far + 8] {
        assert_eq!(engine.write_memory(&exported, offset, &bytes), Ok(()));
        let mut read = [0; 8];
        assert_eq!(engine.read_memory(&exported, offset, &mut read), Ok(()));
        assert_eq!(read, bytes);
        let address = function(&mut engine, &instance, "fetch");
        let load = bind::<(i64,), i64>(&mut engine, address);
        assert_eq!(
            load.invoke(&mut engine, (offset.cast_signed(),)).ok(),
            Some(0x0123_4567_89ab_cdef)
        );
    }
    let end = pages * PAGE_SIZE;
    assert!(engine.read_memory(&exported, end - 4, &mut [0; 8]).is_err());
    assert!(engine.write_memory(&exported, end - 4, &bytes).is_err());
}

/// Exports a table and a global of typed references to functions that return an i32, and a function of that type
/// and one of another.
const HOLDS_TYPED_REFERENCES: &str = r#"(module
    (type $answers (func (result i32)))
    (table (export "elements") 1 3 (ref null $answers))
    (table (export "everywhere") i64 1 (ref null $answers))
    (global (export "variable") (mut (ref null $answers)) (ref.null $answers))
    (func (export "forty_two") (type $answers) (i32.const 3))
    (func (export "elsewhere") (result i64) (i64.const 4))
    (func (export "call_global") (result i32) (call_ref $answers (global.get 0))))"#;

#[test]
fn typed_function_references_are_written_into_tables_and_globals() {
    let mut engine = engine(Config::default());
    let instance = instantiate(&mut engine, HOLDS_TYPED_REFERENCES, &[]);
    let answer = function(&mut engine, &instance, "forty_two");
    let other = function(&mut engine, &instance, "elsewhere");
    for name in ["elements", "everywhere"] {
        let typed = table(&mut engine, &instance, name);
        assert_eq!(engine.write_table(&typed, 0, Some(&answer)), Ok(()));
        let read = engine
            .read_table(&typed, 0)
            .expect("the element is inside")
            .expect("the element refers to a function");
        let bound = bind::<(), i32>(&mut engine, read);
        assert_eq!(bound.invoke(&mut engine, ()).ok(), Some(3));
        assert_eq!(
            engine.write_table(&typed, 0, Some(&other)),
            Err(WriteTableError::ReferenceTypeMismatch)
        );
        assert_eq!(
            engine.write_table(&typed, 1, Some(&answer)),
            Err(WriteTableError::IndexOutsideTable)
        );
        assert_eq!(engine.grow_table(&typed, 1), Ok(()));
        assert_eq!(engine.count_table_elements(&typed), 2);
        assert_eq!(engine.write_table(&typed, 1, None), Ok(()));
        assert!(matches!(engine.read_table(&typed, 1), Ok(None)));
    }
    let typed = table(&mut engine, &instance, "elements");
    let importer = instantiate(
        &mut engine,
        r#"(module (type $answers (func (result i32)))
            (import "host" "elements" (table 1 3 (ref null $answers)))
            (func (export "caller") (result i32) (call_indirect (type $answers) (i32.const 0))))"#,
        &[import("elements", ExternalAddress::Table(typed))],
    );
    let address = function(&mut engine, &importer, "caller");
    let call = bind::<(), i32>(&mut engine, address);
    assert_eq!(call.invoke(&mut engine, ()).ok(), Some(3));

    let typed = global(&mut engine, &instance, "variable");
    assert_eq!(
        engine.write_global(&typed, Value::FunctionReference(Some(other))),
        Err(WriteGlobalError::ValueTypeMismatch)
    );
    assert_eq!(
        engine.write_global(&typed, Value::FunctionReference(Some(answer))),
        Ok(())
    );
    assert!(matches!(
        engine.read_global(&typed),
        Ok(Value::FunctionReference(Some(_)))
    ));
    let address = function(&mut engine, &instance, "call_global");
    let call_global = bind::<(), i32>(&mut engine, address);
    assert_eq!(call_global.invoke(&mut engine, ()).ok(), Some(3));
}

#[test]
fn a_64_bit_table_is_allocated_written_and_grown() {
    let mut engine = engine(Config::default());
    let instance = instantiate(&mut engine, EXPORTS_A_TABLE, &[]);
    let answer = function(&mut engine, &instance, "forty_two");
    let allocated = engine
        .allocate_table(TableType {
            address_type: AddressType::I64,
            limits: Limits {
                minimum: 1,
                maximum: Some(2),
            },
        })
        .expect("the table");
    assert_eq!(engine.write_table(&allocated, 0, Some(&answer)), Ok(()));
    let read = engine
        .read_table(&allocated, 0)
        .expect("the element is inside")
        .expect("the element refers to a function");
    let bound = bind::<(), i32>(&mut engine, read);
    assert_eq!(bound.invoke(&mut engine, ()).ok(), Some(42));
    assert_eq!(engine.grow_table(&allocated, 1), Ok(()));
    assert_eq!(engine.grow_table(&allocated, 1), Err(GrowTableError));
    assert_eq!(engine.count_table_elements(&allocated), 2);
}

/// Exports a tag and a function that throws its parameter with it.
const THROWS: &str = r#"(module
    (tag $failure (export "failure") (param i32))
    (func (export "throw") (param i32) (throw $failure (local.get 0))))"#;

/// Imports a tag and a function that throws, exports the tag again, and exports a function that calls the thrower
/// and returns what it caught with the tag, or -1.
const CATCHES: &str = r#"(module
    (import "host" "failure" (tag $failure (param i32)))
    (import "host" "throw" (func $throw (param i32)))
    (export "failure" (tag $failure))
    (func (export "catch") (param i32) (result i32)
        (block $caught (result i32)
            (try_table (catch $failure $caught) (call $throw (local.get 0)))
            (i32.const -1))))"#;

fn tag(engine: &mut Engine, instance: &Instance, name: &str) -> ExternalAddress<Engine> {
    let found = engine.find_export(instance, name);
    assert!(
        matches!(found, Some(ExternalAddress::Tag(_))),
        "{name} is an exported tag"
    );
    found.expect("the tag")
}

#[test]
fn tags_link_between_modules_as_the_tag_they_are() {
    let mut engine = engine(Config::default());
    let throws = compile(&engine, THROWS);
    let catches = compile(&engine, CATCHES);
    let first = engine.instantiate(&throws, &[]).expect("the thrower");
    let second = engine.instantiate(&throws, &[]).expect("another thrower");
    let throw = function(&mut engine, &first, "throw");
    let catcher = |engine: &mut Engine, failure| {
        let imports = [
            import("failure", failure),
            import("throw", ExternalAddress::Function(throw.clone())),
        ];
        let instance = engine
            .instantiate(&catches, &imports)
            .expect("the catcher instantiates with the tag");
        let address = function(engine, &instance, "catch");
        (bind::<(i32,), i32>(engine, address), instance)
    };
    let first_tag = tag(&mut engine, &first, "failure");
    let (catch, catching) = catcher(&mut engine, first_tag);
    assert_eq!(catch.invoke(&mut engine, (77,)).ok(), Some(77));
    // The tag the catcher imported and exports again is the thrower's.
    let passed_on = tag(&mut engine, &catching, "failure");
    let (catch_again, _) = catcher(&mut engine, passed_on);
    assert_eq!(catch_again.invoke(&mut engine, (78,)).ok(), Some(78));
    // Another instance of the same module has another tag, which does not catch the first one's exception.
    let other_tag = tag(&mut engine, &second, "failure");
    let (catch_other, _) = catcher(&mut engine, other_tag);
    assert!(matches!(
        catch_other.invoke(&mut engine, (79,)),
        Err(InvokeError::Exception)
    ));
}

/// Exports a function that returns one, and a memory.
const ANSWERS_ONE: &str = r#"(module
    (memory (export "bytes") 1)
    (func (export "one") (result i32) (i32.const 1)))"#;

/// Imports a function and a memory.
const IMPORTS_AN_ANSWER: &str = r#"(module
    (import "host" "one" (func (result i32)))
    (import "host" "bytes" (memory 1)))"#;

#[test]
fn an_address_another_engine_value_made_is_refused_at_instantiation() {
    let config = Config::default();
    let mut first = engine(config.clone());
    let mut second = engine(config);
    let answers = compile(&first, ANSWERS_ONE);
    let made_by_first = first
        .instantiate(&answers, &[])
        .expect("the module instantiates");
    // The second engine value holds an instance at the same place among its own.
    let made_by_second = second
        .instantiate(&answers, &[])
        .expect("the module instantiates");
    let importer = compile(&second, IMPORTS_AN_ANSWER);
    let foreign = function(&mut first, &made_by_first, "one");
    let own_memory = second
        .find_export(&made_by_second, "bytes")
        .expect("the memory");
    let refused = second.instantiate(
        &importer,
        &[
            import("one", ExternalAddress::Function(foreign)),
            import("bytes", own_memory),
        ],
    );
    assert!(matches!(
        refused,
        Err(InstantiateError::ForeignAddress { .. })
    ));
    let foreign_memory = first
        .find_export(&made_by_first, "bytes")
        .expect("the memory");
    let own_answer = function(&mut second, &made_by_second, "one");
    let refused = second.instantiate(
        &importer,
        &[
            import("one", ExternalAddress::Function(own_answer)),
            import("bytes", foreign_memory),
        ],
    );
    assert!(matches!(
        refused,
        Err(InstantiateError::ForeignAddress { .. })
    ));
}

#[test]
#[should_panic(expected = "engine value that made it")]
fn a_memory_another_engine_value_made_is_refused() {
    let config = Config::default();
    let mut first = engine(config.clone());
    let mut second = engine(config);
    let pages = 1;
    let foreign = first
        .allocate_memory(memory_type(AddressType::I32, pages, Sharing::Unshared))
        .expect("the memory");
    second
        .allocate_memory(memory_type(AddressType::I32, pages, Sharing::Unshared))
        .expect("the memory");
    let mut byte = [0];
    let _ = second.read_memory(&foreign, 0, &mut byte);
}

#[test]
#[should_panic(expected = "engine value that made it")]
fn a_function_another_engine_value_made_is_refused_when_bound() {
    let config = Config::default();
    let mut first = engine(config.clone());
    let mut second = engine(config);
    let answers = compile(&first, ANSWERS_ONE);
    let made_by_first = first
        .instantiate(&answers, &[])
        .expect("the module instantiates");
    second
        .instantiate(&answers, &[])
        .expect("the module instantiates");
    let foreign = function(&mut first, &made_by_first, "one");
    let _ = <Engine as FunctionTypeBinder<(), i32>>::bind_engine_function(&mut second, foreign);
}
