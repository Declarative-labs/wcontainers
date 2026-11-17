//! What each DXGI format can be used for, as Direct3D 10 to 12 report it (`CheckFormatSupport`,
//! `CheckFeatureSupport(D3D12_FEATURE_FORMAT_SUPPORT)`, `CheckMultisampleQualityLevels`), answered
//! from the back end's `Caps`. An ability is reported only where the back end has it natively;
//! typed buffer views, stream output, tiling, video and the formats the back end lacks are absent.

use wcontainers_gpu::{Caps, Features, Format, FormatUse};

use crate::format::{BACK_BUFFER_FORMATS, dxgi_format, dxgi_vertex_format};

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
        21 | 46 => Role::DepthView,
        22 | 47 => Role::StencilView,
        _ => Role::Whole,
    }
}

/// Whether a texture of `format` may have some sample count above one.
fn multisampled(caps: &Caps, format: Format) -> bool {
    [2, 4, 8, 16]
        .into_iter()
        .any(|n| caps.formats.takes_samples(format, n))
}

/// What `value` (a `DXGI_FORMAT`) can be used for on this back end.
pub fn format_support(caps: &Caps, value: u32) -> FormatSupport {
    use support1::*;
    use support2::*;
    // DXGI_FORMAT_UNKNOWN: buffers without a format.
    if value == 0 {
        return FormatSupport {
            support1: BUFFER,
            support2: 0,
        };
    }
    let Some((format, typeless)) = dxgi_format(value) else {
        return FormatSupport::default();
    };
    let uses = caps.formats.get(format);
    let reinterpret = caps.has(Features::FORMAT_REINTERPRET);
    let block = format.block().0 > 1;
    let depth = format.is_depth_stencil();
    let texture = uses.intersects(
        FormatUse::SAMPLE
            | FormatUse::RENDER
            | FormatUse::DEPTH_STENCIL
            | FormatUse::STORAGE_READ
            | FormatUse::STORAGE_WRITE,
    );
    // The texture shapes a format may have: no 1D or 3D for blocks (3D blocks need an adapter
    // feature `Caps` does not carry) and no 3D for depth.
    let shapes = if !texture {
        0
    } else if block {
        TEXTURE2D | TEXTURECUBE | MIP
    } else if depth {
        TEXTURE1D | TEXTURE2D | TEXTURECUBE | MIP
    } else {
        TEXTURE1D | TEXTURE2D | TEXTURE3D | TEXTURECUBE | MIP
    };
    let cast = if reinterpret && texture {
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
    let filter = sample && has(FormatUse::FILTER);
    let multisample_load =
        sample && caps.has(Features::MULTISAMPLE_READ) && multisampled(caps, format);
    let mut bit = |on: bool, b: u32| {
        if on {
            s.support1 |= b;
        }
    };
    match role(value) {
        Role::DepthView => {
            bit(
                sample,
                shapes | SHADER_LOAD | SHADER_SAMPLE_COMPARISON | SHADER_GATHER_COMPARISON,
            );
            bit(filter, SHADER_SAMPLE | SHADER_GATHER);
            bit(multisample_load, MULTISAMPLE_LOAD);
            return s;
        }
        Role::StencilView => {
            bit(sample, shapes | SHADER_LOAD);
            bit(multisample_load, MULTISAMPLE_LOAD);
            return s;
        }
        Role::Whole if depth => {
            let target = has(FormatUse::DEPTH_STENCIL);
            bit(target, shapes | DEPTH_STENCIL);
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
    bit(matches!(value, 42 | 57), IA_INDEX_BUFFER);
    bit(sample, SHADER_LOAD);
    bit(filter, SHADER_SAMPLE | SHADER_GATHER);
    // R32_FLOAT and R16_UNORM also view depth textures stored as D32_FLOAT and D16_UNORM.
    let depth_twin = match value {
        41 => Some(Format::Depth32Float),
        56 => Some(Format::Depth16Unorm),
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
    bit(has(FormatUse::RESOLVE), MULTISAMPLE_RESOLVE);
    bit(multisample_load, MULTISAMPLE_LOAD);
    bit(BACK_BUFFER_FORMATS.contains(&value), DISPLAY);
    // The swap chain's buffers take views as their sRGB twins.
    bit(matches!(value, 28 | 29 | 87 | 91), BACK_BUFFER_CAST);
    bit(storage_write, TYPED_UNORDERED_ACCESS_VIEW);
    if storage_write {
        s.support2 |= UAV_TYPED_STORE;
        if has(FormatUse::STORAGE_READ) {
            s.support2 |= UAV_TYPED_LOAD;
        }
    }
    if has(FormatUse::STORAGE_ATOMIC) {
        s.support2 |= match value {
            42 | 43 => {
                UAV_ATOMIC_ADD
                    | UAV_ATOMIC_BITWISE_OPS
                    | UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE
                    | UAV_ATOMIC_EXCHANGE
                    | UAV_ATOMIC_SIGNED_MIN_OR_MAX
                    | UAV_ATOMIC_UNSIGNED_MIN_OR_MAX
            }
            41 => UAV_ATOMIC_EXCHANGE,
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
    match value {
        19..=22 | 44..=47 | 103 | 104 | 105 | 110 => Some(2),
        66 => None,
        0..=115 => Some(1),
        _ => None,
    }
}
