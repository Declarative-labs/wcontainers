//! The GL values the back end last set, and each block as the GL values it stands for, converted once
//! when the block is made. A block is applied only when its handle differs from the last one applied,
//! and then only the values that differ from what GL holds are set.

use glow::HasContext;
use wcontainers_gpu::{
    BlendDesc, BlendFactor, BlendOp, CompareFn, CullMode, DepthStencilDesc, FrontFace,
    ProvokingVertex, RasterDesc, StencilFace, StencilOp, StepMode, Topology, VertexFormat,
    VertexLayoutDesc,
};

/// What GL holds now, for the values blocks and draws set.
#[derive(Clone, Debug, PartialEq)]
pub struct GlValues {
    pub blend: bool,
    pub blend_equation: [u32; 2],
    pub blend_func: [u32; 4],
    pub color_mask: [bool; 4],
    pub alpha_to_coverage: bool,
    pub depth_test: bool,
    pub depth_func: u32,
    pub depth_mask: bool,
    pub stencil_test: bool,
    /// Per face (front, back): compare, reference, read mask.
    pub stencil_func: [(u32, u8, u8); 2],
    /// Per face: fail, depth fail, pass.
    pub stencil_op: [[u32; 3]; 2],
    pub stencil_write_mask: u8,
    pub cull: Option<u32>,
    pub front_face: u32,
    pub polygon_offset: Option<(f32, f32)>,
    pub scissor_test: bool,
    pub viewport: [i32; 4],
    pub depth_range: [f32; 2],
    pub scissor: [i32; 4],
    pub blend_color: [f32; 4],
    pub clip_depth: u32,
    pub provoking: u32,
}

impl GlValues {
    /// GL's initial values, with the viewport and scissor unknown so the first draw sets them.
    pub fn initial(clip_depth: u32) -> GlValues {
        GlValues {
            blend: false,
            blend_equation: [glow::FUNC_ADD; 2],
            blend_func: [glow::ONE, glow::ZERO, glow::ONE, glow::ZERO],
            color_mask: [true; 4],
            alpha_to_coverage: false,
            depth_test: false,
            depth_func: glow::LESS,
            depth_mask: true,
            stencil_test: false,
            stencil_func: [(glow::ALWAYS, 0, 0xff); 2],
            stencil_op: [[glow::KEEP; 3]; 2],
            stencil_write_mask: 0xff,
            cull: None,
            front_face: glow::CCW,
            polygon_offset: None,
            scissor_test: false,
            viewport: [i32::MIN; 4],
            depth_range: [0.0, 1.0],
            scissor: [i32::MIN; 4],
            blend_color: [0.0; 4],
            clip_depth,
            provoking: crate::platform::LAST_VERTEX,
        }
    }
}

/// Sets `field` to `value` with `set` when they differ.
macro_rules! set_if {
    ($field:expr, $value:expr, $set:expr) => {{
        let value = $value;
        if $field != value {
            $field = value;
            #[allow(unused_unsafe)]
            // SAFETY: plain GL state calls on the current context.
            unsafe {
                $set(value)
            };
        }
    }};
}

fn enable(gl: &glow::Context, cap: u32, on: bool) {
    // SAFETY: plain GL state calls on the current context.
    unsafe { if on { gl.enable(cap) } else { gl.disable(cap) } }
}

/// A blend block as the GL values it stands for.
#[derive(Debug)]
pub struct GlBlend {
    on: bool,
    equation: [u32; 2],
    func: [u32; 4],
    color_mask: [bool; 4],
    alpha_to_coverage: bool,
}

impl GlBlend {
    /// `create_blend` has refused what WebGL 2 lacks; every target blends as target 0.
    pub fn new(d: &BlendDesc) -> GlBlend {
        let t = d.targets[0];
        let (equation, func) = match t.blend {
            Some((c, a)) => (
                [op(c.op), op(a.op)],
                [factor(c.src), factor(c.dst), factor(a.src), factor(a.dst)],
            ),
            None => (
                [glow::FUNC_ADD; 2],
                [glow::ONE, glow::ZERO, glow::ONE, glow::ZERO],
            ),
        };
        use wcontainers_gpu::ColorWrites as C;
        let w = t.writes;
        GlBlend {
            on: t.blend.is_some(),
            equation,
            func,
            color_mask: [
                w.contains(C::R),
                w.contains(C::G),
                w.contains(C::B),
                w.contains(C::A),
            ],
            alpha_to_coverage: d.alpha_to_coverage,
        }
    }
}

