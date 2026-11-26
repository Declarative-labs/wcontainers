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

/// The program block: the shaders one draw or dispatch runs.
#[derive(Debug)]
pub enum Program<'a, B: crate::Gpu> {
    /// A vertex shader and an optional fragment shader (depth-only passes have none). Every location
    /// the fragment shader reads, the vertex shader writes, with the same type, interpolation and
    /// sampling (WebGPU's and GLSL ES's linking rule; unstated ones are WGSL's defaults: a float
    /// perspective at the centre, a flat value from the first vertex); it may write more. Shaders converted apart agree through
    /// [`crate::reflect::VariantKey::output_interpolation`] and [`crate::reflect::VariantKey::linked`].
    Render {
        vertex: &'a B::Shader,
        fragment: Option<&'a B::Shader>,
    },
    Compute(&'a B::Shader),
}
