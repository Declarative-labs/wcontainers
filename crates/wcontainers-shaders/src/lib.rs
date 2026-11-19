//! Turns Direct3D shader bytecode into the common form of `wcontainers-gpu` (a validated naga module)
//! with its reflection: DXBC through dxbc-spirv's own intermediate form, lowered straight into naga.

extern crate alloc;

mod build;
pub mod builtin;
pub mod cache;
mod container;
mod dxbc;
mod dxil;
mod ir;
mod lower;
pub mod refusal;
pub mod stage;
mod variant;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use wcontainers_gpu::{
    DrawBaseSource, Reflection, ShaderModule, ShaderStage, StageExtras, VariantFlags, VariantKey,
};

/// Shader bytecode in one of the forms the fronts hand over.
#[derive(Clone, Copy, Debug)]
pub enum Source<'a> {
    /// Direct3D 10 and 11 bytecode (shader model 4 and 5).
    Dxbc(&'a [u8]),
    /// Direct3D 12 bytecode (shader model 6).
    Dxil(&'a [u8]),
    SpirV(&'a [u32]),
}

/// A converted shader.
#[derive(Clone, Debug)]
pub struct Converted {
    /// One module per program stage the shader becomes: one for vertex, pixel and compute shaders;
    /// for a geometry shader its compute form and the vertex program that draws its output
    /// ([`stage::Stages`]).
    pub modules: Vec<ShaderModule>,
    pub reflection: Reflection,
}

/// Why a shader did not convert.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// The converter refused the bytecode, with its reason.
    Rejected(String),
    /// The shader uses something this conversion does not provide yet, named.
    Unsupported(&'static str),
    /// naga's SPIR-V reader cannot read what dxil-spirv wrote: one of the reader's gaps (plan 6.2).
    Reader(String),
    /// The converter's output is not what this crate understands.
    Malformed(String),
    /// The converted module failed naga's validation.
    Invalid(String),
}

impl core::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ConvertError::Rejected(s) => write!(f, "rejected: {s}"),
            ConvertError::Unsupported(s) => write!(f, "unsupported: {s}"),
            ConvertError::Reader(s) => write!(f, "spirv-reader: {s}"),
            ConvertError::Malformed(s) => write!(f, "malformed: {s}"),
            ConvertError::Invalid(s) => write!(f, "invalid: {s}"),
        }
    }
}

/// The optional abilities of naga's validation that WebGPU's core language has.
pub const WEBGPU_CAPABILITIES: Capabilities = Capabilities::MULTISAMPLED_SHADING
    .union(Capabilities::CUBE_ARRAY_TEXTURES)
    .union(Capabilities::SHADER_FLOAT16_IN_FLOAT32);

/// The name of every converted and built program's entry point.
pub const ENTRY_POINT: &str = "main";

/// The name of the draw arguments' offset in a program reading its draw bases from them.
pub use build::ARGUMENTS_OFFSET;

/// The refusal of a view's component mapping that a depth texture's read cannot take exactly.
pub const SWIZZLED_DEPTH: &str = "SwizzledDepth";

/// What goes into every content key besides the source and the variant: this crate's version and the
/// BLAKE3 hash of everything that makes the module (the converters' C++ as built, this crate's Rust
/// and the carried naga).
pub const CONVERTER_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+",
    env!("WCS_CONVERTER_SOURCES")
);

/// The refusal of a vertex program writing storage views, which WebGPU's vertex stage cannot.
const VERTEX_STORAGE_WRITE: &str = "VertexStorageWrite";

