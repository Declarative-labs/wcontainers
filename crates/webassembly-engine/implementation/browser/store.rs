//! Memories, tables and globals: the JavaScript API's objects, and for vector globals a small module of each.

use std::cell::OnceCell;
use std::sync::Arc;

use js_sys::Function;
use js_sys::WebAssembly::{Global, Instance, Memory, Module, Table};
use wasm_bindgen::{JsCast, JsValue};
use wasm_encoder::{RefType, ValType};
use wcontainers_webassembly_engine::AddressType;
use wcontainers_webassembly_engine::global::{
    GlobalStore, Mutability, ReadGlobalError, Value, WriteGlobalError,
};
use wcontainers_webassembly_engine::memory::{
    GrowMemoryError, MemoryStore, MemoryType, PAGE_SIZE, ReadMemoryError, Sharing, WriteMemoryError,
};
use wcontainers_webassembly_engine::table::{
    GrowTableError, ReadTableError, TableStore, TableType, WriteTableError,
};

use crate::adapter::{
    self, CopySlot, GLOBAL, GUEST_MEMORY, NAMESPACE, READ, SCRATCH_MEMORY, WRITE, set_slot, slot,
};
use crate::declared::global_kinds;
use crate::{BrowserEngine, BrowserSharedMemory, glue, new_id};

/// A memory: the JavaScript API's object, and the exports of its access module, which copy out of it and into it.
#[derive(Clone)]
pub struct BrowserMemory {
    memory: Memory,
    memory64: bool,
    read: Function,
    write: Function,
}

/// Answers whether the memory has the 64-bit address type.
pub fn memory64(memory: &BrowserMemory) -> bool {
    memory.memory64
}

/// Returns the object the JavaScript API knows the memory by.
pub fn memory_object(memory: &BrowserMemory) -> &Memory {
    &memory.memory
}

thread_local! {
    /// A memory access module for each sharing and address type, by whether shared and whether 64-bit.
    static MEMORY_ACCESS: [[OnceCell<Module>; 2]; 2] = const { [const { [const { OnceCell::new() }; 2] }; 2] };
}

/// Instantiates the access module of the address type and sharing with the memory, or answers nothing when the memory
/// has another address type.
fn memory_access(memory: &Memory, memory64: bool, shared: bool) -> Option<Instance> {
    MEMORY_ACCESS.with(|modules| {
        let module = modules[usize::from(shared)][usize::from(memory64)].get_or_init(|| {
            glue::compile(&adapter::memory_access(memory64, shared))
                .expect("a memory access module is valid")
        });
        glue::instantiate_with_two(
            module,
            NAMESPACE,
            SCRATCH_MEMORY,
            &wasm_bindgen::memory(),
            GUEST_MEMORY,
            memory,
        )
        .ok()
    })
}

/// Describes a memory of the address type, or of either when it is not known, with its access module.
pub fn memory_of(memory: Memory, memory64: Option<bool>) -> BrowserMemory {
    let shared = glue::is_shared(&memory);
    let (memory64, instance) = memory64
        .map_or_else(|| vec![false, true], |memory64| vec![memory64])
        .into_iter()
        .find_map(|memory64| {
            memory_access(&memory, memory64, shared).map(|instance| (memory64, instance))
        })
        .expect("a memory has one of the address types its access module is made for");
    BrowserMemory {
        memory,
        memory64,
        read: glue::instance_export(&instance, READ).unchecked_into(),
        write: glue::instance_export(&instance, WRITE).unchecked_into(),
    }
}

/// Copies between the memory and our own bytes at the address through the access module's export, answering whether
/// every byte lay inside the memory.
#[inline(always)]
fn copy(
    memory: &BrowserMemory,
    copier: &Function,
    offset: u64,
    own_address: usize,
    length: usize,
) -> bool {
    if !memory.memory64 && offset > u64::from(u32::MAX) {
        return false;
    }
    set_slot(CopySlot::GuestOffset as usize, u128::from(offset));
    set_slot(CopySlot::OwnAddress as usize, own_address as u128);
    set_slot(CopySlot::Length as usize, length as u128);
    glue::call(copier) || {
        glue::take_thrown();
        false
    }
}

impl<HostContext> MemoryStore for BrowserEngine<HostContext> {
    type MemoryAddress = BrowserMemory;

