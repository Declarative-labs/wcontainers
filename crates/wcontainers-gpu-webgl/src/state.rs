//! The GL values the back end last set, and each block as the GL values it stands for, converted once
//! when the block is made. A block is applied only when its handle differs from the last one applied,
//! and then only the values that differ from what GL holds are set.

use glow::HasContext;
use wcontainers_gpu::{
    BlendDesc, BlendFactor, BlendOp, CompareFn, CullMode, DepthStencilDesc, FillMode, FrontFace,
    MAX_COLOR_TARGETS, ProvokingVertex, RasterDesc, RenderTargetBlend, StencilFace, StencilOp,
    StepMode, Topology, VertexFormat, VertexLayoutDesc,
};

use crate::format;
use crate::platform::Platform;

/// What GL holds now, for the values blocks and draws set.
#[derive(Clone, Debug, PartialEq)]
pub struct GlValues {
    /// Per colour target: blending, its equation and factors, and the write mask.
    pub targets: [GlTarget; MAX_COLOR_TARGETS],
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
    /// Slope factor, constant units and clamp, when the offset is on.
    pub polygon_offset: Option<(f32, f32, f32)>,
    /// Whether the line offset is on (WEBGL_polygon_mode).
    pub line_offset: bool,
    pub fill_lines: bool,
    pub depth_clamp: bool,
    /// How many clip distances are enabled, from distance 0.
    pub clip_distances: u8,
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
            targets: [GlTarget::OFF; MAX_COLOR_TARGETS],
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
            line_offset: false,
            fill_lines: false,
            depth_clamp: false,
            clip_distances: 0,
            scissor_test: false,
            viewport: [i32::MIN; 4],
            depth_range: [0.0, 1.0],
            scissor: [i32::MIN; 4],
            blend_color: [0.0; 4],
            clip_depth,
            provoking: glow::LAST_VERTEX_CONVENTION,
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

/// One colour target's blending and write mask.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlTarget {
    pub on: bool,
    pub equation: [u32; 2],
    pub func: [u32; 4],
    pub mask: [bool; 4],
}

impl GlTarget {
    /// GL's initial values: no blending, every channel written.
    pub const OFF: GlTarget = GlTarget {
        on: false,
        equation: [glow::FUNC_ADD; 2],
        func: [glow::ONE, glow::ZERO, glow::ONE, glow::ZERO],
        mask: [true; 4],
    };

    fn new(t: &RenderTargetBlend) -> GlTarget {
        use wcontainers_gpu::ColorWrites as C;
        let w = t.writes;
        let mask = [
            w.contains(C::R),
            w.contains(C::G),
            w.contains(C::B),
            w.contains(C::A),
        ];
        match t.blend {
            Some((c, a)) => GlTarget {
                on: true,
                equation: [op(c.op), op(a.op)],
                func: [factor(c.src), factor(c.dst), factor(a.src), factor(a.dst)],
                mask,
            },
            None => GlTarget {
                mask,
                ..GlTarget::OFF
            },
        }
    }
}

/// A blend block as the GL values it stands for.
#[derive(Debug)]
pub struct GlBlend {
    targets: [GlTarget; MAX_COLOR_TARGETS],
    /// Every target as target 0, set by GL's calls for all targets at once.
    same: bool,
    alpha_to_coverage: bool,
    /// Whether a factor reads the fragment's second output.
    pub second_source: bool,
}

impl GlBlend {
    /// `create_blend` has refused what the context lacks.
    pub fn new(d: &BlendDesc) -> GlBlend {
        let targets = if d.same_for_all {
            [GlTarget::new(&d.targets[0]); MAX_COLOR_TARGETS]
        } else {
            d.targets.each_ref().map(GlTarget::new)
        };
        GlBlend {
            same: targets.iter().all(|t| *t == targets[0]),
            targets,
            alpha_to_coverage: d.alpha_to_coverage,
            second_source: d
                .targets
                .iter()
                .filter_map(|t| t.blend)
                .any(|(c, a)| [c.src, c.dst, a.src, a.dst].into_iter().any(second_source)),
        }
    }
}

/// Sets every target's `T` to `value` by one call for all targets, when any differs.
fn all_targets<T: PartialEq + Copy>(
    v: &mut GlValues,
    value: T,
    field: fn(&mut GlTarget) -> &mut T,
    set: impl FnOnce(T),
) {
    if v.targets.iter_mut().any(|t| *field(t) != value) {
        set(value);
        for t in &mut v.targets {
            *field(t) = value;
        }
    }
}

