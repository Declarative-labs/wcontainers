//! What a back end can do. Each back end fills one [`Caps`] when its device is made; fronts read it once
//! and choose their route: the native way, a counted slow path, or reporting the feature absent through
//! their own API's capability mechanism.

use crate::format::Format;
use bitflags::bitflags;

bitflags! {
    /// Optional abilities. A missing bit means the guest-visible feature is either built by a front or a
    /// shared helper as a counted slow path, or reported absent to the program.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Features: u64 {
        /// Compute programs and [`crate::Backend::dispatch`]. Also what geometry, hull and domain stages,
        /// stream output and unusual vertex fetch are emulated with.
        const COMPUTE = 1 << 0;
        /// Vertex programs may read storage buffers (the pass-through vertex stage of emulated geometry).
        const VERTEX_STORAGE_READ = 1 << 1;
        /// Vertex programs may write storage buffers and textures (Direct3D 11.1 UAVs in every stage).
        const VERTEX_STORAGE_WRITE = 1 << 2;
        /// Fragment programs may write storage buffers and textures (pixel shader UAVs).
        const FRAGMENT_STORAGE_WRITE = 1 << 3;
        /// [`crate::Backend::capture_vertices`]: vertex results written to buffers by fixed hardware.
        const TRANSFORM_FEEDBACK = 1 << 4;
        /// [`crate::Backend::read_back_now`] answers without leaving the current task.
        const READ_BACK_NOW = 1 << 5;
        /// Triangle fans and line loops are drawn natively by [`crate::Backend::draw_fan_or_loop`].
        const FAN_AND_LOOP = 1 << 6;
        /// [`crate::FillMode::Line`] and [`crate::FillMode::Point`].
        const POLYGON_MODE = 1 << 7;
        /// The last vertex of a primitive may be the provoking one.
        const PROVOKING_VERTEX_LAST = 1 << 8;
        /// The first vertex of a primitive may be the provoking one.
        const PROVOKING_VERTEX_FIRST = 1 << 9;
        /// Depth may be clamped instead of clipped ([`crate::RasterDesc::depth_clip`] false).
        const DEPTH_CLAMP = 1 << 10;
        /// Clip space depth from minus one to one, as OpenGL defines it, without rewriting shaders.
        const CLIP_DEPTH_MINUS_ONE = 1 << 11;
        /// User clip distances out of the last vertex stage.
        const CLIP_DISTANCE = 1 << 12;
        /// User cull distances out of the last vertex stage.
        const CULL_DISTANCE = 1 << 13;
        /// Two fragment outputs feeding one blend (dual-source blending).
        const DUAL_SOURCE_BLEND = 1 << 14;
        /// A different blend and write mask per color target. Without it a front whose targets differ
        /// draws once per group of targets that agree (slow path [`crate::SlowPath::TargetsSplit`]).
        const INDEPENDENT_BLEND = 1 << 15;
        /// Logic operations instead of blending.
        const LOGIC_OP = 1 << 16;
        /// Lines wider than one pixel.
        const WIDE_LINES = 1 << 17;
        /// Points larger than one pixel, sized by the vertex program.
        const POINT_SIZE = 1 << 18;
        /// Fragment programs run per sample, and read and write the sample mask.
        const SAMPLE_SHADING = 1 << 19;
        /// [`crate::RasterDesc::sample_mask`] other than all ones on multisampled targets. Without it a
        /// front with such a mask binds the program variant that writes the mask, which needs
        /// [`Features::SAMPLE_SHADING`] (slow path [`crate::SlowPath::SampleMaskInShader`]); without
        /// either, the front reports single-sample targets only.
        const SAMPLE_MASK = 1 << 47;
        /// Multisampled textures can be read in programs.
        const MULTISAMPLE_READ = 1 << 20;
        /// Query sets of [`crate::QueryKind::Occlusion`]: exact sample counts. Every back end has
        /// [`crate::QueryKind::OcclusionAny`].
        const OCCLUSION_COUNT = 1 << 21;
        /// GPU timestamps, written between passes ([`crate::Backend::write_timestamp`]).
        const TIMESTAMPS = 1 << 22;
        /// Draw and dispatch arguments read from a buffer.
        const INDIRECT = 1 << 23;
        /// Indirect draws honour a non-zero first instance.
        const INDIRECT_FIRST_INSTANCE = 1 << 24;
        /// Direct draws honour a non-zero base vertex and first instance.
        const BASE_VERTEX_INSTANCE = 1 << 25;
        /// Primitive restart can be switched off for strips.
        const RESTART_SWITCH = 1 << 26;
        /// Views may reinterpret any format of the same block size, not only sRGB twins.
        const FORMAT_REINTERPRET = 1 << 27;
        /// Views may swizzle channels.
        const VIEW_SWIZZLE = 1 << 28;
        /// Views of a part of a texture (mip and layer ranges, other dimensions).
        const TEXTURE_VIEWS = 1 << 29;
        /// Separate textures and samplers in programs, any pairing.
        const SEPARATE_SAMPLERS = 1 << 30;
        /// 16-bit floats in programs.
        const SHADER_F16 = 1 << 31;
        /// Subgroup (wave) operations in programs.
        const SUBGROUPS = 1 << 32;
        /// The primitive index as a fragment input.
        const PRIMITIVE_INDEX = 1 << 33;
        /// 64-bit integers in programs.
        const SHADER_I64 = 1 << 34;
        /// 64-bit floats in programs.
        const SHADER_F64 = 1 << 35;
        /// Large tables of resources indexed in programs ([`crate::Backend::create_resource_table`]).
        const RESOURCE_TABLES = 1 << 36;
        /// Buffers stay mapped and coherent while the GPU uses them.
        const PERSISTENT_MAPPING = 1 << 37;
        /// Sampler border colors other than transparent black.
        const BORDER_COLOR = 1 << 38;
        /// Tiled (sparse) resources.
        const SPARSE = 1 << 39;
        /// [`crate::Backend::prepare`] builds a state combination in the background.
        const BACKGROUND_PREPARE = 1 << 40;
        /// The depth bounds test of [`crate::DepthStencilDesc::depth_bounds`].
        const DEPTH_BOUNDS = 1 << 41;
        /// A different stencil reference and masks for front and back faces in one draw.
        const STENCIL_PER_FACE = 1 << 42;
        /// [`crate::AddressMode::MirrorOnce`] in samplers.
        const MIRROR_ONCE = 1 << 43;
        /// Clip space depth from zero to one, as Direct3D and Vulkan define it, without rewriting shaders.
        const CLIP_DEPTH_ZERO_ONE = 1 << 44;
        /// [`crate::RasterDesc`]'s depth bias clamp. Without it a front whose draw has a non-zero clamp
        /// applies the bias in the program (slow path [`crate::SlowPath::DepthBiasInShader`]).
        const DEPTH_BIAS_CLAMP = 1 << 45;
        /// Every block-compressed (BC) format is sampled natively. Without it a back end still declares
        /// the BC formats it serves in [`FormatCaps`], storing them decoded (slow path
        /// [`crate::SlowPath::BlockDecoded`]).
        const BLOCK_COMPRESSION = 1 << 46;
    }
}

