//! The GL values the back end last set, and the conversion of each block into them. A block is applied
//! only when its handle differs from the last one applied, and then only the values that differ from
//! what GL holds are set.

use glow::HasContext;
use wcontainers_gpu::{
    BlendDesc, BlendFactor, BlendOp, CompareFn, CullMode, DepthStencilDesc, FrontFace, RasterDesc,
    StencilOp, Topology, VertexFormat,
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

pub fn blend(gl: &glow::Context, v: &mut GlValues, d: &BlendDesc) {
    let t = d.targets[0];
    set_if!(v.blend, t.blend.is_some(), |on| enable(gl, glow::BLEND, on));
    if let Some((color, alpha)) = t.blend {
        set_if!(
            v.blend_equation,
            [op(color.op), op(alpha.op)],
            |e: [u32; 2]| gl.blend_equation_separate(e[0], e[1])
        );
        let funcs = [
            factor(color.src),
            factor(color.dst),
            factor(alpha.src),
            factor(alpha.dst),
        ];
        set_if!(v.blend_func, funcs, |f: [u32; 4]| gl
            .blend_func_separate(f[0], f[1], f[2], f[3]));
    }
    let w = t.writes;
    use wcontainers_gpu::ColorWrites as C;
    let mask = [
        w.contains(C::R),
        w.contains(C::G),
        w.contains(C::B),
        w.contains(C::A),
    ];
    set_if!(v.color_mask, mask, |m: [bool; 4]| gl
        .color_mask(m[0], m[1], m[2], m[3]));
    set_if!(v.alpha_to_coverage, d.alpha_to_coverage, |on| enable(
        gl,
        glow::SAMPLE_ALPHA_TO_COVERAGE,
        on
    ));
}

/// The depth and stencil block, with the draw's stencil reference (one for both faces, as WebGL
/// requires).
pub fn depth_stencil(gl: &glow::Context, v: &mut GlValues, d: &DepthStencilDesc, reference: u8) {
    set_if!(v.depth_test, d.depth_compare.is_some(), |on| enable(
        gl,
        glow::DEPTH_TEST,
        on
    ));
    if let Some(f) = d.depth_compare {
        set_if!(v.depth_func, compare(f), |f| gl.depth_func(f));
    }
    // GL writes no depth with the test off, as an absent test means in Direct3D.
    set_if!(v.depth_mask, d.depth_write, |on| gl.depth_mask(on));
    set_if!(v.stencil_test, d.stencil.is_some(), |on| enable(
        gl,
        glow::STENCIL_TEST,
        on
    ));
    if let Some(faces) = d.stencil {
        for (i, (face, gl_face)) in [(faces.0, glow::FRONT), (faces.1, glow::BACK)]
            .into_iter()
            .enumerate()
        {
            let func = (compare(face.compare), reference, d.stencil_read_mask[0]);
            set_if!(v.stencil_func[i], func, |f: (u32, u8, u8)| gl
                .stencil_func_separate(
                    gl_face,
                    f.0,
                    i32::from(f.1),
                    u32::from(f.2)
                ));
            let ops = [
                stencil(face.fail),
                stencil(face.depth_fail),
                stencil(face.pass),
            ];
            set_if!(v.stencil_op[i], ops, |o: [u32; 3]| gl
                .stencil_op_separate(gl_face, o[0], o[1], o[2]));
        }
        set_if!(v.stencil_write_mask, d.stencil_write_mask[0], |m: u8| gl
            .stencil_mask(u32::from(m)));
    }
}

/// The raster block. `flipped` says the program negates y (no EXT_clip_control), which reverses every
/// triangle's winding in window space.
pub fn raster(
    gl: &glow::Context,
    v: &mut GlValues,
    d: &RasterDesc,
    flipped: bool,
    clip_control: impl Fn(u32),
) {
    let cull = match d.cull {
        CullMode::None => None,
        CullMode::Front => Some(glow::FRONT),
        CullMode::Back => Some(glow::BACK),
    };
    if v.cull != cull {
        match cull {
            Some(face) => {
                if v.cull.is_none() {
                    enable(gl, glow::CULL_FACE, true);
                }
                // SAFETY: a plain GL state call.
                unsafe { gl.cull_face(face) };
            }
            None => enable(gl, glow::CULL_FACE, false),
        }
        v.cull = cull;
    }
    let ccw = (d.front == FrontFace::CounterClockwise) != flipped;
    set_if!(v.front_face, if ccw { glow::CCW } else { glow::CW }, |f| gl
        .front_face(f));
    let bias = f32::from_bits(d.depth_bias);
    let slope = f32::from_bits(d.depth_bias_slope);
    let offset = (bias != 0.0 || slope != 0.0).then_some((slope, bias));
    if v.polygon_offset != offset {
        enable(gl, glow::POLYGON_OFFSET_FILL, offset.is_some());
        if let Some((factor, units)) = offset {
            // SAFETY: a plain GL state call.
            unsafe { gl.polygon_offset(factor, units) };
        }
        v.polygon_offset = offset;
    }
    set_if!(v.scissor_test, d.scissor, |on| enable(
        gl,
        glow::SCISSOR_TEST,
        on
    ));
    if !flipped {
        let depth = if d.depth_minus_one {
            crate::platform::NEGATIVE_ONE_TO_ONE
        } else {
            crate::platform::ZERO_TO_ONE
        };
        set_if!(v.clip_depth, depth, clip_control);
    }
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
pub fn attribute(f: VertexFormat) -> Option<(i32, u32, bool, bool)> {
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
