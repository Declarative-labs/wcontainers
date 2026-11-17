//! What the fast-path defaults keep between calls: built-in programs and blocks made at first use, and
//! the ring for per-draw data. Each built-in program is its own module, holding only the bindings and
//! types its entry points use, and is made only when a default first draws with it, so a back end never
//! meets one it cannot run (WebGL 2 never meets a cube-array texture). Each back end holds one
//! [`Helpers`] and hands it out through [`crate::Backend::helpers`]; a back end that overrides every fast
//! path never fills it.

use crate::backend::Backend;
use crate::error::GpuError;
use crate::resource::{AddressMode, BorderColor, Filter, SamplerDesc, ViewDimension};
use crate::shader::{Program, ShaderModule, ShaderStage};
use crate::state::{
    BlendDesc, ColorWrites, CullMode, DepthStencilDesc, FillMode, FrontFace, ProvokingVertex,
    RasterDesc, RenderTargetBlend, VertexLayoutDesc,
};
use crate::{Features, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// What every built-in module starts with. Binding numbers follow the binding rule of the plan (section
/// 6.4): uniform buffers from 0, samplers from 16, textures from 32.
const PRELUDE: &str = r#"
struct Params { value: vec4<u32>, extra: vec4<u32> }
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_full(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VsOut(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0), uv);
}
"#;

const PARAMS: &str = "@group(0) @binding(0) var<uniform> params: Params;\n";

/// The direction through texel coordinate uv of cube face `face`, by the face table Direct3D, Vulkan,
/// OpenGL and WebGPU share.
const CUBE_DIR: &str = r#"
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
"#;

/// How a mip program reads the level above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MipRead {
    /// One sample through the linear sampler: formats with [`crate::FormatUse::FILTER`].
    Filtered,
    /// Four (eight in 3D) samples at texel centres through the nearest sampler, blended in the program
    /// with the weights a linear filter uses: float formats the back end cannot filter.
    Unfiltered,
    /// Integer loads decoded as unsigned or signed normalised values (scale in `params.value.x`),
    /// blended in the program, encoded back: textures a front stores by
    /// [`crate::SlowPath::NormAsInteger`].
    Unorm,
    Snorm,
}

/// The built-in programs, by use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Builtin {
    /// Fills a level from the level above, read through a view of this dimension.
    Mip(ViewDimension, MipRead),
    ClearFloat,
    ClearUint,
    ClearSint,
    ClearDepth,
    /// No fragment shader: stencil-only clears.
    ClearStencil,
}

impl Builtin {
    /// Whether the program reads the uniform block at binding 0.
    pub(crate) fn uses_params(self) -> bool {
        !matches!(
            self,
            Builtin::ClearStencil
                | Builtin::Mip(ViewDimension::D2, MipRead::Filtered | MipRead::Unfiltered)
        )
    }

    /// Whether the program samples through the sampler at binding 16.
    pub(crate) fn uses_sampler(self) -> bool {
        matches!(
            self,
            Builtin::Mip(_, MipRead::Filtered | MipRead::Unfiltered)
        )
    }

    /// The program's module source.
    fn source(self) -> String {
        let mut s = String::from(PRELUDE);
        if self.uses_params() {
            s.push_str(PARAMS);
        }
        let body = match self {
            Builtin::Mip(dim, read) => return mip_source(s, dim, read),
            Builtin::ClearFloat => {
                "@fragment fn fs() -> @location(0) vec4<f32> { return bitcast<vec4<f32>>(params.value); }"
            }
            Builtin::ClearUint => {
                "@fragment fn fs() -> @location(0) vec4<u32> { return params.value; }"
            }
            Builtin::ClearSint => {
                "@fragment fn fs() -> @location(0) vec4<i32> { return bitcast<vec4<i32>>(params.value); }"
            }
            Builtin::ClearDepth => {
                "@fragment fn fs() -> @builtin(frag_depth) f32 { return bitcast<f32>(params.extra.y); }"
            }
            Builtin::ClearStencil => "",
        };
        s.push_str(body);
        s
    }

    /// Whether the program has a fragment stage.
    fn has_fragment(self) -> bool {
        self != Builtin::ClearStencil
    }
}

