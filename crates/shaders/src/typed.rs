//! Typed buffers of a program another route brought into the common form, read and written as the
//! DXBC and DXIL routes read and write theirs (`variant::texel`): through a storage buffer of words
//! (`TexelBufferRoute::Storage`) or a 2D texture of the view's format (`TexelBufferRoute::Texture`),
//! each element decoded from and encoded into the bound view's format, kept to the view's place and
//! length from `HelperValue::TexelBufferView`.

use crate::variant::texel::{TexelBuffer, TexelPlace};
use crate::{ConvertError, ENTRY_POINT};
use alloc::vec::Vec;
use naga::{
    AddressSpace, Arena, AtomicFunction, Expression, Function, GlobalVariable, Handle, ImageClass,
    ImageDimension, Literal, Module, ScalarKind, Span, StorageAccess, Type, TypeInner,
};
use wcontainers_gpu::{
    reflect::Reflection, reflect::SLOT_KINDS, reflect::SlotKind, reflect::TexelBufferRoute,
    reflect::VariantKey, shader::ShaderStage,
};

/// One typed buffer of a module and the functions its program calls for it, stand-ins whose bodies
/// [`buffers`] writes. Each function is placed in the module's function arena before every function
/// calling it.
pub struct TypedBuffer {
    /// The view: a storage buffer (`AddressSpace::Storage`, its access as the slot is bound) of any
    /// type, at the binding the reflection lists for a texture slot (read only) or a storage slot. Its
    /// type, and for the texture route its space, become the route's.
    pub global: Handle<GlobalVariable>,
    /// The kind of value the program reads and writes, which a view of another kind gives by its bits.
    pub kind: ScalarKind,
    /// `fn(index: u32) -> vec4<kind>`: the element, zero past the view's length (the defaults in
    /// components the format lacks).
    pub load: Option<Handle<Function>>,
    /// `fn(index: u32, value: vec4<kind>)`: the element written, nothing past the length.
    pub store: Option<Handle<Function>>,
    /// `fn() -> u32`: the view's element count.
    pub length: Option<Handle<Function>>,
    /// `fn(index: u32, value: kind) -> kind` (a compare-exchange `fn(index: u32, value: kind,
    /// compare: kind) -> kind`): the operation on the element, giving its value before; nothing and
    /// zero past the length. Only views of 32-bit one-component integer formats take them.
    pub atomics: Vec<(TypedAtomic, Handle<Function>)>,
}

/// An atomic operation on a typed buffer's element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypedAtomic {
    Add,
    Subtract,
    And,
    ExclusiveOr,
    InclusiveOr,
    Min,
    Max,
    Exchange,
    CompareExchange,
}

/// What [`buffers`] takes a module and its typed buffers to be.
const MALFORMED: &str = "a typed buffer as wcontainers_shaders::typed describes it";

