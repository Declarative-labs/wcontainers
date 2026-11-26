//! The interface's plain values in wgpu's words.

use wcontainers_gpu::{
    format::FORMAT_FACTS, format::Format, resource::AddressMode, resource::CompareFn,
    resource::Filter, resource::ViewDimension, state::BlendFactor, state::BlendOp,
    state::ColorWrites, state::CullMode, state::FillMode, state::FrontFace, state::IndexFormat,
    state::StencilFace, state::StencilOp, state::Topology, state::VertexFormat,
};
use wgpu::TextureFormat as T;

/// The other sRGB form of a format, which views of its textures may take.
pub(crate) fn srgb_twin(format: T) -> Option<T> {
    (format.remove_srgb_suffix() != format)
        .then(|| format.remove_srgb_suffix())
        .or_else(|| (format.add_srgb_suffix() != format).then(|| format.add_srgb_suffix()))
}

/// The usages of a view of `texture` in `format`, whose usages on the device are `allowed`: the
/// texture's, less the attachment and storage usages a view in its other sRGB form lacks, as
/// WebGPU's `createView` takes them; None (the texture's) for a view in the texture's own format.
pub(crate) fn view_usage(
    texture: &wgpu::Texture,
    format: T,
    allowed: wgpu::TextureUsages,
) -> Option<wgpu::TextureUsages> {
    let checked = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::STORAGE_BINDING;
    (format != texture.format()).then(|| texture.usage() - (checked - allowed))
}

/// The wgpu format a texture of `format` is made in (converted formats in their wider one,
/// packed.rs); None for formats WebGPU does not hold, which the back end declares without bits.
pub(crate) fn texture_format(format: Format) -> Option<T> {
    use Format::*;
    Some(match format {
        R8Unorm => T::R8Unorm,
        R8Snorm => T::R8Snorm,
        R8Uint => T::R8Uint,
        R8Sint => T::R8Sint,
        R16Unorm => T::R16Unorm,
        R16Snorm => T::R16Snorm,
        R16Uint => T::R16Uint,
        R16Sint => T::R16Sint,
        R16Float => T::R16Float,
        Rg8Unorm => T::Rg8Unorm,
        Rg8Snorm => T::Rg8Snorm,
        Rg8Uint => T::Rg8Uint,
        Rg8Sint => T::Rg8Sint,
        R32Uint => T::R32Uint,
        R32Sint => T::R32Sint,
        R32Float => T::R32Float,
        Rg16Unorm => T::Rg16Unorm,
        Rg16Snorm => T::Rg16Snorm,
        Rg16Uint => T::Rg16Uint,
        Rg16Sint => T::Rg16Sint,
        Rg16Float => T::Rg16Float,
        Rgba8Unorm | A8Unorm | B5g6r5Unorm | Bgr5a1Unorm | Bgra4Unorm | Rgbg8Unorm | Grgb8Unorm => {
            T::Rgba8Unorm
        }
        Rgba8UnormSrgb => T::Rgba8UnormSrgb,
        Rgba8Snorm => T::Rgba8Snorm,
        Rgba8Uint => T::Rgba8Uint,
        Rgba8Sint => T::Rgba8Sint,
        Bgra8Unorm | Bgrx8Unorm => T::Bgra8Unorm,
        Bgra8UnormSrgb | Bgrx8UnormSrgb => T::Bgra8UnormSrgb,
        Rgb10a2Unorm => T::Rgb10a2Unorm,
        Rgb10a2Uint => T::Rgb10a2Uint,
        Rg11b10Float => T::Rg11b10Ufloat,
        Rgb9e5Float => T::Rgb9e5Ufloat,
        Rg32Uint => T::Rg32Uint,
        Rg32Sint => T::Rg32Sint,
        Rg32Float => T::Rg32Float,
        Rgba16Unorm => T::Rgba16Unorm,
        Rgba16Snorm => T::Rgba16Snorm,
        Rgba16Uint => T::Rgba16Uint,
        Rgba16Sint => T::Rgba16Sint,
        Rgba16Float => T::Rgba16Float,
        Rgba32Uint | Rgb32Uint => T::Rgba32Uint,
        Rgba32Sint | Rgb32Sint => T::Rgba32Sint,
        Rgba32Float | Rgb32Float => T::Rgba32Float,
        Stencil8 => T::Stencil8,
        Depth16Unorm => T::Depth16Unorm,
        Depth24Plus => T::Depth24Plus,
        Depth24PlusStencil8 => T::Depth24PlusStencil8,
        Depth32Float => T::Depth32Float,
        Depth32FloatStencil8 => T::Depth32FloatStencil8,
        Bc1RgbaUnorm => T::Bc1RgbaUnorm,
        Bc1RgbaUnormSrgb => T::Bc1RgbaUnormSrgb,
        Bc2RgbaUnorm => T::Bc2RgbaUnorm,
        Bc2RgbaUnormSrgb => T::Bc2RgbaUnormSrgb,
        Bc3RgbaUnorm => T::Bc3RgbaUnorm,
        Bc3RgbaUnormSrgb => T::Bc3RgbaUnormSrgb,
        Bc4RUnorm => T::Bc4RUnorm,
        Bc4RSnorm => T::Bc4RSnorm,
        Bc5RgUnorm => T::Bc5RgUnorm,
        Bc5RgSnorm => T::Bc5RgSnorm,
        Bc6hRgbUfloat => T::Bc6hRgbUfloat,
        Bc6hRgbFloat => T::Bc6hRgbFloat,
        Bc7RgbaUnorm => T::Bc7RgbaUnorm,
        Bc7RgbaUnormSrgb => T::Bc7RgbaUnormSrgb,
        Etc2Rgb8Unorm => T::Etc2Rgb8Unorm,
        Etc2Rgb8UnormSrgb => T::Etc2Rgb8UnormSrgb,
        Etc2Rgba8Unorm => T::Etc2Rgba8Unorm,
        Etc2Rgba8UnormSrgb => T::Etc2Rgba8UnormSrgb,
        Astc4x4Unorm => T::Astc {
            block: wgpu::AstcBlock::B4x4,
            channel: wgpu::AstcChannel::Unorm,
        },
        Astc4x4UnormSrgb => T::Astc {
            block: wgpu::AstcBlock::B4x4,
            channel: wgpu::AstcChannel::UnormSrgb,
        },
    })
}