    type AllocateMemoryError = JsValue;
    fn allocate_memory(&mut self, memory_type: MemoryType) -> Result<BrowserMemory, JsValue> {
        let memory64 = memory_type.address_type == AddressType::I64;
        let memory = glue::new_memory(
            memory_type.limits.minimum as f64,
            memory_type.limits.maximum.map(|maximum| maximum as f64),
            memory_type.sharing == Sharing::Shared,
            memory64,
        )?;
        Ok(memory_of(memory, Some(memory64)))
    }

    fn count_memory_pages(&self, memory: &BrowserMemory) -> u64 {
        glue::memory_bytes(&memory.memory) as u64 / PAGE_SIZE
    }

    fn grow_memory(&mut self, memory: &BrowserMemory, pages: u64) -> Result<(), GrowMemoryError> {
        glue::grow_memory(&memory.memory, pages as f64, memory.memory64)
            .then_some(())
            .ok_or(GrowMemoryError)
    }

    #[inline]
    fn read_memory(
        &self,
        memory: &BrowserMemory,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), ReadMemoryError> {
        copy(
            memory,
            &memory.read,
            offset,
            into.as_mut_ptr() as usize,
            into.len(),
        )
        .then_some(())
        .ok_or(ReadMemoryError)
    }

    #[inline]
    fn write_memory(
        &mut self,
        memory: &BrowserMemory,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), WriteMemoryError> {
        copy(
            memory,
            &memory.write,
            offset,
            bytes.as_ptr() as usize,
            bytes.len(),
        )
        .then_some(())
        .ok_or(WriteMemoryError)
    }

    type SharedMemory = BrowserSharedMemory;

    fn to_shared_memory(&self, memory: &BrowserMemory) -> Option<BrowserSharedMemory> {
        glue::is_shared(&memory.memory).then(|| {
            let registration = new_id(glue::forget_shared_memory);
            glue::register_shared_memory(registration.registered.id, &memory.memory);
            BrowserSharedMemory {
                registration: Arc::new(registration),
                memory64: memory.memory64,
            }
        })
    }

    fn add_shared_memory(&mut self, memory: &BrowserSharedMemory) -> BrowserMemory {
        let object = glue::registered_shared_memory(memory.registration.registered.id)
            .expect("a shared memory is added in a worker it was made in or posted to");
        memory_of(object, Some(memory.memory64))
    }
}

/// A table: the JavaScript API's object, its address type, and whether its elements refer to functions.
#[derive(Clone)]
pub struct BrowserTable {
    table: Table,
    table64: bool,
    functions: bool,
}

/// Returns the object the JavaScript API knows the table by.
pub fn table_object(table: &BrowserTable) -> &Table {
    &table.table
}

/// Describes a table, whose elements refer to functions or not as its module declares.
pub fn table_of(table: Table, functions: bool) -> BrowserTable {
    let table64 = glue::is_table64(&table);
    BrowserTable {
        table,
        table64,
        functions,
    }
}

impl<HostContext> TableStore for BrowserEngine<HostContext> {
    type TableAddress = BrowserTable;

    type AllocateTableError = JsValue;
    fn allocate_table(&mut self, table_type: TableType) -> Result<BrowserTable, JsValue> {
        let table64 = table_type.address_type == AddressType::I64;
        Ok(BrowserTable {
            table: glue::new_table(
                table_type.limits.minimum as f64,
                table_type.limits.maximum.map(|maximum| maximum as f64),
                table64,
            )?,
            table64,
            functions: true,
        })
    }

    fn count_table_elements(&self, table: &BrowserTable) -> u64 {
        glue::table_length(&table.table) as u64
    }

    fn grow_table(&mut self, table: &BrowserTable, elements: u64) -> Result<(), GrowTableError> {
        glue::grow_table(&table.table, elements as f64, table.table64)
            .then_some(())
            .ok_or(GrowTableError)
    }

    fn read_table(
        &self,
        table: &BrowserTable,
        index: u64,
    ) -> Result<Option<Function>, ReadTableError> {
        let element = if table.functions {
            glue::table_element(&table.table, index as f64, table.table64)
        } else {
            JsValue::UNDEFINED
        };
        if element.is_null() {
            Ok(None)
        } else if glue::is_function(&element) {
            Ok(Some(element.unchecked_into()))
        } else if index >= self.count_table_elements(table) {
            Err(ReadTableError::IndexOutsideTable)
        } else {
            Err(ReadTableError::ElementNotFunction)
        }
    }