pub fn blend(gl: &glow::Context, v: &mut GlValues, b: &GlBlend) {
    set_if!(v.blend, b.on, |on| enable(gl, glow::BLEND, on));
    if b.on {
        set_if!(v.blend_equation, b.equation, |e: [u32; 2]| gl
            .blend_equation_separate(e[0], e[1]));
        set_if!(v.blend_func, b.func, |f: [u32; 4]| gl
            .blend_func_separate(f[0], f[1], f[2], f[3]));
    }
    set_if!(v.color_mask, b.color_mask, |m: [bool; 4]| gl
        .color_mask(m[0], m[1], m[2], m[3]));
    set_if!(v.alpha_to_coverage, b.alpha_to_coverage, |on| enable(
        gl,
        glow::SAMPLE_ALPHA_TO_COVERAGE,
        on
    ));
}

/// A depth and stencil block as the GL values it stands for.
#[derive(Debug)]
pub struct GlDepthStencil {
    depth_func: Option<u32>,
    depth_mask: bool,
    stencil: Option<GlStencil>,
}

/// Both faces' compare and operations (fail, depth fail, pass), with one read and one write mask, as
/// WebGL requires.
#[derive(Debug)]
struct GlStencil {
    compare: [u32; 2],
    ops: [[u32; 3]; 2],
    read_mask: u8,
    write_mask: u8,
}

impl GlDepthStencil {
    /// `create_depth_stencil` has refused per-face masks and depth bounds.
    pub fn new(d: &DepthStencilDesc) -> GlDepthStencil {
        let ops = |f: StencilFace| [stencil(f.fail), stencil(f.depth_fail), stencil(f.pass)];
        GlDepthStencil {
            depth_func: d.depth_compare.map(compare),
            depth_mask: d.depth_write,
            stencil: d.stencil.map(|(front, back)| GlStencil {
                compare: [compare(front.compare), compare(back.compare)],
                ops: [ops(front), ops(back)],
                read_mask: d.stencil_read_mask[0],
                write_mask: d.stencil_write_mask[0],
            }),
        }
    }
}

/// The depth and stencil block, with the draw's stencil reference (one for both faces, as WebGL
/// requires).
pub fn depth_stencil(gl: &glow::Context, v: &mut GlValues, d: &GlDepthStencil, reference: u8) {
    set_if!(v.depth_test, d.depth_func.is_some(), |on| enable(
        gl,
        glow::DEPTH_TEST,
        on
    ));
    if let Some(f) = d.depth_func {
        set_if!(v.depth_func, f, |f| gl.depth_func(f));
    }
    // GL writes no depth with the test off, as an absent test means in Direct3D.
    set_if!(v.depth_mask, d.depth_mask, |on| gl.depth_mask(on));
    set_if!(v.stencil_test, d.stencil.is_some(), |on| enable(
        gl,
        glow::STENCIL_TEST,
        on
    ));
    if let Some(s) = &d.stencil {
        for (i, gl_face) in [glow::FRONT, glow::BACK].into_iter().enumerate() {
            let func = (s.compare[i], reference, s.read_mask);
            set_if!(v.stencil_func[i], func, |f: (u32, u8, u8)| gl
                .stencil_func_separate(
                    gl_face,
                    f.0,
                    i32::from(f.1),
                    u32::from(f.2)
                ));
            set_if!(v.stencil_op[i], s.ops[i], |o: [u32; 3]| gl
                .stencil_op_separate(gl_face, o[0], o[1], o[2]));
        }
        set_if!(v.stencil_write_mask, s.write_mask, |m: u8| gl
            .stencil_mask(u32::from(m)));
    }
}

/// A raster block as the GL values it stands for.
#[derive(Debug)]
pub struct GlRaster {
    cull: Option<u32>,
    front_face: u32,
    /// Slope factor and constant units, when either is not zero.
    polygon_offset: Option<(f32, f32)>,
    scissor_test: bool,
    /// The clip depth convention, where EXT_clip_control sets it.
    clip_depth: Option<u32>,
    provoking: u32,
}