/// The aspect a view reads: the stencil, the depth of a format with both, or all of it.
pub(crate) fn aspect(stencil: bool, format: Format) -> wgpu::TextureAspect {
    match (stencil, FORMAT_FACTS[format as usize].kind) {
        (true, _) => wgpu::TextureAspect::StencilOnly,
        (false, wcontainers_gpu::format::FormatKind::DepthStencil) => {
            wgpu::TextureAspect::DepthOnly
        }
        _ => wgpu::TextureAspect::All,
    }
}

pub(crate) fn view_dimension(d: ViewDimension) -> wgpu::TextureViewDimension {
    match d {
        ViewDimension::D2 => wgpu::TextureViewDimension::D2,
        ViewDimension::D2Array => wgpu::TextureViewDimension::D2Array,
        ViewDimension::Cube => wgpu::TextureViewDimension::Cube,
        ViewDimension::CubeArray => wgpu::TextureViewDimension::CubeArray,
        ViewDimension::D3 => wgpu::TextureViewDimension::D3,
    }
}

/// None for the formats WebGPU lacks, which the back end converts.
pub(crate) fn vertex_format(f: VertexFormat) -> Option<wgpu::VertexFormat> {
    use VertexFormat::*;
    use wgpu::VertexFormat as W;
    Some(match f {
        Uint8x1 => W::Uint8,
        Uint8x2 => W::Uint8x2,
        Uint8x4 => W::Uint8x4,
        Sint8x1 => W::Sint8,
        Sint8x2 => W::Sint8x2,
        Sint8x4 => W::Sint8x4,
        Unorm8x1 => W::Unorm8,
        Unorm8x2 => W::Unorm8x2,
        Unorm8x4 => W::Unorm8x4,
        Snorm8x1 => W::Snorm8,
        Snorm8x2 => W::Snorm8x2,
        Snorm8x4 => W::Snorm8x4,
        Uint16x1 => W::Uint16,
        Uint16x2 => W::Uint16x2,
        Uint16x4 => W::Uint16x4,
        Sint16x1 => W::Sint16,
        Sint16x2 => W::Sint16x2,
        Sint16x4 => W::Sint16x4,
        Unorm16x1 => W::Unorm16,
        Unorm16x2 => W::Unorm16x2,
        Unorm16x4 => W::Unorm16x4,
        Snorm16x1 => W::Snorm16,
        Snorm16x2 => W::Snorm16x2,
        Snorm16x4 => W::Snorm16x4,
        Float16x1 => W::Float16,
        Float16x2 => W::Float16x2,
        Float16x4 => W::Float16x4,
        Float32x1 => W::Float32,
        Float32x2 => W::Float32x2,
        Float32x3 => W::Float32x3,
        Float32x4 => W::Float32x4,
        Uint32x1 => W::Uint32,
        Uint32x2 => W::Uint32x2,
        Uint32x3 => W::Uint32x3,
        Uint32x4 => W::Uint32x4,
        Sint32x1 => W::Sint32,
        Sint32x2 => W::Sint32x2,
        Sint32x3 => W::Sint32x3,
        Sint32x4 => W::Sint32x4,
        Bgra8Unorm => W::Unorm8x4Bgra,
        Unorm10_10_10_2 => W::Unorm10_10_10_2,
        Uint8x3 | Sint8x3 | Unorm8x3 | Snorm8x3 | Uint16x3 | Sint16x3 | Unorm16x3 | Snorm16x3
        | Float16x3 | Uint10_10_10_2 | Float11_11_10 | Bgrx8Unorm => return None,
    })
}

