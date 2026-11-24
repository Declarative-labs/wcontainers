//! What `D3D12CreateDevice` and `CheckFeatureSupport` answer, from the back end's `Caps` (plan
//! section 9): feature level 11_0 on a back end with compute, storage writes in fragment programs,
//! the 11_0 sizes and the raster and blend abilities of `LEVEL_11_0_FIXED_FUNCTION`, resource binding
//! tier 3 only with big resource tables; ray tracing, mesh shaders, variable rate shading,
//! sampler feedback, tiled resources, rasterizer ordered views, conservative rasterisation and 64-bit
//! integers absent.

use wcontainers_directx_graphics_infrastructure::required::{self, Required};
use wcontainers_directx_graphics_infrastructure::stage_registers::{INPUT_SLOTS, OUTPUT_REGISTERS};
use wcontainers_directx_graphics_infrastructure::{
    FormatSupport, atomic_operations, dxgi, dxgi_format, dxgi_vertex_format,
    format_support as shared_format_support, storage_viewed, support1, support2, takes_samples,
};
use wcontainers_gpu::{Caps, Features, Format, FormatUse};

pub const FEATURE_LEVEL_11_0: u32 = required::level::L11_0;
/// Feature level 11_0's sizes (`D3D12_REQ_TEXTURE2D_U_OR_V_DIMENSION`, which cube faces share,
/// `D3D12_REQ_TEXTURE3D_U_V_OR_W_DIMENSION`, `D3D12_REQ_TEXTURE2D_ARRAY_AXIS_DIMENSION`).
pub const LEVEL_11_0_TEXTURE_2D: u32 = 16384;
pub const LEVEL_11_0_TEXTURE_3D: u32 = 2048;
pub const LEVEL_11_0_TEXTURE_LAYERS: u32 = 2048;
/// Resource binding tier 1's counts a stage binds at 11_0 (Microsoft's hardware tiers table):
/// shader resource views (`D3D12_COMMONSHADER_INPUT_RESOURCE_SLOT_COUNT`), constant buffers
/// (`D3D12_COMMONSHADER_CONSTANT_BUFFER_API_SLOT_COUNT`), samplers
/// (`D3D12_COMMONSHADER_SAMPLER_SLOT_COUNT`) and unordered access views, of either kind (8 at 11_0);
/// and the targets drawn at once (`D3D12_SIMULTANEOUS_RENDER_TARGET_COUNT`).
pub const TIER_1_SHADER_RESOURCES: u32 = 128;
pub const TIER_1_CONSTANT_BUFFERS: u32 = 14;
pub const TIER_1_SAMPLERS: u32 = 16;
pub const TIER_1_UNORDERED_ACCESS_AT_11_0: u32 = 8;
pub const LEVEL_11_0_TARGETS: u32 = 8;
/// What 11_0 asks of the raster and blend blocks that the front draws only through the back end:
/// a depth bias clamp, independent blending and a partial sample mask. Depth clamping is done in
/// the programs where the back end lacks it (`DepthClampInShader`); wireframe without the back
/// end's polygon mode (`FillModeConverted`) is refused by name until the stage helper draws it.
pub const LEVEL_11_0_FIXED_FUNCTION: Features = Features::DEPTH_BIAS_CLAMP
    .union(Features::INDEPENDENT_BLEND)
    .union(Features::SAMPLE_MASK);
/// `D3D_SHADER_MODEL_6_0`.
pub const SHADER_MODEL_6_0: u32 = 0x60;

/// The highest feature level the front reports, or None when `D3D12CreateDevice` fails as on
/// hardware below 11_0: WebGL 2, or an adapter short of 11_0's abilities
/// ([`has_abilities_of_11_0`]) or of a format ability Microsoft's 11_0 table requires
/// ([`unmet_format_requirements`]).
pub fn feature_level(caps: &Caps) -> Option<u32> {
    (has_abilities_of_11_0(caps) && unmet_format_requirements(caps).is_empty())
        .then_some(FEATURE_LEVEL_11_0)
}

