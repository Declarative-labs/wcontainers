//! Gallium's formats in the interface's terms, and what the device can do with each.

use wcontainers_gpu::{Format, FormatCaps, FormatUse, VertexFormat};

use crate::gallium::*;

/// Every Gallium format the interface holds, beside the interface's format of the same bits.
const FORMATS: &[(pipe_format, Format)] = &[
    (PIPE_FORMAT_R8_UNORM, Format::R8Unorm),
    (PIPE_FORMAT_R8_SNORM, Format::R8Snorm),
    (PIPE_FORMAT_R8_UINT, Format::R8Uint),
    (PIPE_FORMAT_R8_SINT, Format::R8Sint),
    (PIPE_FORMAT_R16_UNORM, Format::R16Unorm),
    (PIPE_FORMAT_R16_SNORM, Format::R16Snorm),
    (PIPE_FORMAT_R16_UINT, Format::R16Uint),
    (PIPE_FORMAT_R16_SINT, Format::R16Sint),
    (PIPE_FORMAT_R16_FLOAT, Format::R16Float),
    (PIPE_FORMAT_R8G8_UNORM, Format::Rg8Unorm),
    (PIPE_FORMAT_R8G8_SNORM, Format::Rg8Snorm),
    (PIPE_FORMAT_R8G8_UINT, Format::Rg8Uint),
    (PIPE_FORMAT_R8G8_SINT, Format::Rg8Sint),
    (PIPE_FORMAT_R32_UINT, Format::R32Uint),
    (PIPE_FORMAT_R32_SINT, Format::R32Sint),
    (PIPE_FORMAT_R32_FLOAT, Format::R32Float),
    (PIPE_FORMAT_R16G16_UNORM, Format::Rg16Unorm),
    (PIPE_FORMAT_R16G16_SNORM, Format::Rg16Snorm),
    (PIPE_FORMAT_R16G16_UINT, Format::Rg16Uint),
    (PIPE_FORMAT_R16G16_SINT, Format::Rg16Sint),
    (PIPE_FORMAT_R16G16_FLOAT, Format::Rg16Float),
    (PIPE_FORMAT_R8G8B8A8_UNORM, Format::Rgba8Unorm),
    (PIPE_FORMAT_R8G8B8A8_SRGB, Format::Rgba8UnormSrgb),
    (PIPE_FORMAT_R8G8B8A8_SNORM, Format::Rgba8Snorm),
    (PIPE_FORMAT_R8G8B8A8_UINT, Format::Rgba8Uint),
    (PIPE_FORMAT_R8G8B8A8_SINT, Format::Rgba8Sint),
    (PIPE_FORMAT_B8G8R8A8_UNORM, Format::Bgra8Unorm),
    (PIPE_FORMAT_B8G8R8A8_SRGB, Format::Bgra8UnormSrgb),
    (PIPE_FORMAT_B8G8R8X8_UNORM, Format::Bgrx8Unorm),
    (PIPE_FORMAT_B8G8R8X8_SRGB, Format::Bgrx8UnormSrgb),
    (PIPE_FORMAT_R10G10B10A2_UNORM, Format::Rgb10a2Unorm),
    (PIPE_FORMAT_R10G10B10A2_UINT, Format::Rgb10a2Uint),
    (PIPE_FORMAT_R11G11B10_FLOAT, Format::Rg11b10Float),
    (PIPE_FORMAT_R9G9B9E5_FLOAT, Format::Rgb9e5Float),
    (PIPE_FORMAT_B5G6R5_UNORM, Format::B5g6r5Unorm),
    (PIPE_FORMAT_B5G5R5A1_UNORM, Format::Bgr5a1Unorm),
    (PIPE_FORMAT_B4G4R4A4_UNORM, Format::Bgra4Unorm),
    (PIPE_FORMAT_R8G8_B8G8_422_UNORM, Format::Rgbg8Unorm),
    (PIPE_FORMAT_G8R8_G8B8_422_UNORM, Format::Grgb8Unorm),
    (PIPE_FORMAT_R32G32_UINT, Format::Rg32Uint),
    (PIPE_FORMAT_R32G32_SINT, Format::Rg32Sint),
    (PIPE_FORMAT_R32G32_FLOAT, Format::Rg32Float),
    (PIPE_FORMAT_R16G16B16A16_UNORM, Format::Rgba16Unorm),
    (PIPE_FORMAT_R16G16B16A16_SNORM, Format::Rgba16Snorm),
    (PIPE_FORMAT_R16G16B16A16_UINT, Format::Rgba16Uint),
    (PIPE_FORMAT_R16G16B16A16_SINT, Format::Rgba16Sint),
    (PIPE_FORMAT_R16G16B16A16_FLOAT, Format::Rgba16Float),
    (PIPE_FORMAT_R32G32B32_UINT, Format::Rgb32Uint),
    (PIPE_FORMAT_R32G32B32_SINT, Format::Rgb32Sint),
    (PIPE_FORMAT_R32G32B32_FLOAT, Format::Rgb32Float),
    (PIPE_FORMAT_R32G32B32A32_UINT, Format::Rgba32Uint),
    (PIPE_FORMAT_R32G32B32A32_SINT, Format::Rgba32Sint),
    (PIPE_FORMAT_R32G32B32A32_FLOAT, Format::Rgba32Float),
    (PIPE_FORMAT_A8_UNORM, Format::A8Unorm),
    (PIPE_FORMAT_S8_UINT, Format::Stencil8),
    (PIPE_FORMAT_Z16_UNORM, Format::Depth16Unorm),
    (PIPE_FORMAT_Z24X8_UNORM, Format::Depth24Plus),
    (PIPE_FORMAT_Z24_UNORM_S8_UINT, Format::Depth24PlusStencil8),
    (PIPE_FORMAT_Z32_FLOAT, Format::Depth32Float),
    (
        PIPE_FORMAT_Z32_FLOAT_S8X24_UINT,
        Format::Depth32FloatStencil8,
    ),
    (PIPE_FORMAT_DXT1_RGBA, Format::Bc1RgbaUnorm),
    (PIPE_FORMAT_DXT1_SRGBA, Format::Bc1RgbaUnormSrgb),
    // Read with alpha one (`OPAQUE_IN_ALPHA`).
    (PIPE_FORMAT_DXT1_RGB, Format::Bc1RgbaUnorm),
    (PIPE_FORMAT_DXT1_SRGB, Format::Bc1RgbaUnormSrgb),
    (PIPE_FORMAT_DXT3_RGBA, Format::Bc2RgbaUnorm),
    (PIPE_FORMAT_DXT3_SRGBA, Format::Bc2RgbaUnormSrgb),
    (PIPE_FORMAT_DXT5_RGBA, Format::Bc3RgbaUnorm),
    (PIPE_FORMAT_DXT5_SRGBA, Format::Bc3RgbaUnormSrgb),
    (PIPE_FORMAT_RGTC1_UNORM, Format::Bc4RUnorm),
    (PIPE_FORMAT_RGTC1_SNORM, Format::Bc4RSnorm),
    (PIPE_FORMAT_RGTC2_UNORM, Format::Bc5RgUnorm),
    (PIPE_FORMAT_RGTC2_SNORM, Format::Bc5RgSnorm),
    (PIPE_FORMAT_BPTC_RGB_UFLOAT, Format::Bc6hRgbUfloat),
    (PIPE_FORMAT_BPTC_RGB_FLOAT, Format::Bc6hRgbFloat),
    (PIPE_FORMAT_BPTC_RGBA_UNORM, Format::Bc7RgbaUnorm),
    (PIPE_FORMAT_BPTC_SRGBA, Format::Bc7RgbaUnormSrgb),
    (PIPE_FORMAT_ETC2_RGB8, Format::Etc2Rgb8Unorm),
    (PIPE_FORMAT_ETC2_SRGB8, Format::Etc2Rgb8UnormSrgb),
    (PIPE_FORMAT_ETC2_RGBA8, Format::Etc2Rgba8Unorm),
    (PIPE_FORMAT_ETC2_SRGBA8, Format::Etc2Rgba8UnormSrgb),
    (PIPE_FORMAT_ASTC_4x4, Format::Astc4x4Unorm),
    (PIPE_FORMAT_ASTC_4x4_SRGB, Format::Astc4x4UnormSrgb),
];