    fn write_table(
        &mut self,
        table: &BrowserTable,
        index: u64,
        function: Option<&Function>,
    ) -> Result<(), WriteTableError> {
        let null = JsValue::NULL;
        if table.functions
            && glue::set_table_element(
                &table.table,
                index as f64,
                function.map_or(&null, AsRef::as_ref),
                table.table64,
            )
        {
            Ok(())
        } else if index >= self.count_table_elements(table) {
            Err(WriteTableError::IndexOutsideTable)
        } else {
            Err(WriteTableError::ReferenceTypeMismatch)
        }
    }
}

/// A global: the JavaScript API's object, the value type it is read by, and for a float or a vector its access module.
#[derive(Clone)]
pub struct BrowserGlobal {
    global: Global,
    /// Nothing for a type this engine does not read; `funcref` for any function reference type.
    value_type: Option<ValType>,
    /// Not known, and not used, when the type is not.
    mutability: Mutability,
    through_slot: Option<SlotAccess>,
}

/// The exports that read a global into the first slot and write it from there, keeping bits JavaScript would not.
#[derive(Clone)]
struct SlotAccess {
    read: Function,
    write: Option<Function>,
}

/// Returns the object the JavaScript API knows the global by.
pub fn global_object(global: &BrowserGlobal) -> &Global {
    &global.global
}

thread_local! {
    /// The access module of each value type and mutability read through the slot, made when first used.
    static GLOBAL_ACCESS: OnceCell<Vec<(ValType, Mutability, Module)>> = const { OnceCell::new() };
}

/// Returns the exports of the access module of the global's type, which moves its value through the first slot.
fn slot_access(global: &Global, value_type: ValType, mutability: Mutability) -> SlotAccess {
    let instance = GLOBAL_ACCESS.with(|modules| {
        let (_, _, module) = modules
            .get_or_init(|| {
                global_kinds()
                    .filter(|(value_type, _)| adapter::global_through_slot(*value_type))
                    .map(|(value_type, mutability)| {
                        let bytes =
                            adapter::global_access(value_type, mutability == Mutability::Variable);
                        let module = glue::compile(&bytes).expect("an access module is valid");
                        (value_type, mutability, module)
                    })
                    .collect()
            })
            .iter()
            .find(|(known_type, known_mutability, _)| {
                *known_type == value_type && *known_mutability == mutability
            })
            .expect("each value type read through the slot has an access module");
        glue::instantiate_with_two(
            module,
            NAMESPACE,
            GLOBAL,
            global,
            SCRATCH_MEMORY,
            &wasm_bindgen::memory(),
        )
        .expect("a global instantiates the access module of its type")
    });
    SlotAccess {
        read: glue::instance_export(&instance, READ).unchecked_into(),
        write: (mutability == Mutability::Variable)
            .then(|| glue::instance_export(&instance, WRITE).unchecked_into()),
    }
}

/// Describes a global read by the value type and mutability, or one this engine does not read when given nothing.
pub fn global_of(global: Global, kind: Option<(ValType, Mutability)>) -> BrowserGlobal {
    match kind {
        Some((value_type, mutability)) => BrowserGlobal {
            through_slot: adapter::global_through_slot(value_type)
                .then(|| slot_access(&global, value_type, mutability)),
            global,
            value_type: Some(value_type),
            mutability,
        },
        None => BrowserGlobal {
            global,
            value_type: None,
            mutability: Mutability::Variable,
            through_slot: None,
        },
    }
}

/// Moves the global's value into the first slot.
fn read_through_slot(global: &BrowserGlobal) -> Result<u128, ReadGlobalError> {
    let access = global.through_slot.as_ref().ok_or(ReadGlobalError)?;
    glue::call(&access.read);
    Ok(slot(0))
}

