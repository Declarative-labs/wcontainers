//! Gallium's state objects as the interface's blocks, and the state st/mesa sets between draws.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_uint, c_void};

use wcontainers_gpu::{
    AddressMode, Backend, BlendComponent, BlendDesc, BlendFactor, BlendOp, BorderColor,
    ColorWrites, CompareFn, CullMode, DepthStencilDesc, Features, FillMode, Filter, FrontFace,
    LogicOp, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, Object, ProvokingVertex, RasterDesc,
    RenderTargetBlend, SamplerDesc, SlowPath, StencilFace, StencilOp, StepMode, VertexAttribute,
    VertexFormat, VertexLayoutDesc, VertexStream,
};
use wcontainers_nir::mesa::{NearestSampler, Nir, Wrap};
use wcontainers_shaders::stage::{STREAM_BUFFERS, StreamOutput};

use crate::context::{self, Context};
use crate::gallium::*;
use crate::resource;
use crate::screen::DRIVER_NAME;
use crate::stream;

/// Ends the process naming state the driver does not take yet: a crash with the reason in the
/// case's log, never a draw that ignores it.
pub fn refuse(what: impl std::fmt::Display) -> ! {
    eprintln!("{}: refused: {what}", DRIVER_NAME.to_string_lossy());
    std::process::abort()
}

/// A refusal returned to the draw or call that ends the process with it.
pub fn refused<T>(why: std::fmt::Arguments<'_>) -> Result<T, String> {
    Err(why.to_string())
}

/// The channels of a colour, as Gallium's blend colour holds them.
pub const COLOR_CHANNELS: usize = size_of::<pipe_blend_color>() / size_of::<f32>();

/// The program stages the driver takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Vertex,
    Fragment,
}

impl Stage {
    pub const ALL: [Stage; 2] = [Stage::Vertex, Stage::Fragment];

    pub fn nir(self) -> wcontainers_nir::mesa::Stage {
        match self {
            Stage::Vertex => wcontainers_nir::mesa::Stage::Vertex,
            Stage::Fragment => wcontainers_nir::mesa::Stage::Fragment,
        }
    }
}

/// One uniform buffer a stage reads: a range of a resource, or bytes the frontend handed over.
#[derive(Default)]
pub struct Constants {
    pub resource: *mut pipe_resource,
    pub offset: u32,
    pub size: u32,
    pub bytes: Vec<u8>,
}

/// What one stage has bound.
pub struct StageState<B: Backend> {
    pub constants: Vec<Constants>,
    pub views: Vec<*mut pipe_sampler_view>,
    pub samplers: Vec<*const Sampler<B>>,
    pub shader: *const Shader,
}

impl<B: Backend> Default for StageState<B> {
    fn default() -> Self {
        StageState {
            constants: (0..PIPE_MAX_CONSTANT_BUFFERS)
                .map(|_| Constants::default())
                .collect(),
            views: vec![std::ptr::null_mut(); PIPE_MAX_SHADER_SAMPLER_VIEWS as usize],
            samplers: vec![std::ptr::null(); PIPE_MAX_SAMPLERS as usize],
            shader: std::ptr::null(),
        }
    }
}

/// Both stages' bindings.
pub struct Stages<B: Backend>(pub [StageState<B>; Stage::ALL.len()]);

impl<B: Backend> Default for Stages<B> {
    fn default() -> Self {
        Stages([StageState::default(), StageState::default()])
    }
}

impl<B: Backend> Stages<B> {
    pub fn get(&self, stage: Stage) -> &StageState<B> {
        &self.0[stage as usize]
    }

    fn get_mut(&mut self, stage: Stage) -> &mut StageState<B> {
        &mut self.0[stage as usize]
    }
}

/// A bound vertex stream: a resource holding a reference, and where in it the stream starts.
pub struct VertexBuffer {
    pub resource: *mut pipe_resource,
    pub offset: u32,
}

/// The state set between draws that is not a stage's.
pub struct Bound<B: Backend> {
    pub blend: *const Blend,
    pub depth_stencil: *const DepthStencil<B>,
    pub raster: *const Raster<B>,
    pub elements: *const Elements<B>,
    pub vertex_buffers: Vec<VertexBuffer>,
    pub blend_color: [u32; COLOR_CHANNELS],
    pub stencil_ref: [u8; 2],
    pub sample_mask: u32,
    pub viewport: pipe_viewport_state,
    pub scissor: pipe_scissor_state,
}