impl GlRaster {
    /// `flipped` says programs negate y (no EXT_clip_control), which reverses every triangle's winding
    /// in window space. `create_raster` has refused what the context lacks.
    pub fn new(d: &RasterDesc, flipped: bool) -> GlRaster {
        let ccw = (d.front == FrontFace::CounterClockwise) != flipped;
        let bias = f32::from_bits(d.depth_bias);
        let slope = f32::from_bits(d.depth_bias_slope);
        GlRaster {
            cull: match d.cull {
                CullMode::None => None,
                CullMode::Front => Some(glow::FRONT),
                CullMode::Back => Some(glow::BACK),
            },
            front_face: if ccw { glow::CCW } else { glow::CW },
            polygon_offset: (bias != 0.0 || slope != 0.0).then_some((slope, bias)),
            scissor_test: d.scissor,
            clip_depth: (!flipped).then_some(if d.depth_minus_one {
                crate::platform::NEGATIVE_ONE_TO_ONE
            } else {
                crate::platform::ZERO_TO_ONE
            }),
            provoking: match d.provoking {
                ProvokingVertex::First => crate::platform::FIRST_VERTEX,
                ProvokingVertex::Last => crate::platform::LAST_VERTEX,
            },
        }
    }
}

pub fn raster(
    gl: &glow::Context,
    v: &mut GlValues,
    r: &GlRaster,
    platform: &crate::platform::Platform,
) {
    if v.cull != r.cull {
        match r.cull {
            Some(face) => {
                if v.cull.is_none() {
                    enable(gl, glow::CULL_FACE, true);
                }
                // SAFETY: a plain GL state call.
                unsafe { gl.cull_face(face) };
            }
            None => enable(gl, glow::CULL_FACE, false),
        }
        v.cull = r.cull;
    }
    set_if!(v.front_face, r.front_face, |f| gl.front_face(f));
    if v.polygon_offset != r.polygon_offset {
        enable(gl, glow::POLYGON_OFFSET_FILL, r.polygon_offset.is_some());
        if let Some((factor, units)) = r.polygon_offset {
            // SAFETY: a plain GL state call.
            unsafe { gl.polygon_offset(factor, units) };
        }
        v.polygon_offset = r.polygon_offset;
    }
    set_if!(v.scissor_test, r.scissor_test, |on| enable(
        gl,
        glow::SCISSOR_TEST,
        on
    ));
    if let Some(depth) = r.clip_depth {
        set_if!(v.clip_depth, depth, |depth| platform.clip_control(
            gl,
            crate::platform::UPPER_LEFT,
            depth
        ));
    }
    set_if!(v.provoking, r.provoking, |mode| platform
        .provoking_vertex(gl, mode));
}

/// A vertex layout block as the attribute pointers it stands for.
#[derive(Debug)]
pub struct GlLayout {
    pub attributes: Vec<GlAttribute>,
}

/// One attribute's pointer, less its buffer and the draw's offsets.
#[derive(Clone, Copy, Debug)]
pub struct GlAttribute {
    pub location: u32,
    pub stream: usize,
    pub offset: u32,
    pub stride: u32,
    /// Whether the draw's first instance (else its base vertex) moves the stream.
    pub per_instance: bool,
    pub components: i32,
    pub ty: u32,
    pub normalized: bool,
    pub integer: bool,
    pub divisor: u32,
}

impl GlLayout {
    /// None when an attribute names a missing stream or has a format GL ES cannot read as stored.
    pub fn new(d: &VertexLayoutDesc) -> Option<GlLayout> {
        let mut attributes = Vec::new();
        for a in d.attributes.iter().flatten() {
            let stream = d.streams.get(usize::from(a.stream)).copied().flatten()?;
            let (components, ty, normalized, integer) = attribute(a.format)?;
            // Stride 0 (one element for every vertex and instance) and step rate 0 never advance:
            // GL's stride 0 means tightly packed and its divisor 0 per vertex, so both take a divisor
            // no instance count reaches.
            let divisor = match stream.step {
                _ if stream.stride == 0 => crate::NEVER_ADVANCES,
                StepMode::Vertex => 0,
                StepMode::Instance(0) => crate::NEVER_ADVANCES,
                StepMode::Instance(n) => n,
            };
            attributes.push(GlAttribute {
                location: a.location,
                stream: usize::from(a.stream),
                offset: a.offset,
                stride: stream.stride,
                per_instance: matches!(stream.step, StepMode::Instance(_)),
                components,
                ty,
                normalized,
                integer,
                divisor,
            });
        }
        Some(GlLayout { attributes })
    }
}

