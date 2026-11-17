//! What the fast-path defaults keep between calls: built-in programs and blocks made at first use (each
//! program only when a default first draws with it, so a back end never builds one it cannot run), and
//! the ring for per-draw data. Each back end holds one [`Helpers`] and hands it out through
//! [`crate::Backend::helpers`]; a back end that overrides every fast path never fills it.

use crate::backend::Backend;
use crate::error::GpuError;
use crate::resource::{AddressMode, BorderColor, Filter, SamplerDesc};
use crate::shader::{Program, ShaderModule, ShaderStage};
use crate::state::{
    BlendDesc, ColorWrites, CullMode, DepthStencilDesc, FillMode, FrontFace, ProvokingVertex,
    RasterDesc, RenderTargetBlend, VertexLayoutDesc,
};
use crate::{MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS};
use alloc::string::String;

/// The built-in shaders. Binding numbers follow the binding rule of the plan (section 6.4): uniform
/// buffers from 0, samplers from 16, textures from 32.
const SOURCE: &str = r#"
struct Params { value: vec4<u32>, extra: vec4<u32> }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(16) var samp: sampler;
@group(0) @binding(32) var src2d: texture_2d<f32>;
@group(0) @binding(33) var src3d: texture_3d<f32>;
@group(0) @binding(34) var src2darray: texture_2d_array<f32>;
@group(0) @binding(35) var srccube: texture_cube<f32>;
@group(0) @binding(36) var srccubearray: texture_cube_array<f32>;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> }

@vertex fn vs_full(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VsOut(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0), uv);
}
@fragment fn fs_mip2d(v: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src2d, samp, v.uv, 0.0);
}
@fragment fn fs_mip3d(v: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src3d, samp, vec3<f32>(v.uv, bitcast<f32>(params.extra.x)), 0.0);
}
@fragment fn fs_mip2darray(v: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src2darray, samp, v.uv, params.extra.z, 0.0);
}
// The direction through texel coordinate uv of cube face `face`, by the face table Direct3D, Vulkan,
// OpenGL and WebGPU share.
fn cube_dir(uv: vec2<f32>, face: u32) -> vec3<f32> {
    let s = uv.x * 2.0 - 1.0;
    let t = uv.y * 2.0 - 1.0;
    switch face {
        case 0u: { return vec3<f32>(1.0, -t, -s); }
        case 1u: { return vec3<f32>(-1.0, -t, s); }
        case 2u: { return vec3<f32>(s, 1.0, t); }
        case 3u: { return vec3<f32>(s, -1.0, -t); }
        case 4u: { return vec3<f32>(s, -t, 1.0); }
        default: { return vec3<f32>(-s, -t, -1.0); }
    }
}
@fragment fn fs_mipcube(v: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(srccube, samp, cube_dir(v.uv, params.extra.z), 0.0);
}
@fragment fn fs_mipcubearray(v: VsOut) -> @location(0) vec4<f32> {
    let layer = params.extra.z;
    return textureSampleLevel(srccubearray, samp, cube_dir(v.uv, layer % 6u), layer / 6u, 0.0);
}
@fragment fn fs_clear_float() -> @location(0) vec4<f32> { return bitcast<vec4<f32>>(params.value); }
@fragment fn fs_clear_uint() -> @location(0) vec4<u32> { return params.value; }
@fragment fn fs_clear_sint() -> @location(0) vec4<i32> { return bitcast<vec4<i32>>(params.value); }
@fragment fn fs_clear_depth() -> @builtin(frag_depth) f32 { return bitcast<f32>(params.extra.y); }
"#;

/// The built-in programs, by use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Builtin {
    Mip2d,
    Mip3d,
    Mip2dArray,
    MipCube,
    MipCubeArray,
    ClearFloat,
    ClearUint,
    ClearSint,
    ClearDepth,
    /// No fragment shader: stencil-only clears.
    ClearStencil,
}

/// How many built-in programs there are.
const BUILTINS: usize = 10;

const FRAGMENTS: [Option<&str>; BUILTINS] = [
    Some("fs_mip2d"),
    Some("fs_mip3d"),
    Some("fs_mip2darray"),
    Some("fs_mipcube"),
    Some("fs_mipcubearray"),
    Some("fs_clear_float"),
    Some("fs_clear_uint"),
    Some("fs_clear_sint"),
    Some("fs_clear_depth"),
    None,
];

/// Bytes of the built-in shaders' uniform block.
pub(crate) const PARAMS_SIZE: usize = 32;

/// Blocks the defaults draw with, made once.
#[derive(Debug)]
pub(crate) struct Builtins<B: Backend> {
    pub blend: B::Blend,
    pub raster: B::Raster,
    pub vertex_layout: B::VertexLayout,
    pub no_depth: B::DepthStencil,
    /// Depth and stencil written unconditionally: depth only, stencil only, both.
    pub write_depth_stencil: [B::DepthStencil; 3],
    pub linear: B::Sampler,
}

impl<B: Backend> Clone for Builtins<B> {
    fn clone(&self) -> Self {
        Builtins {
            blend: self.blend.clone(),
            raster: self.raster.clone(),
            vertex_layout: self.vertex_layout.clone(),
            no_depth: self.no_depth.clone(),
            write_depth_stencil: self.write_depth_stencil.clone(),
            linear: self.linear.clone(),
        }
    }
}

/// The ring [`crate::Backend::write_buffer_ring`] writes into.
#[derive(Debug)]
pub(crate) struct Ring<B: Backend> {
    pub buffer: B::Buffer,
    pub size: u64,
    pub head: u64,
}

