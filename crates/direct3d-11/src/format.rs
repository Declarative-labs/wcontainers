//! DXGI formats (their numbers in `dxgiformat.h`) as the interface's formats.

use wcontainers_gpu::{FORMAT_FACTS, Format, FormatKind, IndexFormat, ScalarKind, VertexFormat};

use crate::desc::level;
use crate::error::{Error, Result, not_implemented};
use wcontainers_directx_graphics_infrastructure::{BACK_BUFFER_FORMATS, dxgi, dxgi_vertex_format};

/// Whether a swap chain of a device at feature level `level` takes back buffers of DXGI format
/// `dxgi` (`D3D11_FORMAT_SUPPORT_DISPLAY`).
pub fn displayed(dxgi: u32, level: u32) -> bool {
    BACK_BUFFER_FORMATS.contains(&dxgi)
        && (level >= level::L10_0 || level9_support(dxgi, level) & support::DISPLAY != 0)
}

use crate::desc::support;
use crate::desc::support::{
    BACK_BUFFER_CAST, BLENDABLE, CPU_LOCKABLE, DEPTH_STENCIL, DISPLAY, IA_INDEX_BUFFER,
    IA_VERTEX_BUFFER, MIP, MIP_AUTOGEN, MULTISAMPLE_RENDERTARGET, MULTISAMPLE_RESOLVE,
    RENDER_TARGET, SHADER_LOAD, SHADER_SAMPLE, TEXTURE2D, TEXTURE3D, TEXTURECUBE,
};

/// Masks of the 10Level9 table below: 2D textures made and mapped only.
const HELD: u32 = TEXTURE2D | CPU_LOCKABLE;
/// 2D textures sampled, with mips, of one face or six.
const SAMPLED: u32 = TEXTURE2D | TEXTURECUBE | SHADER_LOAD | SHADER_SAMPLE | MIP | CPU_LOCKABLE;
/// 2D textures sampled and drawn into, blended, of one level.
const FLAT_TARGET: u32 =
    TEXTURE2D | SHADER_LOAD | SHADER_SAMPLE | RENDER_TARGET | BLENDABLE | CPU_LOCKABLE;
/// Textures of every 10Level9 kind drawn into, multisampled, not filtered or blended.
const UNFILTERED_TARGET: u32 = TEXTURE2D
    | TEXTURE3D
    | TEXTURECUBE
    | SHADER_LOAD
    | MIP
    | MIP_AUTOGEN
    | RENDER_TARGET
    | CPU_LOCKABLE
    | MULTISAMPLE_RESOLVE
    | MULTISAMPLE_RENDERTARGET;
const TARGET: u32 = UNFILTERED_TARGET | SHADER_SAMPLE | BLENDABLE;
const SHOWN: u32 = TARGET | DISPLAY | BACK_BUFFER_CAST;
const DEPTH: u32 = TEXTURE2D | DEPTH_STENCIL | MULTISAMPLE_RENDERTARGET;

