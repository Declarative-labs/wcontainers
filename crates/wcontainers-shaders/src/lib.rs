//! Turns Direct3D shader bytecode into the common form of `wcontainers-gpu` (a validated naga module)
//! with its reflection: DXBC through dxbc-spirv's own intermediate form, lowered straight into naga.

extern crate alloc;

mod build;
pub mod builtin;
pub mod cache;
mod common;
pub mod container;
mod dxbc;
mod dxil;
mod ir;
mod lower;
pub mod refusal;
pub mod stage;
pub mod staged;
pub mod typed;
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
pub use container::token;

/// What goes into every content key besides the source and the variant: this crate's version and the
/// BLAKE3 hash of everything that makes the module (the converters' C++ as built, this crate's Rust
/// and the carried naga).
pub const CONVERTER_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+",
    env!("WCS_CONVERTER_SOURCES")
);

/// `VariantFlags::GEOMETRY_REPLAY` asked of a shader that is not a geometry shader.
const REPLAY_OUTSIDE_GEOMETRY: &str = "a geometry shader's replay form asked of another shader";

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
                // dxbc-spirv keeps a domain shader's control point count to itself.
                if let StageExtras::Domain {
                    input_control_points,
                    ..
                } = &mut reflection.extras
                {
                    *input_control_points =
                        container::dxbc_input_control_points(bytes).unwrap_or(0) as u8;
                }
                let inputs = container::dxbc_signature(bytes, true);
                container::place_system_values(&mut reflection.inputs, &inputs);
                let outputs = container::dxbc_signature(bytes, false);
                container::place_system_values(&mut reflection.outputs, &outputs);
                (module, reflection, stage)
            }
            Source::Dxil(bytes) => dxil::lower(bytes, variant)?,
            Source::SpirV(_) => return Err(ConvertError::Unsupported(crate::refusal::SPIR_V)),
        };
        varied(&mut module, &mut reflection, stage, variant)?;
        Ok((module, reflection, stage))
    })?;
    finished(
        module,
        reflection,
        stage,
        variant,
        cache::shader_key(source, variant),
    )
}

/// The rewrites a variant asks of a program in the common form, its stage's form already made: a
/// vertex program without a position given one outside every clip volume, then the rewrites of 6.4.
fn varied(
    module: &mut naga::Module,
    reflection: &mut Reflection,
    stage: ShaderStage,
    variant: &VariantKey,
) -> Result<(), ConvertError> {
    stage::clip_unpositioned(module)?;
    // A geometry shader's forms take no rewrite of the last vertex stage: its pass-through does.
    let geometry = matches!(reflection.extras, StageExtras::Geometry { .. });
    if variant.flags.contains(VariantFlags::GEOMETRY_REPLAY) && !geometry {
        return Err(ConvertError::Malformed(REPLAY_OUTSIDE_GEOMETRY.into()));
    }
    let rewritten = if geometry {
        ShaderStage::Compute
    } else {
        stage
    };
    variant::apply(module, rewritten, variant, reflection)?;
    variant::offset::strides(module, reflection);
    Ok(())
}