/// Sets the mutable global to the value of the bits through the first slot.
fn write_through_slot(global: &BrowserGlobal, bits: u128) -> Result<(), WriteGlobalError> {
    let write = global
        .through_slot
        .as_ref()
        .and_then(|access| access.write.as_ref())
        .ok_or(WriteGlobalError::Constant)?;
    set_slot(0, bits);
    glue::call(write);
    Ok(())
}

/// Returns a new global of a type read through the slot, holding the value of the bits.
fn defined_global(value_type: ValType, mutable: bool, bits: u128) -> Result<Global, JsValue> {
    let module = glue::compile(&adapter::defined_global(value_type, mutable, bits))?;
    let instance = glue::instantiate(&module, &glue::new_imports())?;
    Ok(glue::instance_export(&instance, GLOBAL).unchecked_into())
}

impl<HostContext> GlobalStore for BrowserEngine<HostContext> {
    type GlobalAddress = BrowserGlobal;

    type AllocateGlobalError = JsValue;
    fn allocate_global(
        &mut self,
        mutability: Mutability,
        value: Value<Function>,
    ) -> Result<BrowserGlobal, JsValue> {
        let mutable = mutability == Mutability::Variable;
        let (global, value_type) = match value {
            Value::I32(value) => (glue::new_i32_global(mutable, value)?, ValType::I32),
            Value::I64(value) => (glue::new_i64_global(mutable, value)?, ValType::I64),
            Value::F32(value) => (
                defined_global(ValType::F32, mutable, value.to_bits().into())?,
                ValType::F32,
            ),
            Value::F64(value) => (
                defined_global(ValType::F64, mutable, value.to_bits().into())?,
                ValType::F64,
            ),
            Value::V128(value) => (
                defined_global(ValType::V128, mutable, value)?,
                ValType::V128,
            ),
            Value::FunctionReference(function) => (
                glue::new_function_global(
                    mutable,
                    function.as_ref().map_or(&JsValue::NULL, AsRef::as_ref),
                )?,
                ValType::Ref(RefType::FUNCREF),
            ),
        };
        Ok(global_of(global, Some((value_type, mutability))))
    }

    fn read_global(&self, global: &BrowserGlobal) -> Result<Value<Function>, ReadGlobalError> {
        let object = &global.global;
        Ok(match global.value_type.ok_or(ReadGlobalError)? {
            ValType::I32 => Value::I32(glue::number_value(object) as i32),
            ValType::I64 => Value::I64(glue::big_int_value(object)),
            ValType::F32 => Value::F32(f32::from_bits(read_through_slot(global)? as u32)),
            ValType::F64 => Value::F64(f64::from_bits(read_through_slot(global)? as u64)),
            ValType::V128 => Value::V128(read_through_slot(global)?),
            ValType::Ref(RefType::FUNCREF) => {
                let element = glue::reference_value(object);
                Value::FunctionReference((!element.is_null()).then(|| element.unchecked_into()))
            }
            ValType::Ref(_) => return Err(ReadGlobalError),
        })
    }

    fn write_global(
        &mut self,
        global: &BrowserGlobal,
        value: Value<Function>,
    ) -> Result<(), WriteGlobalError> {
        if global.value_type.is_none() {
            return Err(WriteGlobalError::ValueTypeMismatch);
        }
        if global.mutability == Mutability::Constant {
            return Err(WriteGlobalError::Constant);
        }
        let object = &global.global;
        match (global.value_type, value) {
            (Some(ValType::I32), Value::I32(value)) => {
                glue::set_number_value(object, f64::from(value))
            }
            (Some(ValType::I64), Value::I64(value)) => glue::set_big_int_value(object, value),
            (Some(ValType::F32), Value::F32(value)) => {
                write_through_slot(global, value.to_bits().into())?;
            }
            (Some(ValType::F64), Value::F64(value)) => {
                write_through_slot(global, value.to_bits().into())?;
            }
            (Some(ValType::V128), Value::V128(value)) => write_through_slot(global, value)?,
            (Some(ValType::Ref(RefType::FUNCREF)), Value::FunctionReference(function)) => {
                if !glue::set_reference_value(
                    object,
                    function.as_ref().map_or(&JsValue::NULL, AsRef::as_ref),
                ) {
                    return Err(WriteGlobalError::ValueTypeMismatch);
                }
            }
            _ => return Err(WriteGlobalError::ValueTypeMismatch),
        }
        Ok(())
    }
}