/// Writes the functions of each of `buffers` for `variant`'s typed buffer route, with the view's
/// format from `texel_formats` (texture slots) or `storage_formats` (storage slots), and adds each
/// view's `HelperValue::TexelBufferView` to `reflection`. `module` has one entry point named
/// [`ENTRY_POINT`]; [`crate::rewrite`] then makes the rest of the variant's rewrites. Refusals are the
/// DXBC route's own names (`refusal::TYPED_BUFFER` where the route is native, ...).
pub fn buffers(
    module: &mut Module,
    reflection: &mut Reflection,
    variant: &VariantKey,
    buffers: &[TypedBuffer],
) -> Result<(), ConvertError> {
    let malformed = || ConvertError::Malformed(MALFORMED.into());
    let stage = match module.entry_points.as_slice() {
        [ep] if ep.name == ENTRY_POINT => match ep.stage {
            naga::ShaderStage::Vertex => ShaderStage::Vertex,
            naga::ShaderStage::Fragment => ShaderStage::Fragment,
            naga::ShaderStage::Compute => ShaderStage::Compute,
            _ => return Err(ConvertError::Unsupported(crate::refusal::STAGE)),
        },
        _ => return Err(malformed()),
    };
    for typed in buffers {
        let global = module
            .global_variables
            .try_get(typed.global)
            .map_err(|_| malformed())?;
        let (Some(binding), AddressSpace::Storage { access }) = (&global.binding, global.space)
        else {
            return Err(malformed());
        };
        let binding = *binding;
        let reflected = reflection
            .bindings
            .iter()
            .find(|r| r.set == binding.group && r.binding == binding.binding)
            .ok_or_else(malformed)?;
        let storage = match reflected.kind {
            SlotKind::Texture => false,
            SlotKind::Storage => true,
            _ => return Err(malformed()),
        };
        let texture = match variant.texel_buffers {
            TexelBufferRoute::Native => {
                return Err(ConvertError::Unsupported(crate::refusal::TYPED_BUFFER));
            }
            TexelBufferRoute::Storage => false,
            TexelBufferRoute::Texture if !storage => true,
            TexelBufferRoute::Texture => {
                return Err(ConvertError::Unsupported(
                    crate::refusal::TYPED_BUFFER_STORAGE_AS_TEXTURE,
                ));
            }
        };
        let formats = match storage {
            true => &variant.storage_formats,
            false => &variant.texel_formats,
        };
        let format = formats
            .iter()
            .find(|(slot, _)| {
                *slot == reflected.binding - SLOT_KINDS[reflected.kind as usize].first_binding
            })
            .ok_or(ConvertError::Unsupported(
                crate::refusal::TYPED_BUFFER_FORMAT,
            ))?
            .1;
        let writes = typed.store.is_some() || !typed.atomics.is_empty();
        if writes && !(storage && access.contains(StorageAccess::STORE)) {
            return Err(malformed());
        }
        let stand_ins = [typed.load, typed.store, typed.length]
            .into_iter()
            .flatten();
        for f in stand_ins.chain(typed.atomics.iter().map(|&(_, f)| f)) {
            module.functions.try_get(f).map_err(|_| malformed())?;
        }
        let layout = TexelBuffer::layout(format)?;
        let declared = match typed.kind {
            ScalarKind::Float => naga::Scalar::F32,
            ScalarKind::Sint => naga::Scalar::I32,
            ScalarKind::Uint => naga::Scalar::U32,
            _ => return Err(malformed()),
        };
        // As the DXBC route makes them: atomic words where an atomic operation uses them (float
        // bits exchanged as u32), or where elements narrower than a word are written.
        let atomic = match (typed.atomics.is_empty(), typed.kind) {
            (false, ScalarKind::Float) => Some(naga::Scalar::U32),
            (false, _) => Some(declared),
            (true, _) if typed.store.is_some() && TexelBuffer::narrow(layout.2) => {
                Some(naga::Scalar::U32)
            }
            (true, _) => None,
        };
        let (ty, space) = match texture {
            true => {
                let inner = TypeInner::Image {
                    dim: ImageDimension::D2,
                    arrayed: false,
                    class: ImageClass::Sampled {
                        kind: TexelBuffer::view_kind(&layout),
                        multi: false,
                    },
                };
                let ty = module
                    .types
                    .insert(Type { name: None, inner }, Span::UNDEFINED);
                (ty, AddressSpace::Handle)
            }
            false => (
                TexelBuffer::words(module, atomic),
                AddressSpace::Storage { access },
            ),
        };
        let global = &mut module.global_variables[typed.global];
        global.ty = ty;
        global.space = space;
        let place = match texture {
            true => TexelPlace::Texture,
            false => TexelPlace::Words,
        };
        let mut t = TexelBuffer::declared(
            typed.global,
            layout,
            typed.kind,
            atomic,
            place,
            stage,
            binding.group,
            binding.binding,
        );
        if let Some(f) = typed.load {
            fill(module, f, &mut t, |m, t| Ok(t.load(m, reflection)))?;
        }
        if let Some(f) = typed.store {
            fill(module, f, &mut t, |m, t| Ok(t.store(m, reflection)))?;
        }
        if let Some(f) = typed.length {
            fill(module, f, &mut t, |m, t| Ok(t.length(m, reflection)))?;
        }
        for &(op, f) in &typed.atomics {
            let fun = atomic_function(op);
            fill(module, f, &mut t, |m, t| t.atomic(m, reflection, fun))?;
        }
    }
    Ok(())
}

/// Writes the function `make` makes for `t` into the stand-in `f`, which must have its arguments and
/// result.
fn fill(
    module: &mut Module,
    f: Handle<Function>,
    t: &mut TexelBuffer,
    make: impl FnOnce(&mut Module, &mut TexelBuffer) -> Result<Handle<Function>, ConvertError>,
) -> Result<(), ConvertError> {
    let signature = |f: &Function| {
        let arguments: Vec<_> = f.arguments.iter().map(|a| a.ty).collect();
        (arguments, f.result.as_ref().map(|r| r.ty))
    };
    let stand_in = signature(&module.functions[f]);
    t.fill(f);
    let made = make(module, t)?;
    if made != f || signature(&module.functions[f]) != stand_in {
        return Err(ConvertError::Malformed(MALFORMED.into()));
    }
    Ok(())
}

/// The naga operation of `op`. A compare-exchange's compared value is the function's third argument;
/// the expression its `compare` names here only marks the comparison.
fn atomic_function(op: TypedAtomic) -> AtomicFunction {
    use AtomicFunction as A;
    match op {
        TypedAtomic::Add => A::Add,
        TypedAtomic::Subtract => A::Subtract,
        TypedAtomic::And => A::And,
        TypedAtomic::ExclusiveOr => A::ExclusiveOr,
        TypedAtomic::InclusiveOr => A::InclusiveOr,
        TypedAtomic::Min => A::Min,
        TypedAtomic::Max => A::Max,
        TypedAtomic::Exchange => A::Exchange { compare: None },
        TypedAtomic::CompareExchange => {
            let mut marks = Arena::new();
            let mark = marks.append(Expression::Literal(Literal::U32(0)), Span::UNDEFINED);
            A::Exchange {
                compare: Some(mark),
            }
        }
    }
}
