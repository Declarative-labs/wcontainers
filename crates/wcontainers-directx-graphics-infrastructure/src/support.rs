//! What each DXGI format can be used for, as Direct3D 10 to 12 report it (`CheckFormatSupport`,
//! `CheckFeatureSupport(D3D12_FEATURE_FORMAT_SUPPORT)`, `CheckMultisampleQualityLevels`), answered
//! from the back end's `Caps`. An ability is reported where the back end has it natively or the
//! fronts serve it by an exact route: sampling and gathering formats the back end cannot filter
//! (`FilterInShader`) and views of another member of a typeless group (twin textures where the back
//! end cannot reinterpret). Typed buffer views and stream output are the front's to add; tiling,
//! video and the formats the back end lacks are absent.

use wcontainers_gpu::{Caps, Features, Format, FormatUse};

use crate::format::{BACK_BUFFER_FORMATS, dxgi, dxgi_format, dxgi_vertex_format, typeless_group};

/// `D3D11_FORMAT_SUPPORT` and `D3D12_FORMAT_SUPPORT1` bits (the same numbers).
pub mod support1 {
    pub const BUFFER: u32 = 0x1;
    pub const IA_VERTEX_BUFFER: u32 = 0x2;
    pub const IA_INDEX_BUFFER: u32 = 0x4;
    pub const SO_BUFFER: u32 = 0x8;
    pub const TEXTURE1D: u32 = 0x10;
    pub const TEXTURE2D: u32 = 0x20;
    pub const TEXTURE3D: u32 = 0x40;
    pub const TEXTURECUBE: u32 = 0x80;
    pub const SHADER_LOAD: u32 = 0x100;
    pub const SHADER_SAMPLE: u32 = 0x200;
    pub const SHADER_SAMPLE_COMPARISON: u32 = 0x400;
    pub const MIP: u32 = 0x1000;
    pub const RENDER_TARGET: u32 = 0x4000;
    pub const BLENDABLE: u32 = 0x8000;
    pub const DEPTH_STENCIL: u32 = 0x10000;
    pub const MULTISAMPLE_RESOLVE: u32 = 0x40000;
    pub const DISPLAY: u32 = 0x80000;
    pub const CAST_WITHIN_BIT_LAYOUT: u32 = 0x100000;
    pub const MULTISAMPLE_RENDERTARGET: u32 = 0x200000;
    pub const MULTISAMPLE_LOAD: u32 = 0x400000;
    pub const SHADER_GATHER: u32 = 0x800000;
    pub const BACK_BUFFER_CAST: u32 = 0x1000000;
    pub const TYPED_UNORDERED_ACCESS_VIEW: u32 = 0x2000000;
    pub const SHADER_GATHER_COMPARISON: u32 = 0x4000000;
}

/// `D3D11_FORMAT_SUPPORT2` and `D3D12_FORMAT_SUPPORT2` bits.
pub mod support2 {
    pub const UAV_ATOMIC_ADD: u32 = 0x1;
    pub const UAV_ATOMIC_BITWISE_OPS: u32 = 0x2;
    pub const UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE: u32 = 0x4;
    pub const UAV_ATOMIC_EXCHANGE: u32 = 0x8;
    pub const UAV_ATOMIC_SIGNED_MIN_OR_MAX: u32 = 0x10;
    pub const UAV_ATOMIC_UNSIGNED_MIN_OR_MAX: u32 = 0x20;
    pub const UAV_TYPED_LOAD: u32 = 0x40;
    pub const UAV_TYPED_STORE: u32 = 0x80;
    pub const OUTPUT_MERGER_LOGIC_OP: u32 = 0x100;
}

/// Both words of a format's support; both zero for a format the API then refuses (`E_FAIL`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormatSupport {
    pub support1: u32,
    pub support2: u32,
}

impl FormatSupport {
    pub fn is_empty(&self) -> bool {
        self.support1 == 0 && self.support2 == 0
    }
}

/// How a DXGI format value reads the depth format it is stored in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// A colour format, or the whole of a depth format (`D32_FLOAT`, `D24_UNORM_S8_UINT`, ...).
    Whole,
    /// `R32_FLOAT_X8X24_TYPELESS`, `R24_UNORM_X8_TYPELESS`: the depth aspect as a shader reads it.
    DepthView,
    /// `X32_TYPELESS_G8X24_UINT`, `X24_TYPELESS_G8_UINT`: the stencil aspect.
    StencilView,
}