/// What a device of feature level 9_1, 9_2 or 9_3 may report of each DXGI format in the first word
/// of `CheckFormatSupport`, as the runtime answers on the RTX 3080 (measured): Microsoft's 10Level9
/// tables ("Format support for Direct3D Feature 10Level9 9.1 hardware", "9.3 hardware") with the
/// optional rows the driver gives; Microsoft has no 9_2 table. Formats not listed have none.
const LEVEL9_SUPPORT: [(u32, [u32; 3]); 51] = [
    (
        dxgi::R32G32B32A32_FLOAT,
        [
            IA_VERTEX_BUFFER,
            IA_VERTEX_BUFFER
                | TEXTURE2D
                | SHADER_LOAD
                | RENDER_TARGET
                | CPU_LOCKABLE
                | MULTISAMPLE_RESOLVE
                | MULTISAMPLE_RENDERTARGET,
            IA_VERTEX_BUFFER | UNFILTERED_TARGET,
        ],
    ),
    (dxgi::R32G32B32_FLOAT, [IA_VERTEX_BUFFER; 3]),
    (
        dxgi::R16G16B16A16_FLOAT,
        [0, UNFILTERED_TARGET, IA_VERTEX_BUFFER | TARGET],
    ),
    (
        dxgi::R16G16B16A16_UNORM,
        [
            0,
            UNFILTERED_TARGET | SHADER_SAMPLE,
            UNFILTERED_TARGET | SHADER_SAMPLE,
        ],
    ),
    (dxgi::R16G16B16A16_SNORM, [IA_VERTEX_BUFFER; 3]),
    (dxgi::R16G16B16A16_SINT, [IA_VERTEX_BUFFER; 3]),
    (
        dxgi::R32G32_FLOAT,
        [
            IA_VERTEX_BUFFER,
            IA_VERTEX_BUFFER,
            IA_VERTEX_BUFFER | UNFILTERED_TARGET & !(MIP | MIP_AUTOGEN),
        ],
    ),
    (dxgi::R8G8B8A8_UNORM, [IA_VERTEX_BUFFER | SHOWN; 3]),
    (dxgi::R8G8B8A8_UNORM_SRGB, [SHOWN; 3]),
    (dxgi::R8G8B8A8_UINT, [IA_VERTEX_BUFFER; 3]),
    (dxgi::R8G8B8A8_SNORM, [SAMPLED; 3]),
    (
        dxgi::R16G16_FLOAT,
        [0, UNFILTERED_TARGET, IA_VERTEX_BUFFER | UNFILTERED_TARGET],
    ),
    (
        dxgi::R16G16_UNORM,
        [
            0,
            UNFILTERED_TARGET | SHADER_SAMPLE,
            UNFILTERED_TARGET | SHADER_SAMPLE,
        ],
    ),
    (
        dxgi::R16G16_SNORM,
        [
            IA_VERTEX_BUFFER | SAMPLED & !(TEXTURECUBE | SHADER_SAMPLE),
            IA_VERTEX_BUFFER | SAMPLED | TEXTURE3D,
            IA_VERTEX_BUFFER | SAMPLED | TEXTURE3D,
        ],
    ),
    (dxgi::R16G16_SINT, [IA_VERTEX_BUFFER; 3]),
    (
        dxgi::R32_FLOAT,
        [
            IA_VERTEX_BUFFER,
            IA_VERTEX_BUFFER | UNFILTERED_TARGET,
            IA_VERTEX_BUFFER | UNFILTERED_TARGET,
        ],
    ),
    (dxgi::R32_UINT, [0, IA_INDEX_BUFFER, IA_INDEX_BUFFER]),
    (dxgi::D24_UNORM_S8_UINT, [DEPTH; 3]),
    (dxgi::R8G8_UNORM, [FLAT_TARGET; 3]),
    (dxgi::R8G8_SNORM, [SAMPLED & !TEXTURECUBE; 3]),
    (dxgi::D16_UNORM, [DEPTH; 3]),
    (
        dxgi::R16_UNORM,
        [0, SAMPLED | TEXTURE3D, SAMPLED | TEXTURE3D],
    ),
    (dxgi::R16_UINT, [IA_INDEX_BUFFER; 3]),
    (
        dxgi::R8_UNORM,
        [SAMPLED | TEXTURE3D | RENDER_TARGET | BLENDABLE; 3],
    ),
    (
        dxgi::A8_UNORM,
        [
            FLAT_TARGET,
            FLAT_TARGET | TEXTURE3D | TEXTURECUBE,
            FLAT_TARGET | TEXTURE3D | TEXTURECUBE,
        ],
    ),
    (dxgi::BC1_UNORM, [SAMPLED; 3]),
    (dxgi::BC1_UNORM_SRGB, [SAMPLED; 3]),
    (dxgi::BC2_UNORM, [SAMPLED; 3]),
    (dxgi::BC2_UNORM_SRGB, [SAMPLED; 3]),
    (dxgi::BC3_UNORM, [SAMPLED; 3]),
    (dxgi::BC3_UNORM_SRGB, [SAMPLED; 3]),
    (dxgi::B5G6R5_UNORM, [TARGET & !TEXTURE3D; 3]),
    (dxgi::B5G5R5A1_UNORM, [SAMPLED; 3]),
    (dxgi::B8G8R8A8_UNORM, [SHOWN; 3]),
    (dxgi::B8G8R8X8_UNORM, [TARGET; 3]),
    (dxgi::B8G8R8A8_UNORM_SRGB, [SHOWN; 3]),
    (dxgi::B8G8R8X8_UNORM_SRGB, [TARGET; 3]),
    (dxgi::AYUV, [HELD; 3]),
    (dxgi::Y410, [HELD; 3]),
    (dxgi::Y416, [HELD; 3]),
    (dxgi::NV12, [HELD, HELD, FLAT_TARGET]),
    (dxgi::P010, [HELD; 3]),
    (dxgi::OPAQUE_420, [TEXTURE2D; 3]),
    (dxgi::YUY2, [HELD; 3]),
    (dxgi::Y210, [HELD; 3]),
    (dxgi::Y216, [HELD; 3]),
    (dxgi::AI44, [HELD; 3]),
    (dxgi::IA44, [HELD; 3]),
    (dxgi::P8, [HELD; 3]),
    (dxgi::A8P8, [HELD; 3]),
    (dxgi::B4G4R4A4_UNORM, [SAMPLED; 3]),
];

/// The formats a 10Level9 device reports shareable (`D3D11_FORMAT_SUPPORT2_SHAREABLE`), measured
/// on the RTX 3080 at 9_1 to 9_3 alike.
const LEVEL9_SHAREABLE: [u32; 17] = [
    dxgi::R16G16B16A16_FLOAT,
    dxgi::R10G10B10A2_UNORM,
    dxgi::R8G8B8A8_UNORM,
    dxgi::R8G8B8A8_UNORM_SRGB,
    dxgi::R8G8_UNORM,
    dxgi::R8_UNORM,
    dxgi::A8_UNORM,
    dxgi::BC1_UNORM,
    dxgi::BC1_UNORM_SRGB,
    dxgi::BC2_UNORM,
    dxgi::BC2_UNORM_SRGB,
    dxgi::BC3_UNORM,
    dxgi::BC3_UNORM_SRGB,
    dxgi::B8G8R8A8_UNORM,
    dxgi::B8G8R8X8_UNORM,
    dxgi::B8G8R8A8_UNORM_SRGB,
    dxgi::B8G8R8X8_UNORM_SRGB,
];

/// The first word of `CheckFormatSupport` a device of feature level `level` below 10_0 may report
/// for DXGI format `dxgi`.
pub fn level9_support(dxgi: u32, level: u32) -> u32 {
    let column = match level {
        level::L9_1 => 0,
        level::L9_2 => 1,
        _ => 2,
    };
    LEVEL9_SUPPORT
        .iter()
        .find(|row| row.0 == dxgi)
        .map_or(0, |row| row.1[column])
}

/// Whether a device below 10_0 reports DXGI format `dxgi` shareable.
pub fn level9_shareable(dxgi: u32) -> bool {
    LEVEL9_SHAREABLE.contains(&dxgi)
}

/// Whether a device below 10_0 makes shared textures of DXGI format `dxgi`, which is not what it
/// reports (measured on the RTX 3080): `R32_FLOAT` is made though not reported shareable, and
/// `R8G8B8A8_UNORM` and its sRGB twin refused though reported.
pub fn level9_shared(dxgi: u32) -> bool {
    (level9_shareable(dxgi) || dxgi == dxgi::R32_FLOAT)
        && !matches!(dxgi, dxgi::R8G8B8A8_UNORM | dxgi::R8G8B8A8_UNORM_SRGB)
}

/// Whether `format` is signed normalised, which Direct3D blends only from feature level 10_1.
pub fn signed_normalized(format: Format) -> bool {
    matches!(
        format,
        Format::R8Snorm
            | Format::Rg8Snorm
            | Format::Rgba8Snorm
            | Format::R16Snorm
            | Format::Rg16Snorm
            | Format::Rgba16Snorm
    )
}

