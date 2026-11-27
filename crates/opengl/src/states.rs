//! Gallium's state objects as the interface's blocks, made from the Gallium trait's plain data: the
//! blend, depth and stencil, rasterizer, sampler and vertex element states. Each object gives the
//! blocks it made back to the back end when its last holder drops it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use wcontainers_gallium::{
    BlendState, DepthStencilAlphaState, RasterizerState, SamplerState, StencilState, VertexElement,
};
use wcontainers_gpu::{
    Gpu, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, Object, caps::Caps, caps::Features,
    resource::AddressMode, resource::BorderColor, resource::CompareFn, resource::Filter,
    resource::SamplerDesc, slow::SlowPath, state::BlendComponent, state::BlendDesc,
    state::BlendFactor, state::BlendOp, state::ColorWrites, state::CullMode,
    state::DepthStencilDesc, state::FillMode, state::FrontFace, state::LogicOp,
    state::ProvokingVertex, state::RasterDesc, state::RenderTargetBlend, state::StencilFace,
    state::StencilOp, state::StepMode, state::VertexAttribute, state::VertexFormat,
    state::VertexLayoutDesc, state::VertexStream,
};
use wcontainers_nir::mesa::{NearestSampler, Wrap};

use crate::guest::refuse;
use crate::packed::{Packed, packed_format};
use crate::pipe::*;

/// The channels of a colour, as Gallium's blend colour holds them.
pub const COLOR_CHANNELS: usize = 4;

/// Objects whose holders went, given back to the back end when it is next used (Mesa deletes
/// states from inside the driver's own calls, and a bound state outlives its deletion).
pub type Retired<B> = Arc<Mutex<Vec<Object<B>>>>;

/// Gives `object` back to the back end at its next use.
pub fn retire<B: Gpu>(retired: &Retired<B>, object: Object<B>) {
    crate::screen::whole(retired).push(object);
}

/// A blend state: the interface's block for targets that blend, built per framebuffer at the draw
/// (blending does not apply to integer targets, which WebGPU refuses to blend).
pub struct Blend {
    pub desc: BlendDesc,
}

pub struct DepthStencil<B: Gpu> {
    pub block: B::DepthStencil,
    pub depth: bool,
    pub stencil: bool,
    /// Whether back faces have a stencil state of their own; without one they take the front
    /// face's reference too, as Gallium's `pipe_stencil_ref` says.
    pub two_sided: bool,
    /// One block a face, each with that face's masks on both, where the faces' masks differ and
    /// the back end has one of each per draw: the draw is then split by face.
    pub faces: Option<[B::DepthStencil; 2]>,
    pub retired: Retired<B>,
}

impl<B: Gpu> Drop for DepthStencil<B> {
    fn drop(&mut self) {
        match self.faces.take() {
            Some(faces) => faces
                .into_iter()
                .for_each(|f| retire(&self.retired, Object::DepthStencil(f))),
            None => retire(&self.retired, Object::DepthStencil(self.block.clone())),
        }
    }
}

pub struct Raster<B: Gpu> {
    pub block: B::Raster,
    /// What `block` was made from, for the blocks of other sample masks.
    pub desc: RasterDesc,
    pub cull: CullMode,
    /// Where nothing is culled and the back end has one stencil reference and mask per draw: the
    /// block drawing front faces only, then back faces only, for a draw split by face.
    pub faces: Option<[B::Raster; 2]>,
    /// Both faces culled: triangles draw nothing.
    pub cull_both: bool,
    /// Nothing is rasterized (stream output alone).
    pub discard: bool,
    /// The vertex program's depth goes from OpenGL's clip space to the host's.
    pub minus_one: bool,
    pub scissor: bool,
    /// Which vertex of a primitive gives flat-shaded values.
    pub provoking: ProvokingVertex,
    /// The vertex the block takes them from: `provoking` where the back end has it.
    pub host_provoking: ProvokingVertex,
    /// The points' size where the vertex program writes none.
    pub point_size: f32,
    /// `gl_PointCoord`'s t grows with the rows (Gallium's upper-left sprite origin).
    pub point_coord_down: bool,
    pub retired: Retired<B>,
}

impl<B: Gpu> Drop for Raster<B> {
    fn drop(&mut self) {
        retire(&self.retired, Object::Raster(self.block.clone()));
        for face in self.faces.take().into_iter().flatten() {
            retire(&self.retired, Object::Raster(face));
        }
    }
}