fn role(value: u32) -> Role {
    match value {
        dxgi::R32_FLOAT_X8X24_TYPELESS | dxgi::R24_UNORM_X8_TYPELESS => Role::DepthView,
        dxgi::X32_TYPELESS_G8X24_UINT | dxgi::X24_TYPELESS_G8_UINT => Role::StencilView,
        _ => Role::Whole,
    }
}

/// The sample counts above one Direct3D asks every format about.
const SAMPLE_COUNTS: [u32; 4] = [2, 4, 8, 16];

/// The formats whose sampling Direct3D's format support tables leave optional at every feature
/// level: reported only where the back end filters them.
const OPTIONAL_SAMPLE: [u32; 1] = [dxgi::R32G32B32_FLOAT];

/// Whether a texture of `format` may have some sample count above one.
fn multisampled(caps: &Caps, format: Format) -> bool {
    SAMPLE_COUNTS
        .into_iter()
        .any(|n| caps.formats.takes_samples(format, n))
}

/// What `value` (a `DXGI_FORMAT`) can be used for on this back end.
pub fn format_support(caps: &Caps, value: u32) -> FormatSupport {
    use support1::*;
    use support2::*;
    // Buffers without a format.
    if value == dxgi::UNKNOWN {
        return FormatSupport {
            support1: BUFFER,
            support2: 0,
        };
    }
    let Some((format, typeless)) = dxgi_format(value) else {
        return FormatSupport::default();
    };
    let uses = caps.formats.get(format);
    let depth = format.is_depth_stencil();
    let texture = uses.intersects(
        FormatUse::SAMPLE
            | FormatUse::RENDER
            | FormatUse::DEPTH_STENCIL
            | FormatUse::STORAGE_READ
            | FormatUse::STORAGE_WRITE,
    );
    // The texture shapes a format may have (`FormatCaps`): 1D only with blocks one texel high, and
    // no 3D for depth.
    let one_texel_high = format.block().1 == 1;
    let shapes = match texture {
        false => 0,
        true => {
            let mut shapes = TEXTURE2D | TEXTURECUBE | MIP;
            if one_texel_high {
                shapes |= TEXTURE1D;
            }
            if !depth {
                shapes |= TEXTURE3D;
            }
            shapes
        }
    };
    // Views of another member of the group: natively where the back end reinterprets, otherwise
    // through a twin texture of the view's format (`ReinterpretCopy`).
    let cast = if texture && typeless_group(value).is_some() {
        CAST_WITHIN_BIT_LAYOUT
    } else {
        0
    };
    let mut s = FormatSupport::default();
    if typeless {
        s.support1 = shapes | cast;
        return s;
    }
    let has = |u: FormatUse| uses.contains(u);
    let sample = has(FormatUse::SAMPLE);
    // Formats the back end cannot filter are filtered in the program (`FilterInShader`), but for
    // those whose sampling Direct3D leaves optional; integer formats are not sampled.
    let filter = sample
        && format.kind() == wcontainers_gpu::FormatKind::Float
        && (has(FormatUse::FILTER) || !OPTIONAL_SAMPLE.contains(&value));
    // A multisampled texture exists only as a target, so only formats that are targets load samples.
    let target = has(FormatUse::RENDER) || has(FormatUse::DEPTH_STENCIL);
    let multisample_load =
        sample && target && caps.has(Features::MULTISAMPLE_READ) && multisampled(caps, format);
    let mut bit = |on: bool, b: u32| {
        if on {
            s.support1 |= b;
        }
    };
    match role(value) {
        Role::DepthView => {
            // Filtered in the program where the back end cannot filter depth.
            bit(
                sample,
                shapes
                    | cast
                    | SHADER_LOAD
                    | SHADER_SAMPLE
                    | SHADER_GATHER
                    | SHADER_SAMPLE_COMPARISON
                    | SHADER_GATHER_COMPARISON,
            );
            bit(multisample_load, MULTISAMPLE_LOAD);
            return s;
        }
        Role::StencilView => {
            bit(sample, shapes | cast | SHADER_LOAD);
            bit(multisample_load, MULTISAMPLE_LOAD);
            return s;
        }
        Role::Whole if depth => {
            let target = has(FormatUse::DEPTH_STENCIL);
            bit(target, shapes | cast | DEPTH_STENCIL);
            bit(
                target && multisampled(caps, format),
                MULTISAMPLE_RENDERTARGET,
            );
            return s;
        }
        Role::Whole => {}
    }
    let render = has(FormatUse::RENDER);
    let storage_write = has(FormatUse::STORAGE_WRITE);
    bit(texture, shapes | cast);
    bit(dxgi_vertex_format(value).is_some(), IA_VERTEX_BUFFER);
    bit(
        matches!(value, dxgi::R32_UINT | dxgi::R16_UINT),
        IA_INDEX_BUFFER,
    );
    bit(sample, SHADER_LOAD);
    bit(filter, SHADER_SAMPLE | SHADER_GATHER);
    // R32_FLOAT and R16_UNORM also view depth textures stored as D32_FLOAT and D16_UNORM.
    let depth_twin = match value {
        dxgi::R32_FLOAT => Some(Format::Depth32Float),
        dxgi::R16_UNORM => Some(Format::Depth16Unorm),
        _ => None,
    };
    bit(
        depth_twin.is_some_and(|d| caps.formats.get(d).contains(FormatUse::SAMPLE)),
        SHADER_SAMPLE_COMPARISON | SHADER_GATHER_COMPARISON,
    );
    bit(render, RENDER_TARGET);
    bit(render && has(FormatUse::BLEND), BLENDABLE);
    bit(
        render && multisampled(caps, format),
        MULTISAMPLE_RENDERTARGET,
    );
    // Direct3D resolves multisampled targets of float and normalised formats only.
    bit(
        has(FormatUse::RESOLVE)
            && render
            && multisampled(caps, format)
            && format.kind() == wcontainers_gpu::FormatKind::Float,
        MULTISAMPLE_RESOLVE,
    );
    bit(multisample_load, MULTISAMPLE_LOAD);
    bit(BACK_BUFFER_FORMATS.contains(&value), DISPLAY);
    // The swap chain's buffers take views as their sRGB twins.
    bit(
        matches!(
            value,
            dxgi::R8G8B8A8_UNORM
                | dxgi::R8G8B8A8_UNORM_SRGB
                | dxgi::B8G8R8A8_UNORM
                | dxgi::B8G8R8A8_UNORM_SRGB
        ),
        BACK_BUFFER_CAST,
    );
    bit(storage_write, TYPED_UNORDERED_ACCESS_VIEW);
    if storage_write {
        s.support2 |= UAV_TYPED_STORE;
        if has(FormatUse::STORAGE_READ) {
            s.support2 |= UAV_TYPED_LOAD;
        }
    }
    if has(FormatUse::STORAGE_ATOMIC) {
        s.support2 |= match value {
            dxgi::R32_UINT | dxgi::R32_SINT => {
                UAV_ATOMIC_ADD
                    | UAV_ATOMIC_BITWISE_OPS
                    | UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE
                    | UAV_ATOMIC_EXCHANGE
                    | UAV_ATOMIC_SIGNED_MIN_OR_MAX
                    | UAV_ATOMIC_UNSIGNED_MIN_OR_MAX
            }
            dxgi::R32_FLOAT => UAV_ATOMIC_EXCHANGE,
            _ => 0,
        };
    }
    if render && caps.has(Features::LOGIC_OP) && format.kind() == wcontainers_gpu::FormatKind::Uint
    {
        s.support2 |= OUTPUT_MERGER_LOGIC_OP;
    }
    s
}