/// What Microsoft's 11_0 format table requires that this front does not answer present, per
/// format: the missing bits of both words and sample counts (`required::unmet`), of the words'
/// bits Direct3D 12 defines.
pub fn unmet_format_requirements(caps: &Caps) -> Vec<(u32, Required)> {
    required::unmet(
        required::level::L11_0,
        required::DIRECT3D_11_ONLY,
        |value| format_support(caps, value),
        |value, samples| quality_levels(caps, value, samples).is_some_and(|q| q > 0),
    )
}

/// Whether the back end has 11_0's abilities but its formats': compute, storage writes in pixel
/// programs, 11_0's sizes, its input slots and stage registers ([`serves_stage_registers`]),
/// resource binding tier 1's counts and its targets ([`binds_tier_1`]), `D32_FLOAT_S8X24_UINT`
/// depth targets and `LEVEL_11_0_FIXED_FUNCTION`.
pub fn has_abilities_of_11_0(caps: &Caps) -> bool {
    let l = &caps.limits;
    let sizes = l.max_texture_2d >= LEVEL_11_0_TEXTURE_2D
        && l.max_texture_cube >= LEVEL_11_0_TEXTURE_2D
        && l.max_texture_3d >= LEVEL_11_0_TEXTURE_3D
        && l.max_texture_layers >= LEVEL_11_0_TEXTURE_LAYERS;
    let depth_stencil = caps
        .formats
        .get(Format::Depth32FloatStencil8)
        .contains(FormatUse::DEPTH_STENCIL);
    // 11_0 has compute shaders and unordered access views in pixel shaders, and the raster and
    // blend abilities this front serves only through the back end's own.
    let stages = caps.has(Features::COMPUTE | Features::FRAGMENT_STORAGE_WRITE);
    let fixed = caps.has(LEVEL_11_0_FIXED_FUNCTION);
    // 11_1 also needs logic operations and target-independent rasterisation, which no `Caps`
    // capability promises.
    sizes && depth_stencil && stages && fixed && serves_stage_registers(l) && binds_tier_1(l)
}

/// Whether `limits` bind resource binding tier 1's counts in a stage, with the uniform block that
/// holds a converted program's helper constants beside its constant buffers, and draw 11_0's
/// targets at once.
pub fn binds_tier_1(limits: &wcontainers_gpu::Limits) -> bool {
    limits.textures_per_stage >= TIER_1_SHADER_RESOURCES
        && limits.uniform_buffers_per_stage > TIER_1_CONSTANT_BUFFERS
        && limits.samplers_per_stage >= TIER_1_SAMPLERS
        && limits.storage_textures_per_stage >= TIER_1_UNORDERED_ACCESS_AT_11_0
        && limits.storage_buffers_per_stage >= TIER_1_UNORDERED_ACCESS_AT_11_0
        && limits.color_targets >= LEVEL_11_0_TARGETS
}

/// Whether `limits` serve Direct3D's 32 input slots and elements, fetched as laid out up to the
/// back end's streams and attribute locations and in the vertex program past them, a storage
/// buffer each (`VertexPulledInShader`); and its 32 registers from the last vertex stage (vertex,
/// domain or geometry shader) to the pixel stage, one the position, the rest moved onto the back
/// end's inter-stage locations with any the front links beside them (`StageMoves`).
pub fn serves_stage_registers(limits: &wcontainers_gpu::Limits) -> bool {
    let fetched =
        limits.vertex_streams.min(limits.vertex_attributes) + limits.storage_buffers_per_stage;
    fetched >= INPUT_SLOTS && limits.inter_stage_slots + 1 >= OUTPUT_REGISTERS
}

/// `D3D12_FORMAT_SUPPORT1_BUFFER` and `D3D12_FORMAT_SUPPORT1_SHADER_LOAD`, which typed buffer
/// views give: they are loaded.
pub const SUPPORT1_TYPED_BUFFER: u32 = support1::BUFFER | support1::SHADER_LOAD;

