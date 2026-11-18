//! What `D3D12CreateDevice` and `CheckFeatureSupport` answer, from the back end's `Caps` (plan
//! section 9): feature level 11_0 on a back end with compute, storage writes in fragment programs
//! and the 11_0 sizes, resource binding
//! tier 3 only with big resource tables; ray tracing, mesh shaders, variable rate shading,
//! sampler feedback, tiled resources, rasterizer ordered views, conservative rasterisation and 64-bit
//! integers absent.

use wcontainers_directx_graphics_infrastructure::{dxgi_vertex_format, takes_samples};
use wcontainers_gpu::{Caps, Features, Format, FormatUse};

pub const FEATURE_LEVEL_11_0: u32 = 0xb000;
/// `D3D_SHADER_MODEL_6_0`.
pub const SHADER_MODEL_6_0: u32 = 0x60;

/// The highest feature level the front reports, or None when `D3D12CreateDevice` fails as on
/// hardware below 11_0 (WebGL 2, or an adapter short of 11_0's sizes or of `D32_FLOAT_S8X24_UINT`
/// depth targets).
pub fn feature_level(caps: &Caps) -> Option<u32> {
    let l = &caps.limits;
    let sizes = l.max_texture_2d >= 16384
        && l.max_texture_cube >= 16384
        && l.max_texture_3d >= 2048
        && l.max_texture_layers >= 2048;
    let depth_stencil = caps
        .formats
        .get(Format::Depth32FloatStencil8)
        .contains(FormatUse::DEPTH_STENCIL);
    // 11_0 has compute shaders and unordered access views in pixel shaders.
    let stages = caps.has(Features::COMPUTE | Features::FRAGMENT_STORAGE_WRITE);
    if !sizes || !depth_stencil || !stages {
        return None;
    }
    // 11_1 also needs logic operations and target-independent rasterisation, which no `Caps`
    // capability promises.
    Some(FEATURE_LEVEL_11_0)
}

/// `D3D12_FORMAT_SUPPORT1_BUFFER` and `D3D12_FORMAT_SUPPORT1_SHADER_LOAD`, which typed buffer
/// views give: they are loaded.
pub const SUPPORT1_TYPED_BUFFER: u32 = 0x1 | 0x100;

/// Typed buffer views of DXGI format `value` are read: through storage buffers, decoded in the
/// program (`TexelBufferAsStorage`), for the formats that have a vertex fetch form.
pub fn typed_buffer(caps: &Caps, value: u32) -> bool {
    !caps.has(Features::TEXEL_BUFFERS)
        && caps.limits.storage_buffers_per_stage > 0
        && dxgi_vertex_format(value).is_some()
}

/// `D3D12_FEATURE_FEATURE_LEVELS`: the highest of `requested` the device supports (every level
/// up to its own), or None when it supports none of them.
pub fn max_feature_level(caps: &Caps, requested: &[u32]) -> Option<u32> {
    let own = feature_level(caps)?;
    requested.iter().copied().filter(|l| *l <= own).max()
}

/// `D3D12_FEATURE_DATA_D3D12_OPTIONS`, field for field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub double_precision_float_shader_ops: bool,
    pub output_merger_logic_op: bool,
    pub min_precision_support: u32,
    pub tiled_resources_tier: u32,
    pub resource_binding_tier: u32,
    pub ps_specified_stencil_ref_supported: bool,
    pub typed_uav_load_additional_formats: bool,
    pub rovs_supported: bool,
    pub conservative_rasterization_tier: u32,
    pub max_gpu_virtual_address_bits_per_resource: u32,
    pub standard_swizzle_64kb_supported: bool,
    pub cross_node_sharing_tier: u32,
    pub cross_adapter_row_major_texture_supported: bool,
    pub vp_and_rt_array_index_from_any_shader_feeding_rasterizer_supported_without_gs_emulation:
        bool,
    pub resource_heap_tier: u32,
}

/// The formats `TypedUAVLoadAdditionalFormats` promises loads from, beyond `R32_*` (Microsoft's
/// "Typed unordered access view loads": 18 formats with the three of `R32`).
const ADDITIONAL_UAV_LOADS: [Format; 15] = [
    Format::Rgba32Float,
    Format::Rgba32Uint,
    Format::Rgba32Sint,
    Format::Rgba16Float,
    Format::Rgba16Uint,
    Format::Rgba16Sint,
    Format::Rgba8Unorm,
    Format::Rgba8Uint,
    Format::Rgba8Sint,
    Format::R16Float,
    Format::R16Uint,
    Format::R16Sint,
    Format::R8Unorm,
    Format::R8Uint,
    Format::R8Sint,
];

