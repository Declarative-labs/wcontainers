//! Tables and globals whose reference type is a typed function reference, `(ref null $t)` or `(ref $t)`, hold
//! function references; those of a typed struct reference do not.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test as test;
use wasm_encoder::{
    CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, FieldType, FunctionSection,
    GlobalSection, GlobalType, HeapType, ImportSection, Module, RefType, TableSection, TableType,
    TypeSection, ValType,
};
use wcontainers_webassembly_engine::function::{BoundFunction, FunctionTypeBinder};
use wcontainers_webassembly_engine::global::{
    GlobalStore, ReadGlobalError, Value, WriteGlobalError,
};
use wcontainers_webassembly_engine::table::{ReadTableError, TableStore, WriteTableError};
use wcontainers_webassembly_engine::{ExternalAddress, Import, WebAssemblyEngine};
use wcontainers_webassembly_engine_browser::{BrowserEngine, BrowserGlobal, BrowserTable};

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

type Engine = BrowserEngine<i32>;
type Instance = <Engine as WebAssemblyEngine>::Instance;

const NO_VALUES: [ValType; 0] = [];

/// What the function of the typed references returns.
const RETURNED: i32 = i32::MAX;

/// The module's types, in order: a function returning an i32, a function of no values, and a struct with no fields.
const RETURNING: u32 = 0;
const EMPTY: u32 = 1;
const STRUCT: u32 = 2;

const RETURNING_FUNCTION: &str = "returning";
const EMPTY_FUNCTION: &str = "empty";
const NULLABLE_TABLE: &str = "nullable_table";
const NON_NULL_TABLE: &str = "non_null_table";
const STRUCT_TABLE: &str = "struct_table";
const VARIABLE_GLOBAL: &str = "variable_global";
const CONSTANT_GLOBAL: &str = "constant_global";
const STRUCT_GLOBAL: &str = "struct_global";
const NAMESPACE: &str = "typed";

fn reference(nullable: bool, type_index: u32) -> RefType {
    RefType {
        nullable,
        heap_type: HeapType::Concrete(type_index),
    }
}

fn table(element_type: RefType) -> TableType {
    TableType {
        element_type,
        table64: false,
        minimum: 1,
        maximum: Some(1),
        shared: false,
    }
}

fn global(reference: RefType, mutable: bool) -> GlobalType {
    GlobalType {
        val_type: ValType::Ref(reference),
        mutable,
        shared: false,
    }
}

fn types() -> TypeSection {
    let mut types = TypeSection::new();
    types.ty().function(NO_VALUES, [ValType::I32]);
    types.ty().function(NO_VALUES, NO_VALUES);
    types.ty().struct_([] as [FieldType; 0]);
    types
}

/// A module exporting a function of each function type, a table of each kind of typed reference, a variable and a
/// constant global of a typed function reference, and a global of a typed struct reference.
fn typed_module() -> Vec<u8> {
    let mut functions = FunctionSection::new();
    functions.function(RETURNING).function(EMPTY);
    let mut tables = TableSection::new();
    tables
        .table(table(reference(true, RETURNING)))
        .table_with_init(table(reference(false, RETURNING)), &ConstExpr::ref_func(0))
        .table(table(reference(true, STRUCT)));
    let mut globals = GlobalSection::new();
    globals
        .global(
            global(reference(true, RETURNING), true),
            &ConstExpr::ref_func(0),
        )
        .global(
            global(reference(true, RETURNING), false),
            &ConstExpr::ref_null(HeapType::Concrete(RETURNING)),
        )
        .global(
            global(reference(true, STRUCT), true),
            &ConstExpr::ref_null(HeapType::Concrete(STRUCT)),
        );
    let mut exports = ExportSection::new();
    exports
        .export(RETURNING_FUNCTION, ExportKind::Func, 0)
        .export(EMPTY_FUNCTION, ExportKind::Func, 1)
        .export(NULLABLE_TABLE, ExportKind::Table, 0)
        .export(NON_NULL_TABLE, ExportKind::Table, 1)
        .export(STRUCT_TABLE, ExportKind::Table, 2)
        .export(VARIABLE_GLOBAL, ExportKind::Global, 0)
        .export(CONSTANT_GLOBAL, ExportKind::Global, 1)
        .export(STRUCT_GLOBAL, ExportKind::Global, 2);
    let mut returning = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
    returning.instructions().i32_const(RETURNED).end();
    let mut empty = wasm_encoder::Function::new_with_locals_types(NO_VALUES);
    empty.instructions().end();
    let mut code = CodeSection::new();
    code.function(&returning).function(&empty);
    let mut module = Module::new();
    module
        .section(&types())
        .section(&functions)
        .section(&tables)
        .section(&globals)
        .section(&exports)
        .section(&code);
    module.finish()
}