/// `D3D12_FEATURE_FORMAT_SUPPORT` for DXGI format `value`: the shared answers, with typed buffer
/// views, stream output and atomics on typed storage views, which this front serves through the
/// program (`TexelBufferAsStorage`, `StreamOutAsCompute`, `AtomicsThroughBuffer`): stream output
/// into the formats of 32-bit components, as Direct3D lists.
pub fn format_support(caps: &Caps, value: u32) -> FormatSupport {
    if let Some(s) = video_format_support(caps, value) {
        return s;
    }
    let mut s = shared_format_support(caps, value);
    if typed_buffer(caps, value) {
        s.support1 |= SUPPORT1_TYPED_BUFFER;
    }
    // Storage views of formats the back end cannot store into are read and written through a
    // buffer of their texels (`StorageFormatThroughBuffer`).
    if s.support1 & support1::TYPED_UNORDERED_ACCESS_VIEW == 0 && stored_through(caps, value) {
        s.support1 |= support1::TYPED_UNORDERED_ACCESS_VIEW;
        s.support2 |= support2::UAV_TYPED_STORE | support2::UAV_TYPED_LOAD;
    }
    // A storage texture used with atomics is read and written through a buffer of its texels.
    if s.support2 & support2::UAV_TYPED_STORE != 0 && caps.limits.storage_buffers_per_stage > 0 {
        s.support2 |= atomic_operations(value);
    }
    let word = size_of::<u32>() as u32;
    let streamed = match (dxgi_format(value), dxgi_vertex_format(value)) {
        (Some((format, false)), Some((_, size))) => size == word * format.components().count_ones(),
        _ => false,
    };
    if streamed && caps.has(Features::COMPUTE) {
        s.support1 |= support1::SO_BUFFER;
    }
    // Of a format no multisampled texture is made of (`R32G32B32_*` and `B4G4R4A4_UNORM` on core
    // WebGPU), the multisample loads and resolves Microsoft's 11_0 table requires apply to no
    // resource; they are answered as Direct3D's drivers answer them for formats they do not
    // multisample (the RTX 3080 for these).
    let multisampled = dxgi_format(value).is_some_and(|(format, _)| {
        (2..=required::MOST_SAMPLES).any(|n| caps.formats.takes_samples(format, n))
    });
    if s.support1 != 0
        && !multisampled
        && let Some(mut rows) = required::at(required::level::L11_0)
        && let Some((_, row)) = rows.find(|&(d, _)| d == value)
    {
        s.support1 |= row.0 & (support1::MULTISAMPLE_LOAD | support1::MULTISAMPLE_RESOLVE);
    }
    s
}

/// The abilities of a 2D texture of one level that Direct3D defines for `NV12` through views of
/// its planes; `YUY2` has them but for targets.
const NV12_ABILITIES: FormatSupport = FormatSupport {
    support1: support1::TEXTURE2D
        | support1::SHADER_LOAD
        | support1::SHADER_SAMPLE
        | support1::SHADER_GATHER
        | support1::RENDER_TARGET
        | support1::BLENDABLE
        | support1::TYPED_UNORDERED_ACCESS_VIEW,
    support2: support2::UAV_TYPED_STORE,
};

