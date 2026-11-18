//! Turns Direct3D shader bytecode into the common form of `wcontainers-gpu` (a validated naga module)
//! with its reflection: DXBC through dxbc-spirv's own intermediate form, lowered straight into naga.

extern crate alloc;

pub mod cache;
mod dxbc;
mod dxil;
mod ir;
mod lower;
mod variant;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use wcontainers_gpu::{Reflection, ShaderModule, TexelBufferRoute, VariantFlags, VariantKey};

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
    /// One module per program stage the shader becomes: one for vertex, pixel and compute shaders.
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

/// What goes into every content key besides the source and the variant: this crate's version and the
/// BLAKE3 hash of everything that makes the module (the converters' C++ as built, this crate's Rust
/// and the carried naga).
pub const CONVERTER_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+",
    env!("WCS_CONVERTER_SOURCES")
);

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
            Source::Dxbc(bytes) => lower::lower(&dxbc::to_ir(bytes)?, variant)?,
            Source::Dxil(bytes) => dxil::lower(bytes, variant)?,
            Source::SpirV(_) => return Err(ConvertError::Unsupported("SpirV")),
        };
        variant::apply(&mut module, stage, variant, &mut reflection)?;
        Ok((module, reflection, stage))
    })?;
    let info = validate(&module)?;
    let key = cache::shader_key(source, variant);
    Ok(Converted {
        modules: vec![ShaderModule {
            module,
            info,
            entry_point: "main".into(),
            stage,
            key,
        }],
        reflection,
    })
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
        filter_in_shader,
        border_in_shader,
        mirror_once_in_shader,
        lod_bias: _,
        constants_as_storage: _,
        constants_as_texture,
        pulled,
        storage_formats,
        texel_buffers,
        storage_packed,
        texels_through_buffer,
        decoded,
        targets_in_shader,
        min_lod_clamped: _,
        draw_base: _,
        output_interpolation: _,
        samples: _,
        layout,
    } = variant;
    let unbuilt_flags = [
        (VariantFlags::PROVOKING_LAST, "ProvokingRewrite"),
        (VariantFlags::EARLY_TESTS, "EarlyTestsInShader"),
        (VariantFlags::LINE_COVERAGE, "AntialiasedLinesAsQuads"),
        (VariantFlags::CLIP_DISTANCE, "ClipDistanceInShader"),
        (VariantFlags::DEPTH_CLAMP, "DepthClampInShader"),
        (
            VariantFlags::PRIMITIVE_ID_FROM_STAGE,
            "PrimitiveIdAsGeometry",
        ),
    ];
    if let Some((_, name)) = unbuilt_flags.iter().find(|(f, _)| flags.contains(*f)) {
        return Err(ConvertError::Unsupported(name));
    }
    let asked = [
        (*filter_in_shader != 0, "FilterInShader"),
        (*border_in_shader != 0, "BorderInShader"),
        (*mirror_once_in_shader != 0, "MirrorOnceInShader"),
        (*constants_as_texture != 0, "ConstantsAsTexture"),
        (!pulled.is_empty(), "VertexPulledInShader"),
        (!storage_formats.is_empty(), "StorageFormats"),
        (
            *texel_buffers != TexelBufferRoute::Native,
            "TexelBufferRoute",
        ),
        (*storage_packed != 0, "StoragePacked"),
        (*texels_through_buffer != 0, "TexelsThroughBuffer"),
        (!decoded.is_empty(), "NormAsInteger"),
        (!targets_in_shader.is_empty(), "TargetsInShader"),
        (
            *layout != [0; 32] || root_signature.is_some(),
            "RootSignatureLayout",
        ),
    ];
    match asked.iter().find(|(on, _)| *on) {
        Some((_, name)) => Err(ConvertError::Unsupported(name)),
        None => Ok(()),
    }
}

/// The capabilities a module needs: WebGPU's core ones, plus the extensions it uses that a front asks
/// for only where the back end has them (dual-source blending, `VariantFlags::DUAL_SOURCE`, and clip
/// distances, WebGPU's `clip_distances` and GLSL ES's EXT_clip_cull_distance).
pub fn capabilities(module: &naga::Module) -> Capabilities {
    let mut capabilities = WEBGPU_CAPABILITIES;
    for ep in &module.entry_points {
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