/// The state of a draw of the back end's own into the viewport `rect`: no blending, tests or culling.
/// The caller opens the write masks and the scissor.
pub fn plain(gl: &glow::Context, v: &mut GlValues, rect: [i32; 4]) {
    set_if!(v.blend, false, |on| enable(gl, glow::BLEND, on));
    set_if!(v.alpha_to_coverage, false, |on| enable(
        gl,
        glow::SAMPLE_ALPHA_TO_COVERAGE,
        on
    ));
    set_if!(v.depth_test, false, |on| enable(gl, glow::DEPTH_TEST, on));
    set_if!(v.stencil_test, false, |on| enable(
        gl,
        glow::STENCIL_TEST,
        on
    ));
    if v.cull.is_some() {
        enable(gl, glow::CULL_FACE, false);
        v.cull = None;
    }
    set_if!(v.viewport, rect, |r: [i32; 4]| gl
        .viewport(r[0], r[1], r[2], r[3]));
}

pub fn mode(t: Topology) -> u32 {
    match t {
        Topology::PointList => glow::POINTS,
        Topology::LineList => glow::LINES,
        Topology::LineStrip => glow::LINE_STRIP,
        Topology::TriangleList => glow::TRIANGLES,
        Topology::TriangleStrip => glow::TRIANGLE_STRIP,
        Topology::TriangleFan => glow::TRIANGLE_FAN,
        Topology::LineLoop => glow::LINE_LOOP,
    }
}

pub fn compare(f: CompareFn) -> u32 {
    match f {
        CompareFn::Never => glow::NEVER,
        CompareFn::Less => glow::LESS,
        CompareFn::Equal => glow::EQUAL,
        CompareFn::LessEqual => glow::LEQUAL,
        CompareFn::Greater => glow::GREATER,
        CompareFn::NotEqual => glow::NOTEQUAL,
        CompareFn::GreaterEqual => glow::GEQUAL,
        CompareFn::Always => glow::ALWAYS,
    }
}

fn stencil(op: StencilOp) -> u32 {
    match op {
        StencilOp::Keep => glow::KEEP,
        StencilOp::Zero => glow::ZERO,
        StencilOp::Replace => glow::REPLACE,
        StencilOp::IncrementClamp => glow::INCR,
        StencilOp::DecrementClamp => glow::DECR,
        StencilOp::Invert => glow::INVERT,
        StencilOp::IncrementWrap => glow::INCR_WRAP,
        StencilOp::DecrementWrap => glow::DECR_WRAP,
    }
}

fn op(o: BlendOp) -> u32 {
    match o {
        BlendOp::Add => glow::FUNC_ADD,
        BlendOp::Subtract => glow::FUNC_SUBTRACT,
        BlendOp::ReverseSubtract => glow::FUNC_REVERSE_SUBTRACT,
        BlendOp::Min => glow::MIN,
        BlendOp::Max => glow::MAX,
    }
}

/// Whether a factor reads the fragment's second output (dual-source blending).
pub fn second_source(f: BlendFactor) -> bool {
    matches!(
        f,
        BlendFactor::Src1
            | BlendFactor::OneMinusSrc1
            | BlendFactor::Src1Alpha
            | BlendFactor::OneMinusSrc1Alpha
    )
}

