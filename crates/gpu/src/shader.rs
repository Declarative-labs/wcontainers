//! Shaders in the common form, and programs made from them.
//!
//! The common form is a validated [`naga::Module`]. Fronts get one from `wcontainers-shaders` (which
//! converts DXBC, DXIL and later SPIR-V into it) and hand it to the back end, which writes its own language
//! from it (WGSL for WebGPU, GLSL ES 3.00 for WebGL 2).

use alloc::string::String;
use bitflags::bitflags;

/// Pipeline stages a back end runs. Geometry, hull and domain stages never reach a back end: the shared
/// stage helper lowers them to compute and vertex programs first (see the plan, section 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

bitflags! {
    /// A set of stages, for which stages read a binding.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    pub struct StageSet: u8 {
        const VERTEX = 1;
        const FRAGMENT = 2;
        const COMPUTE = 4;
    }
}

/// One entry point of a module in the common form, validated.
#[derive(Clone, Debug)]
pub struct ShaderModule {
    pub module: naga::Module,
    pub info: naga::valid::ModuleInfo,
    pub entry_point: String,
    pub stage: ShaderStage,
    /// The content key this module was converted from (source bytes plus converter version), which the
    /// back end uses to key what it builds from it.
    pub key: [u8; 32],
}

impl ShaderModule {
    /// The entry point `entry` of a WGSL module, for programs written in WGSL (built-ins, the present
    /// copy, tests). The content key is BLAKE3 of the source and the entry point.
    pub fn from_wgsl(
        source: &str,
        entry: &str,
        stage: ShaderStage,
    ) -> Result<Self, crate::GpuError> {
        let module = naga::front::wgsl::parse_str(source)
            .map_err(|_| crate::GpuError::Invalid("a WGSL module does not parse"))?;
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .map_err(|_| crate::GpuError::Invalid("a WGSL module does not validate"))?;
        let key = *blake3::Hasher::new()
            .update(source.as_bytes())
            .update(entry.as_bytes())
            .finalize()
            .as_bytes();
        Ok(ShaderModule {
            module,
            info,
            entry_point: String::from(entry),
            stage,
            key,
        })
    }
}

/// The program block: the shaders one draw or dispatch runs.
#[derive(Debug)]
pub enum Program<'a, B: crate::Backend> {
    /// A vertex shader and an optional fragment shader (depth-only passes have none). Every location
    /// the fragment shader reads, the vertex shader writes, with the same type, interpolation and
    /// sampling (WebGPU's and GLSL ES's linking rule; unstated ones are WGSL's defaults: a float
    /// perspective at the centre, a flat value from the first vertex); it may write more. Shaders converted apart agree through
    /// [`crate::VariantKey::output_interpolation`] and [`crate::VariantKey::linked`].
    Render {
        vertex: &'a B::Shader,
        fragment: Option<&'a B::Shader>,
    },
    Compute(&'a B::Shader),
}