/// None for the factors WebGPU lacks (the constant's alpha alone).
pub(crate) fn blend_factor(f: BlendFactor) -> Option<wgpu::BlendFactor> {
    use wgpu::BlendFactor as W;
    Some(match f {
        BlendFactor::Zero => W::Zero,
        BlendFactor::One => W::One,
        BlendFactor::Src => W::Src,
        BlendFactor::OneMinusSrc => W::OneMinusSrc,
        BlendFactor::SrcAlpha => W::SrcAlpha,
        BlendFactor::OneMinusSrcAlpha => W::OneMinusSrcAlpha,
        BlendFactor::Dst => W::Dst,
        BlendFactor::OneMinusDst => W::OneMinusDst,
        BlendFactor::DstAlpha => W::DstAlpha,
        BlendFactor::OneMinusDstAlpha => W::OneMinusDstAlpha,
        BlendFactor::SrcAlphaSaturated => W::SrcAlphaSaturated,
        BlendFactor::Constant => W::Constant,
        BlendFactor::OneMinusConstant => W::OneMinusConstant,
        BlendFactor::Src1 => W::Src1,
        BlendFactor::OneMinusSrc1 => W::OneMinusSrc1,
        BlendFactor::Src1Alpha => W::Src1Alpha,
        BlendFactor::OneMinusSrc1Alpha => W::OneMinusSrc1Alpha,
        BlendFactor::ConstantAlpha | BlendFactor::OneMinusConstantAlpha => return None,
    })
}

pub(crate) fn blend_op(op: BlendOp) -> wgpu::BlendOperation {
    match op {
        BlendOp::Add => wgpu::BlendOperation::Add,
        BlendOp::Subtract => wgpu::BlendOperation::Subtract,
        BlendOp::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
        BlendOp::Min => wgpu::BlendOperation::Min,
        BlendOp::Max => wgpu::BlendOperation::Max,
    }
}

pub(crate) fn color_writes(w: ColorWrites) -> wgpu::ColorWrites {
    wgpu::ColorWrites::from_bits_truncate(u32::from(w.bits()))
}

