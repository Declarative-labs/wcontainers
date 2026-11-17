//! A pipeline state object's fixed state, in Direct3D 12's own numbers, and the interface blocks
//! made from it.

use wcontainers_gpu::{
    BlendComponent, BlendDesc, BlendFactor, BlendOp, Caps, ColorWrites, CompareFn, CullMode,
    DepthStencilDesc, Features, FillMode, FrontFace, LogicOp, MAX_COLOR_TARGETS, ProvokingVertex,
    RasterDesc, RenderTargetBlend, StencilFace, StencilOp, Topology, VertexFormat,
};

use wcontainers_directx_graphics_infrastructure::dxgi_vertex_format;

use crate::Error;

/// `D3D12_RENDER_TARGET_BLEND_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TargetBlend {
    pub blend_enable: bool,
    pub logic_op_enable: bool,
    pub src: u32,
    pub dst: u32,
    pub op: u32,
    pub src_alpha: u32,
    pub dst_alpha: u32,
    pub op_alpha: u32,
    pub logic_op: u32,
    pub write_mask: u8,
}

/// `D3D12_BLEND_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Blend {
    pub alpha_to_coverage: bool,
    pub independent: bool,
    pub targets: [TargetBlend; MAX_COLOR_TARGETS],
}

/// `D3D12_RASTERIZER_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Raster {
    pub fill: u32,
    pub cull: u32,
    pub front_counter_clockwise: bool,
    pub depth_bias: i32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip: bool,
    pub multisample: bool,
    pub antialiased_line: bool,
    pub forced_sample_count: u32,
    pub conservative: u32,
}

/// `D3D12_DEPTH_STENCILOP_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StencilOps {
    pub fail: u32,
    pub depth_fail: u32,
    pub pass: u32,
    pub func: u32,
}

/// `D3D12_DEPTH_STENCIL_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DepthStencil {
    pub depth_enable: bool,
    pub depth_write_all: bool,
    pub depth_func: u32,
    pub stencil_enable: bool,
    pub stencil_read_mask: u8,
    pub stencil_write_mask: u8,
    pub front: StencilOps,
    pub back: StencilOps,
}

/// `D3D12_INPUT_ELEMENT_DESC`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputElement {
    pub semantic: String,
    pub semantic_index: u32,
    pub format: u32,
    pub slot: u32,
    /// `D3D12_APPEND_ALIGNED_ELEMENT` is resolved by [`resolve_offsets`].
    pub offset: u32,
    pub per_instance: bool,
    pub step_rate: u32,
}

/// `D3D12_APPEND_ALIGNED_ELEMENT`.
pub const APPEND_ALIGNED: u32 = u32::MAX;

fn blend_factor(value: u32) -> Result<BlendFactor, Error> {
    use BlendFactor::*;
    Ok(match value {
        1 => Zero,
        2 => One,
        3 => Src,
        4 => OneMinusSrc,
        5 => SrcAlpha,
        6 => OneMinusSrcAlpha,
        7 => DstAlpha,
        8 => OneMinusDstAlpha,
        9 => Dst,
        10 => OneMinusDst,
        11 => SrcAlphaSaturated,
        14 => Constant,
        15 => OneMinusConstant,
        16 => Src1,
        17 => OneMinusSrc1,
        18 => Src1Alpha,
        19 => OneMinusSrc1Alpha,
        20 => ConstantAlpha,
        21 => OneMinusConstantAlpha,
        _ => return Err(Error::Invalid("unknown blend factor")),
    })
}

fn blend_op(value: u32) -> Result<BlendOp, Error> {
    Ok(match value {
        1 => BlendOp::Add,
        2 => BlendOp::Subtract,
        3 => BlendOp::ReverseSubtract,
        4 => BlendOp::Min,
        5 => BlendOp::Max,
        _ => return Err(Error::Invalid("unknown blend operation")),
    })
}