pub struct Sampler<B: Gpu> {
    pub sampler: B::Sampler,
    /// How it samples an integer texture, by the nearest texel's load: none for wraps such loads
    /// do not make (the mirrored clamp).
    pub nearest: Option<NearestSampler>,
    /// The state asked for, where the program applies its border colour (`BorderInShader`): the
    /// back end lacks border colours, and `sampler` clamps to the edge there.
    pub border_in_program: Option<SamplerDesc>,
    /// The border colour's bits, f32 or integer as the texture's texels.
    pub border_color: [u32; COLOR_CHANNELS],
    /// Its level-of-detail bias, which the program adds (`LodBiasInShader`).
    pub lod_bias: f32,
    pub retired: Retired<B>,
}

impl<B: Gpu> Drop for Sampler<B> {
    fn drop(&mut self) {
        retire(&self.retired, Object::Sampler(self.sampler.clone()));
    }
}

pub struct Elements<B: Gpu> {
    pub desc: VertexLayoutDesc,
    /// The streams the elements read.
    pub streams: u32,
    /// The inputs the vertex program fetches itself whatever their streams' offsets, by location,
    /// and why: a layout the back end cannot read (`VertexPulledInShader`) or an instance step it
    /// lacks (`InstanceRateInShader`).
    pub pulled: Vec<(u32, SlowPath)>,
    /// The back end's layouts of the inputs it reads, by the inputs fetched in the program (a bit
    /// a location).
    pub blocks: RefCell<HashMap<u32, B::VertexLayout>>,
    /// The inputs of packed formats the interface lacks, by location, read as one word each and
    /// unpacked in the program (`VertexFormatConverted`).
    pub packed: Vec<(u32, Packed)>,
    pub retired: Retired<B>,
}

impl<B: Gpu> Drop for Elements<B> {
    fn drop(&mut self) {
        for (_, block) in self.blocks.borrow_mut().drain() {
            retire(&self.retired, Object::VertexLayout(block));
        }
    }
}

impl<B: Gpu> Elements<B> {
    /// The format each input reads, by location.
    pub fn format(&self, location: u32) -> Option<VertexFormat> {
        self.desc
            .attributes
            .iter()
            .flatten()
            .find(|a| a.location == location)
            .map(|a| a.format)
    }

    /// The stream that feeds the input at `location`.
    pub fn stream(&self, location: u32) -> Option<usize> {
        self.desc
            .attributes
            .iter()
            .flatten()
            .find(|a| a.location == location)
            .map(|a| usize::from(a.stream))
    }

    /// The back end's layout of the inputs not in `pulled` (a bit a location), made at its first
    /// use.
    pub fn block(&self, backend: &mut B, pulled: u32) -> Result<B::VertexLayout, String> {
        if let Some(block) = self.blocks.borrow().get(&pulled) {
            return Ok(block.clone());
        }
        let mut desc = self.desc;
        for attribute in &mut desc.attributes {
            if attribute.is_some_and(|a| pulled & (1 << a.location) != 0) {
                *attribute = None;
            }
        }
        for (stream, layout) in desc.streams.iter_mut().enumerate() {
            let read = desc
                .attributes
                .iter()
                .flatten()
                .any(|a| usize::from(a.stream) == stream);
            if !read {
                *layout = None;
            }
        }
        let block = backend
            .create_vertex_layout(&desc)
            .map_err(|e| format!("a vertex layout: {e:?}"))?;
        self.blocks.borrow_mut().insert(pulled, block.clone());
        Ok(block)
    }
}

/// The alignment WebGPU requires of vertex strides and stream offsets (the interface does not
/// report one; asked of the interface track), which an input's offset meets up to its size.
pub const VERTEX_ALIGNMENT: u32 = wgpu::VERTEX_ALIGNMENT as u32;