impl<B: Backend> Default for Bound<B> {
    fn default() -> Self {
        Bound {
            blend: std::ptr::null(),
            depth_stencil: std::ptr::null(),
            raster: std::ptr::null(),
            elements: std::ptr::null(),
            vertex_buffers: Vec::new(),
            blend_color: Default::default(),
            stencil_ref: [0; 2],
            sample_mask: u32::MAX,
            viewport: pipe_viewport_state::default(),
            scissor: pipe_scissor_state::default(),
        }
    }
}

impl<B: Backend> Bound<B> {
    /// Drops the references the bound vertex streams hold.
    pub fn release(&mut self) {
        for stream in self.vertex_buffers.drain(..) {
            // SAFETY: each stream holds one reference.
            unsafe { resource::release(stream.resource) };
        }
    }
}

/// A blend state: the interface's block for targets that blend, built per framebuffer at the draw
/// (blending does not apply to integer targets, which WebGPU refuses to blend).
pub struct Blend {
    pub desc: BlendDesc,
}

pub struct DepthStencil<B: Backend> {
    pub block: B::DepthStencil,
    pub stencil: bool,
    /// One block a face, each with that face's masks on both, where the faces' masks differ and
    /// the back end has one of each per draw: the draw is then split by face.
    pub faces: Option<[B::DepthStencil; 2]>,
}

pub struct Raster<B: Backend> {
    pub block: B::Raster,
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
}

pub struct Sampler<B: Backend> {
    pub sampler: B::Sampler,
    /// How it samples an integer texture, by the nearest texel's load: none for wraps such loads
    /// do not make (the border and the mirrored clamp).
    pub nearest: Option<NearestSampler>,
}

pub struct Elements<B: Backend> {
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
    /// The bytes of each input's components, by location: a fetched input's components must lie
    /// within words.
    pub component_bytes: Vec<(u32, u32)>,
}