/// Sets every target to `t` by GL's calls for all targets, only where they differ.
pub fn uniform(gl: &glow::Context, v: &mut GlValues, t: &GlTarget) {
    all_targets(v, t.on, |x| &mut x.on, |on| enable(gl, glow::BLEND, on));
    if t.on {
        // SAFETY: a plain GL state call on the current context.
        all_targets(
            v,
            t.equation,
            |x| &mut x.equation,
            |e| unsafe { gl.blend_equation_separate(e[0], e[1]) },
        );
        // SAFETY: a plain GL state call on the current context.
        all_targets(
            v,
            t.func,
            |x| &mut x.func,
            |f| unsafe { gl.blend_func_separate(f[0], f[1], f[2], f[3]) },
        );
    }
    // SAFETY: a plain GL state call on the current context.
    all_targets(
        v,
        t.mask,
        |x| &mut x.mask,
        |m| unsafe { gl.color_mask(m[0], m[1], m[2], m[3]) },
    );
}

/// Applies `b`, leaving unwritten the channels of `unwritten`, [`format::CHANNELS`] bits a target
/// from the first, each from red.
pub fn blend(
    gl: &glow::Context,
    v: &mut GlValues,
    b: &GlBlend,
    platform: &Platform,
    unwritten: u32,
) {
    let masked;
    let (targets, same) = if unwritten == 0 {
        (&b.targets, b.same)
    } else {
        masked = core::array::from_fn::<_, MAX_COLOR_TARGETS, _>(|i| {
            let mut t = b.targets[i];
            for (c, on) in t.mask.iter_mut().enumerate() {
                if unwritten >> (i * format::CHANNELS + c) & 1 != 0 {
                    *on = false;
                }
            }
            t
        });
        (&masked, masked.iter().all(|t| *t == masked[0]))
    };
    if same {
        uniform(gl, v, &targets[0]);
    } else {
        // OES_draw_buffers_indexed, which `create_blend` or `begin_pass` asked for.
        for (i, (t, x)) in targets.iter().zip(&mut v.targets).enumerate() {
            let i = i as u32;
            set_if!(x.on, t.on, |on| platform.enable_i(glow::BLEND, i, on));
            if t.on {
                set_if!(x.equation, t.equation, |e| platform.blend_equation_i(i, e));
                set_if!(x.func, t.func, |f| platform.blend_func_i(i, f));
            }
            set_if!(x.mask, t.mask, |m| platform.color_mask_i(i, m));
        }
    }
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
    /// Slope factor, constant units and clamp, when the factor or the units are not zero.
    polygon_offset: Option<(f32, f32, f32)>,
    fill_lines: bool,
    depth_clamp: bool,
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
        let clamp = f32::from_bits(d.depth_bias_clamp);
        GlRaster {
            cull: match d.cull {
                CullMode::None => None,
                CullMode::Front => Some(glow::FRONT),
                CullMode::Back => Some(glow::BACK),
            },
            front_face: if ccw { glow::CCW } else { glow::CW },
            polygon_offset: (bias != 0.0 || slope != 0.0).then_some((slope, bias, clamp)),
            fill_lines: d.fill == FillMode::Line,
            depth_clamp: !d.depth_clip,
            scissor_test: d.scissor,
            clip_depth: (!flipped).then_some(if d.depth_minus_one {
                glow::NEGATIVE_ONE_TO_ONE
            } else {
                glow::ZERO_TO_ONE
            }),
            provoking: match d.provoking {
                ProvokingVertex::First => glow::FIRST_VERTEX_CONVENTION,
                ProvokingVertex::Last => glow::LAST_VERTEX_CONVENTION,
            },
        }
    }

    /// The vertex whose flat values the block's primitives take.
    pub fn provoking(&self) -> ProvokingVertex {
        if self.provoking == glow::FIRST_VERTEX_CONVENTION {
            ProvokingVertex::First
        } else {
            ProvokingVertex::Last
        }
    }
}