/// Why the back end cannot read `element` from its stream itself, if it cannot.
fn pulled_because<B: Gpu>(backend: &B, element: &VertexElement) -> Option<SlowPath> {
    let stride = element.src_stride;
    let offset = u32::from(element.src_offset);
    let format = pipe_format::from(element.src_format);
    // A packed format the interface lacks is read as one word (`VertexFormatConverted`).
    let bytes = if packed_format(format).is_some() {
        u32::BITS / u8::BITS
    } else {
        crate::format::block_bytes(format)
    };
    let unaligned = !stride.is_multiple_of(VERTEX_ALIGNMENT)
        || !offset.is_multiple_of(bytes.min(VERTEX_ALIGNMENT))
        || (stride != 0 && offset + bytes > stride);
    if unaligned {
        Some(SlowPath::VertexPulledInShader)
    } else if element.instance_divisor > 1
        && !backend
            .caps()
            .features
            .contains(Features::INSTANCE_STEP_RATE)
    {
        Some(SlowPath::InstanceRateInShader)
    } else {
        None
    }
}

fn factor(f: u32) -> BlendFactor {
    match f {
        PIPE_BLENDFACTOR_ONE => BlendFactor::One,
        PIPE_BLENDFACTOR_SRC_COLOR => BlendFactor::Src,
        PIPE_BLENDFACTOR_SRC_ALPHA => BlendFactor::SrcAlpha,
        PIPE_BLENDFACTOR_DST_ALPHA => BlendFactor::DstAlpha,
        PIPE_BLENDFACTOR_DST_COLOR => BlendFactor::Dst,
        PIPE_BLENDFACTOR_SRC_ALPHA_SATURATE => BlendFactor::SrcAlphaSaturated,
        PIPE_BLENDFACTOR_CONST_COLOR => BlendFactor::Constant,
        PIPE_BLENDFACTOR_CONST_ALPHA => BlendFactor::ConstantAlpha,
        PIPE_BLENDFACTOR_SRC1_COLOR => BlendFactor::Src1,
        PIPE_BLENDFACTOR_SRC1_ALPHA => BlendFactor::Src1Alpha,
        PIPE_BLENDFACTOR_INV_SRC_COLOR => BlendFactor::OneMinusSrc,
        PIPE_BLENDFACTOR_INV_SRC_ALPHA => BlendFactor::OneMinusSrcAlpha,
        PIPE_BLENDFACTOR_INV_DST_ALPHA => BlendFactor::OneMinusDstAlpha,
        PIPE_BLENDFACTOR_INV_DST_COLOR => BlendFactor::OneMinusDst,
        PIPE_BLENDFACTOR_INV_CONST_COLOR => BlendFactor::OneMinusConstant,
        PIPE_BLENDFACTOR_INV_CONST_ALPHA => BlendFactor::OneMinusConstantAlpha,
        PIPE_BLENDFACTOR_INV_SRC1_COLOR => BlendFactor::OneMinusSrc1,
        PIPE_BLENDFACTOR_INV_SRC1_ALPHA => BlendFactor::OneMinusSrc1Alpha,
        _ => BlendFactor::Zero,
    }
}

fn blend_op(f: u32) -> BlendOp {
    match f {
        PIPE_BLEND_SUBTRACT => BlendOp::Subtract,
        PIPE_BLEND_REVERSE_SUBTRACT => BlendOp::ReverseSubtract,
        PIPE_BLEND_MIN => BlendOp::Min,
        PIPE_BLEND_MAX => BlendOp::Max,
        PIPE_BLEND_ADD => BlendOp::Add,
        _ => refuse(format_args!("an advanced blend equation")),
    }
}

fn logic_op(op: u32) -> LogicOp {
    match op {
        PIPE_LOGICOP_CLEAR => LogicOp::Clear,
        PIPE_LOGICOP_NOR => LogicOp::Nor,
        PIPE_LOGICOP_AND_INVERTED => LogicOp::AndInverted,
        PIPE_LOGICOP_COPY_INVERTED => LogicOp::CopyInverted,
        PIPE_LOGICOP_AND_REVERSE => LogicOp::AndReverse,
        PIPE_LOGICOP_INVERT => LogicOp::Invert,
        PIPE_LOGICOP_XOR => LogicOp::Xor,
        PIPE_LOGICOP_NAND => LogicOp::Nand,
        PIPE_LOGICOP_AND => LogicOp::And,
        PIPE_LOGICOP_EQUIV => LogicOp::Equivalent,
        PIPE_LOGICOP_NOOP => LogicOp::Noop,
        PIPE_LOGICOP_OR_INVERTED => LogicOp::OrInverted,
        PIPE_LOGICOP_OR_REVERSE => LogicOp::OrReverse,
        PIPE_LOGICOP_OR => LogicOp::Or,
        PIPE_LOGICOP_SET => LogicOp::Set,
        _ => LogicOp::Copy,
    }
}

