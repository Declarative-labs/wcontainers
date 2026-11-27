//! The GL values the back end last set, and each block as the GL values it stands for, converted once
//! when the block is made. A block is applied only when its handle differs from the last one applied,
//! and then only the values that differ from what GL holds are set.

use wcontainers_gpu::{
    MAX_COLOR_TARGETS, resource::CompareFn, state::BlendDesc, state::BlendFactor, state::BlendOp,
    state::CullMode, state::DepthStencilDesc, state::FillMode, state::FrontFace,
    state::ProvokingVertex, state::RasterDesc, state::RenderTargetBlend, state::StencilFace,
    state::StencilOp, state::StepMode, state::Topology, state::VertexFormat,
    state::VertexLayoutDesc,
};

use crate::format;
use wcontainers_opengl_es::OpenGlEs;
use wcontainers_opengl_es::constants as gles;

/// What GL holds now, for the values blocks and draws set.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GlValues {
    /// Per colour target: blending, its equation and factors, and the write mask.
    pub(crate) targets: [GlTarget; MAX_COLOR_TARGETS],
    pub(crate) alpha_to_coverage: bool,
    pub(crate) depth_test: bool,
    pub(crate) depth_func: u32,
    pub(crate) depth_mask: bool,
    pub(crate) stencil_test: bool,
    /// Per face (front, back): compare, reference, read mask.
    pub(crate) stencil_func: [(u32, u8, u8); 2],
    /// Per face: fail, depth fail, pass.
    pub(crate) stencil_op: [[u32; 3]; 2],
    pub(crate) stencil_write_mask: u8,
    pub(crate) cull: Option<u32>,
    pub(crate) front_face: u32,
    /// Slope factor, constant units and clamp, when the offset is on.
    pub(crate) polygon_offset: Option<(f32, f32, f32)>,
    /// Whether the line offset is on (WEBGL_polygon_mode).
    pub(crate) line_offset: bool,
    pub(crate) fill_lines: bool,
    pub(crate) depth_clamp: bool,
    /// How many clip distances are enabled, from distance 0.
    pub(crate) clip_distances: u8,
    pub(crate) scissor_test: bool,
    pub(crate) viewport: [i32; 4],
    pub(crate) depth_range: [f32; 2],
    pub(crate) scissor: [i32; 4],
    pub(crate) blend_color: [f32; 4],
    pub(crate) clip_depth: u32,
    pub(crate) provoking: u32,
}

impl GlValues {
    /// GL's initial values, with the viewport and scissor unknown so the first draw sets them.
    pub(crate) fn initial(clip_depth: u32) -> GlValues {
        GlValues {
            targets: [GlTarget::OFF; MAX_COLOR_TARGETS],
            alpha_to_coverage: false,
            depth_test: false,
            depth_func: gles::LESS,
            depth_mask: true,
            stencil_test: false,
            stencil_func: [(gles::ALWAYS, 0, 0xff); 2],
            stencil_op: [[gles::KEEP; 3]; 2],
            stencil_write_mask: 0xff,
            cull: None,
            front_face: gles::CCW,
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
            provoking: gles::LAST_VERTEX_CONVENTION,
        }
    }
}

/// Sets `field` to `value` with `set` when they differ.
macro_rules! set_if {
    ($field:expr, $value:expr, $set:expr) => {{
        let value = $value;
        if $field != value {
            $field = value;
            $set(value);
        }
    }};
}

fn enable(gl: &impl OpenGlEs, cap: u32, on: bool) {
    if on { gl.enable(cap) } else { gl.disable(cap) }
}

/// One colour target's blending and write mask.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GlTarget {
    pub(crate) on: bool,
    pub(crate) equation: [u32; 2],
    pub(crate) func: [u32; 4],
    pub(crate) mask: [bool; 4],
}

impl GlTarget {
    /// GL's initial values: no blending, every channel written.
    pub(crate) const OFF: GlTarget = GlTarget {
        on: false,
        equation: [gles::FUNC_ADD; 2],
        func: [gles::ONE, gles::ZERO, gles::ONE, gles::ZERO],
        mask: [true; 4],
    };