/// Formats held in an interface format that has alpha where they have none, read with alpha one:
/// BC1 without alpha decodes as BC1 with it but for the alpha of its three-colour blocks' fourth
/// colour, which is one without alpha and zero with it.
const OPAQUE_IN_ALPHA: &[pipe_format] = &[PIPE_FORMAT_DXT1_RGB, PIPE_FORMAT_DXT1_SRGB];

/// Whether views of `format` read alpha one in place of the alpha of its interface format.
pub fn opaque_in_alpha(format: pipe_format) -> bool {
    OPAQUE_IN_ALPHA.contains(&format)
}

/// The interface's format of a Gallium format, if it holds it.
/// The stencil of a depth-stencil format, as a sampler view names it (OpenGL's stencil texturing):
/// a view of the texture's own format reading its stencil.
const STENCIL_VIEWS: &[(pipe_format, Format)] = &[
    (PIPE_FORMAT_X24S8_UINT, Format::Depth24PlusStencil8),
    (PIPE_FORMAT_X32_S8X24_UINT, Format::Depth32FloatStencil8),
];

/// The depth-stencil format whose stencil a view of `format` reads, for the stencil views.
pub fn stencil_view(format: pipe_format) -> Option<Format> {
    STENCIL_VIEWS
        .iter()
        .find(|&&(gallium, _)| gallium == format)
        .map(|&(_, ours)| ours)
}