/// The DXGI formats the front names elsewhere. NV12 (a plane of 8-bit luma, then a plane of
/// interleaved chroma pairs at half the width and height) is held as two textures
/// (`Texture::chroma`), its planes viewed through the R8_TYPELESS and R8G8_TYPELESS groups; YUY2
/// (a pair of pixels in four bytes, Y0 U Y1 V) as an `rgba8unorm` texture of half the width,
/// viewed through `R8G8B8A8_UNORM` and `_UINT`.
pub use dxgi::{
    NV12, R8_TYPELESS, R8G8_TYPELESS, R16_UINT, R32_FLOAT, R32_SINT, R32_TYPELESS, R32_UINT,
    UNKNOWN, YUY2,
};

/// Whether stream output buffers take elements of DXGI format `dxgi` (`D3D11_FORMAT_SUPPORT_SO_BUFFER`):
/// one to four 32-bit components.
pub fn stream_output_format(dxgi: u32) -> bool {
    matches!(
        dxgi,
        dxgi::R32G32B32A32_FLOAT..=dxgi::R32G32B32A32_SINT
            | dxgi::R32G32B32_FLOAT..=dxgi::R32G32B32_SINT
            | dxgi::R32G32_FLOAT..=dxgi::R32G32_SINT
            | dxgi::R32_FLOAT..=dxgi::R32_SINT
    )
}

/// Whether Direct3D takes typed storage views of DXGI format `dxgi`: not of sRGB, depth,
/// shared-exponent, two-pixel, block-compressed, three-component, 16-bit packed or
/// `B8G8R8X8_UNORM` formats.
pub fn storage_viewed(dxgi: u32) -> bool {
    !never_target_or_storage(dxgi)
        && !is_srgb(dxgi)
        && !matches!(
            dxgi,
            dxgi::R32G32B32_TYPELESS
                ..=dxgi::R32G32B32_SINT
                    | dxgi::D32_FLOAT
                    | dxgi::D16_UNORM
                    | dxgi::B5G6R5_UNORM
                    | dxgi::B5G5R5A1_UNORM
                    | dxgi::B4G4R4A4_UNORM
                    | dxgi::B8G8R8X8_UNORM
        )
}

/// Whether programs load and store storage textures of `format` in one program: WebGPU's
/// read-write storage textures are of its one-channel 32-bit formats.
pub fn loaded_and_stored(format: Format) -> bool {
    matches!(format, Format::R32Float | Format::R32Uint | Format::R32Sint)
}

/// The formats Microsoft's typed storage loads page lists as supported as a set, which
/// `D3D11_OPTIONS2.TypedUAVLoadAdditionalFormats` answers for.
pub const TYPED_LOADS_AS_A_SET: [u32; 15] = [
    dxgi::R32G32B32A32_FLOAT,
    dxgi::R32G32B32A32_UINT,
    dxgi::R32G32B32A32_SINT,
    dxgi::R16G16B16A16_FLOAT,
    dxgi::R16G16B16A16_UINT,
    dxgi::R16G16B16A16_SINT,
    dxgi::R8G8B8A8_UNORM,
    dxgi::R8G8B8A8_UINT,
    dxgi::R8G8B8A8_SINT,
    dxgi::R16_FLOAT,
    dxgi::R16_UINT,
    dxgi::R16_SINT,
    dxgi::R8_UNORM,
    dxgi::R8_UINT,
    dxgi::R8_SINT,
];

/// Whether DXGI format `dxgi` is one of BC6H's and BC7's, which come with feature level 11_0.
pub fn from_11_0(dxgi: u32) -> bool {
    matches!(dxgi, dxgi::BC6H_TYPELESS..=dxgi::BC7_UNORM_SRGB)
}

/// Whether Direct3D takes typed buffer views of DXGI format `dxgi` among the formats of one
/// texel: not of sRGB, alpha-only, shared-exponent or 16-bit packed formats.
pub fn buffer_viewed(dxgi: u32) -> bool {
    !is_srgb(dxgi)
        && !matches!(
            dxgi,
            dxgi::A8_UNORM
                | dxgi::R9G9B9E5_SHAREDEXP
                | dxgi::B5G6R5_UNORM
                | dxgi::B5G5R5A1_UNORM
                | dxgi::B4G4R4A4_UNORM
        )
}

/// Whether `format` holds integers, which are neither filtered nor resolved.
pub fn is_integer(format: Format) -> bool {
    matches!(
        FORMAT_FACTS[format as usize].kind,
        FormatKind::Uint | FormatKind::Sint
    )
}

/// The plane of an NV12 texture a view of DXGI format `view` names (0 luma, 1 chroma), or none
/// for a format no view of NV12 takes.
pub fn nv12_plane(view: u32) -> Option<usize> {
    match view {
        dxgi::R8_UNORM | dxgi::R8_UINT => Some(0),
        dxgi::R8G8_UNORM | dxgi::R8G8_UINT => Some(1),
        _ => None,
    }
}

/// Whether textures of DXGI format `dxgi` are a plane of 8-bit luma and one of chroma pairs at
/// half size: NV12, and 420_OPAQUE, whose layout is the driver's and which is held as NV12.
pub fn two_planes(dxgi: u32) -> bool {
    matches!(dxgi, NV12 | dxgi::OPAQUE_420)
}

/// Bytes of a row and rows of a subresource of DXGI format `dxgi` stored as `format`, as the
/// program lays it out: NV12's chroma rows follow its luma rows at the same pitch.
pub fn subresource_rows(dxgi: u32, format: Format, width: u32, height: u32) -> (u32, u32) {
    let (row, rows) = row_bytes(format, width, height);
    if two_planes(dxgi) {
        (row, rows + rows / 2)
    } else {
        (row, rows)
    }
}

