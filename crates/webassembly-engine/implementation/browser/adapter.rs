//! The small modules this engine writes: they move values between a function's arguments and results and the
//! scratch slots of this thread, so that a call needs no JavaScript value but the function itself.

use core::any::{Any, TypeId};
use core::cell::Cell;
use core::mem::size_of;

use wasm_encoder::{
    CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, Function, FunctionSection,
    GlobalSection, GlobalType, ImportSection, InstructionSink, MemArg, MemArgTarget, MemoryType,
    Module, RefType, TypeSection, ValType,
};
use wcontainers_webassembly_engine::memory::PAGE_SIZE;

/// The most arguments a bound function takes, each in a slot of its own.
pub const SLOTS: usize = 16;

/// One value, in the bytes a WebAssembly load or store of its type reads or writes at the slot's start.
type Slot = Cell<u128>;

thread_local! {
    /// The arguments of the call being made and then its result, in this thread's part of our own memory.
    static SCRATCH: [Slot; SLOTS] = const { [const { Cell::new(0) }; SLOTS] };
}

/// The namespace every import of these modules is in.
pub const NAMESPACE: &str = "a";
/// The name of the function an adapter calls.
pub const CALLEE: &str = "f";
/// The name of our own memory, which holds the scratch slots.
pub const SCRATCH_MEMORY: &str = "m";
/// The name of the function an adapter exports.
pub const ENTRY: &str = "e";
/// The name of a global these modules import or export.
pub const GLOBAL: &str = "g";
/// The name of a guest memory a memory access module imports.
pub const GUEST_MEMORY: &str = "g";
/// The name of the function that writes a global's value into the first slot, or copies out of a guest memory.
pub const READ: &str = "r";
/// The name of the function that sets a global to the value in the first slot, or copies into a guest memory.
pub const WRITE: &str = "w";

/// No value types: the parameters, results or locals of a function that has none.
const NO_VALUES: [ValType; 0] = [];

/// Returns the slot's value.
#[inline(always)]
pub fn slot(index: usize) -> u128 {
    SCRATCH.with(|scratch| scratch[index].get())
}

/// Sets the slot's value.
#[inline(always)]
pub fn set_slot(index: usize, value: u128) {
    SCRATCH.with(|scratch| scratch[index].set(value));
}

/// Returns where this thread's slots start in our own memory.
fn scratch_address() -> i32 {
    SCRATCH
        .with(|scratch| scratch.as_ptr() as usize as u32)
        .cast_signed()
}

/// Returns the WebAssembly value type a Rust type stands for, or nothing when it stands for none.
pub fn value_type<T: 'static>() -> Option<ValType> {
    let id = TypeId::of::<T>();
    if id == TypeId::of::<i32>() || id == TypeId::of::<u32>() {
        Some(ValType::I32)
    } else if id == TypeId::of::<i64>() || id == TypeId::of::<u64>() {
        Some(ValType::I64)
    } else if id == TypeId::of::<f32>() {
        Some(ValType::F32)
    } else if id == TypeId::of::<f64>() {
        Some(ValType::F64)
    } else if id == TypeId::of::<u128>() || id == TypeId::of::<i128>() {
        Some(ValType::V128)
    } else {
        None
    }
}

/// Returns the value types of a Rust result: none for `()`, one for a value type, and nothing for any other type.
pub fn result_types<T: 'static>() -> Option<Vec<ValType>> {
    if TypeId::of::<T>() == TypeId::of::<()>() {
        Some(Vec::new())
    } else {
        value_type::<T>().map(|value_type| vec![value_type])
    }
}

/// Returns the bits a value of a value type leaves in a slot.
#[inline(always)]
pub fn to_slot<T: 'static>(value: T) -> u128 {
    let value: &dyn Any = &value;
    if let Some(value) = value.downcast_ref::<i32>() {
        u128::from(value.cast_unsigned())
    } else if let Some(value) = value.downcast_ref::<u32>() {
        u128::from(*value)
    } else if let Some(value) = value.downcast_ref::<i64>() {
        u128::from(value.cast_unsigned())
    } else if let Some(value) = value.downcast_ref::<u64>() {
        u128::from(*value)
    } else if let Some(value) = value.downcast_ref::<f32>() {
        u128::from(value.to_bits())
    } else if let Some(value) = value.downcast_ref::<f64>() {
        u128::from(value.to_bits())
    } else if let Some(value) = value.downcast_ref::<u128>() {
        *value
    } else if let Some(value) = value.downcast_ref::<i128>() {
        value.cast_unsigned()
    } else {
        u128::MIN
    }
}

