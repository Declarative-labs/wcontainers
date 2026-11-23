//! Memories, tables and globals: the JavaScript API's objects, and for vector globals a small module of each.

use std::cell::OnceCell;
use std::sync::Arc;

use js_sys::Function;
use js_sys::WebAssembly::{Global, Instance, Memory, Module, Table};
use wasm_bindgen::{JsCast, JsValue};
use wasm_encoder::ValType;
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
    self, GLOBAL, GLOBAL_VALUE_TYPES, GUEST_MEMORY, GUEST_OFFSET, LENGTH, NAMESPACE, OWN_ADDRESS,
    READ, SCRATCH_MEMORY, WRITE, set_slot, slot,
};
use crate::{BrowserEngine, BrowserSharedMemory, glue, new_id};

/// A memory: the JavaScript API's object, and the exports of its access module, which copy out of it and into it.
#[derive(Clone)]
pub struct BrowserMemory {
    memory: Memory,
    memory64: bool,
    read: Function,
    write: Function,
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
        .expect("a memory has a 32-bit or a 64-bit address type");
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
    set_slot(GUEST_OFFSET, u128::from(offset));
    set_slot(OWN_ADDRESS, own_address as u128);
    set_slot(LENGTH, length as u128);
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
        glue::grow_memory(&memory.memory, pages as f64)
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
            glue::register_shared_memory(registration.id, &memory.memory);
            BrowserSharedMemory {
                registration: Arc::new(registration),
                memory64: memory.memory64,
            }
        })
    }

    fn add_shared_memory(&mut self, memory: &BrowserSharedMemory) -> BrowserMemory {
        let object = glue::registered_shared_memory(memory.registration.id)
            .expect("a shared memory is added in a worker it was made in or posted to");
        memory_of(object, Some(memory.memory64))
    }
}

impl<HostContext> TableStore for BrowserEngine<HostContext> {
    type TableAddress = Table;

    type AllocateTableError = JsValue;
    fn allocate_table(&mut self, table_type: TableType) -> Result<Table, JsValue> {
        glue::new_table(
            table_type.limits.minimum as f64,
            table_type.limits.maximum.map(|maximum| maximum as f64),
            table_type.address_type == AddressType::I64,
        )
    }

    fn count_table_elements(&self, table: &Table) -> u64 {
        glue::table_length(table) as u64
    }

    fn grow_table(&mut self, table: &Table, elements: u64) -> Result<(), GrowTableError> {
        glue::grow_table(table, elements as f64)
            .then_some(())
            .ok_or(GrowTableError)
    }

    fn read_table(&self, table: &Table, index: u64) -> Result<Option<Function>, ReadTableError> {
        if index >= self.count_table_elements(table) {
            return Err(ReadTableError::IndexOutsideTable);
        }
        let element = glue::table_element(table, index as f64);
        if element.is_null() {
            Ok(None)
        } else if glue::is_function(&element) {
            Ok(Some(element.unchecked_into()))
        } else {
            Err(ReadTableError::ElementNotFunction)
        }
    }

    fn write_table(
        &mut self,
        table: &Table,
        index: u64,
        function: Option<&Function>,
    ) -> Result<(), WriteTableError> {
        if index >= self.count_table_elements(table) {
            return Err(WriteTableError::IndexOutsideTable);
        }
        let null = JsValue::NULL;
        glue::set_table_element(table, index as f64, function.map_or(&null, AsRef::as_ref))
            .then_some(())
            .ok_or(WriteTableError::ReferenceTypeMismatch)
    }
}

/// A global: the JavaScript API's object, the value type found for it, and for a vector its access module.
#[derive(Clone)]
pub struct BrowserGlobal {
    global: Global,
    /// Nothing for a reference type other than `funcref`, which `Value` does not hold.
    value_type: Option<ValType>,
    mutability: Mutability,
    vector: Option<VectorAccess>,
}

/// The exports that read a vector global into the first slot and write it from there, which JavaScript cannot.
#[derive(Clone)]
struct VectorAccess {
    read: Function,
    write: Option<Function>,
}

/// Returns the object the JavaScript API knows the global by.
pub fn global_object(global: &BrowserGlobal) -> &Global {
    &global.global
}

thread_local! {
    /// A module for each value type and mutability, which instantiates only with a global of that type.
    static GLOBAL_ACCESS: OnceCell<Vec<(ValType, Mutability, Module)>> = const { OnceCell::new() };
}