    fn new(t: &RenderTargetBlend) -> GlTarget {
        use wcontainers_gpu::state::ColorWrites as C;
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
pub(crate) fn uniform(gl: &impl OpenGlEs, v: &mut GlValues, t: &GlTarget) {
    all_targets(v, t.on, |x| &mut x.on, |on| enable(gl, gles::BLEND, on));
    if t.on {
        all_targets(
            v,
            t.equation,
            |x| &mut x.equation,
            |e| gl.blend_equation_separate(e[0], e[1]),
        );
        all_targets(
            v,
            t.func,
            |x| &mut x.func,
            |f| gl.blend_func_separate(f[0], f[1], f[2], f[3]),
        );
    }
    all_targets(
        v,
        t.mask,
        |x| &mut x.mask,
        |m| gl.color_mask(m[0], m[1], m[2], m[3]),
    );
}

/// Applies `b`, leaving unwritten the channels of `unwritten`, [`format::CHANNELS`] bits a target
/// from the first, each from red.
pub(crate) fn blend(gl: &impl OpenGlEs, v: &mut GlValues, b: &GlBlend, unwritten: u32) {
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
            set_if!(x.on, t.on, |on| if on {
                gl.enablei_oes(gles::BLEND, i)
            } else {
                gl.disablei_oes(gles::BLEND, i)
            });
            if t.on {
                set_if!(x.equation, t.equation, |e: [u32; 2]| gl
                    .blend_equation_separatei_oes(i, e[0], e[1]));
                set_if!(x.func, t.func, |f: [u32; 4]| gl
                    .blend_func_separatei_oes(i, f[0], f[1], f[2], f[3]));
            }
            set_if!(x.mask, t.mask, |m: [bool; 4]| gl
                .color_maski_oes(i, m[0], m[1], m[2], m[3]));
        }
    }
    set_if!(v.alpha_to_coverage, b.alpha_to_coverage, |on| enable(
        gl,
        gles::SAMPLE_ALPHA_TO_COVERAGE,
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
/// requires). Err names what is missing where the faces' compare or operations differ: the OpenGL ES
/// crate has neither glStencilFuncSeparate nor glStencilOpSeparate yet (NEEDED-FROM-API-CRATES.md), so
/// only both faces at once can be set.
pub(crate) fn depth_stencil(
    gl: &impl OpenGlEs,
    v: &mut GlValues,
    d: &GlDepthStencil,
    reference: u8,
) -> Result<(), &'static str> {
    set_if!(v.depth_test, d.depth_func.is_some(), |on| enable(
        gl,
        gles::DEPTH_TEST,
        on
    ));
    if let Some(f) = d.depth_func {
        set_if!(v.depth_func, f, |f| gl.depth_func(f));
    }
    // GL writes no depth with the test off, as an absent test means in Direct3D.
    set_if!(v.depth_mask, d.depth_mask, |on| gl.depth_mask(on));
    set_if!(v.stencil_test, d.stencil.is_some(), |on| enable(
        gl,
        gles::STENCIL_TEST,
        on
    ));
    if let Some(s) = &d.stencil {
        if s.compare[0] != s.compare[1] || s.ops[0] != s.ops[1] {
            return Err(
                "faces of differing stencil compare or operations (glStencilFuncSeparate, glStencilOpSeparate)",
            );
        }
        let func = (s.compare[0], reference, s.read_mask);
        set_if!(v.stencil_func, [func; 2], |f: [(u32, u8, u8); 2]| gl
            .stencil_func(f[0].0, i32::from(f[0].1), u32::from(f[0].2)));
        set_if!(v.stencil_op, [s.ops[0]; 2], |o: [[u32; 3]; 2]| gl
            .stencil_op(o[0][0], o[0][1], o[0][2]));
        set_if!(v.stencil_write_mask, s.write_mask, |m: u8| gl
            .stencil_mask(u32::from(m)));
    }
    Ok(())
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
                CullMode::Front => Some(gles::FRONT),
                CullMode::Back => Some(gles::BACK),
            },
            front_face: if ccw { gles::CCW } else { gles::CW },
            polygon_offset: (bias != 0.0 || slope != 0.0).then_some((slope, bias, clamp)),
            fill_lines: d.fill == FillMode::Line,
            depth_clamp: !d.depth_clip,
            scissor_test: d.scissor,
            clip_depth: (!flipped).then_some(if d.depth_minus_one {
                gles::NEGATIVE_ONE_TO_ONE_EXT
            } else {
                gles::ZERO_TO_ONE_EXT
            }),
            provoking: match d.provoking {
                ProvokingVertex::First => gles::FIRST_VERTEX_CONVENTION,
                ProvokingVertex::Last => gles::LAST_VERTEX_CONVENTION,
            },
        }
    }

    /// The vertex whose flat values the block's primitives take.
    pub fn provoking(&self) -> ProvokingVertex {
        if self.provoking == gles::FIRST_VERTEX_CONVENTION {
            ProvokingVertex::First
        } else {
            ProvokingVertex::Last
        }
    }
}