/// State the fast-path defaults keep. Back ends make it with `Helpers::default()` and never look inside.
#[derive(Debug)]
pub struct Helpers<B: Backend> {
    pub(crate) builtins: Option<Builtins<B>>,
    /// The built-in shaders in the common form, read at first use.
    module: Option<(naga::Module, naga::valid::ModuleInfo)>,
    programs: [Option<B::Program>; BUILTINS],
    pub(crate) ring: Option<Ring<B>>,
}

impl<B: Backend> Default for Helpers<B> {
    fn default() -> Self {
        Helpers {
            builtins: None,
            module: None,
            programs: [const { None }; BUILTINS],
            ring: None,
        }
    }
}

/// The raster block every built-in draw uses.
pub(crate) const RASTER: RasterDesc = RasterDesc {
    fill: FillMode::Solid,
    cull: CullMode::None,
    front: FrontFace::CounterClockwise,
    depth_bias: 0,
    depth_bias_slope: 0,
    depth_bias_clamp: 0,
    depth_clip: true,
    scissor: true,
    multisample_lines: false,
    line_width: 0x3f80_0000,
    provoking: ProvokingVertex::First,
    depth_minus_one: false,
    sample_mask: !0,
};

/// The built-in program `which` on `backend`, made the first time a default draws with it.
pub(crate) fn program<B: Backend>(backend: &mut B, which: Builtin) -> Result<B::Program, GpuError> {
    if let Some(p) = &backend.helpers().programs[which as usize] {
        return Ok(p.clone());
    }
    if backend.helpers().module.is_none() {
        let module = naga::front::wgsl::parse_str(SOURCE)
            .map_err(|_| GpuError::Invalid("built-in shaders do not parse"))?;
        // Cube arrays are read only by the cube-array mip program, which runs only where they exist.
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::CUBE_ARRAY_TEXTURES,
        )
        .validate(&module)
        .map_err(|_| GpuError::Invalid("built-in shaders do not validate"))?;
        backend.helpers().module = Some((module, info));
    }
    let shader = |backend: &mut B, entry: &str, stage: ShaderStage| {
        let Some((module, info)) = &backend.helpers().module else {
            unreachable!("the module was read above")
        };
        let mut key = [0u8; 32];
        key[..9].copy_from_slice(b"builtin1:");
        let n = entry.len().min(23);
        key[9..9 + n].copy_from_slice(&entry.as_bytes()[..n]);
        let module = ShaderModule {
            module: module.clone(),
            info: info.clone(),
            entry_point: String::from(entry),
            stage,
            key,
        };
        backend.create_shader(&module)
    };
    let vertex = shader(backend, "vs_full", ShaderStage::Vertex)?;
    let fragment = match FRAGMENTS[which as usize] {
        Some(name) => Some(shader(backend, name, ShaderStage::Fragment)?),
        None => None,
    };
    let program = backend.create_program(&Program::Render {
        vertex: &vertex,
        fragment: fragment.as_ref(),
    })?;
    if let Some(fragment) = fragment {
        backend.destroy(crate::Object::Shader(fragment));
    }
    backend.destroy(crate::Object::Shader(vertex));
    backend.helpers().programs[which as usize] = Some(program.clone());
    Ok(program)
}

/// Makes the built-in blocks on `backend` the first time a default needs them.
pub(crate) fn builtins<B: Backend>(backend: &mut B) -> Result<Builtins<B>, GpuError> {
    if let Some(b) = &backend.helpers().builtins {
        return Ok(b.clone());
    }
    let blend = backend.create_blend(&BlendDesc {
        targets: [RenderTargetBlend {
            blend: None,
            writes: ColorWrites::all(),
        }; MAX_COLOR_TARGETS],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    })?;
    let raster = backend.create_raster(&RASTER)?;
    let vertex_layout = backend.create_vertex_layout(&VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    })?;
    let no_depth = backend.create_depth_stencil(&depth_stencil(false, false))?;
    let write_depth_stencil = [
        backend.create_depth_stencil(&depth_stencil(true, false))?,
        backend.create_depth_stencil(&depth_stencil(false, true))?,
        backend.create_depth_stencil(&depth_stencil(true, true))?,
    ];
    let linear = backend.create_sampler(&SamplerDesc {
        min: Filter::Linear,
        mag: Filter::Linear,
        mip: Filter::Nearest,
        address: [AddressMode::Clamp; 3],
        lod_range: [0, 0x7f7f_ffff],
        anisotropy: 1,
        compare: None,
        border: BorderColor::TransparentBlack,
    })?;
    let b = Builtins {
        blend,
        raster,
        vertex_layout,
        no_depth,
        write_depth_stencil,
        linear,
    };
    backend.helpers().builtins = Some(b.clone());
    Ok(b)
}

/// A depth-stencil block that writes the given aspects unconditionally, or tests nothing.
fn depth_stencil(depth: bool, stencil: bool) -> DepthStencilDesc {
    use crate::resource::CompareFn;
    use crate::state::{StencilFace, StencilOp};
    let face = StencilFace {
        compare: CompareFn::Always,
        fail: StencilOp::Replace,
        depth_fail: StencilOp::Replace,
        pass: StencilOp::Replace,
    };
    DepthStencilDesc {
        depth_compare: depth.then_some(CompareFn::Always),
        depth_write: depth,
        stencil: stencil.then_some((face, face)),
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    }
}