/// The interface format of a DXGI format a texture or view names: `InvalidArg` for a number that is
/// no DXGI format, `NotImplemented` for a DXGI format this front does not serve (typeless formats,
/// among others).
pub fn texture_format_of(dxgi: u32) -> Result<Format> {
    texture_format(dxgi).map_or_else(|| unserved(dxgi), Ok)
}

/// Not served for a defined DXGI format, invalid for others.
/// `DXGI_FORMAT_SAMPLER_FEEDBACK_MIN_MIP_OPAQUE` to `DXGI_FORMAT_A4B4G4R4_UNORM`: the formats
/// defined after `V408`, which the infrastructure crate's table does not name.
const PAST_V408: std::ops::RangeInclusive<u32> = 189..=191;
const UNSERVED_FORMAT: &str = "a DXGI format this front does not serve";

fn unserved<T>(dxgi: u32) -> Result<T> {
    if (dxgi::R32G32B32A32_TYPELESS..=dxgi::V408).contains(&dxgi) || PAST_V408.contains(&dxgi) {
        not_implemented(UNSERVED_FORMAT)
    } else {
        Err(Error::InvalidArg("unknown format"))
    }
}

/// The interface format a texture of DXGI format `dxgi` is stored as. A typeless texture is stored
/// as a member of its group: the depth format where it is bound for depth (or the group holds only
/// depth with stencil), otherwise a colour format whose views as itself and its sRGB twin need no
/// reinterpreting.
pub fn storage_format(dxgi: u32, depth_bound: bool) -> Result<Format> {
    use Format::*;
    let typeless = match (dxgi, depth_bound) {
        (dxgi::R32G32B32A32_TYPELESS, _) => Rgba32Float,
        (dxgi::R32G32B32_TYPELESS, _) => Rgb32Float,
        (dxgi::R16G16B16A16_TYPELESS, _) => Rgba16Float,
        (dxgi::R32G32_TYPELESS, _) => Rg32Float,
        // The depth and stencil aspects' view formats name their group's textures too.
        (
            dxgi::R32G8X24_TYPELESS
            | dxgi::R32_FLOAT_X8X24_TYPELESS
            | dxgi::X32_TYPELESS_G8X24_UINT,
            _,
        ) => Depth32FloatStencil8,
        (dxgi::R10G10B10A2_TYPELESS, _) => Rgb10a2Unorm,
        (dxgi::R8G8B8A8_TYPELESS, _) => Rgba8Unorm,
        (dxgi::R16G16_TYPELESS, _) => Rg16Float,
        (dxgi::R32_TYPELESS, true) => Depth32Float,
        (dxgi::R32_TYPELESS, false) => R32Float,
        (dxgi::R24G8_TYPELESS | dxgi::R24_UNORM_X8_TYPELESS | dxgi::X24_TYPELESS_G8_UINT, _) => {
            Depth24PlusStencil8
        }
        (dxgi::R8G8_TYPELESS, _) => Rg8Unorm,
        (dxgi::R16_TYPELESS, true) => Depth16Unorm,
        (dxgi::R16_TYPELESS, false) => R16Float,
        (dxgi::R8_TYPELESS, _) => R8Unorm,
        (dxgi::BC1_TYPELESS, _) => Bc1RgbaUnorm,
        (dxgi::BC2_TYPELESS, _) => Bc2RgbaUnorm,
        (dxgi::BC3_TYPELESS, _) => Bc3RgbaUnorm,
        (dxgi::BC4_TYPELESS, _) => Bc4RUnorm,
        (dxgi::BC5_TYPELESS, _) => Bc5RgUnorm,
        (dxgi::B8G8R8A8_TYPELESS, _) => Bgra8Unorm,
        (dxgi::B8G8R8X8_TYPELESS, _) => Bgrx8Unorm,
        (dxgi::BC6H_TYPELESS, _) => Bc6hRgbUfloat,
        (dxgi::BC7_TYPELESS, _) => Bc7RgbaUnorm,
        // Its bits: such textures are never bound, so nothing reads them as values.
        (dxgi::R10G10B10_XR_BIAS_A2_UNORM, _) => Rgb10a2Unorm,
        // The luma plane; the chroma plane is a texture of its own.
        (NV12 | dxgi::OPAQUE_420, _) => R8Unorm,
        (YUY2, _) => Rgba8Unorm,
        _ => return texture_format_of(dxgi),
    };
    Ok(typeless)
}

/// The interface format and aspect (true for stencil) of a view of DXGI format `view` of a texture
/// stored as `storage` with DXGI format `texture`. A typed texture is viewed as its own format; a
/// typeless one as any member of its group, and Direct3D's depth views (`R24_UNORM_X8_TYPELESS`,
/// `X24_TYPELESS_G8_UINT`, `R32_FLOAT` of `R32_TYPELESS` and the like) read one aspect of the depth
/// format. A member whose bits another format of the group stores is viewed through a texture of
/// its own format kept beside the texture (`ReinterpretCopy`, see `reinterprets`).
pub fn view_format(view: u32, texture: u32, storage: Format) -> Result<(Format, bool)> {
    use Format::*;
    if is_typeless(view) {
        return Err(Error::InvalidArg("a typeless view format"));
    }
    if texture == YUY2 {
        return match view {
            dxgi::R8G8B8A8_UNORM => Ok((Rgba8Unorm, false)),
            dxgi::R8G8B8A8_UINT => Ok((Rgba8Uint, false)),
            _ => Err(Error::InvalidArg(
                "a view of a YUY2 texture is R8G8B8A8_UNORM or _UINT",
            )),
        };
    }
    if view == dxgi::R10G10B10_XR_BIAS_A2_UNORM {
        return Err(Error::InvalidArg(
            "a view of the extended-range format, which Direct3D never binds",
        ));
    }
    if view != texture && typeless_group(view) != texture {
        return Err(Error::InvalidArg(
            "a view format outside the texture's own or its typeless group",
        ));
    }
    let aspect = match (storage, view) {
        (Depth32FloatStencil8, dxgi::D32_FLOAT_S8X24_UINT | dxgi::R32_FLOAT_X8X24_TYPELESS) => {
            Some(false)
        }
        (Depth32FloatStencil8, dxgi::X32_TYPELESS_G8X24_UINT) => Some(true),
        (Depth32Float, dxgi::D32_FLOAT | dxgi::R32_FLOAT)
        | (Depth16Unorm, dxgi::D16_UNORM | dxgi::R16_UNORM) => Some(false),
        (Depth24PlusStencil8, dxgi::D24_UNORM_S8_UINT | dxgi::R24_UNORM_X8_TYPELESS) => Some(false),
        (Depth24PlusStencil8, dxgi::X24_TYPELESS_G8_UINT) => Some(true),
        _ => None,
    };
    if let Some(stencil) = aspect {
        return Ok((storage, stencil));
    }
    let format = texture_format(view).map_or_else(|| unserved(view), Ok)?;
    Ok((format, false))
}

