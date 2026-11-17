//! DXIL through dxil-spirv (C++, built by `build.rs`) to SPIR-V, read by naga's SPIR-V reader, then
//! given the binding rule of plan section 6.4 and stripped of what WGSL cannot express.

use crate::ConvertError;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int, c_void};
use naga::{AddressSpace, Binding, BuiltIn, Expression, ImageClass, Literal, Module, TypeInner};
use wcontainers_gpu::{
    DrawBaseSource, ReflectedBinding, Reflection, ShaderStage, SlotKind, SourceStage, StageExtras,
    VariantKey,
};

unsafe extern "C" {
    fn wcs_dxil_to_spirv(
        data: *const u8,
        size: usize,
        out: *mut *mut u32,
        out_words: *mut usize,
        error: *mut *mut c_char,
    ) -> c_int;
    fn wcs_free(p: *mut c_void);
}

/// DXIL to SPIR-V words.
fn to_spirv(dxil: &[u8]) -> Result<Vec<u32>, ConvertError> {
    let mut out = core::ptr::null_mut();
    let mut words = 0;
    let mut error = core::ptr::null_mut();
    // SAFETY: the pointers are valid for the call; the C side allocates `out` and `error` with malloc.
    let status =
        unsafe { wcs_dxil_to_spirv(dxil.as_ptr(), dxil.len(), &mut out, &mut words, &mut error) };
    if status != 0 {
        let reason = if error.is_null() {
            String::from("dxil-spirv gave no reason")
        } else {
            // SAFETY: a NUL-terminated string from the C side, freed right after.
            let s = unsafe { core::ffi::CStr::from_ptr(error) }
                .to_string_lossy()
                .into();
            unsafe { wcs_free(error.cast()) };
            s
        };
        return Err(ConvertError::Rejected(reason));
    }
    // SAFETY: `out` holds `words` words written by the C side, freed right after.
    let spirv = unsafe { core::slice::from_raw_parts(out, words) }.to_vec();
    unsafe { wcs_free(out.cast()) };
    Ok(spirv)
}

/// Converts DXIL into a naga module and its reflection.
pub(crate) fn lower(
    dxil: &[u8],
    variant: &VariantKey,
) -> Result<(Module, Reflection, ShaderStage), ConvertError> {
    let spirv = to_spirv(dxil)?;
    let options = naga::front::spv::Options {
        adjust_coordinate_space: false,
        strict_capabilities: false,
        block_ctx_dump_prefix: None,
    };
    let mut module = naga::front::spv::Frontend::new(spirv.iter().copied(), &options)
        .parse()
        .map_err(|e| ConvertError::Reader(format!("{e}")))?;
    let ep = module
        .entry_points
        .first()
        .ok_or(ConvertError::Malformed("no entry point".into()))?;
    let (stage, source_stage, set) = match ep.stage {
        naga::ShaderStage::Vertex => (ShaderStage::Vertex, SourceStage::Vertex, 0),
        naga::ShaderStage::Fragment => (ShaderStage::Fragment, SourceStage::Pixel, 1),
        naga::ShaderStage::Compute => (ShaderStage::Compute, SourceStage::Compute, 0),
        _ => return Err(ConvertError::Unsupported("Stage")),
    };
    let extras = match stage {
        ShaderStage::Compute => StageExtras::Compute {
            workgroup_size: ep.workgroup_size,
        },
        _ => StageExtras::None,
    };
    module.entry_points[0].name = "main".into();
    module.entry_points[0].early_depth_test = None;
    draw_bases(&mut module, variant)?;
    let mut reflection = Reflection {
        stage: source_stage,
        inputs: Vec::new(),
        outputs: Vec::new(),
        bindings: Vec::new(),
        helpers: Vec::new(),
        helper_size: 0,
        sampled_pairs: Vec::new(),
        extras,
    };
    bindings(&mut module, set, &mut reflection)?;
    Ok((module, reflection, stage))
}

/// dxil-spirv reads `SV_VertexID` and `SV_InstanceID` as `VertexIndex - BaseVertex` and `InstanceIndex -
/// BaseInstance`; WGSL has no base built-ins. With no base in the variant the front promises bases of
/// zero, so the bases become zero.
fn draw_bases(module: &mut Module, variant: &VariantKey) -> Result<(), ConvertError> {
    let f = &mut module.entry_points[0].function;
    let mut i = 0;
    while i < f.arguments.len() {
        let base = matches!(
            f.arguments[i].binding,
            Some(Binding::BuiltIn(
                BuiltIn::BaseVertex | BuiltIn::BaseInstance
            ))
        );
        if !base {
            i += 1;
            continue;
        }
        if variant.draw_base != DrawBaseSource::None {
            return Err(ConvertError::Unsupported("DxilDrawBase"));
        }
        let sint = matches!(module.types[f.arguments[i].ty].inner, TypeInner::Scalar(s) if s.kind == naga::ScalarKind::Sint);
        for (_, e) in f.expressions.iter_mut() {
            if let Expression::FunctionArgument(n) = e {
                if *n as usize == i {
                    *e = Expression::Literal(if sint {
                        Literal::I32(0)
                    } else {
                        Literal::U32(0)
                    });
                } else if *n as usize > i {
                    *n -= 1;
                }
            }
        }
        f.arguments.remove(i);
    }
    Ok(())
}

/// Numbers every resource by the rule of 6.4 for the stage's set (Direct3D 10 and 11's layout until
/// root signatures number Direct3D 12's), reading dxil-spirv's register (binding) and space (group).
fn bindings(
    module: &mut Module,
    set: u32,
    reflection: &mut Reflection,
) -> Result<(), ConvertError> {
    for (_, var) in module.global_variables.iter_mut() {
        let Some(binding) = var.binding.as_mut() else {
            continue;
        };
        let kind = match (var.space, &module.types[var.ty].inner) {
            (AddressSpace::Uniform, _) => SlotKind::Uniform,
            (AddressSpace::Storage { .. }, _) => SlotKind::Storage,
            (AddressSpace::Handle, TypeInner::Sampler { .. }) => SlotKind::Sampler,
            (
                AddressSpace::Handle,
                TypeInner::Image {
                    class: ImageClass::Storage { .. },
                    ..
                },
            ) => SlotKind::Storage,
            (AddressSpace::Handle, TypeInner::Image { .. }) => SlotKind::Texture,
            (AddressSpace::Handle, TypeInner::BindingArray { .. }) => {
                return Err(ConvertError::Unsupported("ResourceArray"));
            }
            _ => return Err(ConvertError::Unsupported("ResourceKind")),
        };
        let (space, register) = (binding.group, binding.binding);
        let number = kind
            .binding(register)
            .ok_or(ConvertError::Unsupported("BindingsBeyondLimit"))?;
        *binding = naga::ResourceBinding {
            group: set,
            binding: number,
        };
        reflection.bindings.push(ReflectedBinding {
            kind,
            space,
            register,
            count: 1,
            set,
            binding: number,
            added: None,
        });
    }
    Ok(())
}