pub fn options(caps: &Caps) -> Options {
    Options {
        double_precision_float_shader_ops: caps.has(Features::SHADER_F64),
        output_merger_logic_op: caps.has(Features::LOGIC_OP),
        // D3D12_SHADER_MIN_PRECISION_SUPPORT_NONE: minimum precision runs at full precision.
        min_precision_support: 0,
        tiled_resources_tier: 0,
        resource_binding_tier: if caps.has(Features::RESOURCE_TABLES) {
            3
        } else {
            1
        },
        ps_specified_stencil_ref_supported: false,
        typed_uav_load_additional_formats: ADDITIONAL_UAV_LOADS
            .iter()
            .all(|f| caps.formats.get(*f).contains(FormatUse::STORAGE_READ)),
        rovs_supported: false,
        conservative_rasterization_tier: 0,
        max_gpu_virtual_address_bits_per_resource: VIRTUAL_ADDRESS_BITS,
        standard_swizzle_64kb_supported: false,
        cross_node_sharing_tier: 0,
        cross_adapter_row_major_texture_supported: false,
        vp_and_rt_array_index_from_any_shader_feeding_rasterizer_supported_without_gs_emulation:
            false,
        resource_heap_tier: 2,
    }
}

/// `D3D12_FEATURE_DATA_D3D12_OPTIONS1`: wave operations where the back end has subgroups, with
/// the lane counts WebGPU bounds subgroups by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options1 {
    pub wave_ops: bool,
    pub wave_lane_count_min: u32,
    pub wave_lane_count_max: u32,
    pub total_lane_count: u32,
    pub expanded_compute_resource_states: bool,
    pub int64_shader_ops: bool,
}

pub fn options1(caps: &Caps) -> Options1 {
    let waves = caps.has(Features::SUBGROUPS);
    Options1 {
        wave_ops: waves,
        wave_lane_count_min: if waves { 4 } else { 0 },
        wave_lane_count_max: if waves { 128 } else { 0 },
        total_lane_count: 0,
        expanded_compute_resource_states: true,
        int64_shader_ops: caps.has(Features::SHADER_I64),
    }
}

/// The highest shader model, given the highest the program asks about
/// (`D3D12_FEATURE_SHADER_MODEL` answers the lower of the two); a value that names no shader model
/// the Windows 10 runtime knows (5.1, or 6.0 to 6.6) is refused, as that runtime refuses it.
pub fn shader_model(asked: u32) -> Result<u32, crate::Error> {
    match asked {
        0x51 | 0x60..=0x66 => Ok(asked.min(SHADER_MODEL_6_0)),
        _ => Err(crate::Error::Invalid("no such shader model")),
    }
}

/// `D3D_ROOT_SIGNATURE_VERSION_1_1`, the highest version `root::deserialize` reads.
pub const ROOT_SIGNATURE_1_1: u32 = 2;

/// `D3D12_FEATURE_ROOT_SIGNATURE`: the lower of the version asked about and 1.1.
pub fn root_signature(asked: u32) -> u32 {
    asked.min(ROOT_SIGNATURE_1_1)
}

/// The bits of GPU virtual address a resource and the whole device may span
/// (`D3D12_FEATURE_GPU_VIRTUAL_ADDRESS_SUPPORT`). Addresses are numbers of the front's own
/// (`address.rs`), so any width holds; this is the width discrete GPUs report.
pub const VIRTUAL_ADDRESS_BITS: u32 = 40;

/// `D3D12_FEATURE_DATA_ARCHITECTURE1`: no tile-based renderer, memory or cache unity, or isolated
/// MMU is promised; the browser says nothing of any of them, and these answers make a program
/// copy through upload heaps as it must on a discrete GPU.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Architecture {
    pub tile_based_renderer: bool,
    pub uma: bool,
    pub cache_coherent_uma: bool,
    pub isolated_mmu: bool,
}

/// The tiers of the later option blocks that name absent features, all zero
/// (`NOT_SUPPORTED`): ray tracing (OPTIONS5), variable rate shading (OPTIONS6), mesh shaders and
/// sampler feedback (OPTIONS7).
pub const ABSENT_TIER: u32 = 0;

/// `D3D12_FEATURE_DATA_D3D12_OPTIONS2::DepthBoundsTestSupported`.
pub fn depth_bounds(caps: &Caps) -> bool {
    caps.has(Features::DEPTH_BOUNDS)
}

/// `CheckFeatureSupport(D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS)` for a `DXGI_FORMAT` value, as
/// Direct3D 12 answers: one quality level for one sample whatever the format; for more, one where a
/// resource of the format may have that count and none otherwise; None (the call fails) for no
/// samples, or for `R1_UNORM` (66), which Direct3D 12 takes for no resource.
pub fn quality_levels(caps: &Caps, format: u32, samples: u32) -> Option<u32> {
    match samples {
        _ if format == 66 => None,
        0 => None,
        1 => Some(1),
        _ => Some(u32::from(takes_samples(caps, format, samples))),
    }
}