/// The source of a mip program. `params.extra.x` holds a 3D slice's depth coordinate (f32 bits),
/// `params.extra.z` the layer, or the cube face (with the cube times six in a cube array).
fn mip_source(mut s: String, dim: ViewDimension, read: MipRead) -> String {
    let (ty, three) = match dim {
        ViewDimension::D2 => ("texture_2d", false),
        ViewDimension::D2Array => ("texture_2d_array", false),
        ViewDimension::Cube => ("texture_cube", false),
        ViewDimension::CubeArray => ("texture_cube_array", false),
        ViewDimension::D3 => ("texture_3d", true),
    };
    let scalar = match read {
        MipRead::Filtered | MipRead::Unfiltered => "f32",
        MipRead::Unorm => "u32",
        MipRead::Snorm => "i32",
    };
    if matches!(dim, ViewDimension::Cube | ViewDimension::CubeArray) {
        s.push_str(CUBE_DIR);
    }
    if matches!(read, MipRead::Filtered | MipRead::Unfiltered) {
        s.push_str("@group(0) @binding(16) var samp: sampler;\n");
    }
    s.push_str(&format!(
        "@group(0) @binding(32) var src: {ty}<{scalar}>;\n"
    ));
    // Sampling coordinates for a normalised position `c` (a vec2, or a vec3 in 3D).
    let coord = match dim {
        ViewDimension::D2 | ViewDimension::D3 => "c",
        ViewDimension::D2Array => "c, params.extra.z",
        ViewDimension::Cube => "cube_dir(c, params.extra.z)",
        ViewDimension::CubeArray => "cube_dir(c, params.extra.z % 6u), params.extra.z / 6u",
    };
    let (vec, uv) = if three {
        ("vec3", "vec3<f32>(v.uv, bitcast<f32>(params.extra.x))")
    } else {
        ("vec2", "v.uv")
    };
    if read == MipRead::Filtered {
        s.push_str(&format!(
            "@fragment fn fs(v: VsOut) -> @location(0) vec4<f32> {{\n    let c = {uv};\n    \
             return textureSampleLevel(src, samp, {coord}, 0.0);\n}}\n"
        ));
        return s;
    }
    // `at` reads one texel of the level above, clamped to its edge, as a float.
    let size = format!(
        "{vec}<i32>(textureDimensions(src){})",
        if three { "" } else { ".xy" }
    );
    let fetch = match read {
        MipRead::Unfiltered => format!(
            "let c = (vec{n}<f32>(t) + 0.5) / vec{n}<f32>(size);\n    \
             return textureSampleLevel(src, samp, {coord}, 0.0);",
            n = if three { 3 } else { 2 }
        ),
        _ => {
            let layer = if dim == ViewDimension::D2Array {
                ", params.extra.z"
            } else {
                ""
            };
            let x = format!("vec4<f32>(textureLoad(src, t{layer}, 0)) / f32(params.value.x)");
            if read == MipRead::Snorm {
                format!("return max({x}, vec4<f32>(-1.0));")
            } else {
                format!("return {x};")
            }
        }
    };
    s.push_str(&format!(
        "fn at(p: {vec}<i32>) -> vec4<f32> {{\n    let size = {size};\n    \
         let t = clamp(p, {vec}<i32>(0), size - 1);\n    {fetch}\n}}\n"
    ));
    // The weights a linear filter gives at the centre of the texel being written.
    let blend = if three {
        "let a = mix(mix(at(i), at(i + vec3<i32>(1, 0, 0)), f.x), \
         mix(at(i + vec3<i32>(0, 1, 0)), at(i + vec3<i32>(1, 1, 0)), f.x), f.y);\n    \
         let b = mix(mix(at(i + vec3<i32>(0, 0, 1)), at(i + vec3<i32>(1, 0, 1)), f.x), \
         mix(at(i + vec3<i32>(0, 1, 1)), at(i + vec3<i32>(1, 1, 1)), f.x), f.y);\n    \
         let r = mix(a, b, f.z);"
    } else {
        "let r = mix(mix(at(i), at(i + vec2<i32>(1, 0)), f.x), \
         mix(at(i + vec2<i32>(0, 1)), at(i + vec2<i32>(1, 1)), f.x), f.y);"
    };
    let (out, ret) = match read {
        MipRead::Unorm => (
            "vec4<u32>",
            "vec4<u32>(round(clamp(r, vec4<f32>(0.0), vec4<f32>(1.0)) * f32(params.value.x)))",
        ),
        MipRead::Snorm => (
            "vec4<i32>",
            "vec4<i32>(round(clamp(r, vec4<f32>(-1.0), vec4<f32>(1.0)) * f32(params.value.x)))",
        ),
        _ => ("vec4<f32>", "r"),
    };
    s.push_str(&format!(
        "@fragment fn fs(v: VsOut) -> @location(0) {out} {{\n    \
         let p = {uv} * {vec}<f32>({size}) - 0.5;\n    \
         let i = {vec}<i32>(floor(p));\n    let f = p - floor(p);\n    {blend}\n    return {ret};\n}}\n"
    ));
    s
}

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
    pub nearest: B::Sampler,
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
            nearest: self.nearest.clone(),
        }
    }
}