/// The interface's comparison of Gallium's.
pub fn compare(f: u32) -> CompareFn {
    match f {
        PIPE_FUNC_NEVER => CompareFn::Never,
        PIPE_FUNC_LESS => CompareFn::Less,
        PIPE_FUNC_EQUAL => CompareFn::Equal,
        PIPE_FUNC_LEQUAL => CompareFn::LessEqual,
        PIPE_FUNC_GREATER => CompareFn::Greater,
        PIPE_FUNC_NOTEQUAL => CompareFn::NotEqual,
        PIPE_FUNC_GEQUAL => CompareFn::GreaterEqual,
        _ => CompareFn::Always,
    }
}

fn stencil_op(op: u32) -> StencilOp {
    match op {
        PIPE_STENCIL_OP_ZERO => StencilOp::Zero,
        PIPE_STENCIL_OP_REPLACE => StencilOp::Replace,
        PIPE_STENCIL_OP_INCR => StencilOp::IncrementClamp,
        PIPE_STENCIL_OP_DECR => StencilOp::DecrementClamp,
        PIPE_STENCIL_OP_INCR_WRAP => StencilOp::IncrementWrap,
        PIPE_STENCIL_OP_DECR_WRAP => StencilOp::DecrementWrap,
        PIPE_STENCIL_OP_INVERT => StencilOp::Invert,
        _ => StencilOp::Keep,
    }
}

/// The interface's colour write mask of Gallium's.
pub fn writes(mask: u32) -> ColorWrites {
    let mut w = ColorWrites::empty();
    for (bit, ours) in [
        (PIPE_MASK_R, ColorWrites::R),
        (PIPE_MASK_G, ColorWrites::G),
        (PIPE_MASK_B, ColorWrites::B),
        (PIPE_MASK_A, ColorWrites::A),
    ] {
        if mask & bit != 0 {
            w |= ours;
        }
    }
    w
}

/// `pipe_context::create_blend_state`.
pub fn blend(state: &BlendState) -> Blend {
    let independent = state.independent_blend_enable != 0;
    let mut targets = [RenderTargetBlend {
        blend: None,
        writes: ColorWrites::all(),
    }; MAX_COLOR_TARGETS];
    for (target, rt) in targets.iter_mut().zip(&state.rt) {
        let rt = if independent { rt } else { &state.rt[0] };
        target.writes = writes(rt.colormask);
        if rt.blend_enable != 0 {
            target.blend = Some((
                BlendComponent {
                    src: factor(rt.rgb_src_factor),
                    dst: factor(rt.rgb_dst_factor),
                    op: blend_op(rt.rgb_func),
                },
                BlendComponent {
                    src: factor(rt.alpha_src_factor),
                    dst: factor(rt.alpha_dst_factor),
                    op: blend_op(rt.alpha_func),
                },
            ));
        }
    }
    Blend {
        desc: BlendDesc {
            targets,
            same_for_all: !independent,
            alpha_to_coverage: state.alpha_to_coverage != 0,
            logic_op: (state.logicop_enable != 0).then(|| logic_op(state.logicop_func)),
        },
    }
}

fn face(state: &StencilState) -> StencilFace {
    StencilFace {
        compare: compare(state.func),
        fail: stencil_op(state.fail_op),
        depth_fail: stencil_op(state.zfail_op),
        pass: stencil_op(state.zpass_op),
    }
}

/// The low byte of a stencil mask, which Gallium declares in 8 bits.
fn mask_byte(mask: u32) -> u8 {
    mask.to_le_bytes()[0]
}