/// A module that imports a nullable typed function reference table and a variable global of that type, and exports
/// them again under their own names.
fn reexporting_module() -> Vec<u8> {
    let mut imports = ImportSection::new();
    imports
        .import(
            NAMESPACE,
            NULLABLE_TABLE,
            EntityType::Table(table(reference(true, RETURNING))),
        )
        .import(
            NAMESPACE,
            VARIABLE_GLOBAL,
            EntityType::Global(global(reference(true, RETURNING), true)),
        );
    let mut exports = ExportSection::new();
    exports.export(NULLABLE_TABLE, ExportKind::Table, 0).export(
        VARIABLE_GLOBAL,
        ExportKind::Global,
        0,
    );
    let mut module = Module::new();
    module.section(&types()).section(&imports).section(&exports);
    module.finish()
}

fn new_engine() -> Engine {
    let Ok(engine) = BrowserEngine::new((), i32::default());
    engine
}

fn instantiate(engine: &mut Engine, bytes: &[u8], imports: &[Import<'_, Engine>]) -> Instance {
    let module = engine.compile(bytes).expect("the module compiles");
    engine
        .instantiate(&module, imports)
        .expect("the module instantiates")
}

fn function(engine: &mut Engine, instance: &Instance, name: &str) -> js_sys::Function {
    let Some(ExternalAddress::Function(function)) = engine.find_export(instance, name) else {
        panic!("the module exports a function under {name}");
    };
    function
}

fn exported_table(engine: &mut Engine, instance: &Instance, name: &str) -> BrowserTable {
    let Some(ExternalAddress::Table(table)) = engine.find_export(instance, name) else {
        panic!("the module exports a table under {name}");
    };
    table
}

fn exported_global(engine: &mut Engine, instance: &Instance, name: &str) -> BrowserGlobal {
    let Some(ExternalAddress::Global(global)) = engine.find_export(instance, name) else {
        panic!("the module exports a global under {name}");
    };
    global
}

/// Calls the function, which takes nothing and returns an i32.
fn call(engine: &mut Engine, function: js_sys::Function) -> i32 {
    FunctionTypeBinder::<(), i32>::bind_engine_function(engine, function)
        .expect("the function returns an i32")
        .invoke(engine, ())
        .expect("the function returns")
}

/// Checks a nullable table of the type of `returning`: it starts null, takes `returning` and null, and refuses `empty`.
fn check_nullable_table(
    engine: &mut Engine,
    table: &BrowserTable,
    returning: &js_sys::Function,
    empty: &js_sys::Function,
) {
    assert_eq!(engine.read_table(table, 0), Ok(None));
    assert_eq!(engine.write_table(table, 0, Some(returning)), Ok(()));
    let read = engine
        .read_table(table, 0)
        .expect("the element is a function reference")
        .expect("the element is not null");
    assert_eq!(call(engine, read), RETURNED);
    assert_eq!(
        engine.write_table(table, 0, Some(empty)),
        Err(WriteTableError::ReferenceTypeMismatch)
    );
    assert_eq!(engine.write_table(table, 0, None), Ok(()));
    assert_eq!(engine.read_table(table, 0), Ok(None));
    assert_eq!(
        engine.read_table(table, 1),
        Err(ReadTableError::IndexOutsideTable)
    );
}

/// Checks a variable global of the type of `returning`, which holds it: it reads as `returning`, takes null and
/// `returning` again, and refuses `empty`.
fn check_variable_global(
    engine: &mut Engine,
    global: &BrowserGlobal,
    returning: &js_sys::Function,
    empty: &js_sys::Function,
) {
    let Ok(Value::FunctionReference(Some(read))) = engine.read_global(global) else {
        panic!("the global holds a function reference");
    };
    assert_eq!(call(engine, read), RETURNED);
    assert_eq!(
        engine.write_global(global, Value::FunctionReference(Some(empty.clone()))),
        Err(WriteGlobalError::ValueTypeMismatch)
    );
    assert_eq!(
        engine.write_global(global, Value::I32(RETURNED)),
        Err(WriteGlobalError::ValueTypeMismatch)
    );
    assert_eq!(
        engine.write_global(global, Value::FunctionReference(None)),
        Ok(())
    );
    assert_eq!(
        engine.read_global(global),
        Ok(Value::FunctionReference(None))
    );
    assert_eq!(
        engine.write_global(global, Value::FunctionReference(Some(returning.clone()))),
        Ok(())
    );
}

#[test]
fn a_typed_function_reference_table_holds_function_references() {
    let mut engine = new_engine();
    let instance = instantiate(&mut engine, &typed_module(), &[]);
    let returning = function(&mut engine, &instance, RETURNING_FUNCTION);
    let empty = function(&mut engine, &instance, EMPTY_FUNCTION);
    let nullable = exported_table(&mut engine, &instance, NULLABLE_TABLE);
    check_nullable_table(&mut engine, &nullable, &returning, &empty);

    let non_null = exported_table(&mut engine, &instance, NON_NULL_TABLE);
    let element = engine
        .read_table(&non_null, 0)
        .expect("the element is a function reference")
        .expect("the element is not null");
    assert_eq!(call(&mut engine, element), RETURNED);
    assert_eq!(
        engine.write_table(&non_null, 0, None),
        Err(WriteTableError::ReferenceTypeMismatch)
    );
    assert_eq!(
        engine.write_table(&non_null, 0, Some(&empty)),
        Err(WriteTableError::ReferenceTypeMismatch)
    );
    assert_eq!(engine.write_table(&non_null, 0, Some(&returning)), Ok(()));
}

#[test]
fn a_typed_struct_reference_table_holds_no_function_references() {
    let mut engine = new_engine();
    let instance = instantiate(&mut engine, &typed_module(), &[]);
    let returning = function(&mut engine, &instance, RETURNING_FUNCTION);
    let table = exported_table(&mut engine, &instance, STRUCT_TABLE);
    assert_eq!(
        engine.read_table(&table, 0),
        Err(ReadTableError::ElementNotFunction)
    );
    assert_eq!(
        engine.write_table(&table, 0, Some(&returning)),
        Err(WriteTableError::ReferenceTypeMismatch)
    );
}

#[test]
fn a_typed_function_reference_global_holds_a_function_reference() {
    let mut engine = new_engine();
    let instance = instantiate(&mut engine, &typed_module(), &[]);
    let returning = function(&mut engine, &instance, RETURNING_FUNCTION);
    let empty = function(&mut engine, &instance, EMPTY_FUNCTION);
    let variable = exported_global(&mut engine, &instance, VARIABLE_GLOBAL);
    check_variable_global(&mut engine, &variable, &returning, &empty);

    let constant = exported_global(&mut engine, &instance, CONSTANT_GLOBAL);
    assert_eq!(
        engine.read_global(&constant),
        Ok(Value::FunctionReference(None))
    );
    assert_eq!(
        engine.write_global(&constant, Value::FunctionReference(Some(returning))),
        Err(WriteGlobalError::Constant)
    );

    let structure = exported_global(&mut engine, &instance, STRUCT_GLOBAL);
    assert_eq!(engine.read_global(&structure), Err(ReadGlobalError));
    assert_eq!(
        engine.write_global(&structure, Value::FunctionReference(None)),
        Err(WriteGlobalError::ValueTypeMismatch)
    );
}

#[test]
fn a_typed_table_and_global_another_module_exports_again_keep_their_type() {
    let mut engine = new_engine();
    let instance = instantiate(&mut engine, &typed_module(), &[]);
    let returning = function(&mut engine, &instance, RETURNING_FUNCTION);
    let empty = function(&mut engine, &instance, EMPTY_FUNCTION);
    let table = exported_table(&mut engine, &instance, NULLABLE_TABLE);
    let global = exported_global(&mut engine, &instance, VARIABLE_GLOBAL);
    let imports = [
        Import {
            module_name: NAMESPACE,
            item_name: NULLABLE_TABLE,
            address: ExternalAddress::Table(table),
        },
        Import {
            module_name: NAMESPACE,
            item_name: VARIABLE_GLOBAL,
            address: ExternalAddress::Global(global),
        },
    ];
    let again = instantiate(&mut engine, &reexporting_module(), &imports);
    let table = exported_table(&mut engine, &again, NULLABLE_TABLE);
    check_nullable_table(&mut engine, &table, &returning, &empty);
    let global = exported_global(&mut engine, &again, VARIABLE_GLOBAL);
    check_variable_global(&mut engine, &global, &returning, &empty);
}