/// Numeric limits. Each field says what it bounds; fronts compare them with what their API promises.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub max_texture_2d: u32,
    pub max_texture_cube: u32,
    pub max_texture_3d: u32,
    pub max_texture_layers: u32,
    pub max_buffer_size: u64,
    pub max_uniform_buffer_size: u32,
    pub max_storage_buffer_size: u64,
    /// Uniform buffers one stage may read, including the one that holds a converted program's helper
    /// constants.
    pub uniform_buffers_per_stage: u32,
    /// Sampled textures one stage may read.
    pub textures_per_stage: u32,
    /// Samplers one stage may use. With combined samplers this bounds texture-sampler pairs instead.
    pub samplers_per_stage: u32,
    pub storage_buffers_per_stage: u32,
    pub storage_textures_per_stage: u32,
    pub vertex_streams: u32,
    pub vertex_attributes: u32,
    pub max_vertex_stride: u32,
    pub color_targets: u32,
    /// Bytes one sample may hold across all color targets of a pass (WebGPU's 32 by default; back ends ask
    /// the adapter for its most). A front whose API allows fatter targets runs each draw once per group
    /// of targets that fits, each run starting from a saved copy of depth and stencil, with storage writes
    /// and queries kept only in the first run (slow path [`crate::SlowPath::TargetsSplit`]).
    pub color_bytes_per_sample: u32,
    /// Values passed from the vertex stage to the fragment stage, in four-component slots.
    pub inter_stage_slots: u32,
    /// Sample counts a target may have, as a bit per count (bit 0 for one sample).
    pub sample_counts: u32,
    pub min_uniform_offset_alignment: u32,
    pub min_storage_offset_alignment: u32,
    /// The granule of offsets and sizes the API itself copies, writes and clears buffers in (4 on WebGPU,
    /// 1 on WebGL 2). The interface takes any byte range; a back end with a granule above one handles the
    /// unaligned edges itself (slow path [`crate::SlowPath::UnalignedBufferEdge`]).
    pub buffer_copy_alignment: u32,
    /// Bytes of small per-draw constants ([`crate::DrawCall::immediates`]).
    pub immediate_bytes: u32,
    pub compute_workgroup_storage: u32,
    pub compute_invocations: u32,
    pub compute_size: [u32; 3],
    pub max_dispatch: u32,
    pub max_point_size: f32,
    pub max_line_width: f32,
    /// Largest anisotropy a sampler may ask for.
    pub max_anisotropy: u16,
    /// Transform feedback buffers, when [`Features::TRANSFORM_FEEDBACK`] is set.
    pub capture_buffers: u32,
}