pub fn format(format: pipe_format) -> Option<Format> {
    FORMATS
        .iter()
        .find(|(gallium, _)| *gallium == format)
        .map(|(_, ours)| *ours)
}

/// Every Gallium format a vertex attribute may be stored in that the interface reads.
const VERTEX_FORMATS: &[(pipe_format, VertexFormat)] = &[
    (PIPE_FORMAT_R8_UINT, VertexFormat::Uint8x1),
    (PIPE_FORMAT_R8G8_UINT, VertexFormat::Uint8x2),
    (PIPE_FORMAT_R8G8B8_UINT, VertexFormat::Uint8x3),
    (PIPE_FORMAT_R8G8B8A8_UINT, VertexFormat::Uint8x4),
    (PIPE_FORMAT_R8_SINT, VertexFormat::Sint8x1),
    (PIPE_FORMAT_R8G8_SINT, VertexFormat::Sint8x2),
    (PIPE_FORMAT_R8G8B8_SINT, VertexFormat::Sint8x3),
    (PIPE_FORMAT_R8G8B8A8_SINT, VertexFormat::Sint8x4),
    (PIPE_FORMAT_R8_UNORM, VertexFormat::Unorm8x1),
    (PIPE_FORMAT_R8G8_UNORM, VertexFormat::Unorm8x2),
    (PIPE_FORMAT_R8G8B8_UNORM, VertexFormat::Unorm8x3),
    (PIPE_FORMAT_R8G8B8A8_UNORM, VertexFormat::Unorm8x4),
    (PIPE_FORMAT_R8_SNORM, VertexFormat::Snorm8x1),
    (PIPE_FORMAT_R8G8_SNORM, VertexFormat::Snorm8x2),
    (PIPE_FORMAT_R8G8B8_SNORM, VertexFormat::Snorm8x3),
    (PIPE_FORMAT_R8G8B8A8_SNORM, VertexFormat::Snorm8x4),
    (PIPE_FORMAT_R16_UINT, VertexFormat::Uint16x1),
    (PIPE_FORMAT_R16G16_UINT, VertexFormat::Uint16x2),
    (PIPE_FORMAT_R16G16B16_UINT, VertexFormat::Uint16x3),
    (PIPE_FORMAT_R16G16B16A16_UINT, VertexFormat::Uint16x4),
    (PIPE_FORMAT_R16_SINT, VertexFormat::Sint16x1),
    (PIPE_FORMAT_R16G16_SINT, VertexFormat::Sint16x2),
    (PIPE_FORMAT_R16G16B16_SINT, VertexFormat::Sint16x3),
    (PIPE_FORMAT_R16G16B16A16_SINT, VertexFormat::Sint16x4),
    (PIPE_FORMAT_R16_UNORM, VertexFormat::Unorm16x1),
    (PIPE_FORMAT_R16G16_UNORM, VertexFormat::Unorm16x2),
    (PIPE_FORMAT_R16G16B16_UNORM, VertexFormat::Unorm16x3),
    (PIPE_FORMAT_R16G16B16A16_UNORM, VertexFormat::Unorm16x4),
    (PIPE_FORMAT_R16_SNORM, VertexFormat::Snorm16x1),
    (PIPE_FORMAT_R16G16_SNORM, VertexFormat::Snorm16x2),
    (PIPE_FORMAT_R16G16B16_SNORM, VertexFormat::Snorm16x3),
    (PIPE_FORMAT_R16G16B16A16_SNORM, VertexFormat::Snorm16x4),
    (PIPE_FORMAT_R16_FLOAT, VertexFormat::Float16x1),
    (PIPE_FORMAT_R16G16_FLOAT, VertexFormat::Float16x2),
    (PIPE_FORMAT_R16G16B16_FLOAT, VertexFormat::Float16x3),
    (PIPE_FORMAT_R16G16B16A16_FLOAT, VertexFormat::Float16x4),
    (PIPE_FORMAT_R32_FLOAT, VertexFormat::Float32x1),
    (PIPE_FORMAT_R32G32_FLOAT, VertexFormat::Float32x2),
    (PIPE_FORMAT_R32G32B32_FLOAT, VertexFormat::Float32x3),
    (PIPE_FORMAT_R32G32B32A32_FLOAT, VertexFormat::Float32x4),
    (PIPE_FORMAT_R32_UINT, VertexFormat::Uint32x1),
    (PIPE_FORMAT_R32G32_UINT, VertexFormat::Uint32x2),
    (PIPE_FORMAT_R32G32B32_UINT, VertexFormat::Uint32x3),
    (PIPE_FORMAT_R32G32B32A32_UINT, VertexFormat::Uint32x4),
    (PIPE_FORMAT_R32_SINT, VertexFormat::Sint32x1),
    (PIPE_FORMAT_R32G32_SINT, VertexFormat::Sint32x2),
    (PIPE_FORMAT_R32G32B32_SINT, VertexFormat::Sint32x3),
    (PIPE_FORMAT_R32G32B32A32_SINT, VertexFormat::Sint32x4),
    (PIPE_FORMAT_B8G8R8A8_UNORM, VertexFormat::Bgra8Unorm),
    (PIPE_FORMAT_R10G10B10A2_UNORM, VertexFormat::Unorm10_10_10_2),
    (PIPE_FORMAT_R10G10B10A2_UINT, VertexFormat::Uint10_10_10_2),
    (PIPE_FORMAT_R11G11B10_FLOAT, VertexFormat::Float11_11_10),
];