/// `pipe_context::create_depth_stencil_alpha_state`.
pub fn depth_stencil<B: Gpu>(
    backend: &mut B,
    retired: &Retired<B>,
    state: &DepthStencilAlphaState,
) -> DepthStencil<B> {
    let depth = state.depth_enabled != 0;
    let [front, back] = &state.stencil;
    let back_on = back.enabled != 0;
    let back = if back_on { back } else { front };
    let stencil = front.enabled != 0;
    let desc = DepthStencilDesc {
        depth_compare: depth.then(|| compare(state.depth_func)),
        depth_write: depth && state.depth_writemask != 0,
        stencil: stencil.then(|| (face(front), face(back))),
        stencil_read_mask: [mask_byte(front.valuemask), mask_byte(back.valuemask)],
        stencil_write_mask: [mask_byte(front.writemask), mask_byte(back.writemask)],
        depth_bounds: (state.depth_bounds_test != 0).then(|| {
            [
                (state.depth_bounds_min as f32).to_bits(),
                (state.depth_bounds_max as f32).to_bits(),
            ]
        }),
    };
    let [read, write] = [desc.stencil_read_mask, desc.stencil_write_mask];
    let split = stencil
        && (read[0] != read[1] || write[0] != write[1])
        && !backend.caps().features.contains(Features::STENCIL_PER_FACE);
    let mut create = |desc: &DepthStencilDesc| match backend.create_depth_stencil(desc) {
        Ok(block) => block,
        Err(error) => refuse(format!("a depth and stencil state: {error:?}")),
    };
    let faces = split.then(|| {
        [0, 1].map(|face| {
            create(&DepthStencilDesc {
                stencil_read_mask: [read[face]; 2],
                stencil_write_mask: [write[face]; 2],
                ..desc
            })
        })
    });
    let block = match &faces {
        Some([front, _]) => front.clone(),
        None => create(&desc),
    };
    DepthStencil {
        block,
        depth,
        stencil,
        two_sided: back_on,
        faces,
        retired: retired.clone(),
    }
}

fn fill_mode(mode: u32) -> FillMode {
    match mode {
        PIPE_POLYGON_MODE_LINE => FillMode::Line,
        PIPE_POLYGON_MODE_POINT => FillMode::Point,
        _ => FillMode::Solid,
    }
}

/// The host's depth steps in one unit of OpenGL's polygon offset. The host's step is 2^-n of an
/// n-bit depth format (Direct3D's and Vulkan's rule, which WebGPU takes), and a positive offset of
/// one such step at a depth halfway between two stored values can round to the value the depth
/// had without it; OpenGL's unit is the smallest difference that is always resolved, so it is
/// two steps, as Mesa's Direct3D 12 and Zink drivers take it.
const DEPTH_STEPS_PER_OFFSET_UNIT: f32 = 2.0;

/// `pipe_context::create_rasterizer_state`.
pub fn raster<B: Gpu>(backend: &mut B, retired: &Retired<B>, state: &RasterizerState) -> Raster<B> {
    let cull_face = state.cull_face;
    let (front_fill, back_fill) = (state.fill_front, state.fill_back);
    let fill = match cull_face {
        PIPE_FACE_FRONT => fill_mode(back_fill),
        PIPE_FACE_BACK => fill_mode(front_fill),
        _ if front_fill == back_fill => fill_mode(front_fill),
        _ => refuse(format_args!("front and back faces filled differently")),
    };
    if state.poly_stipple_enable != 0 {
        refuse(format_args!("polygon stippling"));
    }
    let provoking = if state.flatshade_first != 0 {
        ProvokingVertex::First
    } else {
        ProvokingVertex::Last
    };
    // The block takes the convention the back end has; flat values from the other one are
    // checked at the draw, where the fragment program is known.
    let caps = backend.caps().clone();
    let host_provoking = match provoking {
        ProvokingVertex::Last if !caps.features.contains(Features::PROVOKING_VERTEX_LAST) => {
            ProvokingVertex::First
        }
        ProvokingVertex::First if !caps.features.contains(Features::PROVOKING_VERTEX_FIRST) => {
            ProvokingVertex::Last
        }
        wanted => wanted,
    };
    let biased = state.offset_tri != 0;
    let bias = |v: f32| if biased { v.to_bits() } else { 0 };
    let desc = RasterDesc {
        fill,
        cull: match cull_face {
            PIPE_FACE_FRONT => CullMode::Front,
            PIPE_FACE_BACK => CullMode::Back,
            _ => CullMode::None,
        },
        front: if state.front_ccw != 0 {
            FrontFace::CounterClockwise
        } else {
            FrontFace::Clockwise
        },
        depth_bias: bias(state.offset_units * DEPTH_STEPS_PER_OFFSET_UNIT),
        depth_bias_slope: bias(state.offset_scale),
        depth_bias_clamp: bias(state.offset_clamp),
        depth_clip: state.depth_clip_near != 0,
        scissor: state.scissor != 0,
        // Without wide lines the back end draws one width, to which the screen's range clamps
        // OpenGL's; Mesa's own states for drawing triangles leave the width zero.
        line_width: if caps.features.contains(Features::WIDE_LINES) {
            state.line_width
        } else {
            caps.limits.max_line_width
        }
        .to_bits(),
        provoking: host_provoking,
        depth_minus_one: false,
        sample_mask: u32::MAX,
    };
    let mut create = |desc: &RasterDesc| match backend.create_raster(desc) {
        Ok(block) => block,
        Err(error) => refuse(format!("a rasterizer state: {error:?}")),
    };
    let block = create(&desc);
    let faces = (desc.cull == CullMode::None
        && !caps.features.contains(Features::STENCIL_PER_FACE))
    .then(|| [CullMode::Back, CullMode::Front].map(|cull| create(&RasterDesc { cull, ..desc })));
    Raster {
        block,
        desc,
        cull: desc.cull,
        faces,
        cull_both: cull_face == PIPE_FACE_FRONT_AND_BACK,
        discard: state.rasterizer_discard != 0,
        minus_one: state.clip_halfz == 0,
        scissor: desc.scissor,
        provoking,
        host_provoking,
        point_size: state.point_size,
        point_coord_down: state.sprite_coord_mode == PIPE_SPRITE_COORD_UPPER_LEFT,
        retired: retired.clone(),
    }
}