impl<B: Backend> Elements<B> {
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
fn pulled_because<B: Backend>(backend: &B, element: &pipe_vertex_element) -> Option<SlowPath> {
    let stride = element.src_stride;
    let offset = u32::from(element.src_offset);
    let bytes = resource::block_bytes(pipe_format::from(element.src_format));
    let unaligned = !stride.is_multiple_of(VERTEX_ALIGNMENT)
        || !offset.is_multiple_of(bytes.min(VERTEX_ALIGNMENT))
        || (stride != 0 && offset + bytes > stride);
    if unaligned {
        Some(SlowPath::VertexPulledInShader)
    } else if element.instance_divisor > 1 && !backend.caps().has(Features::INSTANCE_STEP_RATE) {
        Some(SlowPath::InstanceRateInShader)
    } else {
        None
    }
}

/// A program stage as st/mesa handed it over: its NIR, kept to build each variant from.
pub struct Shader {
    pub stage: Stage,
    pub nir: Nir,
    /// What a vertex program streams while stream output is bound: the stage helper's
    /// declaration, none, or why it cannot.
    pub streamed: Result<Option<StreamOutput>, &'static str>,
}

fn factor(f: pipe_blendfactor) -> BlendFactor {
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

fn blend_op(f: pipe_blend_func) -> BlendOp {
    match f {
        PIPE_BLEND_SUBTRACT => BlendOp::Subtract,
        PIPE_BLEND_REVERSE_SUBTRACT => BlendOp::ReverseSubtract,
        PIPE_BLEND_MIN => BlendOp::Min,
        PIPE_BLEND_MAX => BlendOp::Max,
        PIPE_BLEND_ADD => BlendOp::Add,
        _ => refuse(format_args!("an advanced blend equation")),
    }
}

fn logic_op(op: pipe_logicop) -> LogicOp {
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

fn compare(f: pipe_compare_func) -> CompareFn {
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

fn stencil_op(op: pipe_stencil_op) -> StencilOp {
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
fn writes(mask: c_uint) -> ColorWrites {
    let mut w = ColorWrites::empty();
    for (bit, ours) in [
        (PIPE_MASK_R, ColorWrites::R),
        (PIPE_MASK_G, ColorWrites::G),
        (PIPE_MASK_B, ColorWrites::B),
        (PIPE_MASK_A, ColorWrites::A),
    ] {
        if mask & bit as c_uint != 0 {
            w |= ours;
        }
    }
    w
}

/// A state object of this driver, given to Mesa as a pointer it hands back.
fn object<T>(value: T) -> *mut c_void {
    Box::into_raw(Box::new(value)).cast()
}

/// # Safety
/// `object` came from [`object`] with a `T` and is dropped once.
unsafe fn drop_object<T>(object: *mut c_void) -> T {
    // SAFETY: the caller's.
    *unsafe { Box::from_raw(object.cast::<T>()) }
}

unsafe extern "C" fn create_blend_state(
    _: *mut pipe_context,
    state: *const pipe_blend_state,
) -> *mut c_void {
    // SAFETY: Mesa passes a blend state.
    let state = unsafe { &*state };
    let independent = state.independent_blend_enable() != 0;
    let mut targets = [RenderTargetBlend {
        blend: None,
        writes: ColorWrites::all(),
    }; MAX_COLOR_TARGETS];
    for (target, rt) in targets.iter_mut().zip(&state.rt) {
        let rt = if independent { rt } else { &state.rt[0] };
        target.writes = writes(rt.colormask());
        if rt.blend_enable() != 0 {
            target.blend = Some((
                BlendComponent {
                    src: factor(rt.rgb_src_factor() as pipe_blendfactor),
                    dst: factor(rt.rgb_dst_factor() as pipe_blendfactor),
                    op: blend_op(rt.rgb_func() as pipe_blend_func),
                },
                BlendComponent {
                    src: factor(rt.alpha_src_factor() as pipe_blendfactor),
                    dst: factor(rt.alpha_dst_factor() as pipe_blendfactor),
                    op: blend_op(rt.alpha_func() as pipe_blend_func),
                },
            ));
        }
    }
    object(Blend {
        desc: BlendDesc {
            targets,
            same_for_all: !independent,
            alpha_to_coverage: state.alpha_to_coverage() != 0,
            logic_op: (state.logicop_enable() != 0)
                .then(|| logic_op(state.logicop_func() as pipe_logicop)),
        },
    })
}

unsafe extern "C" fn bind_blend_state<B: Backend>(pipe: *mut pipe_context, state: *mut c_void) {
    // SAFETY: Mesa passes this driver's context.
    unsafe { Context::<B>::of(pipe) }.bound.blend = state.cast();
}

unsafe extern "C" fn delete_blend_state(_: *mut pipe_context, state: *mut c_void) {
    // SAFETY: made by create_blend_state, deleted once.
    unsafe { drop_object::<Blend>(state) };
}

fn face(state: &pipe_stencil_state) -> StencilFace {
    StencilFace {
        compare: compare(state.func() as pipe_compare_func),
        fail: stencil_op(state.fail_op() as pipe_stencil_op),
        depth_fail: stencil_op(state.zfail_op() as pipe_stencil_op),
        pass: stencil_op(state.zpass_op() as pipe_stencil_op),
    }
}

unsafe extern "C" fn create_depth_stencil_alpha_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *const pipe_depth_stencil_alpha_state,
) -> *mut c_void {
    // SAFETY: Mesa passes this driver's context and a state.
    let (context, state) = unsafe { (Context::<B>::of(pipe), &*state) };
    let depth = state.depth_enabled() != 0;
    let [front, back] = &state.stencil;
    let back_on = back.enabled() != 0;
    let back = if back_on { back } else { front };
    let stencil = front.enabled() != 0;
    let desc = DepthStencilDesc {
        depth_compare: depth.then(|| compare(state.depth_func() as pipe_compare_func)),
        depth_write: depth && state.depth_writemask() != 0,
        stencil: (front.enabled() != 0).then(|| (face(front), face(back))),
        stencil_read_mask: [front.valuemask() as u8, back.valuemask() as u8],
        stencil_write_mask: [front.writemask() as u8, back.writemask() as u8],
        depth_bounds: (state.depth_bounds_test() != 0).then(|| {
            [
                (state.depth_bounds_min as f32).to_bits(),
                (state.depth_bounds_max as f32).to_bits(),
            ]
        }),
    };
    let mut backend = context.screen().backend();
    let [read, write] = [desc.stencil_read_mask, desc.stencil_write_mask];
    let split = stencil
        && (read[0] != read[1] || write[0] != write[1])
        && !backend.caps().has(Features::STENCIL_PER_FACE);
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
    object(DepthStencil::<B> {
        block,
        stencil,
        faces,
    })
}

unsafe extern "C" fn bind_depth_stencil_alpha_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *mut c_void,
) {
    // SAFETY: Mesa passes this driver's context.
    unsafe { Context::<B>::of(pipe) }.bound.depth_stencil = state.cast();
}

unsafe extern "C" fn delete_depth_stencil_alpha_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *mut c_void,
) {
    // SAFETY: made by create_depth_stencil_alpha_state, deleted once, on this driver's context.
    let (context, state) = unsafe {
        (
            Context::<B>::of(pipe),
            drop_object::<DepthStencil<B>>(state),
        )
    };
    let screen = context.screen();
    match state.faces {
        Some(faces) => faces
            .into_iter()
            .for_each(|f| screen.retire(Object::DepthStencil(f))),
        None => screen.retire(Object::DepthStencil(state.block)),
    }
}

fn fill_mode(mode: c_uint) -> FillMode {
    if mode == PIPE_POLYGON_MODE_LINE as c_uint {
        FillMode::Line
    } else if mode == PIPE_POLYGON_MODE_POINT as c_uint {
        FillMode::Point
    } else {
        FillMode::Solid
    }
}

unsafe extern "C" fn create_rasterizer_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *const pipe_rasterizer_state,
) -> *mut c_void {
    // SAFETY: Mesa passes this driver's context and a state.
    let (context, state) = unsafe { (Context::<B>::of(pipe), &*state) };
    let cull_face = state.cull_face();
    let (front_fill, back_fill) = (state.fill_front(), state.fill_back());
    let fill = match cull_face {
        PIPE_FACE_FRONT => fill_mode(back_fill),
        PIPE_FACE_BACK => fill_mode(front_fill),
        _ if front_fill == back_fill => fill_mode(front_fill),
        _ => refuse(format_args!("front and back faces filled differently")),
    };
    if state.poly_stipple_enable() != 0 {
        refuse(format_args!("polygon stippling"));
    }
    let provoking = if state.flatshade_first() != 0 {
        ProvokingVertex::First
    } else {
        ProvokingVertex::Last
    };
    // The block takes the convention the back end has; flat values from the other one are
    // checked at the draw, where the fragment program is known.
    let caps = context.screen().backend().caps().clone();
    let host_provoking = match provoking {
        ProvokingVertex::Last if !caps.has(Features::PROVOKING_VERTEX_LAST) => {
            ProvokingVertex::First
        }
        ProvokingVertex::First if !caps.has(Features::PROVOKING_VERTEX_FIRST) => {
            ProvokingVertex::Last
        }
        wanted => wanted,
    };
    let biased = state.offset_tri() != 0;
    let bias = |v: f32| if biased { v.to_bits() } else { 0 };
    let desc = RasterDesc {
        fill,
        cull: match cull_face {
            PIPE_FACE_FRONT => CullMode::Front,
            PIPE_FACE_BACK => CullMode::Back,
            _ => CullMode::None,
        },
        front: if state.front_ccw() != 0 {
            FrontFace::CounterClockwise
        } else {
            FrontFace::Clockwise
        },
        depth_bias: bias(state.offset_units),
        depth_bias_slope: bias(state.offset_scale),
        depth_bias_clamp: bias(state.offset_clamp),
        depth_clip: state.depth_clip_near() != 0,
        scissor: state.scissor() != 0,
        // Without wide lines the back end draws one width, to which the screen's range clamps
        // OpenGL's; Mesa's own states for drawing triangles leave the width zero.
        line_width: if caps.has(Features::WIDE_LINES) {
            state.line_width
        } else {
            caps.limits.max_line_width
        }
        .to_bits(),
        provoking: host_provoking,
        depth_minus_one: false,
        sample_mask: u32::MAX,
    };
    let mut backend = context.screen().backend();
    let mut create = |desc: &RasterDesc| match backend.create_raster(desc) {
        Ok(block) => block,
        Err(error) => refuse(format!("a rasterizer state: {error:?}")),
    };
    let block = create(&desc);
    let faces = (desc.cull == CullMode::None && !caps.has(Features::STENCIL_PER_FACE)).then(|| {
        [CullMode::Back, CullMode::Front].map(|cull| create(&RasterDesc { cull, ..desc }))
    });
    object(Raster::<B> {
        block,
        cull: desc.cull,
        faces,
        cull_both: cull_face == PIPE_FACE_FRONT_AND_BACK,
        discard: state.rasterizer_discard() != 0,
        minus_one: state.clip_halfz() == 0,
        scissor: desc.scissor,
        provoking,
        host_provoking,
    })
}

unsafe extern "C" fn bind_rasterizer_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *mut c_void,
) {
    // SAFETY: Mesa passes this driver's context.
    unsafe { Context::<B>::of(pipe) }.bound.raster = state.cast();
}

