//! The types a module declares for the tables and globals it exports, which the JavaScript API does not tell, kept
//! beside the module in glue.js under each export's name.

use wasm_bindgen::JsValue;
use wasm_encoder::ValType as EncodedType;
use wasmparser::{
    AbstractHeapType, BinaryReaderError, CompositeInnerType, ExternalKind, GlobalType, HeapType,
    Parser, Payload, RefType, TypeRef, UnpackedIndex, ValType, WasmFeatures,
};
use wcontainers_webassembly_engine::global::Mutability;

use crate::adapter::GLOBAL_VALUE_TYPES;
use crate::glue;

/// Each value type and mutability a global may have that this engine reads, in the order of the numbers glue.js
/// keeps for them.
pub fn global_kinds() -> impl Iterator<Item = (EncodedType, Mutability)> {
    GLOBAL_VALUE_TYPES.into_iter().flat_map(|value_type| {
        [Mutability::Constant, Mutability::Variable].map(move |mutability| (value_type, mutability))
    })
}

/// Answers whether a reference of the type refers to a function, from the types the module declares, by whether each
/// is a function type.
fn refers_to_functions(reference: RefType, function_types: &[bool]) -> bool {
    match reference.heap_type() {
        HeapType::Abstract {
            ty: AbstractHeapType::Func | AbstractHeapType::NoFunc,
            ..
        } => true,
        HeapType::Concrete(UnpackedIndex::Module(index))
        | HeapType::Exact(UnpackedIndex::Module(index)) => usize::try_from(index)
            .ok()
            .and_then(|index| function_types.get(index).copied())
            .unwrap_or(false),
        _ => false,
    }
}

/// Returns the value type and mutability this engine reads a global of the type by, or nothing when it reads no
/// other reference than one to a function.
fn global_kind(global: GlobalType, function_types: &[bool]) -> Option<(EncodedType, Mutability)> {
    let value_type = match global.content_type {
        ValType::I32 => EncodedType::I32,
        ValType::I64 => EncodedType::I64,
        ValType::F32 => EncodedType::F32,
        ValType::F64 => EncodedType::F64,
        ValType::V128 => EncodedType::V128,
        ValType::Ref(reference) => refers_to_functions(reference, function_types)
            .then_some(EncodedType::Ref(wasm_encoder::RefType::FUNCREF))?,
    };
    let mutability = if global.mutable {
        Mutability::Variable
    } else {
        Mutability::Constant
    };
    Some((value_type, mutability))
}

/// Returns what glue.js keeps for the module's exports: under the name of each exported table whose elements refer
/// to functions, `true`; under the name of each exported global this engine reads, its place in `global_kinds`.
pub fn export_types(bytes: &[u8]) -> Result<JsValue, BinaryReaderError> {
    let mut function_types = Vec::new();
    let mut tables = Vec::new();
    let mut globals = Vec::new();
    let kept = glue::new_export_types();
    let mut parser = Parser::new(u64::MIN);
    parser.set_features(WasmFeatures::all());
    for payload in parser.parse_all(bytes) {
        match payload? {
            Payload::TypeSection(reader) => {
                for group in reader {
                    function_types.extend(group?.types().map(|subtype| {
                        matches!(subtype.composite_type.inner, CompositeInnerType::Func(_))
                    }));
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    match import?.ty {
                        TypeRef::Table(table) => tables.push(table.element_type),
                        TypeRef::Global(global) => globals.push(global),
                        _ => {}
                    }
                }
            }
            Payload::TableSection(reader) => {
                for table in reader {
                    tables.push(table?.ty.element_type);
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    globals.push(global?.ty);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export?;
                    let index = usize::try_from(export.index).ok();
                    let kept_value = match export.kind {
                        ExternalKind::Table => index
                            .and_then(|index| tables.get(index))
                            .is_some_and(|reference| {
                                refers_to_functions(*reference, &function_types)
                            })
                            .then_some(JsValue::TRUE),
                        ExternalKind::Global => index
                            .and_then(|index| globals.get(index))
                            .and_then(|global| global_kind(*global, &function_types))
                            .and_then(|kind| global_kinds().position(|known| known == kind))
                            .map(|place| JsValue::from_f64(place as f64)),
                        _ => None,
                    };
                    if let Some(value) = kept_value {
                        glue::keep_export_type(&kept, export.name, &value);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(kept)
}

/// Answers whether the elements of an exported table refer to functions, from what glue.js keeps under its name.
pub fn table_refers_to_functions(kept: &JsValue) -> bool {
    kept.is_truthy()
}

/// Returns the value type and mutability this engine reads an exported global by, from what glue.js keeps under its
/// name, or nothing when it reads no such global.
pub fn exported_global_kind(kept: &JsValue) -> Option<(EncodedType, Mutability)> {
    let place = kept.as_f64()?;
    global_kinds()
        .enumerate()
        .find_map(|(known, kind)| (known as f64 == place).then_some(kind))
}