/// One ring [`crate::Backend::write_buffer_ring`] writes into.
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
    /// Built-in programs made so far.
    pub(crate) programs: Vec<(Builtin, B::Program)>,
    /// The rings, by [`crate::RingKind`].
    pub(crate) rings: [Option<Ring<B>>; 2],
    /// Bytes a default builds before one ring write (a converted fan's indices), kept so no draw
    /// allocates once it has grown.
    pub(crate) scratch: Vec<u8>,
}

impl<B: Backend> Default for Helpers<B> {
    fn default() -> Self {
        Helpers {
            builtins: None,
            programs: Vec::new(),
            rings: [None, None],
            scratch: Vec::new(),
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
    line_width: 0x3f80_0000,
    provoking: ProvokingVertex::First,
    depth_minus_one: false,
    sample_mask: !0,
};

/// The built-in module of `which` in the common form, validated with only the abilities it needs.
pub(crate) fn module(which: Builtin) -> Result<(naga::Module, naga::valid::ModuleInfo), GpuError> {
    let module = naga::front::wgsl::parse_str(&which.source())
        .map_err(|_| GpuError::Invalid("built-in shaders do not parse"))?;
    let caps = if matches!(which, Builtin::Mip(ViewDimension::CubeArray, _)) {
        naga::valid::Capabilities::CUBE_ARRAY_TEXTURES
    } else {
        naga::valid::Capabilities::empty()
    };
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
        .validate(&module)
        .map_err(|_| GpuError::Invalid("built-in shaders do not validate"))?;
    Ok((module, info))
}

/// The built-in program `which` on `backend`, made the first time a default draws with it.
pub(crate) fn program<B: Backend>(backend: &mut B, which: Builtin) -> Result<B::Program, GpuError> {
    if let Some((_, p)) = backend.helpers().programs.iter().find(|(w, _)| *w == which) {
        return Ok(p.clone());
    }
    let source = which.source();
    let (module, info) = module(which)?;
    let shader = |backend: &mut B, entry: &str, stage: ShaderStage| {
        // The content key, made as every converted shader's is: BLAKE3 of what the module comes from,
        // here the built-in's WGSL and the entry point.
        let k = *blake3::Hasher::new()
            .update(source.as_bytes())
            .update(entry.as_bytes())
            .finalize()
            .as_bytes();
        backend.create_shader(&ShaderModule {
            module: module.clone(),
            info: info.clone(),
            entry_point: String::from(entry),
            stage,
            key: k,
        })
    };
    let vertex = shader(backend, "vs_full", ShaderStage::Vertex)?;
    let fragment = if which.has_fragment() {
        Some(shader(backend, "fs", ShaderStage::Fragment)?)
    } else {
        None
    };
    let program = backend.create_program(&Program::Render {
        vertex: &vertex,
        fragment: fragment.as_ref(),
    })?;
    if let Some(fragment) = fragment {
        backend.destroy(crate::Object::Shader(fragment));
    }
    backend.destroy(crate::Object::Shader(vertex));
    backend.helpers().programs.push((which, program.clone()));
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
    // Built-in programs write clip z 0, inside both clip depth conventions: take the one the back end has.
    let raster = backend.create_raster(&RasterDesc {
        depth_minus_one: !backend
            .caps()
            .features
            .contains(Features::CLIP_DEPTH_ZERO_ONE),
        ..RASTER
    })?;
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
    let sampler = |filter| SamplerDesc {
        min: filter,
        mag: filter,
        mip: Filter::Nearest,
        address: [AddressMode::Clamp; 3],
        lod_range: [0, 0x7f7f_ffff],
        anisotropy: 1,
        compare: None,
        border: BorderColor::TransparentBlack,
    };
    let linear = backend.create_sampler(&sampler(Filter::Linear))?;
    let nearest = backend.create_sampler(&sampler(Filter::Nearest))?;
    let b = Builtins {
        blend,
        raster,
        vertex_layout,
        no_depth,
        write_depth_stencil,
        linear,
        nearest,
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

#[cfg(test)]
mod tests;