unsafe extern "C" fn delete_rasterizer_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *mut c_void,
) {
    // SAFETY: made by create_rasterizer_state, deleted once, on this driver's context.
    let (context, state) = unsafe { (Context::<B>::of(pipe), drop_object::<Raster<B>>(state)) };
    let screen = context.screen();
    screen.retire(Object::Raster(state.block));
    for face in state.faces.into_iter().flatten() {
        screen.retire(Object::Raster(face));
    }
}

fn address(wrap: pipe_tex_wrap) -> AddressMode {
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

fn nearest_wrap(wrap: pipe_tex_wrap) -> Option<Wrap> {
    Some(match wrap {
        PIPE_TEX_WRAP_REPEAT => Wrap::Repeat,
        PIPE_TEX_WRAP_CLAMP_TO_EDGE => Wrap::ClampToEdge,
        PIPE_TEX_WRAP_MIRROR_REPEAT => Wrap::MirroredRepeat,
        _ => return None,
    })
}

fn filter(f: c_uint) -> Filter {
    if f == PIPE_TEX_FILTER_LINEAR as c_uint {
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

unsafe extern "C" fn create_sampler_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *const pipe_sampler_state,
) -> *mut c_void {
    // SAFETY: Mesa passes this driver's context and a state.
    let (context, state) = unsafe { (Context::<B>::of(pipe), &*state) };
    if state.lod_bias != 0.0 {
        refuse(format_args!("a sampler's level-of-detail bias"));
    }
    let mip = state.min_mip_filter();
    let lod_range = if mip == PIPE_TEX_MIPFILTER_NONE as c_uint {
        [0, 0]
    } else {
        // The host's levels of detail start at zero, where its magnifying filter takes over, as
        // OpenGL's does at and below zero: a negative bound reads as zero, alike.
        let min = state.min_lod.max(0.0);
        [min.to_bits(), state.max_lod.max(min).to_bits()]
    };
    let desc = SamplerDesc {
        min: filter(state.min_img_filter()),
        mag: filter(state.mag_img_filter()),
        mip: if mip == PIPE_TEX_MIPFILTER_LINEAR as c_uint {
            Filter::Linear
        } else {
            Filter::Nearest
        },
        address: [state.wrap_s(), state.wrap_t(), state.wrap_r()]
            .map(|w| address(w as pipe_tex_wrap)),
        lod_range,
        anisotropy: state.max_anisotropy().max(1) as u16,
        compare: (state.compare_mode() != 0)
            .then(|| compare(state.compare_func() as pipe_compare_func)),
        // SAFETY: a float border colour, which is all the screen takes.
        border: border(unsafe { state.border_color.f }),
    };
    let wraps = [state.wrap_s(), state.wrap_t(), state.wrap_r()];
    let nearest = wraps
        .iter()
        .map(|&w| nearest_wrap(w as pipe_tex_wrap))
        .collect::<Option<Vec<_>>>()
        .and_then(|w| w.try_into().ok())
        .map(|wrap| NearestSampler {
            unit: 0,
            wrap,
            mipmapped: mip != PIPE_TEX_MIPFILTER_NONE as c_uint,
            min_lod: state.min_lod.to_bits(),
            max_lod: state.max_lod.to_bits(),
        });
    match context.screen().backend().create_sampler(&desc) {
        Ok(sampler) => object(Sampler::<B> { sampler, nearest }),
        Err(error) => refuse(format!("a sampler: {error:?}")),
    }
}

unsafe extern "C" fn delete_sampler_state<B: Backend>(pipe: *mut pipe_context, state: *mut c_void) {
    // SAFETY: made by create_sampler_state, deleted once, on this driver's context.
    let (context, state) = unsafe { (Context::<B>::of(pipe), drop_object::<Sampler<B>>(state)) };
    context.screen().retire(Object::Sampler(state.sampler));
}

unsafe extern "C" fn bind_sampler_states<B: Backend>(
    pipe: *mut pipe_context,
    shader: mesa_shader_stage,
    start: c_uint,
    count: c_uint,
    samplers: *mut *mut c_void,
) {
    // SAFETY: Mesa passes this driver's context and `count` samplers, or null.
    let context = unsafe { Context::<B>::of(pipe) };
    let Some(stage) = context::stage(shader) else {
        return;
    };
    let slots = &mut context.stages.get_mut(stage).samplers;
    for i in 0..count as usize {
        slots[start as usize + i] = if samplers.is_null() {
            std::ptr::null()
        } else {
            // SAFETY: as above.
            unsafe { *samplers.add(i) }.cast()
        };
    }
}

unsafe extern "C" fn create_vertex_elements_state<B: Backend>(
    pipe: *mut pipe_context,
    count: c_uint,
    elements: *const pipe_vertex_element,
) -> *mut c_void {
    // SAFETY: Mesa passes this driver's context and `count` elements.
    let (context, elements) = unsafe {
        (
            Context::<B>::of(pipe),
            std::slice::from_raw_parts(elements, count as usize),
        )
    };
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    let mut streams = 0;
    let mut pulled = Vec::new();
    let mut component_bytes = Vec::new();
    let backend = context.screen().backend();
    for (location, element) in elements.iter().enumerate() {
        if let Some(path) = pulled_because(&*backend, element) {
            pulled.push((location as u32, path));
        }
        let format = pipe_format::from(element.src_format);
        component_bytes.push((location as u32, resource::component_bytes(format)));
        let stream = element.vertex_buffer_index();
        let Some(format) = crate::format::vertex_format(pipe_format::from(element.src_format))
        else {
            refuse(format_args!("a vertex format the screen does not report"));
        };
        desc.streams[usize::from(stream)] = Some(VertexStream {
            stride: element.src_stride,
            step: match element.instance_divisor {
                0 => StepMode::Vertex,
                n => StepMode::Instance(n),
            },
        });
        desc.attributes[location] = Some(VertexAttribute {
            location: location as u32,
            stream,
            offset: u32::from(element.src_offset),
            format,
        });
        streams = streams.max(u32::from(stream) + 1);
    }
    object(Elements::<B> {
        desc,
        streams,
        pulled,
        blocks: RefCell::new(HashMap::new()),
        component_bytes,
    })
}

unsafe extern "C" fn bind_vertex_elements_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *mut c_void,
) {
    // SAFETY: Mesa passes this driver's context.
    unsafe { Context::<B>::of(pipe) }.bound.elements = state.cast();
}

unsafe extern "C" fn delete_vertex_elements_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *mut c_void,
) {
    // SAFETY: made by create_vertex_elements_state, deleted once, on this driver's context.
    let (context, state) = unsafe { (Context::<B>::of(pipe), drop_object::<Elements<B>>(state)) };
    for (_, block) in state.blocks.into_inner() {
        context.screen().retire(Object::VertexLayout(block));
    }
}