/// Returns the value of a value type, or `()`, that the bits of a slot hold.
#[inline(always)]
pub fn from_slot<T: 'static>(bits: u128) -> T {
    let mut value: Option<T> = None;
    let slot: &mut dyn Any = &mut value;
    if let Some(slot) = slot.downcast_mut::<Option<i32>>() {
        *slot = Some((bits as u32).cast_signed());
    } else if let Some(slot) = slot.downcast_mut::<Option<u32>>() {
        *slot = Some(bits as u32);
    } else if let Some(slot) = slot.downcast_mut::<Option<i64>>() {
        *slot = Some((bits as u64).cast_signed());
    } else if let Some(slot) = slot.downcast_mut::<Option<u64>>() {
        *slot = Some(bits as u64);
    } else if let Some(slot) = slot.downcast_mut::<Option<f32>>() {
        *slot = Some(f32::from_bits(bits as u32));
    } else if let Some(slot) = slot.downcast_mut::<Option<f64>>() {
        *slot = Some(f64::from_bits(bits as u64));
    } else if let Some(slot) = slot.downcast_mut::<Option<u128>>() {
        *slot = Some(bits);
    } else if let Some(slot) = slot.downcast_mut::<Option<i128>>() {
        *slot = Some(bits.cast_signed());
    } else if let Some(slot) = slot.downcast_mut::<Option<()>>() {
        *slot = Some(());
    }
    value.expect("binding a function checks that each of its types is a value type")
}

/// Returns the type a memory is imported under that takes any memory of its address type and sharing.
fn any_memory(memory64: bool, shared: bool) -> MemoryType {
    let addresses = if memory64 {
        u128::from(u64::MAX)
    } else {
        u128::from(u32::MAX)
    } + 1;
    MemoryType {
        minimum: u64::MIN,
        maximum: shared.then_some((addresses / u128::from(PAGE_SIZE)) as u64),
        memory64,
        shared,
        page_size_log2: None,
    }
}

/// Returns the type our own memory is imported under.
fn scratch_memory() -> MemoryType {
    any_memory(false, cfg!(target_feature = "atomics"))
}

/// Returns where in the scratch memory the slot lies, from the address on the stack.
fn slot_argument(index: usize) -> MemArg {
    MemArg {
        offset: (index * size_of::<Slot>()) as u64,
        align: u32::MIN,
        target: MemArgTarget::Memory(u32::MIN),
        ordering: None,
    }
}

fn load(sink: &mut InstructionSink<'_>, value_type: ValType, index: usize) {
    sink.i32_const(scratch_address());
    let argument = slot_argument(index);
    match value_type {
        ValType::I32 => sink.i32_load(argument),
        ValType::I64 => sink.i64_load(argument),
        ValType::F32 => sink.f32_load(argument),
        ValType::F64 => sink.f64_load(argument),
        ValType::V128 | ValType::Ref(_) => sink.v128_load(argument),
    };
}

/// Stores the value under the address that is on the stack below it.
fn store(sink: &mut InstructionSink<'_>, value_type: ValType, index: usize) {
    let argument = slot_argument(index);
    match value_type {
        ValType::I32 => sink.i32_store(argument),
        ValType::I64 => sink.i64_store(argument),
        ValType::F32 => sink.f32_store(argument),
        ValType::F64 => sink.f64_store(argument),
        ValType::V128 | ValType::Ref(_) => sink.v128_store(argument),
    };
}

fn assemble(
    types: &TypeSection,
    imports: &ImportSection,
    functions: &FunctionSection,
    exports: &ExportSection,
    bodies: &[Function],
) -> Vec<u8> {
    let mut code = CodeSection::new();
    for body in bodies {
        code.function(body);
    }
    let mut module = Module::new();
    module
        .section(types)
        .section(imports)
        .section(functions)
        .section(exports)
        .section(&code);
    module.finish()
}

/// A module that imports a function of the given type, and exports one without arguments or results that calls it
/// with the arguments in the slots and leaves its results in the slots.
pub fn caller(parameters: &[ValType], results: &[ValType]) -> Vec<u8> {
    let mut types = TypeSection::new();
    types
        .ty()
        .function(parameters.iter().copied(), results.iter().copied());
    types.ty().function(NO_VALUES, NO_VALUES);
    let mut imports = ImportSection::new();
    imports.import(NAMESPACE, CALLEE, EntityType::Function(0));
    imports.import(NAMESPACE, SCRATCH_MEMORY, scratch_memory());
    let mut functions = FunctionSection::new();
    functions.function(1);
    let mut exports = ExportSection::new();
    exports.export(ENTRY, ExportKind::Func, 1);
    let mut body = Function::new_with_locals_types(results.iter().copied());
    let mut sink = body.instructions();
    for (index, value_type) in parameters.iter().enumerate() {
        load(&mut sink, *value_type, index);
    }
    sink.call(0);
    for index in (0..results.len()).rev() {
        sink.local_set(index as u32);
    }
    for (index, value_type) in results.iter().enumerate() {
        sink.i32_const(scratch_address());
        sink.local_get(index as u32);
        store(&mut sink, *value_type, index);
    }
    sink.end();
    assemble(&types, &imports, &functions, &exports, &[body])
}