fn address(wrap: u32) -> AddressMode {
    match wrap {
        PIPE_TEX_WRAP_REPEAT => AddressMode::Repeat,
        PIPE_TEX_WRAP_CLAMP_TO_EDGE => AddressMode::Clamp,
        PIPE_TEX_WRAP_CLAMP_TO_BORDER => AddressMode::Border,
        PIPE_TEX_WRAP_MIRROR_REPEAT => AddressMode::Mirror,
        PIPE_TEX_WRAP_MIRROR_CLAMP_TO_EDGE => AddressMode::MirrorOnce,
        _ => refuse(format_args!(
            "a texture wrap mode the screen does not report"
        )),
    }
}

/// The largest level of detail nearest mipmap selection still rounds to the base level.
const BASE_LEVEL_ONLY_MAX_LOD: f32 = 0.25;

fn nearest_wrap(wrap: u32) -> Option<Wrap> {
    Some(match wrap {
        PIPE_TEX_WRAP_REPEAT => Wrap::Repeat,
        PIPE_TEX_WRAP_CLAMP_TO_EDGE => Wrap::ClampToEdge,
        PIPE_TEX_WRAP_MIRROR_REPEAT => Wrap::MirroredRepeat,
        PIPE_TEX_WRAP_CLAMP_TO_BORDER => Wrap::ClampToBorder,
        _ => return None,
    })
}

fn filter(f: u32) -> Filter {
    if f == PIPE_TEX_FILTER_LINEAR {
        Filter::Linear
    } else {
        Filter::Nearest
    }
}

fn border(color: [f32; COLOR_CHANNELS]) -> BorderColor {
    match color {
        [0.0, 0.0, 0.0, 0.0] => BorderColor::TransparentBlack,
        [0.0, 0.0, 0.0, 1.0] => BorderColor::OpaqueBlack,
        [1.0, 1.0, 1.0, 1.0] => BorderColor::OpaqueWhite,
        _ => BorderColor::Custom(color.map(f32::to_bits)),
    }
}

/// The lower bound of a sampler's level of detail. The Gallium trait's `SamplerState` has no
/// `min_lod` (its generator kept only the second name of p_state.h:487's `float min_lod,
/// max_lod;`), so every sampler is refused until it carries it (NEEDED, G10).
fn min_lod(_: &SamplerState) -> f32 {
    refuse(format_args!(
        "a sampler: the Gallium trait carries no min_lod (G10)"
    ))
}