/// Takes the NIR st/mesa hands over (or Mesa's own converter makes of a TGSI program, which
/// Gallium's helpers hand over) in the shape every variant starts from.
fn create_shader(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
    stage: Stage,
) -> *mut c_void {
    // SAFETY: Mesa passes this driver's context and a shader state; its NIR is the driver's from
    // now on.
    let (state, screen) = unsafe { (&*state, (*pipe).screen) };
    let raw: *mut c_void = match state.type_ {
        // SAFETY: a NIR state's union holds its shader.
        PIPE_SHADER_IR_NIR => unsafe { state.ir.nir }.cast(),
        // SAFETY: a TGSI state's tokens; the converter reads the screen's NIR options.
        PIPE_SHADER_IR_TGSI => unsafe { tgsi_to_nir(state.tokens.cast(), screen, false) }.cast(),
        _ => refuse(format_args!("a program neither in NIR nor in TGSI")),
    };
    if raw.is_null() {
        refuse(format_args!("a TGSI program Mesa's converter did not take"));
    }
    // SAFETY: as above.
    let mut nir = unsafe { Nir::from_raw(raw) };
    nir.uniform_blocks_as_words();
    nir.split_resource_arrays();
    let mut streamed = Ok(None);
    match stage {
        Stage::Vertex => {
            nir.number_vertex_elements();
            streamed = match nir.stream_outputs::<STREAM_BUFFERS>() {
                Some((outputs, _)) if outputs.is_empty() => Ok(None),
                Some((outputs, strides)) => Ok(Some(stream::declaration(&outputs, strides))),
                None => Err("a streamed value that is neither at a location nor the position"),
            };
        }
        Stage::Fragment => nir.fragcolor_to_buffers(PIPE_MAX_COLOR_BUFS),
    }
    object(Shader {
        stage,
        nir,
        streamed,
    })
}