/// Converts a shader into the common form, as the variant asks. `root_signature` is the Direct3D 12
/// root signature the shader is used with.
pub fn convert(
    source: Source<'_>,
    variant: &VariantKey,
    root_signature: Option<&[u8]>,
) -> Result<Converted, ConvertError> {
    unbuilt(variant, root_signature)?;
    let (module, reflection, stage) = contained(|| {
        let (mut module, mut reflection, stage) = match source {
            Source::Dxbc(bytes) => {
                let (module, mut reflection, stage) = lower::lower(&dxbc::to_ir(bytes)?, variant)?;
                let inputs = container::dxbc_signature(bytes, true);
                container::place_system_values(&mut reflection.inputs, &inputs);
                let outputs = container::dxbc_signature(bytes, false);
                container::place_system_values(&mut reflection.outputs, &outputs);
                (module, reflection, stage)
            }
            Source::Dxil(bytes) => dxil::lower(bytes, variant)?,
            Source::SpirV(_) => return Err(ConvertError::Unsupported(crate::refusal::SPIR_V)),
        };
        // WebGPU gives vertex programs no writable storage.
        let writable = |g: &naga::GlobalVariable| {
            matches!(g.space, naga::AddressSpace::Storage { access }
                if access.contains(naga::StorageAccess::STORE))
        };
        if stage == ShaderStage::Vertex && module.global_variables.iter().any(|(_, g)| writable(g))
        {
            return Err(ConvertError::Unsupported(VERTEX_STORAGE_WRITE));
        }
        stage::clip_unpositioned(&mut module)?;
        variant::apply(&mut module, stage, variant, &mut reflection)?;
        variant::offset::strides(&module, &mut reflection);
        Ok((module, reflection, stage))
    })?;
    let info = validate(&module)?;
    let key = cache::shader_key(source, variant);
    let mut modules = vec![ShaderModule {
        module,
        info,
        entry_point: ENTRY_POINT.into(),
        stage,
        key,
    }];
    if let StageExtras::Geometry { .. } = reflection.extras {
        geometry_variant(variant)?;
        let named_layer = variant
            .flags
            .contains(VariantFlags::LAYER_FROM_STAGE)
            .then(|| stage::named_layer_location(variant));
        let mut module = stage::passthrough(&reflection, named_layer)?;
        if variant.flags.contains(VariantFlags::CLIP_DISTANCE) {
            // The pass-through is the last vertex stage: it passes the distances to the pixel program.
            let key = VariantKey {
                flags: VariantFlags::CLIP_DISTANCE | variant.flags & VariantFlags::LAYER_FROM_STAGE,
                linked: variant.linked,
                ..VariantKey::default()
            };
            let mut own = reflection.clone();
            variant::apply(&mut module, ShaderStage::Vertex, &key, &mut own)?;
        }
        modules.push(ShaderModule {
            info: validate(&module)?,
            module,
            entry_point: ENTRY_POINT.into(),
            stage: ShaderStage::Vertex,
            key: cache::module_key(key, 1),
        });
    }
    Ok(Converted {
        modules,
        reflection,
    })
}

/// Refuses, by name, the rewrites of the last vertex stage for a geometry shader: its draw's vertex
/// program is the stage helper's, which makes none of them yet.
fn geometry_variant(variant: &VariantKey) -> Result<(), ConvertError> {
    let vertex_flags = VariantFlags::VIEWPORT_ADJUST
        | VariantFlags::VIEWPORT_DEPTH_FLIP
        | VariantFlags::CLIP_DEPTH_REWRITE
        | VariantFlags::DEPTH_CLAMP;
    if variant.flags.intersects(vertex_flags)
        || !variant.pulled.is_empty()
        || variant.draw_base != DrawBaseSource::None
    {
        return Err(ConvertError::Unsupported(crate::refusal::GEOMETRY_VARIANT));
    }
    Ok(())
}

/// Runs a conversion step on bytecode from the guest, so that a panic in it (our bug) fails this call
/// as `Malformed` instead of ending the host. Where panics abort (WebAssembly) it ends the module
/// instance, which the host reports as a failed conversion.
fn contained<T>(f: impl FnOnce() -> Result<T, ConvertError>) -> Result<T, ConvertError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|panic| {
        let text = panic
            .downcast_ref::<&str>()
            .map(|s| String::from(*s))
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        Err(ConvertError::Malformed(format!(
            "the conversion panicked: {text}"
        )))
    })
}