/// Whether a view of `format` of a texture stored as `storage` reads other bits than the storage
/// format's, which no back end's view does (`ReinterpretCopy`).
pub fn reinterprets(format: Format, storage: Format) -> bool {
    FORMAT_FACTS[format as usize].linear != FORMAT_FACTS[storage as usize].linear
}

/// The sRGB format of a linear one, or the linear one of an sRGB one, where the pair exists.
pub fn srgb_twin(dxgi: u32) -> Option<u32> {
    match dxgi {
        dxgi::R8G8B8A8_UNORM => Some(dxgi::R8G8B8A8_UNORM_SRGB),
        dxgi::R8G8B8A8_UNORM_SRGB => Some(dxgi::R8G8B8A8_UNORM),
        dxgi::B8G8R8A8_UNORM => Some(dxgi::B8G8R8A8_UNORM_SRGB),
        dxgi::B8G8R8A8_UNORM_SRGB => Some(dxgi::B8G8R8A8_UNORM),
        dxgi::B8G8R8X8_UNORM => Some(dxgi::B8G8R8X8_UNORM_SRGB),
        dxgi::B8G8R8X8_UNORM_SRGB => Some(dxgi::B8G8R8X8_UNORM),
        _ => None,
    }
}

/// Whether `dxgi` is the typeless format heading a group.
pub fn is_typeless(dxgi: u32) -> bool {
    matches!(
        dxgi,
        dxgi::R32G32B32A32_TYPELESS
            | dxgi::R32G32B32_TYPELESS
            | dxgi::R16G16B16A16_TYPELESS
            | dxgi::R32G32_TYPELESS
            | dxgi::R32G8X24_TYPELESS
            | dxgi::R10G10B10A2_TYPELESS
            | dxgi::R8G8B8A8_TYPELESS
            | dxgi::R16G16_TYPELESS
            | dxgi::R32_TYPELESS
            | dxgi::R24G8_TYPELESS
            | dxgi::R8G8_TYPELESS
            | dxgi::R16_TYPELESS
            | dxgi::R8_TYPELESS
            | dxgi::BC1_TYPELESS
            | dxgi::BC2_TYPELESS
            | dxgi::BC3_TYPELESS
            | dxgi::BC4_TYPELESS
            | dxgi::BC5_TYPELESS
            | dxgi::B8G8R8A8_TYPELESS
            | dxgi::B8G8R8X8_TYPELESS
            | dxgi::BC6H_TYPELESS
            | dxgi::BC7_TYPELESS
    )
}

/// Whether Direct3D never binds a texture of `dxgi` as a render target or unordered access view,
/// whatever the device: the depth-stencil groups with stencil hold no colour target or storage,
/// and shared-exponent, two-pixel and block-compressed formats are only read.
pub fn never_target_or_storage(dxgi: u32) -> bool {
    matches!(dxgi, dxgi::R32G8X24_TYPELESS..=dxgi::X32_TYPELESS_G8X24_UINT | dxgi::R24G8_TYPELESS..=dxgi::X24_TYPELESS_G8_UINT | dxgi::R9G9B9E5_SHAREDEXP..=dxgi::BC5_SNORM | dxgi::BC6H_TYPELESS..=dxgi::BC7_UNORM_SRGB)
}

/// Whether `dxgi` is an sRGB format: never viewed for unordered access, so a texture of one bound
/// for it holds no storage.
pub fn is_srgb(dxgi: u32) -> bool {
    matches!(
        dxgi,
        dxgi::R8G8B8A8_UNORM_SRGB
            | dxgi::BC1_UNORM_SRGB
            | dxgi::BC2_UNORM_SRGB
            | dxgi::BC3_UNORM_SRGB
            | dxgi::B8G8R8A8_UNORM_SRGB
            | dxgi::B8G8R8X8_UNORM_SRGB
            | dxgi::BC7_UNORM_SRGB
    )
}

/// Whether `dxgi` is a typed depth format (`D32_FLOAT_S8X24_UINT`, `D32_FLOAT`,
/// `D24_UNORM_S8_UINT`, `D16_UNORM`): such a texture is bound only for depth, and read through a
/// typeless one.
pub fn depth_only(dxgi: u32) -> bool {
    matches!(
        dxgi,
        dxgi::D32_FLOAT_S8X24_UINT | dxgi::D32_FLOAT | dxgi::D24_UNORM_S8_UINT | dxgi::D16_UNORM
    )
}

/// Whether `dxgi` is a view format reading one aspect of a depth-stencil texture
/// (`R32_FLOAT_X8X24_TYPELESS`, `X32_TYPELESS_G8X24_UINT`, `R24_UNORM_X8_TYPELESS`,
/// `X24_TYPELESS_G8_UINT`).
pub fn depth_aspect_view(dxgi: u32) -> bool {
    matches!(
        dxgi,
        dxgi::R32_FLOAT_X8X24_TYPELESS
            | dxgi::X32_TYPELESS_G8X24_UINT
            | dxgi::R24_UNORM_X8_TYPELESS
            | dxgi::X24_TYPELESS_G8_UINT
    )
}