unsafe extern "C" fn create_vs_state(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
) -> *mut c_void {
    create_shader(pipe, state, Stage::Vertex)
}

unsafe extern "C" fn create_fs_state(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
) -> *mut c_void {
    create_shader(pipe, state, Stage::Fragment)
}

unsafe extern "C" fn bind_vs_state<B: Backend>(pipe: *mut pipe_context, state: *mut c_void) {
    // SAFETY: Mesa passes this driver's context.
    unsafe { Context::<B>::of(pipe) }
        .stages
        .get_mut(Stage::Vertex)
        .shader = state.cast();
}

unsafe extern "C" fn bind_fs_state<B: Backend>(pipe: *mut pipe_context, state: *mut c_void) {
    // SAFETY: Mesa passes this driver's context.
    unsafe { Context::<B>::of(pipe) }
        .stages
        .get_mut(Stage::Fragment)
        .shader = state.cast();
}

unsafe extern "C" fn delete_shader_state<B: Backend>(pipe: *mut pipe_context, state: *mut c_void) {
    // SAFETY: Mesa passes this driver's context and a shader it made, deleted once.
    let context = unsafe { Context::<B>::of(pipe) };
    let screen = context.screen();
    context.programs.forget(screen, state.cast_const().cast());
    // SAFETY: as above.
    drop(unsafe { drop_object::<Shader>(state) });
}