/// The converted shader of a program in the common form, rewritten as `variant` asks: the module
/// validated, under `key`, and for a geometry shader the vertex program drawing its output.
fn finished(
    module: naga::Module,
    reflection: Reflection,
    stage: ShaderStage,
    variant: &VariantKey,
    key: [u8; cache::KEY_BYTES],
) -> Result<Converted, ConvertError> {
    let info = validate(&module)?;
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
        let numbered = variant
            .flags
            .contains(VariantFlags::PRIMITIVE_ID_FROM_STAGE)
            .then(|| stage::primitive_id_location(variant));
        let mut module = stage::passthrough(&reflection, named_layer, numbered)?;
        let last_vertex_stage = VariantFlags::CLIP_DISTANCE | VariantFlags::DEPTH_CLAMP;
        if variant.flags.intersects(last_vertex_stage) {
            // The pass-through is the last vertex stage: it passes the distances and the unclamped
            // depth to the pixel program.
            let passed_on = VariantFlags::LAYER_FROM_STAGE | VariantFlags::PRIMITIVE_ID_FROM_STAGE;
            let key = VariantKey {
                flags: variant.flags & (last_vertex_stage | passed_on),
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

/// Makes the rewrites `variant` asks for (plan 6.4, and 15.4's: arrays of textures, storage textures
/// and samplers split into their elements, gathers whose offset is known only at run time, storage
/// textures of the bound view's format) on a program another route brought into the common form:
/// `module` with one entry point named [`ENTRY_POINT`], bound by 6.4's rule with an array's elements
/// at consecutive bindings from the array's, and `reflection` its reflection. A storage texture keeps
/// the module's format unless `storage_formats` names its slot, so a route that read one without a
/// format refuses it where the key does not name it. The reflection's sampled pairs are taken from the
/// module. As [`convert`] gives them, a vertex program writing storage converts
/// ([`stage::writes_storage`]), one writing no position is given one outside every clip volume, and
/// one reading the draw's first vertex or instance (`BuiltIn::BaseVertex`, `BaseInstance`) reads them
/// from where `draw_base` says.
/// `route_key` is the key the route gives the program it made; the module carries
/// [`cache::rewritten_key`] of it. Geometry, hull and domain programs take [`staged`]'s entries;
/// typed buffers are given their functions by [`typed::buffers`] first.
pub fn rewrite(
    module: naga::Module,
    reflection: Reflection,
    variant: &VariantKey,
    route_key: &[u8; cache::KEY_BYTES],
) -> Result<Converted, ConvertError> {
    unbuilt(variant, None)?;
    let (module, reflection, stage) = contained(move || {
        let (mut module, mut reflection) = (module, reflection);
        let stage = match module.entry_points.as_slice() {
            [ep] if ep.name == ENTRY_POINT => match ep.stage {
                naga::ShaderStage::Vertex => ShaderStage::Vertex,
                naga::ShaderStage::Fragment => ShaderStage::Fragment,
                naga::ShaderStage::Compute => ShaderStage::Compute,
                _ => return Err(ConvertError::Unsupported(crate::refusal::STAGE)),
            },
            _ => return Err(ConvertError::Malformed(ONE_ENTRY_POINT.into())),
        };
        route_rewrites(&mut module, &mut reflection, stage, variant)?;
        // The draw's bases a vertex program reads as built-ins WGSL lacks, as the DXIL route's.
        dxil::draw_bases(&mut module, variant, &mut reflection)?;
        varied(&mut module, &mut reflection, stage, variant)?;
        Ok((module, reflection, stage))
    })?;
    finished(
        module,
        reflection,
        stage,
        variant,
        cache::rewritten_key(route_key, variant),
    )
}

/// The rewrites of 15.4 that [`rewrite`] makes, before the variant's: arrays of textures, storage
/// textures and samplers split (the sampled pairs taken again from the module), gathers at run-time
/// offsets moved, storage textures given the key's format.
fn route_rewrites(
    module: &mut naga::Module,
    reflection: &mut Reflection,
    stage: ShaderStage,
    variant: &VariantKey,
) -> Result<(), ConvertError> {
    if common::split_arrays(module)? {
        // An element picked at run time is sampled inside a switch on the index, which may differ
        // between a quad's lanes.
        lower::derivatives_anywhere(module);
    }
    reflection.sampled_pairs = common::sampled_pairs(module)?;
    common::gather_offsets(module)?;
    common::storage_textures(module, variant, stage, reflection)
}

/// Whether `module` writes storage buffers or textures, which a vertex program does only where the
/// back end has `Features::VERTEX_STORAGE_WRITE` or through the stage helper
/// ([`stage::writes_storage`]).
pub(crate) fn writes_storage(module: &naga::Module) -> bool {
    let writable_image = |mut ty: naga::Handle<naga::Type>| loop {
        match module.types[ty].inner {
            naga::TypeInner::BindingArray { base, .. } => ty = base,
            naga::TypeInner::Image {
                class: naga::ImageClass::Storage { access, .. },
                ..
            } => break access.contains(naga::StorageAccess::STORE),
            _ => break false,
        }
    };
    let writable = |g: &naga::GlobalVariable| match g.space {
        naga::AddressSpace::Storage { access } => access.contains(naga::StorageAccess::STORE),
        naga::AddressSpace::Handle => writable_image(g.ty),
        _ => false,
    };
    module.global_variables.iter().any(|(_, g)| writable(g))
}

/// What [`rewrite`] takes a module to have.
const ONE_ENTRY_POINT: &str = "a module of one entry point named by ENTRY_POINT";

/// Refuses, by name, the rewrites of the last vertex stage for a geometry shader that its draw's vertex
/// program, the stage helper's pass-through, does not make.
fn geometry_variant(variant: &VariantKey) -> Result<(), ConvertError> {
    let vertex_flags = VariantFlags::VIEWPORT_ADJUST
        | VariantFlags::VIEWPORT_DEPTH_FLIP
        | VariantFlags::CLIP_DEPTH_REWRITE;
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
        decoded: _,
        targets_in_shader: _,
        min_lod_clamped: _,
        texture_layers: _,
        swizzled: _,
        draw_base: _,
        output_interpolation: _,
        linked: _,
        samples: _,
        layout,
        register_ranges: _,
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
        (
            *layout != VariantKey::default().layout || root_signature.is_some(),
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
/// subgroup operations in pixel and compute programs, `Features::SUBGROUPS`), and immediates, which
/// only another route's program declares (Vulkan's push constants, through [`rewrite`]). A type
/// handle out of range (a damaged cache record) counts as no type, for the validator to refuse.
pub fn capabilities(module: &naga::Module) -> Capabilities {
    use naga::StorageFormat as S;
    let inner = |ty: naga::Handle<naga::Type>| module.types.get_handle(ty).ok().map(|t| &t.inner);
    let mut capabilities = WEBGPU_CAPABILITIES;
    if module.uses_subgroups() {
        capabilities |= Capabilities::SUBGROUP;
    }
    for (_, global) in module.global_variables.iter() {
        if global.space == naga::AddressSpace::Immediate {
            capabilities |= Capabilities::IMMEDIATES;
        }
        if let Some(naga::TypeInner::Image {
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
        }) = inner(global.ty)
        {
            capabilities |= Capabilities::STORAGE_TEXTURE_16BIT_NORM_FORMATS;
        }
    }
    for ep in &module.entry_points {
        let primitive = |ty: naga::Handle<naga::Type>, b: &Option<naga::Binding>| {
            let index = Some(naga::Binding::BuiltIn(naga::BuiltIn::PrimitiveIndex));
            *b == index
                || matches!(inner(ty), Some(naga::TypeInner::Struct { members, .. })
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
        let bindings: Vec<&naga::Binding> = match inner(result.ty) {
            Some(naga::TypeInner::Struct { members, .. }) => {
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

/// Validates a module with the capabilities it needs ([`capabilities`]). Contained, since a module read
/// from a cache file may hold handles nothing has checked yet.
fn validate(module: &naga::Module) -> Result<naga::valid::ModuleInfo, ConvertError> {
    contained(|| {
        Validator::new(ValidationFlags::all(), capabilities(module))
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
    })
}