fn logic_op(value: u32) -> Result<LogicOp, Error> {
    use LogicOp::*;
    Ok(match value {
        0 => Clear,
        1 => Set,
        2 => Copy,
        3 => CopyInverted,
        4 => Noop,
        5 => Invert,
        6 => And,
        7 => Nand,
        8 => Or,
        9 => Nor,
        10 => Xor,
        11 => Equivalent,
        12 => AndReverse,
        13 => AndInverted,
        14 => OrReverse,
        15 => OrInverted,
        _ => return Err(Error::Invalid("unknown logic operation")),
    })
}

/// `D3D12_COMPARISON_FUNC`.
pub fn compare(value: u32) -> Result<CompareFn, Error> {
    use CompareFn::*;
    Ok(match value {
        1 => Never,
        2 => Less,
        3 => Equal,
        4 => LessEqual,
        5 => Greater,
        6 => NotEqual,
        7 => GreaterEqual,
        8 => Always,
        _ => return Err(Error::Invalid("unknown comparison")),
    })
}

fn stencil_op(value: u32) -> Result<StencilOp, Error> {
    use StencilOp::*;
    Ok(match value {
        1 => Keep,
        2 => Zero,
        3 => Replace,
        4 => IncrementClamp,
        5 => DecrementClamp,
        6 => Invert,
        7 => IncrementWrap,
        8 => DecrementWrap,
        _ => return Err(Error::Invalid("unknown stencil operation")),
    })
}

impl Blend {
    /// The blend block for `targets` color targets. Routes the back end lacks (independent blending,
    /// logic operations) are refused until their slow paths exist.
    pub fn block(&self, caps: &Caps, targets: usize) -> Result<BlendDesc, Error> {
        let mut out = BlendDesc {
            targets: [RenderTargetBlend {
                blend: None,
                writes: ColorWrites::all(),
            }; MAX_COLOR_TARGETS],
            same_for_all: !self.independent,
            alpha_to_coverage: self.alpha_to_coverage,
            logic_op: None,
        };
        let used = if self.independent { targets.max(1) } else { 1 };
        for (i, t) in self.targets.iter().take(used).enumerate() {
            if t.logic_op_enable {
                if !caps.has(Features::LOGIC_OP) {
                    return Err(Error::NotImplemented("logic operations without LOGIC_OP"));
                }
                out.logic_op = Some(logic_op(t.logic_op)?);
            }
            out.targets[i] = RenderTargetBlend {
                blend: t
                    .blend_enable
                    .then(|| -> Result<_, Error> {
                        Ok((
                            BlendComponent {
                                src: blend_factor(t.src)?,
                                dst: blend_factor(t.dst)?,
                                op: blend_op(t.op)?,
                            },
                            BlendComponent {
                                src: blend_factor(t.src_alpha)?,
                                dst: blend_factor(t.dst_alpha)?,
                                op: blend_op(t.op_alpha)?,
                            },
                        ))
                    })
                    .transpose()?,
                writes: ColorWrites::from_bits_truncate(t.write_mask),
            };
        }
        if self.independent {
            let first = out.targets[0];
            out.same_for_all = out.targets[..targets.max(1)].iter().all(|t| *t == first);
            if !out.same_for_all && !caps.has(Features::INDEPENDENT_BLEND) {
                return Err(Error::NotImplemented(
                    "independent blending without INDEPENDENT_BLEND",
                ));
            }
        } else {
            out.targets = [out.targets[0]; MAX_COLOR_TARGETS];
        }
        Ok(out)
    }
}