unsafe extern "C" fn set_blend_color<B: Backend>(
    pipe: *mut pipe_context,
    color: *const pipe_blend_color,
) {
    // SAFETY: Mesa passes this driver's context and a colour.
    let (context, color) = unsafe { (Context::<B>::of(pipe), &*color) };
    context.bound.blend_color = color.color.map(f32::to_bits);
}

unsafe extern "C" fn set_stencil_ref<B: Backend>(pipe: *mut pipe_context, value: pipe_stencil_ref) {
    // SAFETY: Mesa passes this driver's context.
    unsafe { Context::<B>::of(pipe) }.bound.stencil_ref = value.ref_value;
}

unsafe extern "C" fn set_sample_mask<B: Backend>(pipe: *mut pipe_context, mask: c_uint) {
    // SAFETY: Mesa passes this driver's context.
    unsafe { Context::<B>::of(pipe) }.bound.sample_mask = mask;
}

/// User clip planes are lowered into the programs by st/mesa (the screen reports none).
unsafe extern "C" fn set_clip_state(_: *mut pipe_context, _: *const pipe_clip_state) {}

unsafe extern "C" fn set_constant_buffer<B: Backend>(
    pipe: *mut pipe_context,
    shader: mesa_shader_stage,
    index: c_uint,
    buffer: *const pipe_constant_buffer,
) {
    // SAFETY: Mesa passes this driver's context and a buffer or null.
    let context = unsafe { Context::<B>::of(pipe) };
    let Some(stage) = context::stage(shader) else {
        return;
    };
    let Some(slot) = context
        .stages
        .get_mut(stage)
        .constants
        .get_mut(index as usize)
    else {
        // Unbinding a slot past those the screen reports leaves nothing to drop.
        if buffer.is_null() {
            return;
        }
        refuse(format_args!(
            "a uniform buffer past those the screen reports"
        ));
    };
    let mut held = std::mem::replace(&mut slot.resource, std::ptr::null_mut());
    slot.bytes.clear();
    if !buffer.is_null() {
        // SAFETY: as above.
        let buffer = unsafe { &*buffer };
        slot.offset = buffer.buffer_offset;
        slot.size = buffer.buffer_size;
        if buffer.user_buffer.is_null() {
            // SAFETY: the buffer is a live resource; the slot takes its own reference.
            unsafe { resource::reference(&mut slot.resource, buffer.buffer) };
        } else {
            // SAFETY: the frontend's bytes live for the call; the driver keeps a copy.
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    buffer
                        .user_buffer
                        .cast::<u8>()
                        .add(buffer.buffer_offset as usize),
                    buffer.buffer_size as usize,
                )
            };
            slot.bytes.extend_from_slice(bytes);
            slot.offset = 0;
        }
    }
    // SAFETY: the reference the slot held goes.
    unsafe { resource::reference(&mut held, std::ptr::null_mut()) };
}

unsafe extern "C" fn set_scissor_states<B: Backend>(
    pipe: *mut pipe_context,
    start: c_uint,
    count: c_uint,
    scissors: *const pipe_scissor_state,
) {
    if start == 0 && count > 0 {
        // SAFETY: Mesa passes this driver's context and `count` scissors.
        unsafe { Context::<B>::of(pipe).bound.scissor = *scissors };
    }
}

unsafe extern "C" fn set_viewport_states<B: Backend>(
    pipe: *mut pipe_context,
    start: c_uint,
    count: c_uint,
    viewports: *const pipe_viewport_state,
) {
    if start == 0 && count > 0 {
        // SAFETY: Mesa passes this driver's context and `count` viewports.
        unsafe { Context::<B>::of(pipe).bound.viewport = *viewports };
    }
}