pub(crate) fn compare(c: CompareFn) -> wgpu::CompareFunction {
    use wgpu::CompareFunction as W;
    match c {
        CompareFn::Never => W::Never,
        CompareFn::Less => W::Less,
        CompareFn::Equal => W::Equal,
        CompareFn::LessEqual => W::LessEqual,
        CompareFn::Greater => W::Greater,
        CompareFn::NotEqual => W::NotEqual,
        CompareFn::GreaterEqual => W::GreaterEqual,
        CompareFn::Always => W::Always,
    }
}

fn stencil_op(op: StencilOp) -> wgpu::StencilOperation {
    use wgpu::StencilOperation as W;
    match op {
        StencilOp::Keep => W::Keep,
        StencilOp::Zero => W::Zero,
        StencilOp::Replace => W::Replace,
        StencilOp::IncrementClamp => W::IncrementClamp,
        StencilOp::DecrementClamp => W::DecrementClamp,
        StencilOp::Invert => W::Invert,
        StencilOp::IncrementWrap => W::IncrementWrap,
        StencilOp::DecrementWrap => W::DecrementWrap,
    }
}

pub(crate) fn stencil_face(f: &StencilFace) -> wgpu::StencilFaceState {
    wgpu::StencilFaceState {
        compare: compare(f.compare),
        fail_op: stencil_op(f.fail),
        depth_fail_op: stencil_op(f.depth_fail),
        pass_op: stencil_op(f.pass),
    }
}

pub(crate) fn cull(c: CullMode) -> Option<wgpu::Face> {
    match c {
        CullMode::None => None,
        CullMode::Front => Some(wgpu::Face::Front),
        CullMode::Back => Some(wgpu::Face::Back),
    }
}

pub(crate) fn fill(f: FillMode) -> wgpu::PolygonMode {
    match f {
        FillMode::Solid => wgpu::PolygonMode::Fill,
        FillMode::Line => wgpu::PolygonMode::Line,
        FillMode::Point => wgpu::PolygonMode::Point,
    }
}

pub(crate) fn front_face(f: FrontFace) -> wgpu::FrontFace {
    match f {
        FrontFace::CounterClockwise => wgpu::FrontFace::Ccw,
        FrontFace::Clockwise => wgpu::FrontFace::Cw,
    }
}

/// None for fans and loops, which reach a back end only through `draw_fan_or_loop`.
pub(crate) fn topology(t: Topology) -> Option<wgpu::PrimitiveTopology> {
    use wgpu::PrimitiveTopology as W;
    Some(match t {
        Topology::PointList => W::PointList,
        Topology::LineList => W::LineList,
        Topology::LineStrip => W::LineStrip,
        Topology::TriangleList => W::TriangleList,
        Topology::TriangleStrip => W::TriangleStrip,
        Topology::TriangleFan | Topology::LineLoop => return None,
    })
}

/// None for 8-bit indices, which fronts widen first.
pub(crate) fn index_format(f: IndexFormat) -> Option<wgpu::IndexFormat> {
    match f {
        IndexFormat::U8 => None,
        IndexFormat::U16 => Some(wgpu::IndexFormat::Uint16),
        IndexFormat::U32 => Some(wgpu::IndexFormat::Uint32),
    }
}

pub(crate) fn filter(f: Filter) -> wgpu::FilterMode {
    match f {
        Filter::Nearest => wgpu::FilterMode::Nearest,
        Filter::Linear => wgpu::FilterMode::Linear,
    }
}

pub(crate) fn mip_filter(f: Filter) -> wgpu::MipmapFilterMode {
    match f {
        Filter::Nearest => wgpu::MipmapFilterMode::Nearest,
        Filter::Linear => wgpu::MipmapFilterMode::Linear,
    }
}

/// None for the modes WebGPU lacks (mirror once, and border colours other than the clamp WebGPU
/// gives), which fronts build in the program.
pub(crate) fn address(a: AddressMode) -> Option<wgpu::AddressMode> {
    Some(match a {
        AddressMode::Repeat => wgpu::AddressMode::Repeat,
        AddressMode::Mirror => wgpu::AddressMode::MirrorRepeat,
        AddressMode::Clamp => wgpu::AddressMode::ClampToEdge,
        AddressMode::Border | AddressMode::MirrorOnce => return None,
    })
}