/// `pipe_context::create_sampler_state`, on a screen whose caps are `caps`.
pub fn sampler<B: Gpu>(
    backend: &mut B,
    retired: &Retired<B>,
    caps: &Caps,
    state: &SamplerState,
) -> Sampler<B> {
    let mip = state.min_mip_filter;
    let min_lod = min_lod(state);
    let lod_range = if mip == PIPE_TEX_MIPFILTER_NONE {
        // The base level alone, with the level of detail free to pass zero so that the host
        // still tells minifying from magnifying, as OpenGL does without mipmaps.
        [0, BASE_LEVEL_ONLY_MAX_LOD.to_bits()]
    } else {
        // The host's levels of detail start at zero, where its magnifying filter takes over, as
        // OpenGL's does at and below zero: a negative bound reads as zero, alike, and a bound past
        // the deepest level reads as that level, which it selects all the same.
        let deepest = crate::screen::deepest_level(caps) as f32;
        let min = min_lod.clamp(0.0, deepest);
        [min.to_bits(), state.max_lod.clamp(min, deepest).to_bits()]
    };
    let wraps = [state.wrap_s, state.wrap_t, state.wrap_r];
    let desc = SamplerDesc {
        min: filter(state.min_img_filter),
        mag: filter(state.mag_img_filter),
        mip: if mip == PIPE_TEX_MIPFILTER_LINEAR {
            Filter::Linear
        } else {
            Filter::Nearest
        },
        address: wraps.map(address),
        lod_range,
        anisotropy: u16::try_from(state.max_anisotropy.max(1)).unwrap_or(u16::MAX),
        compare: (state.compare_mode != 0).then(|| compare(state.compare_func)),
        // The colour's bits, float or integer.
        border: border(state.border_color.map(f32::from_bits)),
    };
    let border_bits = state.border_color;
    let nearest = wraps
        .iter()
        .map(|&w| nearest_wrap(w))
        .collect::<Option<Vec<_>>>()
        .and_then(|w| w.try_into().ok())
        .map(|wrap| NearestSampler {
            unit: 0,
            wrap,
            mipmapped: mip != PIPE_TEX_MIPFILTER_NONE,
            min_lod: min_lod.to_bits(),
            max_lod: state.max_lod.to_bits(),
            border: border_bits,
        });
    let border_in_program = (desc.address.contains(&AddressMode::Border)
        && !backend.caps().features.contains(Features::BORDER_COLOR))
    .then_some(desc);
    let host = match border_in_program {
        Some(asked) => SamplerDesc {
            address: asked.address.map(|a| match a {
                AddressMode::Border => AddressMode::Clamp,
                other => other,
            }),
            border: BorderColor::TransparentBlack,
            ..asked
        },
        None => desc,
    };
    match backend.create_sampler(&host) {
        Ok(sampler) => Sampler {
            sampler,
            nearest,
            border_in_program,
            border_color: border_bits,
            lod_bias: state.lod_bias,
            retired: retired.clone(),
        },
        Err(error) => refuse(format!("a sampler: {error:?}")),
    }
}

/// `pipe_context::create_vertex_elements_state`.
pub fn elements<B: Gpu>(
    backend: &mut B,
    retired: &Retired<B>,
    elements: &[VertexElement],
) -> Elements<B> {
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    if elements.len() > MAX_VERTEX_STREAMS {
        refuse(format_args!(
            "{} vertex elements, more than the screen reports",
            elements.len()
        ));
    }
    let mut streams = 0;
    let mut pulled = Vec::new();
    let mut packed = Vec::new();
    for (location, element) in (0u32..).zip(elements) {
        if let Some(path) = pulled_because(&*backend, element) {
            pulled.push((location, path));
        }
        let format = pipe_format::from(element.src_format);
        let stream = element.vertex_buffer_index;
        let unpacked = packed_format(format);
        if let Some(p) = unpacked {
            packed.push((location, p));
        }
        let host = unpacked.map(|_| VertexFormat::Uint32x1);
        let Some(format) = host.or_else(|| crate::format::vertex_format(format)) else {
            refuse(format_args!(
                "a vertex format the screen does not report, pipe_format {format}"
            ));
        };
        let Some(slot) = desc.streams.get_mut(usize::from(stream)) else {
            refuse(format_args!(
                "vertex stream {stream}, past those the screen reports"
            ));
        };
        *slot = Some(VertexStream {
            stride: element.src_stride,
            step: match element.instance_divisor {
                0 => StepMode::Vertex,
                n => StepMode::Instance(n),
            },
        });
        desc.attributes[location as usize] = Some(VertexAttribute {
            location,
            stream,
            offset: u32::from(element.src_offset),
            format,
        });
        streams = streams.max(u32::from(stream) + 1);
    }
    Elements {
        desc,
        streams,
        pulled,
        blocks: RefCell::new(HashMap::new()),
        packed,
        retired: retired.clone(),
    }
}