/// The interface's vertex format of a Gallium format, if it reads it.
pub fn vertex_format(format: pipe_format) -> Option<VertexFormat> {
    VERTEX_FORMATS
        .iter()
        .find(|(gallium, _)| *gallium == format)
        .map(|(_, ours)| *ours)
}

/// What each Gallium binding needs of a format; a binding not listed is not served.
const BINDINGS: &[(u32, FormatUse)] = &[
    (PIPE_BIND_RENDER_TARGET, FormatUse::RENDER),
    (PIPE_BIND_BLENDABLE, FormatUse::BLEND),
    (PIPE_BIND_DEPTH_STENCIL, FormatUse::DEPTH_STENCIL),
    (PIPE_BIND_SAMPLER_VIEW, FormatUse::SAMPLE),
    (PIPE_BIND_VERTEX_BUFFER, FormatUse::VERTEX),
    // Images are written by programs; one read too needs the format's storage reads, which the
    // bind checks.
    (PIPE_BIND_SHADER_IMAGE, FormatUse::STORAGE_WRITE),
    // A frame is shown by copying its picture into the window's surface.
    (
        PIPE_BIND_DISPLAY_TARGET,
        FormatUse::RENDER.union(FormatUse::PRESENT),
    ),
];

/// Whether a resource of `format` in `target` with `samples` samples can serve every one of
/// `bindings` (`pipe_screen::is_format_supported`).
pub fn supported(
    caps: &FormatCaps,
    format: pipe_format,
    target: pipe_texture_target,
    samples: u32,
    storage_samples: u32,
    bindings: u32,
) -> bool {
    if target == PIPE_BUFFER {
        // Vertex streams of the formats the interface reads; texel buffers of the formats the
        // device samples, read through 2D textures of the format (`TexelBufferAsTexture`).
        let plain = PIPE_BIND_VERTEX_BUFFER
            | PIPE_BIND_INDEX_BUFFER
            | PIPE_BIND_CONSTANT_BUFFER
            | PIPE_BIND_STREAM_OUTPUT
            | PIPE_BIND_SHADER_BUFFER
            | PIPE_BIND_COMMAND_ARGS_BUFFER
            | PIPE_BIND_SAMPLER_VIEW;
        let texel = bindings & PIPE_BIND_SAMPLER_VIEW == 0
            || self::format(format).is_some_and(|f| {
                matches!(f.block(), (1, 1, _))
                    && !f.is_depth_stencil()
                    && caps.get(f).contains(FormatUse::SAMPLE)
            });
        // Images of buffers, read and written as storage words by the program
        // (`TexelBufferAsStorage`).
        let image = bindings & PIPE_BIND_SHADER_IMAGE == 0
            || self::format(format)
                .is_some_and(|f| matches!(f.block(), (1, 1, _)) && !f.is_depth_stencil());
        return bindings & !(plain | PIPE_BIND_SHADER_IMAGE) == 0
            && texel
            && image
            && (bindings & PIPE_BIND_VERTEX_BUFFER == 0
                || vertex_format(format).is_some()
                || crate::packed::packed_format(format).is_some());
    }
    let sampled = samples.max(1);
    // A framebuffer without attachments: the pass tracker's empty pass, of the samples its own
    // attachment takes.
    if format == PIPE_FORMAT_NONE {
        return bindings == PIPE_BIND_RENDER_TARGET
            && (sampled == 1 || caps.takes_samples(Format::R8Unorm, sampled));
    }
    if let Some(depth_stencil) = stencil_view(format) {
        return bindings == PIPE_BIND_SAMPLER_VIEW
            && sampled == 1
            && caps.get(depth_stencil).contains(FormatUse::SAMPLE);
    }
    let Some(ours) = self::format(format) else {
        return false;
    };
    let mut needed = FormatUse::empty();
    let mut left = bindings;
    for (binding, uses) in BINDINGS {
        if bindings & binding != 0 {
            needed |= *uses;
            left &= !binding;
        }
    }
    let (_, block_height, _) = ours.block();
    let dimension = match target {
        PIPE_TEXTURE_1D | PIPE_TEXTURE_1D_ARRAY => block_height == 1,
        PIPE_TEXTURE_3D => !ours.is_depth_stencil(),
        PIPE_TEXTURE_2D
        | PIPE_TEXTURE_2D_ARRAY
        | PIPE_TEXTURE_RECT
        | PIPE_TEXTURE_CUBE
        | PIPE_TEXTURE_CUBE_ARRAY => true,
        _ => false,
    };
    let storage = bindings & PIPE_BIND_SHADER_IMAGE == 0 || sampled == 1;
    // The host's multisampled textures are single 2D layers.
    let layers = sampled == 1 || target == PIPE_TEXTURE_2D;
    left == 0
        && storage
        && layers
        && dimension
        && caps.get(ours).contains(needed)
        && sampled == storage_samples.max(1)
        && (sampled == 1 || caps.takes_samples(ours, sampled))
}