pub fn raster(gl: &glow::Context, v: &mut GlValues, r: &GlRaster, platform: &Platform) {
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
        if v.polygon_offset.is_none() || r.polygon_offset.is_none() {
            enable(gl, glow::POLYGON_OFFSET_FILL, r.polygon_offset.is_some());
        }
        match r.polygon_offset {
            // EXT_polygon_offset_clamp, which `create_raster` asked for.
            Some((factor, units, clamp)) if clamp != 0.0 => {
                platform.polygon_offset_clamp(factor, units, clamp)
            }
            // SAFETY: a plain GL state call.
            Some((factor, units, _)) => unsafe { gl.polygon_offset(factor, units) },
            None => {}
        }
        v.polygon_offset = r.polygon_offset;
    }
    // WEBGL_polygon_mode, which `create_raster` asked for when a block draws lines; edges take the
    // offset as filled polygons do in Direct3D.
    set_if!(v.fill_lines, r.fill_lines, |lines| platform
        .polygon_mode(if lines { glow::LINE } else { glow::FILL }));
    let line_offset = r.fill_lines && r.polygon_offset.is_some();
    set_if!(v.line_offset, line_offset, |on| enable(
        gl,
        glow::POLYGON_OFFSET_LINE,
        on
    ));
    // EXT_depth_clamp, which `create_raster` asked for when a block clamps.
    set_if!(v.depth_clamp, r.depth_clamp, |on| enable(
        gl,
        glow::DEPTH_CLAMP,
        on
    ));
    set_if!(v.scissor_test, r.scissor_test, |on| enable(
        gl,
        glow::SCISSOR_TEST,
        on
    ));
    if let Some(depth) = r.clip_depth {
        set_if!(v.clip_depth, depth, |depth| platform.clip_control(
            gl,
            glow::UPPER_LEFT,
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
    /// Two bits per location of [`conversion`]: draws with it use a program variant
    /// ([`wcontainers_gpu::SlowPath::VertexFormatConverted`]); 0 when GL reads every attribute as
    /// stored.
    pub converted: u32,
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
    /// None when an attribute names a missing stream.
    pub fn new(d: &VertexLayoutDesc) -> Option<GlLayout> {
        let mut attributes = Vec::new();
        let mut converted = 0;
        for a in d.attributes.iter().flatten() {
            converted |= conversion(a.format) << (2 * a.location);
            let stream = d.streams.get(usize::from(a.stream)).copied().flatten()?;
            let (components, ty, normalized, integer) = attribute(a.format);
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
        Some(GlLayout {
            attributes,
            converted,
        })
    }
}

/// The state of a draw of the back end's own into the viewport `rect`: no blending, tests or culling.
/// The caller opens the write masks and the scissor.
pub fn plain(gl: &glow::Context, v: &mut GlValues, rect: [i32; 4], platform: &Platform) {
    all_targets(v, false, |t| &mut t.on, |on| enable(gl, glow::BLEND, on));
    set_if!(v.fill_lines, false, |_| platform.polygon_mode(glow::FILL));
    clip_distances(gl, v, 0);
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

/// Enables clip distances 0 to `n` and no others: GL leaves a distance a program does not write
/// undefined while it is enabled.
pub fn clip_distances(gl: &glow::Context, v: &mut GlValues, n: u8) {
    let held = v.clip_distances;
    for i in n.min(held)..n.max(held) {
        enable(gl, glow::CLIP_DISTANCE0 + u32::from(i), i < n);
    }
    v.clip_distances = n;
}

/// Opens every channel of every target's write mask.
pub fn open_masks(gl: &glow::Context, v: &mut GlValues) {
    // SAFETY: a plain GL state call.
    all_targets(
        v,
        [true; 4],
        |t| &mut t.mask,
        |m| unsafe { gl.color_mask(m[0], m[1], m[2], m[3]) },
    );
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

/// GL's blend factor; the second-source ones are WEBGL_blend_func_extended's, which `create_blend`
/// asked for.
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
        BlendFactor::Src1 => glow::SRC1_COLOR,
        BlendFactor::OneMinusSrc1 => glow::ONE_MINUS_SRC1_COLOR,
        BlendFactor::Src1Alpha => glow::SRC1_ALPHA,
        BlendFactor::OneMinusSrc1Alpha => glow::ONE_MINUS_SRC1_ALPHA,
    }
}

/// How a vertex program turns an attribute GL ES cannot read as stored into its value
/// ([`crate::shader::convert_inputs`]): 0 for none, 1 for BGRA bytes swapped, 2 for a word of
/// 10-10-10-2 integers, 3 for a word of 11-11-10 floats.
pub fn conversion(f: VertexFormat) -> u32 {
    match f {
        VertexFormat::Bgra8Unorm | VertexFormat::Bgrx8Unorm => 1,
        VertexFormat::Uint10_10_10_2 => 2,
        VertexFormat::Float11_11_10 => 3,
        _ => 0,
    }
}

/// How GL reads an attribute: components, type, normalised, and whether it is read as integers.
fn attribute(f: VertexFormat) -> (i32, u32, bool, bool) {
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
        // Read as bytes or one word and turned into their values in the program (`conversion`).
        V::Bgra8Unorm => (4, glow::UNSIGNED_BYTE, true, false),
        // Its three bytes as `Unorm8x3`'s, so the fourth component reads one.
        V::Bgrx8Unorm => return attribute(V::Unorm8x3),
        V::Uint10_10_10_2 | V::Float11_11_10 => (1, glow::UNSIGNED_INT, false, true),
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
    (components, ty, norm, int)
}
