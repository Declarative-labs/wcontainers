//! The element size of each storage buffer, the step a storage offset comes in
//! (`ReflectedBinding::stride`, `VariantFlags::STORAGE_OFFSET`).

use super::Result;
use crate::ConvertError;
use naga::{AddressSpace, ArraySize, Handle, Module, TypeInner};
use wcontainers_gpu::{Reflection, SlotKind};

/// The runtime array of a storage variable of type `ty`: the struct member holding it, if any, and
/// its stride. `None` for a variable that is not a runtime array (a counter).
fn runtime_array(module: &Module, ty: Handle<naga::Type>) -> Result<Option<(Option<u32>, u32)>> {
    let array = |ty: Handle<naga::Type>| match module.types[ty].inner {
        TypeInner::Array {
            size: ArraySize::Dynamic,
            stride,
            ..
        } => Some(stride),
        _ => None,
    };
    if let Some(stride) = array(ty) {
        return Ok(Some((None, stride)));
    }
    match &module.types[ty].inner {
        TypeInner::Struct { members, .. } => match (members.len(), members.last()) {
            (1, Some(m)) => Ok(array(m.ty).map(|stride| (Some(0), stride))),
            // A block with sized members before its array: an offset cannot move them alone.
            (_, Some(m)) if array(m.ty).is_some() => {
                Err(ConvertError::Unsupported("StorageOffsetOfBlock"))
            }
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

/// Gives each storage binding of `reflection` its element's stride, which a storage offset comes in.
pub(crate) fn strides(module: &Module, reflection: &mut Reflection) {
    for (_, v) in module.global_variables.iter() {
        let (Some(rb), AddressSpace::Storage { .. }) = (v.binding, v.space) else {
            continue;
        };
        let Ok(Some((_, stride))) = runtime_array(module, v.ty) else {
            continue;
        };
        for b in reflection.bindings.iter_mut() {
            if b.set == rb.group && b.binding == rb.binding && b.kind == SlotKind::Storage {
                b.stride = stride;
            }
        }
    }
}