unsafe extern "C" fn set_sampler_views<B: Backend>(
    pipe: *mut pipe_context,
    shader: mesa_shader_stage,
    start: c_uint,
    count: c_uint,
    trailing: c_uint,
    views: *mut *mut pipe_sampler_view,
) {
    // SAFETY: Mesa passes this driver's context and `count` views, or null.
    let context = unsafe { Context::<B>::of(pipe) };
    let Some(stage) = context::stage(shader) else {
        return;
    };
    let slots = &mut context.stages.get_mut(stage).views;
    for i in 0..(count + trailing) as usize {
        let view = if views.is_null() || i >= count as usize {
            std::ptr::null_mut()
        } else {
            // SAFETY: as above.
            unsafe { *views.add(i) }
        };
        // SAFETY: the views are live; the slot takes its own reference.
        unsafe { crate::view::reference(&mut slots[start as usize + i], view) };
    }
}

unsafe extern "C" fn set_vertex_buffers<B: Backend>(
    pipe: *mut pipe_context,
    count: c_uint,
    buffers: *const pipe_vertex_buffer,
) {
    // SAFETY: Mesa passes this driver's context and `count` buffers.
    let (context, buffers) = unsafe {
        (
            Context::<B>::of(pipe),
            std::slice::from_raw_parts(buffers, count as usize),
        )
    };
    let mut old = std::mem::take(&mut context.bound.vertex_buffers).into_iter();
    for buffer in buffers {
        if buffer.is_user_buffer {
            refuse(format_args!(
                "a vertex stream in client memory (the screen reports none)"
            ));
        }
        // SAFETY: not a user buffer, so a resource.
        let resource = unsafe { buffer.buffer.resource };
        // Each slot holds its own reference, kept where the slot is bound to the same resource
        // again, as Gallium's helpers do.
        let mut held = old.next().map_or(std::ptr::null_mut(), |p| p.resource);
        // SAFETY: both are null or live resources; the slot's reference moves to the new one.
        unsafe { resource::reference(&mut held, resource) };
        context.bound.vertex_buffers.push(VertexBuffer {
            resource,
            offset: buffer.buffer_offset,
        });
    }
    for previous in old {
        // SAFETY: each slot held one reference.
        unsafe { resource::release(previous.resource) };
    }
}

/// Fills the context's state entry points.
pub fn fill<B: Backend>(c: &mut pipe_context) {
    c.create_blend_state = Some(create_blend_state);
    c.bind_blend_state = Some(bind_blend_state::<B>);
    c.delete_blend_state = Some(delete_blend_state);
    c.create_depth_stencil_alpha_state = Some(create_depth_stencil_alpha_state::<B>);
    c.bind_depth_stencil_alpha_state = Some(bind_depth_stencil_alpha_state::<B>);
    c.delete_depth_stencil_alpha_state = Some(delete_depth_stencil_alpha_state::<B>);
    c.create_rasterizer_state = Some(create_rasterizer_state::<B>);
    c.bind_rasterizer_state = Some(bind_rasterizer_state::<B>);
    c.delete_rasterizer_state = Some(delete_rasterizer_state::<B>);
    c.create_sampler_state = Some(create_sampler_state::<B>);
    c.bind_sampler_states = Some(bind_sampler_states::<B>);
    c.delete_sampler_state = Some(delete_sampler_state::<B>);
    c.create_vertex_elements_state = Some(create_vertex_elements_state::<B>);
    c.bind_vertex_elements_state = Some(bind_vertex_elements_state::<B>);
    c.delete_vertex_elements_state = Some(delete_vertex_elements_state::<B>);
    c.create_vs_state = Some(create_vs_state);
    c.bind_vs_state = Some(bind_vs_state::<B>);
    c.delete_vs_state = Some(delete_shader_state::<B>);
    c.create_fs_state = Some(create_fs_state);
    c.bind_fs_state = Some(bind_fs_state::<B>);
    c.delete_fs_state = Some(delete_shader_state::<B>);
    c.set_blend_color = Some(set_blend_color::<B>);
    c.set_stencil_ref = Some(set_stencil_ref::<B>);
    c.set_sample_mask = Some(set_sample_mask::<B>);
    c.set_clip_state = Some(set_clip_state);
    c.set_constant_buffer = Some(set_constant_buffer::<B>);
    c.set_scissor_states = Some(set_scissor_states::<B>);
    c.set_viewport_states = Some(set_viewport_states::<B>);
    c.set_sampler_views = Some(set_sampler_views::<B>);
    c.set_vertex_buffers = Some(set_vertex_buffers::<B>);
}
