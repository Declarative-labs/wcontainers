//! Turns Direct3D shader bytecode into the common form of `wcontainers-gpu` (a validated naga module)
//! with its reflection: DXBC through dxbc-spirv's own intermediate form, lowered straight into naga.

extern crate alloc;

mod dxbc;
mod ir;
mod lower;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use wcontainers_gpu::{Reflection, ShaderModule, VariantKey};

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
/// converters' revisions.
pub const CONVERTER_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+dxbc-spirv.6196cc6c62b7b04acb4f3e713ff3bb1a7e7950fd"
);

/// Converts a shader into the common form, as the variant asks. `root_signature` is the Direct3D 12
/// root signature the shader is used with.
pub fn convert(
    source: Source<'_>,
    variant: &VariantKey,
    root_signature: Option<&[u8]>,
) -> Result<Converted, ConvertError> {
    let _ = root_signature;
    let bytes = match source {
        Source::Dxbc(bytes) => bytes,
        Source::Dxil(_) => return Err(ConvertError::Unsupported("Dxil")),
        Source::SpirV(_) => return Err(ConvertError::Unsupported("SpirV")),
    };
    let ir = dxbc::to_ir(bytes)?;
    let (module, reflection, stage) = lower::lower(&ir, variant)?;
    let info = validate(&module)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    hasher.update(CONVERTER_VERSION.as_bytes());
    hasher.update(format!("{variant:?}").as_bytes());
    Ok(Converted {
        modules: vec![ShaderModule {
            module,
            info,
            entry_point: "main".into(),
            stage,
            key: *hasher.finalize().as_bytes(),
        }],
        reflection,
    })
}

fn validate(module: &naga::Module) -> Result<naga::valid::ModuleInfo, ConvertError> {
    Validator::new(ValidationFlags::all(), WEBGPU_CAPABILITIES)
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