/// Refuses, by name, every rewrite the variant asks for that this conversion does not build yet, so no
/// front gets the plain program under a variant's key.
fn unbuilt(variant: &VariantKey, root_signature: Option<&[u8]>) -> Result<(), ConvertError> {
    let VariantKey {
        flags,
        filter_in_shader: _,
        border_in_shader: _,
        mirror_once_in_shader: _,
        lod_bias: _,
        constants_as_storage: _,
        constants_as_texture: _,
        pulled: _,
        storage_formats: _,
        texel_buffers: _,
        texel_formats: _,
        storage_packed,
        texels_through_buffer: _,
        decoded,
        targets_in_shader: _,
        min_lod_clamped: _,
        texture_layers: _,
        swizzled: _,
        draw_base: _,
        output_interpolation: _,
        linked: _,
        samples: _,
        layout,
    } = variant;
    let unbuilt_flags = [(
        VariantFlags::PROVOKING_LAST,
        crate::refusal::PROVOKING_REWRITE,
    )];
    if let Some((_, name)) = unbuilt_flags.iter().find(|(f, _)| flags.contains(*f)) {
        return Err(ConvertError::Unsupported(name));
    }
    let asked = [
        (*storage_packed != 0, crate::refusal::STORAGE_PACKED),
        (!decoded.is_empty(), crate::refusal::NORM_AS_INTEGER),
        (
            *layout != [0; 32] || root_signature.is_some(),
            crate::refusal::ROOT_SIGNATURE_LAYOUT,
        ),
    ];
    match asked.iter().find(|(on, _)| *on) {
        Some((_, name)) => Err(ConvertError::Unsupported(name)),
        None => Ok(()),
    }
}

/// The capabilities a module needs: WebGPU's core ones, plus the extensions it uses that a front asks
/// for only where the back end has them (dual-source blending, `VariantFlags::DUAL_SOURCE`, clip
/// distances, WebGPU's `clip_distances` and GLSL ES's EXT_clip_cull_distance, and storage views of
/// 16-bit normalised formats, which a front binds only where the format has `STORAGE_WRITE`, and
/// subgroup operations in pixel and compute programs, `Features::SUBGROUPS`).
pub fn capabilities(module: &naga::Module) -> Capabilities {
    use naga::StorageFormat as S;
    let mut capabilities = WEBGPU_CAPABILITIES;
    if module.uses_subgroups() {
        capabilities |= Capabilities::SUBGROUP;
    }
    for (_, global) in module.global_variables.iter() {
        if let naga::TypeInner::Image {
            class:
                naga::ImageClass::Storage {
                    format:
                        S::R16Unorm
                        | S::R16Snorm
                        | S::Rg16Unorm
                        | S::Rg16Snorm
                        | S::Rgba16Unorm
                        | S::Rgba16Snorm,
                    ..
                },
            ..
        } = module.types[global.ty].inner
        {
            capabilities |= Capabilities::STORAGE_TEXTURE_16BIT_NORM_FORMATS;
        }
    }
    for ep in &module.entry_points {
        let primitive = |ty: naga::Handle<naga::Type>, b: &Option<naga::Binding>| {
            let index = Some(naga::Binding::BuiltIn(naga::BuiltIn::PrimitiveIndex));
            *b == index
                || matches!(&module.types[ty].inner, naga::TypeInner::Struct { members, .. }
                    if members.iter().any(|m| m.binding == index))
        };
        if ep
            .function
            .arguments
            .iter()
            .any(|a| primitive(a.ty, &a.binding))
        {
            capabilities |= Capabilities::PRIMITIVE_INDEX;
        }
        let Some(result) = &ep.function.result else {
            continue;
        };
        let bindings: Vec<&naga::Binding> = match &module.types[result.ty].inner {
            naga::TypeInner::Struct { members, .. } => {
                members.iter().filter_map(|m| m.binding.as_ref()).collect()
            }
            _ => result.binding.iter().collect(),
        };
        for b in bindings {
            match b {
                naga::Binding::Location {
                    blend_src: Some(_), ..
                } => capabilities |= Capabilities::DUAL_SOURCE_BLENDING,
                naga::Binding::BuiltIn(naga::BuiltIn::ClipDistances) => {
                    capabilities |= Capabilities::CLIP_DISTANCES
                }
                naga::Binding::BuiltIn(naga::BuiltIn::CullDistance) => {
                    capabilities |= Capabilities::CULL_DISTANCE
                }
                _ => {}
            }
        }
    }
    capabilities
}

/// Validates a module with the capabilities it needs ([`capabilities`]).
fn validate(module: &naga::Module) -> Result<naga::valid::ModuleInfo, ConvertError> {
    let capabilities = capabilities(module);
    Validator::new(ValidationFlags::all(), capabilities)
        .validate(module)
        .map_err(|e| {
            let mut s = format!("{}", e.as_inner());
            let mut source = core::error::Error::source(e.as_inner());
            while let Some(inner) = source {
                s.push_str(&format!(": {inner}"));
                source = inner.source();
            }
            ConvertError::Invalid(s)
        })
}