/// GL's blend factor. `create_blend` refuses the second-source factors, so they never reach here.
pub fn factor(f: BlendFactor) -> u32 {
    match f {
        BlendFactor::Zero => glow::ZERO,
        BlendFactor::One => glow::ONE,
        BlendFactor::Src => glow::SRC_COLOR,
        BlendFactor::OneMinusSrc => glow::ONE_MINUS_SRC_COLOR,
        BlendFactor::SrcAlpha => glow::SRC_ALPHA,
        BlendFactor::OneMinusSrcAlpha => glow::ONE_MINUS_SRC_ALPHA,
        BlendFactor::Dst => glow::DST_COLOR,
        BlendFactor::OneMinusDst => glow::ONE_MINUS_DST_COLOR,
        BlendFactor::DstAlpha => glow::DST_ALPHA,
        BlendFactor::OneMinusDstAlpha => glow::ONE_MINUS_DST_ALPHA,
        BlendFactor::SrcAlphaSaturated => glow::SRC_ALPHA_SATURATE,
        BlendFactor::Constant => glow::CONSTANT_COLOR,
        BlendFactor::OneMinusConstant => glow::ONE_MINUS_CONSTANT_COLOR,
        BlendFactor::ConstantAlpha => glow::CONSTANT_ALPHA,
        BlendFactor::OneMinusConstantAlpha => glow::ONE_MINUS_CONSTANT_ALPHA,
        BlendFactor::Src1
        | BlendFactor::OneMinusSrc1
        | BlendFactor::Src1Alpha
        | BlendFactor::OneMinusSrc1Alpha => {
            unreachable!("create_blend refuses second-source factors")
        }
    }
}

/// How GL reads an attribute: components, type, normalised, and whether it is read as integers.
/// None for the formats GL ES 3.0 cannot read as stored.
fn attribute(f: VertexFormat) -> Option<(i32, u32, bool, bool)> {
    use VertexFormat as V;
    let (n, ty, norm, int) = match f {
        V::Uint8x1 | V::Uint8x2 | V::Uint8x3 | V::Uint8x4 => (0, glow::UNSIGNED_BYTE, false, true),
        V::Sint8x1 | V::Sint8x2 | V::Sint8x3 | V::Sint8x4 => (0, glow::BYTE, false, true),
        V::Unorm8x1 | V::Unorm8x2 | V::Unorm8x3 | V::Unorm8x4 => {
            (0, glow::UNSIGNED_BYTE, true, false)
        }
        V::Snorm8x1 | V::Snorm8x2 | V::Snorm8x3 | V::Snorm8x4 => (0, glow::BYTE, true, false),
        V::Uint16x1 | V::Uint16x2 | V::Uint16x3 | V::Uint16x4 => {
            (0, glow::UNSIGNED_SHORT, false, true)
        }
        V::Sint16x1 | V::Sint16x2 | V::Sint16x3 | V::Sint16x4 => (0, glow::SHORT, false, true),
        V::Unorm16x1 | V::Unorm16x2 | V::Unorm16x3 | V::Unorm16x4 => {
            (0, glow::UNSIGNED_SHORT, true, false)
        }
        V::Snorm16x1 | V::Snorm16x2 | V::Snorm16x3 | V::Snorm16x4 => (0, glow::SHORT, true, false),
        V::Float16x1 | V::Float16x2 | V::Float16x3 | V::Float16x4 => {
            (0, glow::HALF_FLOAT, false, false)
        }
        V::Float32x1 | V::Float32x2 | V::Float32x3 | V::Float32x4 => (0, glow::FLOAT, false, false),
        V::Uint32x1 | V::Uint32x2 | V::Uint32x3 | V::Uint32x4 => {
            (0, glow::UNSIGNED_INT, false, true)
        }
        V::Sint32x1 | V::Sint32x2 | V::Sint32x3 | V::Sint32x4 => (0, glow::INT, false, true),
        V::Unorm10_10_10_2 => (4, glow::UNSIGNED_INT_2_10_10_10_REV, true, false),
        V::Bgra8Unorm | V::Uint10_10_10_2 | V::Float11_11_10 => return None,
    };
    let components = if n != 0 {
        n
    } else {
        // The variants of each type run x1 to x4 in order.
        let base = match ty {
            glow::UNSIGNED_BYTE if int => V::Uint8x1,
            glow::BYTE if int => V::Sint8x1,
            glow::UNSIGNED_BYTE => V::Unorm8x1,
            glow::BYTE => V::Snorm8x1,
            glow::UNSIGNED_SHORT if int => V::Uint16x1,
            glow::SHORT if int => V::Sint16x1,
            glow::UNSIGNED_SHORT => V::Unorm16x1,
            glow::SHORT => V::Snorm16x1,
            glow::HALF_FLOAT => V::Float16x1,
            glow::FLOAT => V::Float32x1,
            glow::UNSIGNED_INT => V::Uint32x1,
            _ => V::Sint32x1,
        };
        f as i32 - base as i32 + 1
    };
    Some((components, ty, norm, int))
}