bitflags! {
    /// What a format can be used for on this back end. A format the back end does not hold at all has
    /// no bits; fronts then store it in another format (slow paths [`crate::SlowPath::FormatConverted`],
    /// [`crate::SlowPath::NormAsInteger`], [`crate::SlowPath::BlockDecoded`]) or report it absent.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
    pub struct FormatUse: u16 {
        /// Sampled and loaded in programs. For a format with depth and stencil, each aspect through its
        /// own view ([`crate::TextureViewDesc::stencil`]); every back end serves both aspects.
        const SAMPLE = 1 << 0;
        const FILTER = 1 << 1;
        const RENDER = 1 << 2;
        const BLEND = 1 << 3;
        const MULTISAMPLE = 1 << 4;
        const RESOLVE = 1 << 5;
        const STORAGE_READ = 1 << 6;
        const STORAGE_WRITE = 1 << 7;
        const VERTEX = 1 << 8;
        const DEPTH_STENCIL = 1 << 9;
        /// Copies into a buffer for read back.
        const COPY_OUT = 1 << 10;
        /// Atomic operations on storage views (Direct3D requires them on `R32_UINT` and `R32_SINT`,
        /// and exchange on `R32_FLOAT`, from 11_0). Without it a front reads and writes such a view
        /// through a buffer holding its texels (slow path [`crate::SlowPath::AtomicsThroughBuffer`]).
        const STORAGE_ATOMIC = 1 << 11;
        /// Can be a surface's format in [`crate::Backend::configure_surface`]. Every back end takes
        /// [`Format::Rgba8Unorm`].
        const PRESENT = 1 << 12;
    }
}

/// Per-format abilities, indexed by [`Format::index`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatCaps(pub [FormatUse; Format::COUNT]);

impl FormatCaps {
    /// What `format` can be used for.
    pub fn get(&self, format: Format) -> FormatUse {
        self.0[format.index()]
    }
}

/// Everything a back end declares about itself.
#[derive(Clone, Debug)]
pub struct Caps {
    pub features: Features,
    pub limits: Limits,
    pub formats: FormatCaps,
}

impl Caps {
    /// Whether every feature in `f` is present.
    pub fn has(&self, f: Features) -> bool {
        self.features.contains(f)
    }
}