pub(crate) fn raster(gl: &impl OpenGlEs, v: &mut GlValues, r: &GlRaster) {
    if v.cull != r.cull {
        match r.cull {
            Some(face) => {
                if v.cull.is_none() {
                    enable(gl, gles::CULL_FACE, true);
                }
                gl.cull_face(face);
            }
            None => enable(gl, gles::CULL_FACE, false),
        }
        v.cull = r.cull;
    }
    set_if!(v.front_face, r.front_face, |f| gl.front_face(f));
    if v.polygon_offset != r.polygon_offset {
        if v.polygon_offset.is_none() || r.polygon_offset.is_none() {
            enable(gl, gles::POLYGON_OFFSET_FILL, r.polygon_offset.is_some());
        }
        match r.polygon_offset {
            // EXT_polygon_offset_clamp, which `create_raster` asked for.
            Some((factor, units, clamp)) if clamp != 0.0 => {
                let _ = gl.polygon_offset_clamp_ext(factor, units, clamp);
            }
            Some((factor, units, _)) => gl.polygon_offset(factor, units),
            None => {}
        }
        v.polygon_offset = r.polygon_offset;
    }
    // WEBGL_polygon_mode, which `create_raster` asked for when a block draws lines; edges take the
    // offset as filled polygons do in Direct3D.
    set_if!(v.fill_lines, r.fill_lines, |lines| polygon_mode(
        gl,
        if lines {
            gles::LINE_ANGLE
        } else {
            gles::FILL_ANGLE
        }
    ));
    let line_offset = r.fill_lines && r.polygon_offset.is_some();
    set_if!(v.line_offset, line_offset, |on| enable(
        gl,
        gles::POLYGON_OFFSET_LINE_ANGLE,
        on
    ));
    // EXT_depth_clamp, which `create_raster` asked for when a block clamps.
    set_if!(v.depth_clamp, r.depth_clamp, |on| enable(
        gl,
        gles::DEPTH_CLAMP_EXT,
        on
    ));
    set_if!(v.scissor_test, r.scissor_test, |on| enable(
        gl,
        gles::SCISSOR_TEST,
        on
    ));
    if let Some(depth) = r.clip_depth {
        set_if!(v.clip_depth, depth, |depth| {
            let _ = gl.clip_control_ext(gles::UPPER_LEFT_EXT, depth);
        });
    }
    set_if!(v.provoking, r.provoking, |mode| {
        let _ = gl.provoking_vertex_angle(mode);
    });
}

/// Fills polygons (`FILL_ANGLE`) or draws their edges (`LINE_ANGLE`), for both faces
/// (WEBGL_polygon_mode).
fn polygon_mode(gl: &impl OpenGlEs, mode: u32) {
    let _ = gl.polygon_mode_angle(gles::FRONT_AND_BACK, mode);
}