/// Whether a resource of `value` may have `samples` samples: one for any format the back end holds,
/// more where it renders or depth-tests the format with that count (for a typeless format, its
/// group's).
pub fn takes_samples(caps: &Caps, value: u32, samples: u32) -> bool {
    if samples == 1 {
        return !format_support(caps, value).is_empty();
    }
    let Some((format, typeless)) = dxgi_format(value) else {
        return false;
    };
    let target = caps
        .formats
        .get(format)
        .intersects(FormatUse::RENDER | FormatUse::DEPTH_STENCIL);
    (typeless || role(value) == Role::Whole)
        && target
        && caps.formats.takes_samples(format, samples)
}

/// The planes of a format (`D3D12_FEATURE_FORMAT_INFO`, `D3D12GetFormatPlaneCount`): two for depth
/// with stencil and the two-plane video formats, one for the others; None for a value that is no
/// format or that Direct3D 12 does not take (`R1_UNORM`).
pub fn plane_count(value: u32) -> Option<u8> {
    use dxgi::*;
    match value {
        R32G8X24_TYPELESS..=X32_TYPELESS_G8X24_UINT
        | R24G8_TYPELESS..=X24_TYPELESS_G8_UINT
        | NV12
        | P010
        | P016
        | NV11 => Some(2),
        R1_UNORM => None,
        UNKNOWN..=B4G4R4A4_UNORM => Some(1),
        _ => None,
    }
}
