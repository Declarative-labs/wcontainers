//! Checks that WasmEdge runs each proposal the translated modules use, compiled ahead of time.
//!
//! Every module here is compiled through the engine's `compile`, which loads a native shared library: WasmEdge
//! either runs it as native code or refuses to load it, so a passing check ran compiled code.

use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder, InvokeError};
use wcontainers_webassembly_engine::{ExternalAddress, WebAssemblyEngine};
use wcontainers_webassembly_engine_wasmedge::{Error, WasmEdgeEngine};

type Engine = WasmEdgeEngine<()>;

/// Compiles and instantiates the module, then invokes its export with the parameters.
fn run<Parameters, Results>(
    text: &str,
    name: &str,
    parameters: Parameters,
) -> Result<Results, InvokeError<Error>>
where
    Engine: FunctionTypeBinder<Parameters, Results>,
{
    let mut engine = Engine::new(Default::default(), ()).expect("the engine is made");
    let bytes = wat::parse_str(text).expect("the text is a module");
    let module = engine
        .compile(&bytes)
        .expect("the module compiles ahead of time");
    let instance = engine
        .instantiate(&module, &[])
        .expect("the module instantiates");
    let Some(ExternalAddress::Function(function)) = engine.find_export(&instance, name) else {
        panic!("{name} is an exported function");
    };
    let function = <Engine as FunctionTypeBinder<Parameters, Results>>::bind_engine_function(
        &mut engine,
        function,
    )
    .expect("the function has the function type");
    function.invoke(&mut engine, parameters)
}

/// Tail calls: a recursion ten million calls deep that only `return_call` keeps from overflowing the stack.
#[test]
fn tail_calls() {
    let text = r#"(module
        (func $count (export "count") (param i64) (result i64)
            (if (result i64) (i64.eqz (local.get 0))
                (then (i64.const 7))
                (else (return_call $count (i64.sub (local.get 0) (i64.const 1)))))))"#;
    assert_eq!(
        run::<(i64,), i64>(text, "count", (10_000_000,)).ok(),
        Some(7)
    );
}

/// Typed function references: `call_ref` and `return_call_ref` through a non-null reference of a declared type.
#[test]
fn typed_function_references() {
    let text = r#"(module
        (type $unary (func (param i32) (result i32)))
        (func $double (type $unary) (i32.mul (local.get 0) (i32.const 2)))
        (elem declare func $double)
        (func $reference (result (ref $unary)) (ref.func $double))
        (func (export "apply") (param i32) (result i32)
            (call_ref $unary (local.get 0) (call $reference)))
        (func (export "tail") (param i32) (result i32)
            (return_call_ref $unary (local.get 0) (ref.as_non_null (call $reference)))))"#;
    assert_eq!(run::<(i32,), i32>(text, "apply", (21,)).ok(), Some(42));
    assert_eq!(run::<(i32,), i32>(text, "tail", (4,)).ok(), Some(8));
}

/// The exception handling of `try_table`, `throw`, `throw_ref` and `exnref`.
const EXCEPTIONS: &str = r#"(module
    (tag $failure (param i32))
    (func $fail (param i32) (throw $failure (local.get 0)))
    (func (export "catch") (param i32) (result i32)
        (block $caught (result i32)
            (try_table (catch $failure $caught) (call $fail (local.get 0)))
            (i32.const -1)))
    (func (export "rethrow") (param i32) (result i32)
        (block $outer (result i32)
            (try_table (catch $failure $outer)
                (block $inner (result i32 exnref)
                    (try_table (catch_ref $failure $inner) (call $fail (local.get 0)))
                    (unreachable))
                (throw_ref))
            (unreachable)))
    (func (export "uncaught") (call $fail (i32.const 1))))"#;

#[test]
fn exception_handling() {
    assert_eq!(run::<(i32,), i32>(EXCEPTIONS, "catch", (5,)).ok(), Some(5));
    assert_eq!(
        run::<(i32,), i32>(EXCEPTIONS, "rethrow", (6,)).ok(),
        Some(6)
    );
    assert!(matches!(
        run::<(), ()>(EXCEPTIONS, "uncaught", ()),
        Err(InvokeError::Exception)
    ));
}

/// Threads: atomic read-modify-write, wait and notify on a shared memory the module defines.
#[test]
fn threads_and_shared_memory() {
    let text = r#"(module
        (memory (export "memory") 1 1 shared)
        (func (export "add_twice") (param i32) (result i32)
            (drop (i32.atomic.rmw.add (i32.const 0) (local.get 0)))
            (i32.atomic.rmw.add (i32.const 0) (local.get 0)))
        (func (export "notify") (result i32) (memory.atomic.notify (i32.const 0) (i32.const 1)))
        (func (export "wait") (result i32)
            (atomic.fence)
            (memory.atomic.wait32 (i32.const 0) (i32.const 99) (i64.const 0))))"#;
    assert_eq!(run::<(i32,), i32>(text, "add_twice", (5,)).ok(), Some(5));
    assert_eq!(run::<(), i32>(text, "notify", ()).ok(), Some(0));
    // The memory does not hold the expected value, so the wait answers "not equal" at once.
    assert_eq!(run::<(), i32>(text, "wait", ()).ok(), Some(1));
}

/// 64-bit memories: loads and stores at 64-bit addresses, `memory.size` and `memory.grow`.
#[test]
fn memory64() {
    let text = r#"(module
        (memory (export "memory") i64 1)
        (func (export "store_load") (param i64 i64) (result i64)
            (i64.store (local.get 0) (local.get 1))
            (i64.load (local.get 0)))
        (func (export "grow") (param i64) (result i64)
            (drop (memory.grow (local.get 0)))
            (memory.size)))"#;
    assert_eq!(
        run::<(i64, i64), i64>(text, "store_load", (65_528, 42)).ok(),
        Some(42)
    );
    assert_eq!(run::<(i64,), i64>(text, "grow", (2,)).ok(), Some(3));
    assert!(matches!(
        run::<(i64, i64), i64>(text, "store_load", (65_536, 42)),
        Err(InvokeError::Trap(_))
    ));
}

/// Multiple memories: a store into one, a copy into the other, a load from that one.
#[test]
fn multiple_memories() {
    let text = r#"(module
        (memory $first 1)
        (memory $second 1)
        (func (export "copy") (param i32) (result i32)
            (i32.store $first (i32.const 0) (local.get 0))
            (memory.copy $second $first (i32.const 8) (i32.const 0) (i32.const 4))
            (i32.load $second (i32.const 8))))"#;
    assert_eq!(run::<(i32,), i32>(text, "copy", (1234,)).ok(), Some(1234));
}