/// A vertex layout block as the attribute pointers it stands for.
#[derive(Debug)]
pub struct GlLayout {
    pub attributes: Vec<GlAttribute>,
    /// Two bits per location of [`conversion`]: draws with it use a program variant
    /// ([`wcontainers_gpu::slow::SlowPath::VertexFormatConverted`]); 0 when GL reads every attribute as
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
pub(crate) fn plain(gl: &impl OpenGlEs, v: &mut GlValues, rect: [i32; 4]) {
    all_targets(v, false, |t| &mut t.on, |on| enable(gl, gles::BLEND, on));
    set_if!(v.fill_lines, false, |_| polygon_mode(gl, gles::FILL_ANGLE));
    clip_distances(gl, v, 0);
    set_if!(v.alpha_to_coverage, false, |on| enable(
        gl,
        gles::SAMPLE_ALPHA_TO_COVERAGE,
        on
    ));
    set_if!(v.depth_test, false, |on| enable(gl, gles::DEPTH_TEST, on));
    set_if!(v.stencil_test, false, |on| enable(
        gl,
        gles::STENCIL_TEST,
        on
    ));
    if v.cull.is_some() {
        enable(gl, gles::CULL_FACE, false);
        v.cull = None;
    }
    set_if!(v.viewport, rect, |r: [i32; 4]| gl
        .viewport(r[0], r[1], r[2], r[3]));
}

/// Enables clip distances 0 to `n` and no others: GL leaves a distance a program does not write
/// undefined while it is enabled.
pub(crate) fn clip_distances(gl: &impl OpenGlEs, v: &mut GlValues, n: u8) {
    let held = v.clip_distances;
    for i in n.min(held)..n.max(held) {
        enable(gl, crate::platform::CLIP_DISTANCE0 + u32::from(i), i < n);
    }
    v.clip_distances = n;
}

/// Opens every channel of every target's write mask.
pub(crate) fn open_masks(gl: &impl OpenGlEs, v: &mut GlValues) {
    all_targets(
        v,
        [true; 4],
        |t| &mut t.mask,
        |m| gl.color_mask(m[0], m[1], m[2], m[3]),
    );
}

pub(crate) fn mode(t: Topology) -> u32 {
    match t {
        Topology::PointList => gles::POINTS,
        Topology::LineList => gles::LINES,
        Topology::LineStrip => gles::LINE_STRIP,
        Topology::TriangleList => gles::TRIANGLES,
        Topology::TriangleStrip => gles::TRIANGLE_STRIP,
        Topology::TriangleFan => gles::TRIANGLE_FAN,
        Topology::LineLoop => gles::LINE_LOOP,
    }
}

pub(crate) fn compare(f: CompareFn) -> u32 {
    match f {
        CompareFn::Never => gles::NEVER,
        CompareFn::Less => gles::LESS,
        CompareFn::Equal => gles::EQUAL,
        CompareFn::LessEqual => gles::LEQUAL,
        CompareFn::Greater => gles::GREATER,
        CompareFn::NotEqual => gles::NOTEQUAL,
        CompareFn::GreaterEqual => gles::GEQUAL,
        CompareFn::Always => gles::ALWAYS,
    }
}

fn stencil(op: StencilOp) -> u32 {
    match op {
        StencilOp::Keep => gles::KEEP,
        StencilOp::Zero => gles::ZERO,
        StencilOp::Replace => gles::REPLACE,
        StencilOp::IncrementClamp => gles::INCR,
        StencilOp::DecrementClamp => gles::DECR,
        StencilOp::Invert => gles::INVERT,
        StencilOp::IncrementWrap => gles::INCR_WRAP,
        StencilOp::DecrementWrap => gles::DECR_WRAP,
    }
}

fn op(o: BlendOp) -> u32 {
    match o {
        BlendOp::Add => gles::FUNC_ADD,
        BlendOp::Subtract => gles::FUNC_SUBTRACT,
        BlendOp::ReverseSubtract => gles::FUNC_REVERSE_SUBTRACT,
        BlendOp::Min => gles::MIN,
        BlendOp::Max => gles::MAX,
    }
}

/// Whether a factor reads the fragment's second output (dual-source blending).
pub(crate) fn second_source(f: BlendFactor) -> bool {
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
pub(crate) fn factor(f: BlendFactor) -> u32 {
    match f {
        BlendFactor::Zero => gles::ZERO,
        BlendFactor::One => gles::ONE,
        BlendFactor::Src => gles::SRC_COLOR,
        BlendFactor::OneMinusSrc => gles::ONE_MINUS_SRC_COLOR,
        BlendFactor::SrcAlpha => gles::SRC_ALPHA,
        BlendFactor::OneMinusSrcAlpha => gles::ONE_MINUS_SRC_ALPHA,
        BlendFactor::Dst => gles::DST_COLOR,
        BlendFactor::OneMinusDst => gles::ONE_MINUS_DST_COLOR,
        BlendFactor::DstAlpha => gles::DST_ALPHA,
        BlendFactor::OneMinusDstAlpha => gles::ONE_MINUS_DST_ALPHA,
        BlendFactor::SrcAlphaSaturated => gles::SRC_ALPHA_SATURATE,
        BlendFactor::Constant => gles::CONSTANT_COLOR,
        BlendFactor::OneMinusConstant => gles::ONE_MINUS_CONSTANT_COLOR,
        BlendFactor::ConstantAlpha => gles::CONSTANT_ALPHA,
        BlendFactor::OneMinusConstantAlpha => gles::ONE_MINUS_CONSTANT_ALPHA,
        BlendFactor::Src1 => crate::platform::SRC1_COLOR,
        BlendFactor::OneMinusSrc1 => crate::platform::ONE_MINUS_SRC1_COLOR,
        BlendFactor::Src1Alpha => crate::platform::SRC1_ALPHA,
        BlendFactor::OneMinusSrc1Alpha => crate::platform::ONE_MINUS_SRC1_ALPHA,
    }
}

/// How a vertex program turns an attribute GL ES cannot read as stored into its value
/// ([`crate::shader::convert_inputs`]): 0 for none, 1 for BGRA bytes swapped, 2 for a word of
/// 10-10-10-2 integers, 3 for a word of 11-11-10 floats.
pub(crate) fn conversion(f: VertexFormat) -> u32 {
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
        V::Uint8x1 | V::Uint8x2 | V::Uint8x3 | V::Uint8x4 => (0, gles::UNSIGNED_BYTE, false, true),
        V::Sint8x1 | V::Sint8x2 | V::Sint8x3 | V::Sint8x4 => (0, gles::BYTE, false, true),
        V::Unorm8x1 | V::Unorm8x2 | V::Unorm8x3 | V::Unorm8x4 => {
            (0, gles::UNSIGNED_BYTE, true, false)
        }
        V::Snorm8x1 | V::Snorm8x2 | V::Snorm8x3 | V::Snorm8x4 => (0, gles::BYTE, true, false),
        V::Uint16x1 | V::Uint16x2 | V::Uint16x3 | V::Uint16x4 => {
            (0, gles::UNSIGNED_SHORT, false, true)
        }
        V::Sint16x1 | V::Sint16x2 | V::Sint16x3 | V::Sint16x4 => (0, gles::SHORT, false, true),
        V::Unorm16x1 | V::Unorm16x2 | V::Unorm16x3 | V::Unorm16x4 => {
            (0, gles::UNSIGNED_SHORT, true, false)
        }
        V::Snorm16x1 | V::Snorm16x2 | V::Snorm16x3 | V::Snorm16x4 => (0, gles::SHORT, true, false),
        V::Float16x1 | V::Float16x2 | V::Float16x3 | V::Float16x4 => {
            (0, gles::HALF_FLOAT, false, false)
        }
        V::Float32x1 | V::Float32x2 | V::Float32x3 | V::Float32x4 => (0, gles::FLOAT, false, false),
        V::Uint32x1 | V::Uint32x2 | V::Uint32x3 | V::Uint32x4 => {
            (0, gles::UNSIGNED_INT, false, true)
        }
        V::Sint32x1 | V::Sint32x2 | V::Sint32x3 | V::Sint32x4 => (0, gles::INT, false, true),
        V::Unorm10_10_10_2 => (4, gles::UNSIGNED_INT_2_10_10_10_REV, true, false),
        // Read as bytes or one word and turned into their values in the program (`conversion`).
        V::Bgra8Unorm => (4, gles::UNSIGNED_BYTE, true, false),
        // Its three bytes as `Unorm8x3`'s, so the fourth component reads one.
        V::Bgrx8Unorm => return attribute(V::Unorm8x3),
        V::Uint10_10_10_2 | V::Float11_11_10 => (1, gles::UNSIGNED_INT, false, true),
    };
    let components = if n != 0 {
        n
    } else {
        // The variants of each type run x1 to x4 in order.
        let base = match ty {
            gles::UNSIGNED_BYTE if int => V::Uint8x1,
            gles::BYTE if int => V::Sint8x1,
            gles::UNSIGNED_BYTE => V::Unorm8x1,
            gles::BYTE => V::Snorm8x1,
            gles::UNSIGNED_SHORT if int => V::Uint16x1,
            gles::SHORT if int => V::Sint16x1,
            gles::UNSIGNED_SHORT => V::Unorm16x1,
            gles::SHORT => V::Snorm16x1,
            gles::HALF_FLOAT => V::Float16x1,
            gles::FLOAT => V::Float32x1,
            gles::UNSIGNED_INT => V::Uint32x1,
            _ => V::Sint32x1,
        };
        f as i32 - base as i32 + 1
    };
    (components, ty, norm, int)
}