/// The interface format a DXGI format is stored as, for the typed formats this front serves.
pub fn texture_format(dxgi: u32) -> Option<Format> {
    use Format::*;
    Some(match dxgi {
        dxgi::R32G32B32A32_FLOAT => Rgba32Float,
        dxgi::R32G32B32A32_UINT => Rgba32Uint,
        dxgi::R32G32B32A32_SINT => Rgba32Sint,
        dxgi::R32G32B32_FLOAT => Rgb32Float,
        dxgi::R32G32B32_UINT => Rgb32Uint,
        dxgi::R32G32B32_SINT => Rgb32Sint,
        dxgi::R16G16B16A16_FLOAT => Rgba16Float,
        dxgi::R16G16B16A16_UNORM => Rgba16Unorm,
        dxgi::R16G16B16A16_UINT => Rgba16Uint,
        dxgi::R16G16B16A16_SNORM => Rgba16Snorm,
        dxgi::R16G16B16A16_SINT => Rgba16Sint,
        dxgi::R32G32_FLOAT => Rg32Float,
        dxgi::R32G32_UINT => Rg32Uint,
        dxgi::R32G32_SINT => Rg32Sint,
        dxgi::D32_FLOAT_S8X24_UINT => Depth32FloatStencil8,
        dxgi::R10G10B10A2_UNORM => Rgb10a2Unorm,
        dxgi::R10G10B10A2_UINT => Rgb10a2Uint,
        dxgi::R11G11B10_FLOAT => Rg11b10Float,
        dxgi::R8G8B8A8_UNORM => Rgba8Unorm,
        dxgi::R8G8B8A8_UNORM_SRGB => Rgba8UnormSrgb,
        dxgi::R8G8B8A8_UINT => Rgba8Uint,
        dxgi::R8G8B8A8_SNORM => Rgba8Snorm,
        dxgi::R8G8B8A8_SINT => Rgba8Sint,
        dxgi::R16G16_FLOAT => Rg16Float,
        dxgi::R16G16_UNORM => Rg16Unorm,
        dxgi::R16G16_UINT => Rg16Uint,
        dxgi::R16G16_SNORM => Rg16Snorm,
        dxgi::R16G16_SINT => Rg16Sint,
        dxgi::D32_FLOAT => Depth32Float,
        dxgi::R32_FLOAT => R32Float,
        dxgi::R32_UINT => R32Uint,
        dxgi::R32_SINT => R32Sint,
        dxgi::D24_UNORM_S8_UINT => Depth24PlusStencil8,
        dxgi::R8G8_UNORM => Rg8Unorm,
        dxgi::R8G8_UINT => Rg8Uint,
        dxgi::R8G8_SNORM => Rg8Snorm,
        dxgi::R8G8_SINT => Rg8Sint,
        dxgi::R16_FLOAT => R16Float,
        dxgi::D16_UNORM => Depth16Unorm,
        dxgi::R16_UNORM => R16Unorm,
        dxgi::R16_UINT => R16Uint,
        dxgi::R16_SNORM => R16Snorm,
        dxgi::R16_SINT => R16Sint,
        dxgi::R8_UNORM => R8Unorm,
        dxgi::R8_UINT => R8Uint,
        dxgi::R8_SNORM => R8Snorm,
        dxgi::R8_SINT => R8Sint,
        dxgi::A8_UNORM => A8Unorm,
        dxgi::R9G9B9E5_SHAREDEXP => Rgb9e5Float,
        dxgi::R8G8_B8G8_UNORM => Rgbg8Unorm,
        dxgi::G8R8_G8B8_UNORM => Grgb8Unorm,
        dxgi::BC1_UNORM => Bc1RgbaUnorm,
        dxgi::BC1_UNORM_SRGB => Bc1RgbaUnormSrgb,
        dxgi::BC2_UNORM => Bc2RgbaUnorm,
        dxgi::BC2_UNORM_SRGB => Bc2RgbaUnormSrgb,
        dxgi::BC3_UNORM => Bc3RgbaUnorm,
        dxgi::BC3_UNORM_SRGB => Bc3RgbaUnormSrgb,
        dxgi::BC4_UNORM => Bc4RUnorm,
        dxgi::BC4_SNORM => Bc4RSnorm,
        dxgi::BC5_UNORM => Bc5RgUnorm,
        dxgi::BC5_SNORM => Bc5RgSnorm,
        dxgi::B5G6R5_UNORM => B5g6r5Unorm,
        dxgi::B5G5R5A1_UNORM => Bgr5a1Unorm,
        dxgi::B8G8R8A8_UNORM => Bgra8Unorm,
        dxgi::B8G8R8X8_UNORM => Bgrx8Unorm,
        dxgi::B8G8R8A8_UNORM_SRGB => Bgra8UnormSrgb,
        dxgi::B8G8R8X8_UNORM_SRGB => Bgrx8UnormSrgb,
        dxgi::BC6H_UF16 => Bc6hRgbUfloat,
        dxgi::BC6H_SF16 => Bc6hRgbFloat,
        dxgi::BC7_UNORM => Bc7RgbaUnorm,
        dxgi::BC7_UNORM_SRGB => Bc7RgbaUnormSrgb,
        dxgi::B4G4R4A4_UNORM => Bgra4Unorm,
        _ => return None,
    })
}