/// Instantiates the access module with the global, or answers nothing when the global has another type.
fn access(module: &Module, value_type: ValType, global: &Global) -> Option<Instance> {
    if value_type == ValType::V128 {
        glue::instantiate_with_two(
            module,
            NAMESPACE,
            GLOBAL,
            global,
            SCRATCH_MEMORY,
            &wasm_bindgen::memory(),
        )
        .ok()
    } else {
        glue::instantiate_with(module, NAMESPACE, GLOBAL, global)
    }
}

/// Describes a global, finding its type by which access module it instantiates.
pub fn global_of(global: Global) -> BrowserGlobal {
    let found = GLOBAL_ACCESS.with(|modules| {
        modules
            .get_or_init(|| {
                GLOBAL_VALUE_TYPES
                    .into_iter()
                    .flat_map(|value_type| {
                        [Mutability::Constant, Mutability::Variable].map(|mutability| {
                            let bytes = adapter::global_access(
                                value_type,
                                mutability == Mutability::Variable,
                            );
                            (
                                value_type,
                                mutability,
                                glue::compile(&bytes).expect("an access module is valid"),
                            )
                        })
                    })
                    .collect()
            })
            .iter()
            .find_map(|(value_type, mutability, module)| {
                access(module, *value_type, &global)
                    .map(|instance| (*value_type, *mutability, instance))
            })
    });
    let Some((value_type, mutability, instance)) = found else {
        return BrowserGlobal {
            global,
            value_type: None,
            mutability: Mutability::Constant,
            vector: None,
        };
    };
    let vector = (value_type == ValType::V128).then(|| VectorAccess {
        read: glue::instance_export(&instance, READ).unchecked_into(),
        write: (mutability == Mutability::Variable)
            .then(|| glue::instance_export(&instance, WRITE).unchecked_into()),
    });
    BrowserGlobal {
        global,
        value_type: Some(value_type),
        mutability,
        vector,
    }
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
        let global = match value {
            Value::I32(value) => glue::new_i32_global(mutable, value)?,
            Value::I64(value) => glue::new_i64_global(mutable, value)?,
            Value::F32(value) => glue::new_f32_global(mutable, value)?,
            Value::F64(value) => glue::new_f64_global(mutable, value)?,
            Value::V128(value) => {
                let module = glue::compile(&adapter::vector_global(mutable, value))?;
                let instance = glue::instantiate(&module, &glue::new_imports())?;
                glue::instance_export(&instance, GLOBAL).unchecked_into()
            }
            Value::FunctionReference(function) => glue::new_function_global(
                mutable,
                function.as_ref().map_or(&JsValue::NULL, AsRef::as_ref),
            )?,
        };
        Ok(global_of(global))
    }

    fn read_global(&self, global: &BrowserGlobal) -> Result<Value<Function>, ReadGlobalError> {
        let object = &global.global;
        Ok(match global.value_type.ok_or(ReadGlobalError)? {
            ValType::I32 => Value::I32(glue::number_value(object) as i32),
            ValType::I64 => Value::I64(glue::big_int_value(object)),
            ValType::F32 => Value::F32(glue::number_value(object) as f32),
            ValType::F64 => Value::F64(glue::number_value(object)),
            ValType::V128 => {
                let vector = global.vector.as_ref().ok_or(ReadGlobalError)?;
                glue::call(&vector.read);
                Value::V128(slot(0))
            }
            ValType::Ref(_) => {
                let element = glue::reference_value(object);
                Value::FunctionReference((!element.is_null()).then(|| element.unchecked_into()))
            }
        })
    }

    fn write_global(
        &mut self,
        global: &BrowserGlobal,
        value: Value<Function>,
    ) -> Result<(), WriteGlobalError> {
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
                glue::set_number_value(object, f64::from(value))
            }
            (Some(ValType::F64), Value::F64(value)) => glue::set_number_value(object, value),
            (Some(ValType::V128), Value::V128(value)) => {
                let write = global
                    .vector
                    .as_ref()
                    .and_then(|vector| vector.write.as_ref());
                let write = write.ok_or(WriteGlobalError::Constant)?;
                set_slot(0, value);
                glue::call(write);
            }
            (Some(ValType::Ref(_)), Value::FunctionReference(function)) => {
                glue::set_reference_value(
                    object,
                    function.as_ref().map_or(&JsValue::NULL, AsRef::as_ref),
                );
            }
            _ => return Err(WriteGlobalError::ValueTypeMismatch),
        }
        Ok(())
    }
}