impl Raster {
    /// The rasterising block, with the pipeline's sample mask.
    pub fn block(&self, caps: &Caps, sample_mask: u32, samples: u32) -> Result<RasterDesc, Error> {
        let fill = match self.fill {
            2 => FillMode::Line,
            3 => FillMode::Solid,
            _ => return Err(Error::Invalid("unknown fill mode")),
        };
        if fill == FillMode::Line && !caps.has(Features::POLYGON_MODE) {
            return Err(Error::NotImplemented("wireframe without POLYGON_MODE"));
        }
        let cull = match self.cull {
            1 => CullMode::None,
            2 => CullMode::Front,
            3 => CullMode::Back,
            _ => return Err(Error::Invalid("unknown cull mode")),
        };
        if self.depth_bias != 0
            || self.slope_scaled_depth_bias != 0.0
            || self.depth_bias_clamp != 0.0
            || !self.depth_clip
            || self.forced_sample_count != 0
            || self.conservative != 0
        {
            return Err(Error::NotImplemented(
                "depth bias, depth clamp, forced sample counts and conservative rasterization",
            ));
        }
        let mask = sample_mask & ((1u64 << samples.min(32)) - 1) as u32;
        if mask != ((1u64 << samples.min(32)) - 1) as u32 && !caps.has(Features::SAMPLE_MASK) {
            return Err(Error::NotImplemented(
                "a partial sample mask without SAMPLE_MASK",
            ));
        }
        Ok(RasterDesc {
            fill,
            cull,
            front: if self.front_counter_clockwise {
                FrontFace::CounterClockwise
            } else {
                FrontFace::Clockwise
            },
            depth_bias: 0,
            depth_bias_slope: 0,
            depth_bias_clamp: 0,
            depth_clip: true,
            scissor: true,
            line_width: 1.0f32.to_bits(),
            provoking: ProvokingVertex::First,
            depth_minus_one: false,
            sample_mask: if caps.has(Features::SAMPLE_MASK) {
                sample_mask
            } else {
                u32::MAX
            },
        })
    }
}

impl DepthStencil {
    /// The depth and stencil block; with no depth-stencil format, both tests are off.
    pub fn block(&self, has_target: bool) -> Result<DepthStencilDesc, Error> {
        let face = |f: &StencilOps| -> Result<StencilFace, Error> {
            Ok(StencilFace {
                compare: compare(f.func)?,
                fail: stencil_op(f.fail)?,
                depth_fail: stencil_op(f.depth_fail)?,
                pass: stencil_op(f.pass)?,
            })
        };
        let depth = has_target && self.depth_enable;
        let stencil = has_target && self.stencil_enable;
        Ok(DepthStencilDesc {
            depth_compare: depth.then(|| compare(self.depth_func)).transpose()?,
            depth_write: depth && self.depth_write_all,
            stencil: stencil
                .then(|| -> Result<_, Error> { Ok((face(&self.front)?, face(&self.back)?)) })
                .transpose()?,
            stencil_read_mask: [self.stencil_read_mask; 2],
            stencil_write_mask: [self.stencil_write_mask; 2],
            depth_bounds: None,
        })
    }
}

/// Fills in `D3D12_APPEND_ALIGNED_ELEMENT` offsets: each such element follows the previous one of
/// its slot.
pub fn resolve_offsets(elements: &mut [InputElement]) -> Result<(), Error> {
    let mut ends = [0u32; wcontainers_gpu::MAX_VERTEX_STREAMS];
    for e in elements {
        let slot = ends
            .get_mut(e.slot as usize)
            .ok_or(Error::Invalid("input slot out of range"))?;
        if e.offset == APPEND_ALIGNED {
            e.offset = *slot;
        }
        *slot = e.offset + vertex_format(e.format)?.1;
    }
    Ok(())
}

/// The vertex format of a `DXGI_FORMAT` value and its size in bytes.
pub fn vertex_format(value: u32) -> Result<(VertexFormat, u32), Error> {
    dxgi_vertex_format(value).ok_or(Error::Invalid("not a vertex format"))
}

/// `D3D_PRIMITIVE_TOPOLOGY`, for the topologies drawn without a geometry stage.
pub fn topology(value: u32) -> Result<Topology, Error> {
    Ok(match value {
        1 => Topology::PointList,
        2 => Topology::LineList,
        3 => Topology::LineStrip,
        4 => Topology::TriangleList,
        5 => Topology::TriangleStrip,
        10..=13 => return Err(Error::NotImplemented("adjacency topologies")),
        33..=64 => return Err(Error::NotImplemented("patch lists")),
        _ => return Err(Error::Invalid("unknown primitive topology")),
    })
}