/// The typeless format heading `dxgi`'s group (formats whose texels are the same bits), or `dxgi`
/// itself for a format in no group.
pub fn typeless_group(dxgi: u32) -> u32 {
    match dxgi {
        dxgi::R32G32B32A32_TYPELESS..=dxgi::R32G32B32A32_SINT => dxgi::R32G32B32A32_TYPELESS,
        dxgi::R32G32B32_TYPELESS..=dxgi::R32G32B32_SINT => dxgi::R32G32B32_TYPELESS,
        dxgi::R16G16B16A16_TYPELESS..=dxgi::R16G16B16A16_SINT => dxgi::R16G16B16A16_TYPELESS,
        dxgi::R32G32_TYPELESS..=dxgi::R32G32_SINT => dxgi::R32G32_TYPELESS,
        dxgi::R32G8X24_TYPELESS..=dxgi::X32_TYPELESS_G8X24_UINT => dxgi::R32G8X24_TYPELESS,
        dxgi::R10G10B10A2_TYPELESS..=dxgi::R10G10B10A2_UINT | dxgi::R10G10B10_XR_BIAS_A2_UNORM => {
            dxgi::R10G10B10A2_TYPELESS
        }
        dxgi::R8G8B8A8_TYPELESS..=dxgi::R8G8B8A8_SINT => dxgi::R8G8B8A8_TYPELESS,
        dxgi::R16G16_TYPELESS..=dxgi::R16G16_SINT => dxgi::R16G16_TYPELESS,
        dxgi::R32_TYPELESS..=dxgi::R32_SINT => dxgi::R32_TYPELESS,
        dxgi::R24G8_TYPELESS..=dxgi::X24_TYPELESS_G8_UINT => dxgi::R24G8_TYPELESS,
        dxgi::R8G8_TYPELESS..=dxgi::R8G8_SINT => dxgi::R8G8_TYPELESS,
        dxgi::R16_TYPELESS..=dxgi::R16_SINT => dxgi::R16_TYPELESS,
        dxgi::R8_TYPELESS..=dxgi::R8_SINT => dxgi::R8_TYPELESS,
        dxgi::BC1_TYPELESS..=dxgi::BC1_UNORM_SRGB => dxgi::BC1_TYPELESS,
        dxgi::BC2_TYPELESS..=dxgi::BC2_UNORM_SRGB => dxgi::BC2_TYPELESS,
        dxgi::BC3_TYPELESS..=dxgi::BC3_UNORM_SRGB => dxgi::BC3_TYPELESS,
        dxgi::BC4_TYPELESS..=dxgi::BC4_SNORM => dxgi::BC4_TYPELESS,
        dxgi::BC5_TYPELESS..=dxgi::BC5_SNORM => dxgi::BC5_TYPELESS,
        dxgi::B8G8R8A8_UNORM | dxgi::B8G8R8A8_TYPELESS | dxgi::B8G8R8A8_UNORM_SRGB => {
            dxgi::B8G8R8A8_TYPELESS
        }
        dxgi::B8G8R8X8_UNORM | dxgi::B8G8R8X8_TYPELESS | dxgi::B8G8R8X8_UNORM_SRGB => {
            dxgi::B8G8R8X8_TYPELESS
        }
        dxgi::BC6H_TYPELESS..=dxgi::BC6H_SF16 => dxgi::BC6H_TYPELESS,
        dxgi::BC7_TYPELESS..=dxgi::BC7_UNORM_SRGB => dxgi::BC7_TYPELESS,
        f => f,
    }
}

/// Whether Direct3D takes render target views in the other formats of its group of a swap chain's
/// buffer of DXGI format `dxgi` (Microsoft's 11_0 table's BackBuffer Castable Even Fully Typed row).
pub fn back_buffer_cast(dxgi: u32) -> bool {
    matches!(
        dxgi,
        dxgi::R10G10B10A2_UNORM
            | dxgi::R10G10B10_XR_BIAS_A2_UNORM
            | dxgi::R8G8B8A8_UNORM
            | dxgi::R8G8B8A8_UNORM_SRGB
            | dxgi::B8G8R8A8_UNORM
            | dxgi::B8G8R8A8_UNORM_SRGB
    )
}

/// Whether Direct3D 11 shares resources of DXGI format `dxgi` between devices (Microsoft's 11_0
/// table's Shared Resource row, as the RTX 3080 answers it).
pub fn shared(dxgi: u32) -> bool {
    matches!(
        typeless_group(dxgi),
        dxgi::R32G32B32A32_TYPELESS
            | dxgi::R16G16B16A16_TYPELESS
            | dxgi::R10G10B10A2_TYPELESS
            | dxgi::R11G11B10_FLOAT
            | dxgi::R8G8B8A8_TYPELESS
            | dxgi::R16G16_TYPELESS
            | dxgi::R32_TYPELESS
            | dxgi::R8G8_TYPELESS
            | dxgi::R16_TYPELESS
            | dxgi::R8_TYPELESS
            | dxgi::A8_UNORM
            | dxgi::BC1_TYPELESS
            | dxgi::BC2_TYPELESS
            | dxgi::BC3_TYPELESS
            | dxgi::B8G8R8A8_TYPELESS
            | dxgi::B8G8R8X8_TYPELESS
            | dxgi::NV12
            | dxgi::OPAQUE_420
            | dxgi::YUY2
    )
}

/// Whether Direct3D copies between `a` and `b`, of different groups, as the same bits (Direct3D 10.1's
/// format conversions): a 32-bit single-channel format and a shared-exponent or packed 4:2:2 one, and an
/// uncompressed colour format of 64 or 128 bits and a block-compressed format of that block size.
pub fn copy_cast(a: u32, b: u32) -> bool {
    let one = |x: u32, y: u32| match x {
        // R32: R9G9B9E5, R8G8_B8G8 and G8R8_G8B8.
        dxgi::R32_TYPELESS | dxgi::R32_FLOAT..=dxgi::R32_SINT => {
            matches!(y, dxgi::R9G9B9E5_SHAREDEXP..=dxgi::G8R8_G8B8_UNORM)
        }
        // R16G16B16A16 and R32G32: BC1 and BC4.
        dxgi::R16G16B16A16_TYPELESS..=dxgi::R32G32_SINT => {
            matches!(y, dxgi::BC1_TYPELESS..=dxgi::BC1_UNORM_SRGB | dxgi::BC4_TYPELESS..=dxgi::BC4_SNORM)
        }
        // R32G32B32A32: BC2, BC3, BC5, BC6H and BC7.
        dxgi::R32G32B32A32_TYPELESS..=dxgi::R32G32B32A32_SINT => {
            matches!(y, dxgi::BC2_TYPELESS..=dxgi::BC3_UNORM_SRGB | dxgi::BC5_TYPELESS..=dxgi::BC5_SNORM | dxgi::BC6H_TYPELESS..=dxgi::BC7_UNORM_SRGB)
        }
        _ => false,
    };
    one(a, b) || one(b, a)
}