/// `D3D12_FEATURE_FORMAT_SUPPORT` of the video formats plan 4.12 stores as ordinary textures:
/// `NV12` has what both its planes' formats (`R8_UNORM`, `R8G8_UNORM`) have of `NV12_ABILITIES`,
/// `YUY2` what `R8G8B8A8_UNORM` has of them but targets, and `420_OPAQUE`, which only video
/// decoders and processors use, is a 2D texture; and `R10G10B10_XR_BIAS_A2_UNORM`'s, of no plane.
/// None for other formats; the video abilities themselves are absent (section 9).
fn video_format_support(caps: &Caps, value: u32) -> Option<FormatSupport> {
    let of = |format: u32| shared_format_support(caps, format);
    let (views, abilities) = match value {
        dxgi::NV12 | dxgi::OPAQUE_420 => {
            let (luma, chroma) = (of(dxgi::R8_UNORM), of(dxgi::R8G8_UNORM));
            let both = FormatSupport {
                support1: luma.support1 & chroma.support1,
                support2: luma.support2 & chroma.support2,
            };
            let abilities = match value {
                dxgi::NV12 => NV12_ABILITIES,
                _ => FormatSupport {
                    support1: support1::TEXTURE2D,
                    support2: 0,
                },
            };
            (both, abilities)
        }
        // 2D textures held in `R10G10B10A2_UNORM`'s bits (`ResourceDesc::texture_format`), cast
        // within that layout by copies. No swap chain of Direct3D 12's flip models takes it, as
        // DXGI rules, so its display and back buffer cast abilities apply to no resource; they are
        // answered as the RTX 3080 answers them.
        dxgi::R10G10B10_XR_BIAS_A2_UNORM => (
            FormatSupport {
                support1: match caps
                    .formats
                    .get(Format::Rgb10a2Unorm)
                    .contains(FormatUse::COPY_OUT)
                {
                    true => u32::MAX,
                    false => 0,
                },
                support2: 0,
            },
            FormatSupport {
                support1: support1::TEXTURE2D
                    | support1::CAST_WITHIN_BIT_LAYOUT
                    | support1::DISPLAY
                    | support1::BACK_BUFFER_CAST,
                support2: 0,
            },
        ),
        dxgi::YUY2 => (
            of(dxgi::R8G8B8A8_UNORM),
            FormatSupport {
                support1: NV12_ABILITIES.support1
                    & !(support1::RENDER_TARGET | support1::BLENDABLE),
                ..NV12_ABILITIES
            },
        ),
        _ => return None,
    };
    Some(FormatSupport {
        support1: views.support1 & abilities.support1,
        support2: views.support2 & abilities.support2,
    })
}

/// Storage views of DXGI format `value` are served through a buffer of their texels where the
/// back end cannot store into its texture format (`StorageFormatThroughBuffer`): its texels are
/// copied out and back whole.
pub fn stored_through(caps: &Caps, value: u32) -> bool {
    storage_viewed(value)
        && caps.has(Features::COMPUTE)
        && caps.limits.storage_buffers_per_stage > 0
        && dxgi_format(value).is_some_and(|(f, _)| {
            let uses = caps.formats.get(f);
            !uses.contains(FormatUse::STORAGE_WRITE) && uses.contains(FormatUse::COPY_OUT)
        })
}

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
    highest_of(feature_level(caps)?, requested)
}

/// The highest of `requested` at or below `own`.
pub fn highest_of(own: u32, requested: &[u32]) -> Option<u32> {
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
        // 64-bit integers are absent for Direct3D 12 (plan section 9).
        int64_shader_ops: false,
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

/// `D3D12_FEATURE_DATA_D3D12_OPTIONS2::DepthBoundsTestSupported`: absent (plan section 9), as the
/// front refuses depth bounds whatever the back end has.
pub const DEPTH_BOUNDS: bool = false;

/// `CheckFeatureSupport(D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS)` for a `DXGI_FORMAT` value, as
/// Direct3D 12 answers: one quality level for one sample whatever the format; for more, one where a
/// resource of the format may have that count and none otherwise; None (the call fails) for no
/// samples, or for `R1_UNORM`, which Direct3D 12 takes for no resource.
pub fn quality_levels(caps: &Caps, format: u32, samples: u32) -> Option<u32> {
    match samples {
        _ if format == dxgi::R1_UNORM => None,
        0 => None,
        1 => Some(1),
        _ => Some(u32::from(takes_samples(caps, format, samples))),
    }
}