/// A module that imports a function taking the index of a host function, and exports one of the given type that
/// leaves its arguments in the slots, calls the import with the index, and returns the results left in the slots.
pub fn host(parameters: &[ValType], results: &[ValType], host_function: u32) -> Vec<u8> {
    let mut types = TypeSection::new();
    types
        .ty()
        .function(parameters.iter().copied(), results.iter().copied());
    types.ty().function([ValType::I32], []);
    let mut imports = ImportSection::new();
    imports.import(NAMESPACE, CALLEE, EntityType::Function(1));
    imports.import(NAMESPACE, SCRATCH_MEMORY, scratch_memory());
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export(ENTRY, ExportKind::Func, 1);
    let mut body = Function::new_with_locals_types(NO_VALUES);
    let mut sink = body.instructions();
    for (index, value_type) in parameters.iter().enumerate() {
        sink.i32_const(scratch_address());
        sink.local_get(index as u32);
        store(&mut sink, *value_type, index);
    }
    sink.i32_const(host_function.cast_signed());
    sink.call(0);
    for (index, value_type) in results.iter().enumerate() {
        load(&mut sink, *value_type, index);
    }
    sink.end();
    assemble(&types, &imports, &functions, &exports, &[body])
}

/// The value types a global may have that `Value` can hold, in the order a global's type is looked for.
pub const GLOBAL_VALUE_TYPES: [ValType; 6] = [
    ValType::I32,
    ValType::I64,
    ValType::F32,
    ValType::F64,
    ValType::V128,
    ValType::Ref(RefType::FUNCREF),
];

/// A module that imports a global of the given type, so that it instantiates only with such a global; for a vector,
/// it also exports `READ` and, when the global is mutable, `WRITE`, which move its value through the first slot.
pub fn global_access(value_type: ValType, mutable: bool) -> Vec<u8> {
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    imports.import(
        NAMESPACE,
        GLOBAL,
        GlobalType {
            val_type: value_type,
            mutable,
            shared: false,
        },
    );
    let mut functions = FunctionSection::new();
    let mut exports = ExportSection::new();
    let mut bodies = Vec::new();
    if value_type == ValType::V128 {
        types.ty().function(NO_VALUES, NO_VALUES);
        imports.import(NAMESPACE, SCRATCH_MEMORY, scratch_memory());
        let mut read = Function::new_with_locals_types(NO_VALUES);
        let mut sink = read.instructions();
        sink.i32_const(scratch_address());
        sink.global_get(0);
        store(&mut sink, value_type, 0);
        sink.end();
        functions.function(0);
        exports.export(READ, ExportKind::Func, 0);
        bodies.push(read);
        if mutable {
            let mut write = Function::new_with_locals_types(NO_VALUES);
            let mut sink = write.instructions();
            load(&mut sink, value_type, 0);
            sink.global_set(0);
            sink.end();
            functions.function(0);
            exports.export(WRITE, ExportKind::Func, 1);
            bodies.push(write);
        }
    }
    assemble(&types, &imports, &functions, &exports, &bodies)
}

/// The slots of a copy between a guest memory and our own.
pub const GUEST_OFFSET: usize = 0;
pub const OWN_ADDRESS: usize = 1;
pub const LENGTH: usize = 2;

/// A module that imports a guest memory of the address type and sharing, and exports `READ`, which copies from it
/// into our own memory, and `WRITE`, which copies back: the slots hold the guest offset, our address and the length.
pub fn memory_access(memory64: bool, shared: bool) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function(NO_VALUES, NO_VALUES);
    let mut imports = ImportSection::new();
    imports.import(NAMESPACE, SCRATCH_MEMORY, scratch_memory());
    imports.import(NAMESPACE, GUEST_MEMORY, any_memory(memory64, shared));
    let mut functions = FunctionSection::new();
    let mut exports = ExportSection::new();
    let guest_offset = if memory64 { ValType::I64 } else { ValType::I32 };
    let (own, guest) = (0, 1);
    let mut bodies = Vec::new();
    for (index, name, into, from) in [(0, READ, own, guest), (1, WRITE, guest, own)] {
        let mut body = Function::new_with_locals_types(NO_VALUES);
        let mut sink = body.instructions();
        for memory in [into, from] {
            if memory == guest {
                load(&mut sink, guest_offset, GUEST_OFFSET);
            } else {
                load(&mut sink, ValType::I32, OWN_ADDRESS);
            }
        }
        load(&mut sink, ValType::I32, LENGTH);
        sink.memory_copy(into, from);
        sink.end();
        functions.function(0);
        exports.export(name, ExportKind::Func, index);
        bodies.push(body);
    }
    assemble(&types, &imports, &functions, &exports, &bodies)
}

/// A module that defines and exports one vector global holding the value.
pub fn vector_global(mutable: bool, value: u128) -> Vec<u8> {
    let mut globals = GlobalSection::new();
    globals.global(
        GlobalType {
            val_type: ValType::V128,
            mutable,
            shared: false,
        },
        &ConstExpr::v128_const(value.cast_signed()),
    );
    let mut exports = ExportSection::new();
    exports.export(GLOBAL, ExportKind::Global, 0);
    let mut module = Module::new();
    module.section(&globals).section(&exports);
    module.finish()
}