/// The interface vertex format of a DXGI format an input element may have.
pub fn vertex_format(dxgi: u32) -> Option<VertexFormat> {
    dxgi_vertex_format(dxgi).map(|(format, _)| format)
}

/// A vertex format of 32-bit components as the 32-bit format of the same bits of the type the
/// vertex program reads it as: Direct3D hands the shader the bits, which a back end's typed
/// inputs take only as their own type. Other formats are kept.
pub fn vertex_format_read_as(format: VertexFormat, scalar: ScalarKind) -> VertexFormat {
    use VertexFormat::*;
    let count = match format {
        Float32x1 | Uint32x1 | Sint32x1 => 0,
        Float32x2 | Uint32x2 | Sint32x2 => 1,
        Float32x3 | Uint32x3 | Sint32x3 => 2,
        Float32x4 | Uint32x4 | Sint32x4 => 3,
        _ => return format,
    };
    let formats = match scalar {
        ScalarKind::Uint => [Uint32x1, Uint32x2, Uint32x3, Uint32x4],
        ScalarKind::Sint => [Sint32x1, Sint32x2, Sint32x3, Sint32x4],
        ScalarKind::Float => [Float32x1, Float32x2, Float32x3, Float32x4],
    };
    formats[count]
}

/// The scalar type a vertex format is read as.
pub fn vertex_scalar(f: VertexFormat) -> ScalarKind {
    use VertexFormat::*;
    match f {
        Uint8x1 | Uint8x2 | Uint8x3 | Uint8x4 | Uint16x1 | Uint16x2 | Uint16x3 | Uint16x4
        | Uint32x1 | Uint32x2 | Uint32x3 | Uint32x4 => ScalarKind::Uint,
        Sint8x1 | Sint8x2 | Sint8x3 | Sint8x4 | Sint16x1 | Sint16x2 | Sint16x3 | Sint16x4
        | Sint32x1 | Sint32x2 | Sint32x3 | Sint32x4 => ScalarKind::Sint,
        _ => ScalarKind::Float,
    }
}

/// Bytes one vertex element of DXGI format `dxgi` takes: a texel of that format.
pub fn vertex_size(dxgi: u32) -> Option<u32> {
    texture_format(dxgi).map(|f| {
        let (_, _, bytes) = FORMAT_FACTS[f as usize].block;
        bytes
    })
}

/// The alignment of a vertex element's offset: the size of its components, at most 4 bytes.
pub fn vertex_alignment(format: VertexFormat) -> u32 {
    use VertexFormat::*;
    match format {
        Uint8x1 | Sint8x1 | Unorm8x1 | Snorm8x1 | Uint8x2 | Sint8x2 | Unorm8x2 | Snorm8x2
        | Uint8x3 | Sint8x3 | Unorm8x3 | Snorm8x3 | Uint8x4 | Sint8x4 | Unorm8x4 | Snorm8x4 => 1,
        Uint16x1 | Sint16x1 | Unorm16x1 | Snorm16x1 | Float16x1 | Uint16x2 | Sint16x2
        | Unorm16x2 | Snorm16x2 | Float16x2 | Uint16x3 | Sint16x3 | Unorm16x3 | Snorm16x3
        | Float16x3 | Uint16x4 | Sint16x4 | Unorm16x4 | Snorm16x4 | Float16x4 => 2,
        _ => size_of::<u32>() as u32,
    }
}

/// The index format of `IASetIndexBuffer`'s format (16 or 32-bit unsigned only, as Direct3D 11 allows).
pub fn index_format(dxgi: u32) -> Option<IndexFormat> {
    match dxgi {
        R16_UINT => Some(IndexFormat::U16),
        R32_UINT => Some(IndexFormat::U32),
        _ => None,
    }
}

/// Bytes a row of `width` texels of `format` takes, and how many rows of blocks `height` texels make.
pub fn row_bytes(format: Format, width: u32, height: u32) -> (u32, u32) {
    let (bw, bh, bytes) = FORMAT_FACTS[format as usize].block;
    (width.div_ceil(bw) * bytes, height.div_ceil(bh))
}

/// The components `format` has, a bit each from red, as a view reads them: a border colour is read
/// as a texel of the view's format (`HelperValue::TextureComponents`). Depth reads as red.
pub fn components(format: Format) -> u32 {
    FORMAT_FACTS[format as usize].components
}

/// Direct3D's conversion of `value` into a channel of normalised `format` (clamped to its range,
/// NaN as zero, scaled and rounded to the nearest integer, halves toward zero as the RTX 3080
/// clears and draws them), as the bits of a channel of its integer twin (`NormAsInteger`).
pub fn encode_norm(format: Format, value: f32) -> u32 {
    let signed = signed_normalized(format);
    let bits = FORMAT_FACTS[format as usize].block.2 * u8::BITS
        / FORMAT_FACTS[format as usize].components.count_ones();
    let max = (1u32 << (bits - u32::from(signed))) - 1;
    let low = if signed { -1.0 } else { 0.0 };
    let v = if value.is_nan() {
        0.0
    } else {
        value.clamp(low, 1.0)
    };
    let scaled = v * max as f32;
    let whole = scaled.trunc();
    // Away from zero only where the rest is more than half a step.
    let rest = (scaled - whole).abs();
    let rounded = if rest > 1.0 - rest {
        whole + scaled.signum()
    } else {
        whole
    };
    rounded as i32 as u32
}

/// The typed DXGI format a texture of `format` is stored for.
pub fn dxgi_of(format: Format) -> Option<u32> {
    (dxgi::UNKNOWN..=dxgi::B4G4R4A4_UNORM).find(|&d| texture_format(d) == Some(format))
}
