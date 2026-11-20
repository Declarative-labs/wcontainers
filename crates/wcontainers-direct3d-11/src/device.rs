//! The device: object creation, each preceded by the checks Direct3D 11 makes at creation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use wcontainers_gpu::{
    AddressMode, Backend, BlendComponent, BlendFactor, BlendOp, BorderColor,
    BufferDesc as GpuBuffer, BufferUsage, Caps, ColorWrites, CompareFn, CullMode,
    DepthStencilDesc as GpuDepthStencil, DeviceStatus, Extent, Features, FillMode, Filter, Format,
    FormatKind, FormatUse, FrontFace, GpuError, LossReason, MAX_COLOR_TARGETS, MapIntent, Object,
    Origin, ProvokingVertex, RasterDesc, RenderTargetBlend, SamplerDesc as GpuSampler, ScalarKind,
    SlowPath, StencilFace, StencilOp, TextureDesc as GpuTexture, TextureDimension, TextureRegion,
    TextureUsage, TextureViewDesc, ViewDimension, WriteMode,
};

use crate::context::ContextState;
use crate::desc::*;
use crate::error::{Error, Result, WARNING_PREFIX, not_implemented};
use crate::format;
use crate::object::*;
use crate::shader::{self, Helpers, StageVariant};
pub use wcontainers_directx_graphics_infrastructure::FormatSupport;

/// The helper constants of both draw stages as written into the back end's data ring, the indirect
/// arguments buffer bound beside them (by id, 0 for none), and the set binding them.
pub(crate) struct HelperBlocks<B: Backend> {
    pub blocks: [Vec<u8>; 2],
    pub arguments: u64,
    /// The ids of the merged samplers bound beside the blocks.
    pub samplers: Vec<u64>,
    /// The id of the depth-stencil view whose aspects are bound beside them for early tests in
    /// the program (0 for none).
    pub early: u64,
    pub set: Kept<B, B::BindingSet>,
}

/// A program's key: its vertex (or compute) shader's id and variant, and its pixel shader's id
/// (0 for none) and variant.
/// A compute helper block as written and its binding set.
/// A compute program's helper block, the merged samplers' identities and the binding set of both.
pub(crate) type ComputeHelpers<B> = (Vec<u8>, Vec<u64>, Kept<B, <B as Backend>::BindingSet>);

pub(crate) type ProgramKey = (u64, StageVariant, u64, StageVariant);

/// A linked program and where its two stages read their helper constants.
pub(crate) struct Linked<B: Backend> {
    pub program: Kept<B, B::Program>,
    pub helpers: [Rc<Helpers>; 2],
}

/// A shader converted again as a variant, and where it reads its helper constants.
pub(crate) struct Variant<B: Backend> {
    pub shader: Kept<B, B::Shader>,
    /// The content key of its module.
    pub key: [u8; 32],
    pub helpers: Rc<Helpers>,
}

/// A vertex layout block's key: its input layout (zero for none), the bound strides, and the
/// vertex program whose input types it gives.
pub(crate) type LayoutKey = (u64, [u32; 32], u64);

/// A stage helper's shaders: the vertex shader's, input layout's, hull and domain shaders' (0
/// for none), geometry shader object's and pixel shader's ids, and on the line route the pixel
/// program's coverage location.
pub(crate) type StageKey = (u64, u64, [u64; 2], u64, u64, Option<u32>);

/// A Direct3D 11 device and its immediate context on back end `B`.
pub struct Device<B: Backend> {
    pub backend: B,
    pub(crate) graveyard: SharedGraveyard<B>,
    next_id: u64,
    level: u32,
    pub(crate) defaults: Defaults<B>,
    pub(crate) ctx: ContextState<B>,
    pub(crate) queries: crate::query::Queries<B>,
    /// Programs by their shaders' ids and variants (a compute program by its shader's id and 0).
    pub(crate) programs: HashMap<ProgramKey, Linked<B>>,
    /// Vertex shaders converted for one pixel shader's interpolation or for a variant, by the vertex
    /// shader's id and variant and the pixel shader's id.
    pub(crate) interpolated: HashMap<(u64, StageVariant, u64), Variant<B>>,
    /// Pixel shaders converted for a variant, by id.
    pub(crate) variants: HashMap<(u64, StageVariant), Variant<B>>,
    /// Counting programs of counted occlusion queries, by the vertex shader's id and variant and the
    /// pixel shader's id and variant; and the pixel counter's, by the vertex shader's id and variant
    /// and a number no pixel shader has.
    pub(crate) counting: HashMap<CountingKey, Kept<B, B::Program>>,
    /// Exact counts of counted occlusion queries, and the targets their draws drew on (the pass
    /// tracker holds them for the counter; the front opens and ends its own passes).
    pub(crate) counter: wcontainers_gpu::OcclusionCounter<B>,
    /// Pixel shader invocations of pipeline statistics queries: the draws replayed with the depth
    /// and stencil tests off (`tests_off`) through a program counting one per invocation.
    pub(crate) pixel_counter: wcontainers_gpu::OcclusionCounter<B>,
    pub(crate) tests_off: Option<Rc<DepthStencil<B>>>,
    pub(crate) passes: wcontainers_gpu::PassTracker<B>,
    /// The helper constants as last written. The front is the only writer of the back end's data
    /// ring, so the blocks stay as written until the front's next write.
    pub(crate) helper_blocks: Option<HelperBlocks<B>>,
    /// The compute program's helper block as last written and its binding set; the draws' blocks
    /// and this share the ring, so writing one forgets the other.
    pub(crate) compute_helpers: Option<ComputeHelpers<B>>,
    /// The lists of bound typed views' formats draws met (`StageVariant::formats`, which numbers
    /// them from one).
    pub(crate) format_lists: Vec<Vec<Format>>,
    pub(crate) format_ids: HashMap<Vec<Format>, u32>,
    /// Combinations drawn and built ahead.
    pub(crate) ahead: crate::ahead::Ahead<B>,
    /// Vertex layout blocks by (input layout id, stride of each slot it reads).
    pub(crate) layouts: HashMap<LayoutKey, Kept<B, B::VertexLayout>>,
    /// What empty slots read.
    pub(crate) zeros: Zeros<B>,
    /// The stage helpers of geometry shaders and stream output, by the vertex shader's, input
    /// layout's, hull and domain shaders' (0 for none), geometry shader object's and pixel
    /// shader's ids.
    pub(crate) staged: HashMap<StageKey, crate::context::StageRun<B>>,
    /// The geometry stage of a draw without a geometry shader object, which a culled vertex
    /// shader's draws take through the stage helper; made at its first use.
    pub(crate) no_geometry: Option<Rc<Geometry<B>>>,
    /// The stage helper of each vertex shader, input layout, hull, domain and pixel shader drawn
    /// together.
    pub(crate) tessellated: HashMap<(u64, u64, u64, u64, u64), crate::context::TessRun<B>>,
    /// The stage helper's filled sizes and primitive counts of a streaming draw.
    pub(crate) stream_filled: Option<Kept<B, B::Buffer>>,
    /// Depth and stencil saved before a draw run once per group of targets, and its description.
    pub(crate) split_depth: Option<(wcontainers_gpu::TextureDesc, Kept<B, B::Texture>)>,
    /// What the program's start built from the cache's record.
    pub(crate) warm: crate::cache::Warm<B>,
    /// Set once the device is removed; later work is dropped and the API reports the removal.
    pub removed: Option<Removal>,
}

/// What empty slots read, made at their first use: Direct3D reads zeros from an empty vertex,
/// constant or texture slot, and samples with its default sampler state through an empty sampler slot.
pub(crate) struct Zeros<B: Backend> {
    /// The largest constant buffer's bytes of zeros, also past any vertex element's offset.
    pub buffer: Option<Kept<B, B::Buffer>>,
    /// The same bytes for empty structured and raw buffer slots.
    pub storage: Option<Kept<B, B::Buffer>>,
    /// A texture of one zero texel per layer and its view, by view dimension and kind.
    pub textures: HashMap<(ViewDimension, FormatKind, u32), ZeroView<B>>,
    /// The zero buffer's bytes as a constants texture, for empty slots read through textures.
    pub constants: Option<WindowCopy<B>>,
    pub sampler: Option<Rc<Sampler<B>>>,
}

/// A texture of zeros and its view.
pub(crate) struct ZeroView<B: Backend> {
    _texture: Kept<B, B::Texture>,
    view: Kept<B, B::TextureView>,
}

/// A counting program's vertex shader id and variant, and pixel shader id (or pixel counter's
/// number) and variant.
pub(crate) type CountingKey = (u64, StageVariant, u64, StageVariant);

/// Bytes of the zero buffer: 4096 constants of 16 bytes.
pub(crate) const ZERO_BYTES: u64 = 65536;

/// Why the device was removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// The program asked for something the front cannot serve and that has no result to refuse it by.
    Unserved,
    /// The back end lost its device.
    Lost(LossReason),
}

/// A back-end error in the front's terms, where it does not remove a device.
fn gpu_error(e: GpuError) -> Error {
    match e {
        GpuError::Absent(f) => {
            eprintln!("{WARNING_PREFIX}: not implemented: the back end lacks {f:?}");
            Error::NotImplemented("a feature the back end lacks")
        }
        GpuError::OutOfMemory => Error::OutOfMemory,
        GpuError::DeviceLost => Error::DeviceRemoved,
        GpuError::Invalid(_) => Error::DeviceRemoved,
    }
}

/// The blocks a `None` state stands for.
pub(crate) struct Defaults<B: Backend> {
    pub blend: Rc<Blend<B>>,
    pub depth_stencil: Rc<DepthStencil<B>>,
    pub raster: Rc<Rasterizer<B>>,
}

/// The feature levels offered when a program asks for none, highest first (`D3D11CreateDevice`).
const DEFAULT_LEVELS: [u32; 6] = [
    level::L11_0,
    level::L10_1,
    level::L10_0,
    level::L9_3,
    level::L9_2,
    level::L9_1,
];

/// Direct3D's largest textures at feature level `level`: the extent of 1D and 2D textures, of a
/// cube's faces and of 3D textures, and the layers of an array (9_x has no arrays).
pub fn level_texture_limits(level: u32) -> [u32; 4] {
    match level {
        level::L9_1 | level::L9_2 => [2048, 512, 256, 1],
        level::L9_3 => [4096, 4096, 256, 1],
        level::L10_0 | level::L10_1 => [8192, 8192, 2048, 512],
        _ => [16384, 16384, 2048, 2048],
    }
}

/// The bytes of a 32-bit counter: a structured buffer's hidden counter, and the stage helper's
/// filled sizes and primitive counts.
pub(crate) const COUNTER_BYTES: usize = size_of::<u32>();

/// The color targets of 9_3 (`D3D_FL9_3_SIMULTANEOUS_RENDER_TARGET_COUNT`).
const L9_3_TARGETS: u32 = 4;

/// Refused: cull distances where the back end has none, outside a vertex shader or without
/// compute programs for the stage helper.
const CULL_DISTANCES_UNSERVED: &str = "cull distances past the vertex stage, or without the back end's or its compute programs \
     (CullDistanceAsGeometry)";

/// Refused: sharing below 10_0, or of more or fewer than one kind.
const GEOMETRY_STREAM_DRAWN_PAST_ZERO: &str =
    "drawing a geometry stream past 0 (the stage helper draws stream 0 alone)";
const SHARING_INVALID: &str = "sharing below 10_0, or not of exactly one kind";

/// The color targets a pass takes at feature level `level`.
fn level_targets(level: u32) -> u32 {
    match level {
        level::L9_1 | level::L9_2 => 1,
        level::L9_3 => L9_3_TARGETS,
        _ => MAX_COLOR_TARGETS as u32,
    }
}

/// A typed buffer view whose format the back end holds no texture of, where typed buffers are
/// read through texture copies (`TexelBufferAsTexture`).
const TYPED_BUFFER_FORMAT_UNHELD: &str =
    "a typed buffer view of a format the back end holds no texture of";
/// Typed unordered access buffers where the back end has no storage buffers.
const TYPED_STORAGE_BUFFERS_UNSERVED: &str =
    "typed unordered access buffers without the back end's storage buffers";
/// Geometry shaders and stream output where the back end has neither compute programs nor
/// transform feedback.
const GEOMETRY_STAGE_UNSERVED: &str = "geometry shaders and stream output without the back end's \
     compute programs or transform feedback";

/// Whether `caps` serves every requirement of feature level `level` (plan section 9): its texture
/// sizes and targets within the adapter's limits; from 10_0 `D32_FLOAT_S8X24_UINT` targets, float
/// targets, and the geometry stage and stream output, which the stage helper serves through compute
/// programs or, below 11_0, through transform feedback (its replay); from 10_1 cube arrays
/// (`cube_arrays`, which `Caps` does not name); at 11_0 compute.
fn serves(caps: &wcontainers_gpu::Caps, level: u32, cube_arrays: bool) -> bool {
    let limits = &caps.limits;
    let [extent, cube_extent, volume, layers] = level_texture_limits(level);
    let sized = limits.max_texture_2d >= extent
        && limits.max_texture_cube >= cube_extent
        && limits.max_texture_3d >= volume
        && limits.max_texture_layers >= layers
        && limits.color_targets >= level_targets(level);
    let renders = |f: Format, u: FormatUse| caps.formats.get(f).contains(u);
    let staged = caps.has(Features::COMPUTE)
        || (level < level::L11_0 && caps.has(Features::TRANSFORM_FEEDBACK));
    sized
        && (level < level::L10_0
            || (renders(Format::Depth32FloatStencil8, FormatUse::DEPTH_STENCIL)
                && renders(Format::Rgba32Float, FormatUse::RENDER)
                && staged))
        && (level < level::L10_1 || cube_arrays)
}

/// The highest feature level `caps` serves, if any, with `cube_arrays` as [`serves`] takes it.
pub fn top_level(caps: &wcontainers_gpu::Caps, cube_arrays: bool) -> Option<u32> {
    DEFAULT_LEVELS
        .into_iter()
        .find(|&l| serves(caps, l, cube_arrays))
}

/// Faces of a cube.
pub(crate) const CUBE_FACES: u32 = 6;

/// Whether `backend` makes textures of more than one cube, which `Caps` does not say: a texture of
/// two cubes of one texel, made and destroyed.
fn makes_cube_arrays<B: Backend>(backend: &mut B) -> bool {
    let made = backend.create_texture(&GpuTexture {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: 1,
        height: 1,
        depth_or_layers: 2 * CUBE_FACES,
        mip_levels: 1,
        samples: 1,
        usage: TextureUsage::SAMPLE,
        cube: true,
        reinterpret: false,
    });
    made.map(|t| backend.destroy(Object::Texture(t))).is_ok()
}

impl<B: Backend> Device<B> {
    /// Makes the device at the first of `levels` (or of Direct3D's default list) the back end serves.
    pub fn new(mut backend: B, levels: &[u32]) -> Result<Self> {
        let cube_arrays = makes_cube_arrays(&mut backend);
        let top = top_level(backend.caps(), cube_arrays).ok_or(Error::Unsupported)?;
        let asked: &[u32] = if levels.is_empty() {
            &DEFAULT_LEVELS
        } else {
            levels
        };
        let level = *asked
            .iter()
            .find(|&&l| l <= top && l >= level::L9_1)
            .ok_or(Error::Unsupported)?;
        let graveyard = Rc::new(RefCell::new(Graveyard {
            objects: Vec::new(),
            ids: Vec::new(),
        }));
        let mut next_id = 1;
        let mut id = || {
            next_id += 1;
            next_id
        };
        let blend_desc = BlendDesc::default_state();
        let ds_desc = DepthStencilDesc::default_state();
        let raster_desc = RasterizerDesc::default_state();
        let blend = backend
            .create_blend(&blend_block(&blend_desc))
            .map_err(gpu_error)?;
        let ds = backend
            .create_depth_stencil(&depth_stencil_block(&ds_desc))
            .map_err(gpu_error)?;
        let raster = backend
            .create_raster(&raster_block(&raster_desc, backend.caps()))
            .map_err(gpu_error)?;
        let warm = crate::cache::Warm::start(&mut backend, &graveyard, &mut id);
        let defaults = Defaults {
            blend: Rc::new(Blend {
                desc: blend_desc,
                gpu: Kept::new(blend, id(), Object::Blend, &graveyard),
                groups: Vec::new(),
            }),
            depth_stencil: Rc::new(DepthStencil {
                desc: ds_desc,
                gpu: Kept::new(ds, id(), Object::DepthStencil, &graveyard),
                read_only: RefCell::new(Vec::new()),
            }),
            raster: Rc::new(Rasterizer {
                desc: raster_desc,
                gpu: Kept::new(raster, id(), Object::Raster, &graveyard),
                masked: RefCell::new(Vec::new()),
            }),
        };
        Ok(Device {
            backend,
            graveyard,
            next_id,
            level,
            defaults,
            ctx: ContextState::new(),
            queries: crate::query::Queries::new(),
            programs: HashMap::new(),
            interpolated: HashMap::new(),
            variants: HashMap::new(),
            counting: HashMap::new(),
            counter: Default::default(),
            pixel_counter: Default::default(),
            tests_off: None,
            passes: Default::default(),
            helper_blocks: None,
            compute_helpers: None,
            format_lists: Vec::new(),
            format_ids: HashMap::new(),
            ahead: crate::ahead::Ahead::new(),
            layouts: HashMap::new(),
            zeros: Zeros {
                buffer: None,
                storage: None,
                textures: HashMap::new(),
                constants: None,
                sampler: None,
            },
            staged: HashMap::new(),
            no_geometry: None,
            tessellated: HashMap::new(),
            stream_filled: None,
            split_depth: None,
            warm,
            removed: None,
        })
    }

    pub fn feature_level(&self) -> u32 {
        self.level
    }

    /// What `CheckFormatSupport` and `D3D11_FEATURE_FORMAT_SUPPORT2` answer for DXGI format
    /// `dxgi`: the infrastructure crate's answer from the back end's uses, with what this front
    /// serves by its own routes and what its feature level allows.
    pub fn format_support(&self, dxgi: u32) -> FormatSupport {
        use crate::desc::support::*;
        use crate::desc::support2::*;
        let caps = self.backend.caps();
        if matches!(dxgi, format::NV12 | format::YUY2) {
            return FormatSupport {
                support1: self.video_format_support(dxgi),
                support2: 0,
            };
        }
        let mut s = wcontainers_directx_graphics_infrastructure::format_support(caps, dxgi);
        let as_integer = format::texture_format(dxgi).and_then(|f| self.held_as_integer(f));
        if let Some(twin) = as_integer.and_then(format::dxgi_of) {
            // Held as its integer twin: what the twin serves, sampled, gathered and its mips
            // generated through decoding, never blended (`BlendInShader` is not built), and not
            // multisampled (plan 4.12).
            let vertex = s.support1 & (IA_VERTEX_BUFFER | BUFFER);
            s = wcontainers_directx_graphics_infrastructure::format_support(caps, twin);
            s.support1 &= !(BLENDABLE
                | MULTISAMPLE_RENDERTARGET
                | MULTISAMPLE_LOAD
                | MULTISAMPLE_RESOLVE
                | IA_VERTEX_BUFFER
                | BUFFER);
            s.support1 |= vertex;
            if s.support1 & SHADER_LOAD != 0 {
                s.support1 |= SHADER_SAMPLE | SHADER_GATHER;
            }
            s.support2 = 0;
        }
        if !format::storage_viewed(dxgi) {
            s.support1 &= !TYPED_UNORDERED_ACCESS_VIEW;
            s.support2 &= !(UAV_TYPED_LOAD | UAV_TYPED_STORE);
        }
        let level = self.level;
        let texture = format::texture_format(dxgi);
        let target = s.support1 & RENDER_TARGET != 0;
        // Staging resources of every format a buffer or texture takes.
        let held = s.support1 & (BUFFER | TEXTURE2D) != 0;
        let mut add = |on: bool, bit: u32| {
            if on {
                s.support1 |= bit;
            }
        };
        // Typed buffers decode each element as the vertex format of the same bytes
        // (`TexelBufferAsStorage`), or are read through a texture copy of the format
        // (`TexelBufferAsTexture`).
        let typed_buffer = if caps.limits.storage_buffers_per_stage > 0 {
            format::vertex_format(dxgi).is_some()
        } else {
            format::vertex_format(dxgi).is_some()
                && texture.is_some_and(|f| caps.formats.get(f).contains(FormatUse::SAMPLE))
        };
        add(typed_buffer, BUFFER);
        add(format::stream_output_format(dxgi), SO_BUFFER);
        add(held, CPU_LOCKABLE);
        // `GenerateMips` filters every format it draws into but integers.
        add(
            target && texture.is_some_and(|f| !format::is_integer(f)),
            MIP_AUTOGEN,
        );
        // Storage views of formats the back end cannot store into go through a buffer of their
        // texels (`StorageFormatThroughBuffer`), which serves loads too.
        let through = texture.is_some_and(|f| {
            format::storage_viewed(dxgi)
                && (self.stored_through(f) || as_integer.is_some_and(|t| self.through_takes(t)))
        });
        if through {
            s.support1 |= TYPED_UNORDERED_ACCESS_VIEW;
            s.support2 |= UAV_TYPED_STORE | UAV_TYPED_LOAD;
        }
        // Atomics on storage textures the back end lacks go through a buffer of their texels
        // (`AtomicsThroughBuffer`).
        if caps.has(Features::COMPUTE) {
            s.support2 |= match dxgi {
                format::R32_UINT | format::R32_SINT => {
                    UAV_ATOMIC_ADD
                        | UAV_ATOMIC_BITWISE_OPS
                        | UAV_ATOMIC_COMPARE_STORE_OR_COMPARE_EXCHANGE
                        | UAV_ATOMIC_EXCHANGE
                        | UAV_ATOMIC_SIGNED_MIN_OR_MAX
                        | UAV_ATOMIC_UNSIGNED_MIN_OR_MAX
                }
                format::R32_FLOAT => UAV_ATOMIC_EXCHANGE,
                _ => 0,
            };
        }
        // What the feature level takes away.
        let mut remove = |off: bool, bit: u32| {
            if off {
                s.support1 &= !bit;
            }
        };
        remove(!format::level_vertex_buffer(dxgi, level), IA_VERTEX_BUFFER);
        remove(!format::level_index_buffer(dxgi, level), IA_INDEX_BUFFER);
        remove(
            level < level::L10_0,
            BUFFER | SO_BUFFER | SHADER_SAMPLE_COMPARISON,
        );
        remove(level < level::L10_1, SHADER_GATHER);
        // Typed storage views and gathers with comparison are shader model 5's.
        remove(
            level < level::L11_0,
            TYPED_UNORDERED_ACCESS_VIEW | SHADER_GATHER_COMPARISON,
        );
        remove(
            level < level::L10_1 && texture.is_some_and(format::signed_normalized)
                || !format::level_blended(dxgi, level),
            BLENDABLE,
        );
        remove(!format::displayed(dxgi, level), DISPLAY);
        if level < level::L11_0 || s.support1 == 0 {
            s.support2 = 0;
        }
        s
    }

    /// `CheckFormatSupport` of NV12 and YUY2: 2D textures of one level, through views of each
    /// plane's format (NV12) or of R8G8B8A8 (YUY2).
    fn video_format_support(&self, dxgi: u32) -> u32 {
        use crate::desc::support::*;
        let formats = &self.backend.caps().formats;
        let uses = if dxgi == format::NV12 {
            formats.get(Format::R8Unorm) & formats.get(Format::Rg8Unorm)
        } else {
            formats.get(Format::Rgba8Unorm)
        };
        if uses.is_empty() {
            return 0;
        }
        let mut bits = TEXTURE2D;
        for (has, bit) in [
            (FormatUse::SAMPLE, SHADER_LOAD),
            (FormatUse::FILTER, SHADER_SAMPLE),
            (FormatUse::RENDER, RENDER_TARGET),
            (FormatUse::COPY_OUT, CPU_LOCKABLE),
        ] {
            // Direct3D reports no target use of YUY2, though it takes target views of it.
            if uses.contains(has) && !(dxgi == format::YUY2 && bit == RENDER_TARGET) {
                bits |= bit;
            }
        }
        bits
    }

    /// Whether textures of `format` drawn into or for depth take `samples` samples.
    fn takes_samples(&self, format: Format, samples: u32) -> bool {
        let uses = self.backend.caps().formats.get(format);
        uses.intersects(FormatUse::RENDER | FormatUse::DEPTH_STENCIL)
            && self.backend.caps().formats.takes_samples(format, samples)
    }

    /// What `CheckMultisampleQualityLevels` answers: one level for a single sample of a format
    /// textures can have, and for a count the back end draws that format with; none otherwise.
    pub fn multisample_quality_levels(&self, dxgi: u32, samples: u32) -> u32 {
        if samples == 1 {
            return u32::from(
                self.format_support(dxgi).support1 & crate::desc::support::TEXTURE2D != 0,
            );
        }
        let format = format::texture_format(dxgi).or_else(|| {
            format::is_typeless(dxgi)
                .then(|| format::storage_format(dxgi, false).ok())
                .flatten()
        });
        u32::from(format.is_some_and(|f| self.takes_samples(f, samples)))
    }

    /// Why the device is removed, if it is, polling the back end first (plan section 4.10).
    pub fn poll_removed(&mut self) -> Option<Removal> {
        if self.removed.is_none()
            && let DeviceStatus::Lost(reason) = self.backend.status()
        {
            eprintln!("{WARNING_PREFIX}: device removed: the back end lost it ({reason:?})");
            self.removed = Some(Removal::Lost(reason));
        }
        self.removed
    }

    /// `DeviceRemoved` once the device is removed.
    pub(crate) fn live(&mut self) -> Result<()> {
        match self.poll_removed() {
            Some(_) => Err(Error::DeviceRemoved),
            None => Ok(()),
        }
    }

    /// Removes the device because of `e`: a request it cannot serve, or the back end's loss.
    pub fn lose(&mut self, e: Error) {
        self.remove(&format!("{e:?}"));
    }

    fn remove(&mut self, why: &str) {
        if self.poll_removed().is_none() {
            eprintln!("{WARNING_PREFIX}: device removed: {why}");
            self.removed = Some(Removal::Unserved);
        }
    }

    /// The back end's answer in the front's terms. A loss or a call the back end refused removes the
    /// device, with the back end's reason.
    pub(crate) fn gpu<T>(&mut self, r: core::result::Result<T, GpuError>) -> Result<T> {
        r.map_err(|e| match e {
            GpuError::Absent(_) | GpuError::OutOfMemory => gpu_error(e),
            GpuError::DeviceLost => {
                if self.poll_removed().is_none() {
                    eprintln!("{WARNING_PREFIX}: device removed: the back end lost it");
                    self.removed = Some(Removal::Lost(LossReason::Removed));
                }
                Error::DeviceRemoved
            }
            GpuError::Invalid(rule) => {
                self.remove(&format!("the back end refused a call: {rule}"));
                Error::DeviceRemoved
            }
        })
    }

    /// The warnings not yet shown: each slow path the first time it is taken.
    pub fn take_warnings(&mut self) -> Vec<&'static str> {
        self.backend
            .slow_paths()
            .take_warnings()
            .into_iter()
            .map(SlowPath::reason)
            .collect()
    }

    /// The zero buffer, made at its first use.
    pub(crate) fn zero_buffer(&mut self) -> Result<B::Buffer> {
        if let Some(b) = &self.zeros.buffer {
            return Ok(b.handle.clone());
        }
        let b = self.make_zeros(BufferUsage::VERTEX | BufferUsage::UNIFORM)?;
        let handle = b.handle.clone();
        self.zeros.buffer = Some(b);
        Ok(handle)
    }

    /// The zero buffer read as a structured or raw buffer (storage usage, which back ends without
    /// storage buffers lack, so apart from the other), made at its first use.
    pub(crate) fn zero_storage(&mut self) -> Result<B::Buffer> {
        if let Some(b) = &self.zeros.storage {
            return Ok(b.handle.clone());
        }
        let b = self.make_zeros(BufferUsage::STORAGE)?;
        let handle = b.handle.clone();
        self.zeros.storage = Some(b);
        Ok(handle)
    }

    fn make_zeros(&mut self, usage: BufferUsage) -> Result<Kept<B, B::Buffer>> {
        let desc = GpuBuffer {
            size: ZERO_BYTES,
            usage: usage | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        };
        let gpu = self.backend.create_buffer(&desc);
        let gpu = self.gpu(gpu)?;
        self.backend.clear_buffer(&gpu, 0, ZERO_BYTES, &[0]);
        let id = self.id();
        Ok(Kept::new(gpu, id, Object::Buffer, &self.graveyard))
    }

    /// A view of zeros of `kind` in `dimension`, of `samples` samples (one, or four for a
    /// multisampled 2D slot), made at its first use.
    pub(crate) fn zero_view(
        &mut self,
        dimension: ViewDimension,
        kind: FormatKind,
        samples: u32,
    ) -> Result<B::TextureView> {
        if let Some(z) = self.zeros.textures.get(&(dimension, kind, samples)) {
            return Ok(z.view.handle.clone());
        }
        let format = match kind {
            FormatKind::Float => Format::Rgba8Unorm,
            FormatKind::Uint => Format::Rgba8Uint,
            FormatKind::Sint => Format::Rgba8Sint,
            _ => return not_implemented("an empty slot a shader reads as depth or stencil"),
        };
        let (texture_dimension, layers) = match dimension {
            ViewDimension::D2 | ViewDimension::D2Array => (TextureDimension::D2, 1),
            ViewDimension::Cube | ViewDimension::CubeArray => (TextureDimension::D2, CUBE_FACES),
            ViewDimension::D3 => (TextureDimension::D3, 1),
        };
        // Multisampled textures take no writes: they are cleared by a pass.
        let filled = if samples > 1 {
            TextureUsage::RENDER
        } else {
            TextureUsage::COPY_DST
        };
        let desc = GpuTexture {
            dimension: texture_dimension,
            format,
            width: 1,
            height: 1,
            depth_or_layers: layers,
            mip_levels: 1,
            samples,
            usage: TextureUsage::SAMPLE | filled,
            cube: layers == CUBE_FACES,
            reinterpret: false,
        };
        let texture = self.backend.create_texture(&desc);
        let texture = self.gpu(texture)?;
        if samples > 1 {
            let r = self.backend.clear_texture(
                &texture,
                &desc,
                format,
                0..1,
                0..1,
                wcontainers_gpu::ClearValue::Color([0; 4]),
            );
            self.gpu(r)?;
        } else {
            self.backend.write_texture(
                &TextureRegion {
                    texture: &texture,
                    mip: 0,
                    layer: 0,
                    origin: Origin { x: 0, y: 0, z: 0 },
                    stencil: false,
                },
                Extent {
                    width: 1,
                    height: 1,
                    depth_or_layers: layers,
                },
                &[0; 24],
                4,
                1,
                WriteMode::Discard,
            );
        }
        let view = self.backend.create_view(
            &texture,
            &TextureViewDesc {
                format,
                dimension,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: layers,
                stencil: false,
            },
        );
        let view = self.gpu(view)?;
        let (t, v) = (self.id(), self.id());
        self.zeros.textures.insert(
            (dimension, kind, samples),
            ZeroView {
                _texture: Kept::new(texture, t, Object::Texture, &self.graveyard),
                view: Kept::new(view.clone(), v, Object::TextureView, &self.graveyard),
            },
        );
        Ok(view)
    }

    /// The sampler of Direct3D's default sampler state, made at its first use.
    pub(crate) fn default_sampler(&mut self) -> Result<B::Sampler> {
        if let Some(s) = &self.zeros.sampler {
            return Ok(s.gpu.handle.clone());
        }
        let s = self.create_sampler_state(&SamplerDesc::default_state())?;
        let handle = s.gpu.handle.clone();
        self.zeros.sampler = Some(s);
        Ok(handle)
    }

    pub(crate) fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Destroys the back-end objects whose front objects went, and forgets what was built from them.
    pub(crate) fn bury(&mut self) {
        let (objects, ids) = {
            let mut g = self.graveyard.borrow_mut();
            (std::mem::take(&mut g.objects), std::mem::take(&mut g.ids))
        };
        if objects.is_empty() {
            return;
        }
        for object in objects {
            self.backend.destroy(object);
        }
        self.programs
            .retain(|k, _| !ids.contains(&k.0) && !ids.contains(&k.2));
        self.interpolated
            .retain(|(v, _, p), _| !ids.contains(v) && !ids.contains(p));
        self.variants.retain(|(p, _), _| !ids.contains(p));
        self.counting
            .retain(|(v, _, p, _), _| !ids.contains(v) && !ids.contains(p));
        self.layouts
            .retain(|(l, _, v), _| !ids.contains(l) && !ids.contains(v));
        self.bury_built();
    }

    /// Destroys what the program and layout caches dropped (their `Kept` handles land in the graveyard).
    fn bury_built(&mut self) {
        let objects = std::mem::take(&mut self.graveyard.borrow_mut().objects);
        for object in objects {
            self.backend.destroy(object);
        }
    }

    /// A format the back end lacks altogether is not served; one it holds without these uses is
    /// refused as a driver without them refuses it.
    /// Whether storage views of `format` are read and written through a buffer of their texels,
    /// where the back end copies the format but cannot store into it (`StorageFormatThroughBuffer`).
    pub(crate) fn stored_through(&self, format: Format) -> bool {
        !self
            .backend
            .caps()
            .formats
            .get(format)
            .contains(FormatUse::STORAGE_WRITE)
            && self.through_takes(format)
    }

    /// Whether storage views of a texture stored as `format` can be read and written through a
    /// buffer of its texels.
    pub(crate) fn through_takes(&self, format: Format) -> bool {
        let caps = self.backend.caps();
        caps.formats.get(format).contains(FormatUse::COPY_OUT)
            && format.block().0 == 1
            && caps.has(Features::COMPUTE)
            && caps.limits.storage_buffers_per_stage > 0
    }

    /// Whether typed buffers are read through texture copies of their buffers, where the back end
    /// has no storage buffers (`TexelBufferAsTexture`).
    pub(crate) fn typed_buffers_textured(&self) -> bool {
        self.backend.caps().limits.storage_buffers_per_stage == 0
    }

    /// The integer twin a texture of normalised `format` is held as where the back end lacks the
    /// format and samples the twin (`NormAsInteger`): programs decode its loads and encode their
    /// outputs.
    pub(crate) fn held_as_integer(&self, format: Format) -> Option<Format> {
        let formats = &self.backend.caps().formats;
        format.integer_twin().filter(|&twin| {
            formats.get(format).is_empty() && formats.get(twin).contains(FormatUse::SAMPLE)
        })
    }

    /// The use storage views of `format` ask of the back end: none where they are stored through
    /// a buffer.
    fn storage_use(&self, format: Format) -> FormatUse {
        if self.stored_through(format) {
            FormatUse::empty()
        } else {
            FormatUse::STORAGE_WRITE
        }
    }

    fn check_format(&self, format: Format, needs: FormatUse) -> Result<()> {
        let has = self.backend.caps().formats.get(format);
        if has.is_empty() {
            not_implemented("a format the back end lacks")
        } else if has.contains(needs) {
            Ok(())
        } else if (needs.contains(FormatUse::DEPTH_STENCIL) && !format.is_depth_stencil())
            || (needs.contains(FormatUse::RENDER)
                && (format.is_depth_stencil() || format.block().0 > 1))
        {
            // Direct3D's own rule: depth formats are bound for depth only, block formats never
            // as targets.
            Err(Error::InvalidArg(
                "the format does not support the bind flags",
            ))
        } else {
            not_implemented("a use of a format the back end lacks")
        }
    }

    pub fn create_buffer(&mut self, d: &BufferDesc, init: Option<&[u8]>) -> Result<Rc<Buffer<B>>> {
        check_buffer(d, init.is_some())?;
        if d.misc_flags & (MISC_GENERATE_MIPS | MISC_TEXTURECUBE | MISC_GDI_COMPATIBLE) != 0 {
            return Err(Error::InvalidArg("a texture's misc flags on a buffer"));
        }
        if d.misc_flags
            & !(MISC_DRAWINDIRECT_ARGS | MISC_ALLOW_RAW_VIEWS | MISC_STRUCTURED | MISC_SHARED)
            != 0
        {
            return not_implemented(
                "buffers with misc flags other than structured, raw, indirect and shared",
            );
        }
        self.live()?;
        self.bury();
        // The buffer is made whole granules of the back end's copies long, so writing the initial data
        // (padded with zeros past the program's bytes) never needs an edge merge.
        let granule = u64::from(self.backend.caps().limits.buffer_copy_alignment.max(1));
        let size = u64::from(d.byte_width).div_ceil(granule) * granule;
        let mut desc = gpu_buffer(d, size);
        // Vertex programs fetch the streams the back end cannot step by their rate; the stage
        // helper's compute programs read vertices and indices and write stream output.
        let caps = self.backend.caps();
        let staged = bind::VERTEX_BUFFER | bind::INDEX_BUFFER | bind::STREAM_OUTPUT;
        if d.bind_flags & bind::VERTEX_BUFFER != 0 && !caps.has(Features::INSTANCE_STEP_RATE)
            || d.bind_flags & staged != 0 && caps.has(Features::COMPUTE)
        {
            desc.usage |= BufferUsage::STORAGE;
        }
        let gpu = self.backend.create_buffer(&desc);
        let gpu = self.gpu(gpu)?;
        if let Some(bytes) = init {
            let bytes = &bytes[..d.byte_width as usize];
            if bytes.len() as u64 == size {
                self.backend
                    .write_buffer(&gpu, 0, bytes, WriteMode::Discard);
            } else {
                let mut padded = bytes.to_vec();
                padded.resize(size as usize, 0);
                self.backend
                    .write_buffer(&gpu, 0, &padded, WriteMode::Discard);
            }
        }
        let shadow = if d.usage == usage::DYNAMIC || d.usage == usage::STAGING {
            let mut s = vec![0; d.byte_width as usize];
            if let Some(bytes) = init {
                s.copy_from_slice(&bytes[..d.byte_width as usize]);
            }
            s
        } else {
            Vec::new()
        };
        let id = self.id();
        Ok(Rc::new(Buffer {
            desc: *d,
            gpu: RefCell::new(Kept::new(gpu, id, Object::Buffer, &self.graveyard)),
            size: Cell::new(size),
            grown: Cell::new(false),
            shadow: RefCell::new(shadow),
            staged: RefCell::new(Staged::Current),
            writes: Cell::new(0),
            windows: RefCell::new(Vec::new()),
            texels: RefCell::new(Vec::new()),
            filled: RefCell::new(None),
        }))
    }

    /// The checks of `CreateTexture1D`, `2D` and `3D` before their initial data is read: the format
    /// and the number of mip levels, or why the description is refused.
    pub fn check_texture(&self, d: &TextureDesc, has_init: bool) -> Result<(Format, u32)> {
        let limits = &self.backend.caps().limits;
        if d.width == 0 || d.height == 0 || d.depth == 0 || d.array_size == 0 {
            return Err(Error::InvalidArg("a texture needs a size"));
        }
        let [extent, cube_extent, volume, level_layers] = level_texture_limits(self.level);
        let cube = d.misc_flags & MISC_TEXTURECUBE != 0;
        let (most, layers) = match d.dimension {
            TexDim::D3 => (limits.max_texture_3d.min(volume), 1),
            _ if cube => (
                limits.max_texture_cube.min(cube_extent),
                limits.max_texture_layers.min(level_layers.max(CUBE_FACES)),
            ),
            _ => (
                limits.max_texture_2d.min(extent),
                limits.max_texture_layers.min(level_layers),
            ),
        };
        if d.width > most || d.height > most || d.depth > most || d.array_size > layers {
            return Err(Error::InvalidArg(
                "the texture is larger than the feature level allows",
            ));
        }
        let full = 32 - d.width.max(d.height).max(d.depth).leading_zeros();
        let mips = if d.mip_levels == 0 {
            full
        } else {
            d.mip_levels
        };
        if mips > full {
            return Err(Error::InvalidArg("more mip levels than the size has"));
        }
        if d.format == format::NV12 {
            if d.dimension != TexDim::D2
                || !d.width.is_multiple_of(2)
                || !d.height.is_multiple_of(2)
                || d.sample_count > 1
                || d.bind_flags & bind::DEPTH_STENCIL != 0
            {
                return Err(Error::InvalidArg(
                    "an NV12 texture is 2D, of one sample and of even width and height",
                ));
            }
            if mips != 1 || d.misc_flags & !MISC_SHARED != 0 {
                return not_implemented("NV12 textures of more than one level or with misc flags");
            }
        }
        if d.format == format::YUY2 {
            if d.dimension != TexDim::D2
                || !d.width.is_multiple_of(2)
                || d.sample_count > 1
                || d.bind_flags & bind::DEPTH_STENCIL != 0
            {
                return Err(Error::InvalidArg(
                    "a YUY2 texture is 2D, of one sample and of even width",
                ));
            }
            if mips != 1 || d.misc_flags & !MISC_SHARED != 0 {
                return not_implemented("YUY2 textures of more than one level or with misc flags");
            }
        }
        let drawn = bind::RENDER_TARGET | bind::UNORDERED_ACCESS;
        if d.bind_flags & drawn != 0
            && (d.bind_flags & bind::DEPTH_STENCIL != 0
                || format::never_target_or_storage(d.format))
            || format::depth_only(d.format) && d.bind_flags & (drawn | bind::SHADER_RESOURCE) != 0
        {
            return Err(Error::InvalidArg(
                "the format does not support the bind flags",
            ));
        }
        // Sharing is from 10_0, of exactly one kind: shared or with a keyed mutex, either through an
        // NT handle or not.
        let sharing = d.misc_flags & (MISC_SHARED | MISC_SHARED_KEYEDMUTEX);
        if (sharing != 0 || d.misc_flags & MISC_SHARED_NTHANDLE != 0)
            && (self.level < level::L10_0 || !sharing.is_power_of_two())
        {
            return Err(Error::InvalidArg(SHARING_INVALID));
        }
        let format = format::storage_format(d.format, d.bind_flags & bind::DEPTH_STENCIL != 0)?;
        let format = self.held_as_integer(format).unwrap_or(format);
        // A count and quality the device reports no level for is invalid, as in Direct3D. The
        // standard pattern is the back end's own; the centred one it cannot place. A single
        // sample has quality 0; whether its format serves the bindings is checked below.
        let levels = if d.sample_count == 1 {
            1
        } else {
            self.multisample_quality_levels(d.format, d.sample_count)
        };
        match d.sample_quality {
            STANDARD_MULTISAMPLE_PATTERN if levels > 0 => {}
            CENTER_MULTISAMPLE_PATTERN if levels > 0 => {
                return not_implemented("samples all at the pixel's centre");
            }
            q if q < levels => {}
            _ => {
                return Err(Error::InvalidArg(
                    "a sample count and quality the device reports no level for",
                ));
            }
        }
        if d.sample_count > 1
            && (d.dimension != TexDim::D2
                || mips != 1
                || d.usage != usage::DEFAULT
                || d.bind_flags & bind::UNORDERED_ACCESS != 0)
        {
            return Err(Error::InvalidArg(
                "a multisampled texture is 2D, of one level and default usage",
            ));
        }
        // The back ends' multisampled textures are one layer; holding an array as a texture per
        // layer (plan 4.2) is not built here.
        if d.sample_count > 1 && d.array_size > 1 {
            return not_implemented("a multisampled texture of more than one layer");
        }
        if d.misc_flags
            & !(MISC_TEXTURECUBE
                | MISC_GENERATE_MIPS
                | MISC_RESOURCE_CLAMP
                | MISC_SHARED
                | MISC_GDI_COMPATIBLE)
            != 0
        {
            return not_implemented(
                "textures with misc flags other than a cube, mip generation, a level clamp, shared \
                 and GDI-compatible",
            );
        }
        // GDI draws into B8G8R8A8 render targets of one sample and default usage.
        if d.misc_flags & MISC_GDI_COMPATIBLE != 0
            && (!matches!(d.format, 87 | 90 | 91)
                || d.bind_flags & bind::RENDER_TARGET == 0
                || d.usage != usage::DEFAULT
                || d.sample_count != 1
                || d.dimension != TexDim::D2)
        {
            return Err(Error::InvalidArg(
                "a GDI-compatible texture is a B8G8R8A8 2D render target of one sample",
            ));
        }
        let both = bind::SHADER_RESOURCE | bind::RENDER_TARGET;
        if d.misc_flags & MISC_GENERATE_MIPS != 0 && d.bind_flags & both != both {
            return Err(Error::InvalidArg(
                "mip generation needs shader resource and render target bindings",
            ));
        }
        if cube && (d.array_size < CUBE_FACES || d.width != d.height || d.dimension != TexDim::D2) {
            return Err(Error::InvalidArg(
                "a cube texture needs at least six square faces",
            ));
        }
        // Arrays of cubes came with 10_1.
        if cube && self.level < level::L10_1 && d.array_size != CUBE_FACES {
            return Err(Error::InvalidArg(
                "a cube texture below 10_1 has exactly six faces",
            ));
        }
        match d.usage {
            usage::IMMUTABLE if !has_init => {
                return Err(Error::InvalidArg("an immutable texture needs initial data"));
            }
            usage::IMMUTABLE
                if d.bind_flags == 0
                    || d.bind_flags
                        & (bind::RENDER_TARGET | bind::DEPTH_STENCIL | bind::UNORDERED_ACCESS)
                        != 0 =>
            {
                return Err(Error::InvalidArg(
                    "an immutable texture is only read, through a binding",
                ));
            }
            usage::STAGING if d.bind_flags != 0 || d.cpu_access_flags == 0 => {
                return Err(Error::InvalidArg(
                    "a staging texture has CPU access and no bind flags",
                ));
            }
            // The CPU writes a dynamic texture and the GPU only samples it.
            usage::DYNAMIC
                if d.cpu_access_flags != cpu::WRITE || d.bind_flags != bind::SHADER_RESOURCE =>
            {
                return Err(Error::InvalidArg(
                    "a dynamic texture is written by the CPU and only sampled",
                ));
            }
            usage::DEFAULT | usage::IMMUTABLE | usage::STAGING | usage::DYNAMIC => {}
            _ => return Err(Error::InvalidArg("unknown usage")),
        }
        if !matches!(d.usage, usage::STAGING | usage::DYNAMIC) && d.cpu_access_flags != 0 {
            return Err(Error::InvalidArg(
                "only staging and dynamic textures have CPU access",
            ));
        }
        let mut needs = FormatUse::empty();
        for (flag, n) in [
            (bind::SHADER_RESOURCE, FormatUse::SAMPLE),
            (bind::RENDER_TARGET, FormatUse::RENDER),
            (bind::DEPTH_STENCIL, FormatUse::DEPTH_STENCIL),
            (bind::UNORDERED_ACCESS, self.storage_use(format)),
        ] {
            if d.bind_flags & flag != 0
                && !(flag == bind::UNORDERED_ACCESS && format::is_srgb(d.format))
            {
                needs |= n;
            }
        }
        // A staging texture the CPU reads is read back from the GPU.
        if d.usage == usage::STAGING && d.cpu_access_flags & cpu::READ != 0 {
            needs |= FormatUse::COPY_OUT;
        }
        if d.bind_flags & bind::UNORDERED_ACCESS != 0 && self.feature_level() < 0xb000 {
            return Err(Error::InvalidArg(
                "unordered access needs feature level 11_0",
            ));
        }
        let bindable = bind::SHADER_RESOURCE
            | bind::RENDER_TARGET
            | bind::DEPTH_STENCIL
            | bind::UNORDERED_ACCESS;
        if d.bind_flags & !bindable != 0 {
            return Err(Error::InvalidArg("bind flags a texture cannot have"));
        }
        if d.dimension == TexDim::D3 && d.bind_flags & bind::DEPTH_STENCIL != 0 {
            return Err(Error::InvalidArg("a 3D texture cannot hold depth"));
        }
        self.check_format(format, needs)?;
        Ok((format, mips))
    }

    pub fn create_texture(
        &mut self,
        d: &TextureDesc,
        init: &[SubresourceData<'_>],
    ) -> Result<Rc<Texture<B>>> {
        let (format, mips) = self.check_texture(d, !init.is_empty())?;
        if format::texture_format(d.format)
            .is_some_and(|f| f != format && f.integer_twin() == Some(format))
        {
            self.backend.slow_paths().hit(SlowPath::NormAsInteger);
        }
        // The back end's cubes are whole: a cube texture of other layer counts is a 2D array
        // whose cube views see its first sixes.
        let cube = d.misc_flags & MISC_TEXTURECUBE != 0 && d.array_size.is_multiple_of(CUBE_FACES);
        let mut usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
        for (flag, u) in [
            (bind::SHADER_RESOURCE, TextureUsage::SAMPLE),
            (bind::RENDER_TARGET, TextureUsage::RENDER),
            (bind::DEPTH_STENCIL, TextureUsage::RENDER),
            (bind::UNORDERED_ACCESS, TextureUsage::STORAGE),
        ] {
            if d.bind_flags & flag != 0
                && !(u == TextureUsage::STORAGE
                    && (format::is_srgb(d.format) || self.stored_through(format)))
            {
                usage |= u;
            }
        }
        // The back end multisamples and resolves only into textures drawn into, which Direct3D does
        // not require of a multisampled texture or of a resolve's destination.
        let resolves_into = d.usage == usage::DEFAULT
            && d.dimension == TexDim::D2
            && resolvable(format)
            && [2, 4, 8, 16].iter().any(|&n| self.takes_samples(format, n));
        if d.sample_count > 1 || resolves_into {
            usage |= TextureUsage::RENDER;
        }
        // A depth buffer is read by pixel programs running their forced early tests themselves
        // (`EarlyTestsInShader`), where the back end can sample its format.
        if d.bind_flags & bind::DEPTH_STENCIL != 0
            && self
                .backend
                .caps()
                .formats
                .get(format)
                .contains(FormatUse::SAMPLE)
        {
            usage |= TextureUsage::SAMPLE;
        }
        // Unordered access views are cleared by clearing passes where the format can be drawn into.
        if d.bind_flags & bind::UNORDERED_ACCESS != 0
            && self
                .backend
                .caps()
                .formats
                .get(format)
                .contains(FormatUse::RENDER)
        {
            usage |= TextureUsage::RENDER;
        }
        let subresources = (mips * d.array_size) as usize;
        if !init.is_empty() && init.len() < subresources {
            return Err(Error::InvalidArg("initial data for every subresource"));
        }
        self.live()?;
        self.bury();
        // NV12's chroma plane: a texture of its own the program sees only through views and
        // through the joined rows of maps, copies and writes.
        let chroma = if d.format == crate::format::NV12 {
            let plane = TextureDesc {
                format: crate::format::R8G8_TYPELESS,
                width: d.width / 2,
                height: d.height / 2,
                mip_levels: 1,
                usage: usage::DEFAULT,
                cpu_access_flags: 0,
                misc_flags: 0,
                ..*d
            };
            Some(self.create_texture(&plane, &[])?)
        } else {
            None
        };
        let (dimension, depth_or_layers) = match d.dimension {
            TexDim::D3 => (TextureDimension::D3, d.depth),
            TexDim::D1 | TexDim::D2 => (TextureDimension::D2, d.array_size),
        };
        let gpu_desc = GpuTexture {
            dimension,
            format,
            // A YUY2 texel is a pixel pair.
            width: if d.format == format::YUY2 {
                d.width / 2
            } else {
                d.width
            },
            height: d.height,
            depth_or_layers,
            mip_levels: mips,
            samples: d.sample_count.max(1),
            usage,
            cube,
            reinterpret: false,
        };
        let gpu = self.backend.create_texture(&gpu_desc);
        let gpu = self.gpu(gpu)?;
        let resolved = TextureDesc {
            mip_levels: mips,
            ..*d
        };
        let id = self.id();
        let texture = Texture {
            desc: resolved,
            format,
            gpu_desc,
            gpu: Kept::new(gpu, id, Object::Texture, &self.graveyard),
            staged: RefCell::new(Vec::new()),
            shadow: RefCell::new(Vec::new()),
            chain: ChainBuffer::None,
            min_lod: Cell::new(0.0),
            chroma,
        };
        for (i, data) in init.iter().take(subresources).enumerate() {
            let (region, extent) = texture
                .whole(i as u32)
                .expect("a subresource of the texture");
            self.write_texels(format, &region, extent, data);
            if let Some(chroma) = &texture.chroma {
                let (region, extent) = chroma.whole(i as u32).expect("the luma's subresource");
                // The chroma rows follow the luma's at the same pitch.
                let at = (data.row_pitch * texture.desc.height) as usize;
                let data = SubresourceData {
                    bytes: &data.bytes[at..],
                    ..*data
                };
                self.write_texels(chroma.format, &region, extent, &data);
            }
        }
        if matches!(d.usage, usage::STAGING | usage::DYNAMIC) {
            let shadow = (0..subresources)
                .map(|i| {
                    let (row, rows, slices) = texture.packed(i as u32 % mips);
                    let mut shadow = vec![0; (row * rows * slices) as usize];
                    // The shadow starts as the initial data, tightly packed.
                    if let Some(data) = init.get(i) {
                        let (row, slice) = (row as usize, (row * rows) as usize);
                        for (r, out) in shadow.chunks_exact_mut(row).enumerate() {
                            let (z, y) = (r / rows as usize, r % rows as usize);
                            let at = z * data.slice_pitch as usize + y * data.row_pitch as usize;
                            out.copy_from_slice(&data.bytes[at..at + row]);
                        }
                        debug_assert_eq!(shadow.len(), slice * slices as usize);
                    }
                    shadow
                })
                .collect::<Vec<_>>();
            *texture.staged.borrow_mut() = shadow.iter().map(|_| Staged::Current).collect();
            *texture.shadow.borrow_mut() = shadow;
        }
        Ok(Rc::new(texture))
    }

    /// Writes texels from the program's rows and slices at their own pitches: one write where the
    /// slices lie row after row, otherwise one per slice.
    pub(crate) fn write_texels(
        &mut self,
        format: Format,
        region: &TextureRegion<'_, B>,
        extent: Extent,
        data: &SubresourceData<'_>,
    ) {
        let (row, rows) = format::row_bytes(format, extent.width, extent.height);
        // Direct3D ignores the row pitch of a single row (a 1D texture).
        let row_pitch = if rows == 1 { row } else { data.row_pitch };
        let slice_pitch = data.slice_pitch;
        let depth = extent.depth_or_layers;
        if format.kind() == FormatKind::DepthStencil {
            // Each aspect apart, from the rows packed (depth-stencil textures are 2D: one slice).
            let texel = format.block().2;
            let mut packed = Vec::with_capacity((row * rows) as usize);
            for y in 0..rows {
                let at = (y * row_pitch) as usize;
                packed.extend_from_slice(&data.bytes[at..at + row as usize]);
            }
            let (d, s) = crate::context::split_depth_stencil(texel, &packed);
            let texels = row / texel;
            let stencil = TextureRegion {
                stencil: true,
                ..*region
            };
            self.backend
                .write_texture(region, extent, &d, texels * 4, rows, WriteMode::Ordered);
            self.backend
                .write_texture(&stencil, extent, &s, texels, rows, WriteMode::Ordered);
            return;
        }
        if row_pitch < row || (depth > 1 && slice_pitch < row_pitch * rows) {
            // Rows or slices laid over each other (a pitch of zero repeats one), packed apart.
            self.backend.slow_paths().hit(SlowPath::RowsRepacked);
            let mut packed = Vec::with_capacity((row * rows * depth) as usize);
            for z in 0..depth {
                for y in 0..rows {
                    let at = (z * slice_pitch + y * row_pitch) as usize;
                    packed.extend_from_slice(&data.bytes[at..at + row as usize]);
                }
            }
            self.backend
                .write_texture(region, extent, &packed, row, rows, WriteMode::Ordered);
            return;
        }
        if depth == 1 || slice_pitch == row_pitch * rows {
            self.backend.write_texture(
                region,
                extent,
                data.bytes,
                row_pitch,
                rows,
                WriteMode::Ordered,
            );
            return;
        }
        for z in 0..extent.depth_or_layers {
            let at = (z * slice_pitch) as usize;
            let slice = TextureRegion {
                origin: Origin {
                    z: region.origin.z + z,
                    ..region.origin
                },
                ..*region
            };
            self.backend.write_texture(
                &slice,
                Extent {
                    depth_or_layers: 1,
                    ..extent
                },
                &data.bytes[at..],
                row_pitch,
                rows,
                WriteMode::Ordered,
            );
        }
    }

    /// A view of `texture`; `kind` is the bind flag the view needs (render target, depth stencil or
    /// shader resource), and `None` views the whole texture as Direct3D's defaults say.
    pub fn create_view(
        &mut self,
        texture: &Rc<Texture<B>>,
        desc: Option<&ViewDesc>,
        kind: u32,
    ) -> Result<Rc<View<B>>> {
        if let Some(chroma) = &texture.chroma {
            // A view of NV12 names one plane by its format; the luma plane is viewed as the
            // members of R8's group, the chroma plane (a texture of R8G8's) as its own.
            match format::nv12_plane(desc.map_or(format::UNKNOWN, |d| d.format)) {
                Some(1) => return self.create_view(chroma, desc, kind),
                Some(_) => {}
                None => {
                    return Err(Error::InvalidArg(
                        "a view of an NV12 texture names one plane's format",
                    ));
                }
            }
        }
        let t = &texture.desc;
        if t.bind_flags & kind == 0 {
            return Err(Error::InvalidArg(
                "the resource lacks the bind flag the view needs",
            ));
        }
        let read_only = desc.map_or(0, |d| d.read_only);
        if read_only & !(DSV_READ_ONLY_DEPTH | DSV_READ_ONLY_STENCIL) != 0
            || read_only != 0 && (kind != bind::DEPTH_STENCIL || self.feature_level() < 0xb000)
        {
            return Err(Error::InvalidArg(
                "read-only depth-stencil views need feature level 11_0",
            ));
        }
        let arrayed = t.array_size > 1;
        let cube = t.misc_flags & MISC_TEXTURECUBE != 0 && kind == bind::SHADER_RESOURCE;
        // Views drawn into are one layer at a pass; shader and unordered access views see theirs.
        let drawn = kind & (bind::RENDER_TARGET | bind::DEPTH_STENCIL) != 0;
        let multisampled = t.sample_count > 1;
        let whole = ViewDesc {
            format: t.format,
            dimension: match (t.dimension, arrayed) {
                (TexDim::D2, false) if multisampled => ViewDim::Texture2DMS,
                (TexDim::D2, true) if multisampled => ViewDim::Texture2DMSArray,
                (TexDim::D2, _) if cube && t.array_size < 12 => ViewDim::TextureCube,
                (TexDim::D2, _) if cube => ViewDim::TextureCubeArray,
                (TexDim::D1, false) => ViewDim::Texture1D,
                (TexDim::D1, true) => ViewDim::Texture1DArray,
                (TexDim::D2, false) => ViewDim::Texture2D,
                (TexDim::D2, true) => ViewDim::Texture2DArray,
                (TexDim::D3, _) => ViewDim::Texture3D,
            },
            first_mip: 0,
            mip_count: if kind == bind::SHADER_RESOURCE {
                t.mip_levels
            } else {
                1
            },
            first_layer: 0,
            layer_count: u32::MAX,
            read_only: 0,
        };
        let mut v = desc.copied().unwrap_or(whole);
        if v.format == format::UNKNOWN {
            v.format = t.format;
        }
        if v.mip_count == u32::MAX {
            v.mip_count = t.mip_levels - v.first_mip.min(t.mip_levels);
        }
        let of = match v.dimension {
            ViewDim::Texture1D | ViewDim::Texture1DArray => TexDim::D1,
            ViewDim::Texture3D => TexDim::D3,
            _ => TexDim::D2,
        };
        let ms_view = matches!(
            v.dimension,
            ViewDim::Texture2DMS | ViewDim::Texture2DMSArray
        );
        // Direct3D takes a multisampled view of a single-sampled texture: its first level.
        if of != t.dimension || (multisampled && !ms_view) {
            return Err(Error::InvalidArg(
                "a view of a dimension the texture does not have",
            ));
        }
        // Layers a view may see: a 3D view's are the depth slices of its first level.
        let layers = match t.dimension {
            TexDim::D3 => (t.depth >> v.first_mip.min(31)).max(1),
            _ => t.array_size,
        };
        if ms_view {
            // One level, which the descriptions of these views do not name.
            v.first_mip = 0;
            v.mip_count = 1;
        }
        if matches!(
            v.dimension,
            ViewDim::Texture1D | ViewDim::Texture2D | ViewDim::Texture2DMS
        ) || (v.dimension == ViewDim::Texture3D && kind == bind::SHADER_RESOURCE)
        {
            v.first_layer = 0;
            v.layer_count = if v.dimension == ViewDim::Texture3D {
                layers
            } else {
                1
            };
        }
        if v.layer_count == u32::MAX {
            // A cube array sees the whole cubes left.
            v.layer_count = layers - v.first_layer.min(layers);
            if v.dimension == ViewDim::TextureCubeArray {
                v.layer_count -= v.layer_count % CUBE_FACES;
            }
        }
        if v.dimension == ViewDim::TextureCube && desc.is_none() {
            v.layer_count = CUBE_FACES;
        }
        if texture.chain == ChainBuffer::Read && kind != bind::SHADER_RESOURCE {
            return Err(Error::InvalidArg(
                "a sequential swap chain's buffer past the first, which is read only",
            ));
        }
        if v.first_mip + v.mip_count > t.mip_levels
            || v.mip_count == 0
            || v.first_layer + v.layer_count > layers
            || v.layer_count == 0
            || (kind != bind::SHADER_RESOURCE && v.mip_count != 1)
        {
            return Err(Error::InvalidArg("the view reaches past the resource"));
        }
        let (format, stencil) = match format::srgb_twin(t.format) {
            Some(twin)
                if texture.chain == ChainBuffer::Drawn
                    && kind == bind::RENDER_TARGET
                    && v.format == twin =>
            {
                (format::texture_format_of(twin)?, false)
            }
            _ if texture.chroma.is_some() => {
                format::view_format(v.format, format::R8_TYPELESS, texture.format)?
            }
            // Direct3D also takes R8G8B8A8_SNORM target views of YUY2.
            _ if t.format == format::YUY2 && kind == bind::RENDER_TARGET && v.format == 31 => {
                (Format::Rgba8Snorm, false)
            }
            _ => format::view_format(v.format, t.format, texture.format)?,
        };
        let (format, decoded) = match self.held_as_integer(format) {
            Some(twin) => (twin, Some(format)),
            None => (format, None),
        };
        // Depth-stencil views take the D formats and only they; the others read depth through
        // their own members of the group (R24_UNORM_X8_TYPELESS and the like).
        if (kind == bind::DEPTH_STENCIL) != matches!(v.format, 20 | 40 | 45 | 55) {
            return Err(Error::InvalidArg(
                "a view format the view's binding cannot take",
            ));
        }
        let cube = matches!(
            v.dimension,
            ViewDim::TextureCube | ViewDim::TextureCubeArray
        );
        if cube
            && (t.misc_flags & MISC_TEXTURECUBE == 0
                || !v.layer_count.is_multiple_of(CUBE_FACES)
                || (v.dimension == ViewDim::TextureCube && v.layer_count != CUBE_FACES))
        {
            return Err(Error::InvalidArg(
                "a cube view needs faces in sixes of a cube texture",
            ));
        }
        // A view drawn into is one layer (or one slice of a 3D level, chosen at the pass): with
        // no shader choosing a layer, Direct3D draws into the first.
        let (dimension, base_layer, layer_count) = if drawn {
            match t.dimension {
                TexDim::D3 => (ViewDimension::D3, 0, 1),
                _ => (ViewDimension::D2, v.first_layer, 1),
            }
        } else {
            // A multisampled texture is one layer, which programs read as an array of one
            // (`VariantKey::texture_layers`).
            let dimension = match v.dimension {
                ViewDim::Texture1D | ViewDim::Texture2D | ViewDim::Texture2DMS => ViewDimension::D2,
                ViewDim::Texture2DMSArray if multisampled => ViewDimension::D2,
                ViewDim::Texture1DArray | ViewDim::Texture2DArray | ViewDim::Texture2DMSArray => {
                    ViewDimension::D2Array
                }
                ViewDim::Texture3D => ViewDimension::D3,
                ViewDim::TextureCube => ViewDimension::Cube,
                ViewDim::TextureCubeArray => ViewDimension::CubeArray,
            };
            match dimension {
                ViewDimension::D3 => (dimension, 0, 1),
                _ => (dimension, v.first_layer, v.layer_count),
            }
        };
        // Direct3D reads stencil in the second component and the interface's stencil views give it
        // in the first: programs reading such a view move it (`StageVariant::stencil`).
        self.live()?;
        self.bury();
        // A storage view of some slices of a 3D level, which no back end's view sees, is made on
        // a texture of those slices alone (`View::window`).
        let window = kind == bind::UNORDERED_ACCESS
            && t.dimension == TexDim::D3
            && (v.first_layer != 0 || v.layer_count != (t.depth >> v.first_mip).max(1));
        let twin = if window {
            let e = texture.mip_extent(v.first_mip);
            let gpu = self.backend.create_texture(&GpuTexture {
                format,
                width: e.width,
                height: e.height,
                depth_or_layers: v.layer_count,
                mip_levels: 1,
                ..texture.gpu_desc
            });
            let gpu = self.gpu(gpu)?;
            let id = self.id();
            Some(Kept::new(gpu, id, Object::Texture, &self.graveyard))
        } else if !stencil && format::reinterprets(format, texture.format) {
            if kind == bind::DEPTH_STENCIL {
                return not_implemented(
                    "a depth view reinterpreting a typeless texture's bits as another format",
                );
            }
            let gpu = self.backend.create_texture(&GpuTexture {
                format,
                ..texture.gpu_desc
            });
            let gpu = self.gpu(gpu)?;
            let id = self.id();
            Some(Kept::new(gpu, id, Object::Texture, &self.graveyard))
        } else {
            None
        };
        let gpu = self.backend.create_view(
            twin.as_ref().map_or(&texture.gpu.handle, |t| &t.handle),
            &TextureViewDesc {
                format,
                dimension,
                base_mip: if window { 0 } else { v.first_mip },
                mip_count: v.mip_count,
                base_layer,
                layer_count,
                stencil,
            },
        );
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(View {
            texture: texture.clone(),
            desc: v,
            format,
            decoded,
            gpu: Kept::new(gpu, id, Object::TextureView, &self.graveyard),
            twin,
            window,
            buffer: None,
            stencil_aspect: RefCell::new(None),
            layers: RefCell::new(Vec::new()),
        }))
    }

    /// A render target view of `count` elements of `buffer` from element `first`: a view of a
    /// one-row texture of the elements, copied from and into the buffer around each pass drawing
    /// into it (`View::buffer`).
    pub fn create_buffer_target_view(
        &mut self,
        buffer: &Rc<Buffer<B>>,
        format: u32,
        first: u32,
        count: u32,
    ) -> Result<Rc<View<B>>> {
        let gpu_format = format::texture_format_of(format)
            .map_err(|_| Error::InvalidArg("a format a buffer's target view cannot have"))?;
        let (block_width, block_height, bytes) = gpu_format.block();
        let end = (u64::from(first) + u64::from(count)) * u64::from(bytes);
        if buffer.desc.bind_flags & bind::RENDER_TARGET == 0
            || format::is_typeless(format)
            || (block_width, block_height) != (1, 1)
            || count == 0
            || end > u64::from(buffer.desc.byte_width)
        {
            return Err(Error::InvalidArg("a target view of a buffer"));
        }
        let elements = self.create_texture(
            &TextureDesc {
                dimension: TexDim::D2,
                width: count,
                height: 1,
                depth: 1,
                mip_levels: 1,
                array_size: 1,
                format,
                sample_count: 1,
                sample_quality: 0,
                usage: usage::DEFAULT,
                bind_flags: bind::RENDER_TARGET,
                cpu_access_flags: 0,
                misc_flags: 0,
            },
            &[],
        )?;
        let view = self.create_view(&elements, None, bind::RENDER_TARGET)?;
        let mut view = Rc::into_inner(view).expect("a view only made here");
        view.buffer = Some((buffer.clone(), u64::from(first) * u64::from(bytes)));
        Ok(Rc::new(view))
    }

    /// An unordered access view of one level of a texture.
    pub fn create_texture_uav(
        &mut self,
        texture: &Rc<Texture<B>>,
        desc: Option<&ViewDesc>,
    ) -> Result<Rc<ShaderView<B>>> {
        let v = self.create_view(texture, desc, bind::UNORDERED_ACCESS)?;
        if format::is_srgb(v.desc.format) {
            return Err(Error::InvalidArg("an sRGB unordered access view"));
        }
        self.check_format(v.format, self.storage_use(v.format))?;
        Ok(Rc::new(ShaderView::Texture(v)))
    }

    /// An unordered access view of `count` elements of a buffer from `first`: raw (32-bit words of
    /// format `R32_TYPELESS`) or structured, with a hidden counter for append and counter views
    /// (`flags`: `D3D11_BUFFER_UAV_FLAG`).
    pub fn create_buffer_uav(
        &mut self,
        buffer: &Rc<Buffer<B>>,
        format: u32,
        first: u32,
        count: u32,
        flags: u32,
    ) -> Result<Rc<ShaderView<B>>> {
        use buffer_uav::{APPEND, COUNTER_FLAG, RAW};
        let raw = flags & RAW != 0;
        let (offset, size, texel) =
            self.buffer_span(buffer, bind::UNORDERED_ACCESS, format, first, count, raw)?;
        let structured = buffer.desc.misc_flags & MISC_STRUCTURED != 0;
        let counted = flags & (APPEND | COUNTER_FLAG);
        if counted == APPEND | COUNTER_FLAG || (counted != 0 && (!structured || raw)) {
            return Err(Error::InvalidArg(
                "append and counter views are structured, one or the other",
            ));
        }
        self.live()?;
        self.bury();
        let counter = if counted != 0 {
            let desc = BufferDesc {
                byte_width: COUNTER_BYTES as u32,
                usage: usage::DEFAULT,
                bind_flags: bind::UNORDERED_ACCESS,
                cpu_access_flags: 0,
                misc_flags: 0,
                structure_byte_stride: 0,
            };
            let gpu = self
                .backend
                .create_buffer(&gpu_buffer(&desc, COUNTER_BYTES as u64));
            let gpu = self.gpu(gpu)?;
            self.backend
                .write_buffer(&gpu, 0, &[0; COUNTER_BYTES], WriteMode::Discard);
            let id = self.id();
            Some(Kept::new(gpu, id, Object::Buffer, &self.graveyard))
        } else {
            None
        };
        Ok(Rc::new(ShaderView::Buffer(BufferSpan {
            buffer: buffer.clone(),
            offset,
            size,
            counter,
            texel,
        })))
    }

    /// A shader resource view of `count` elements of a buffer from `first`: raw or structured.
    pub fn create_buffer_srv(
        &mut self,
        buffer: &Rc<Buffer<B>>,
        format: u32,
        first: u32,
        count: u32,
        raw: bool,
    ) -> Result<Rc<ShaderView<B>>> {
        let (offset, size, texel) =
            self.buffer_span(buffer, bind::SHADER_RESOURCE, format, first, count, raw)?;
        self.live()?;
        Ok(Rc::new(ShaderView::Buffer(BufferSpan {
            buffer: buffer.clone(),
            offset,
            size,
            counter: None,
            texel,
        })))
    }

    /// The bytes (offset, size) a view of `count` elements from `first` of a buffer binds: raw
    /// (32-bit words, format `R32_TYPELESS`), structured (the buffer's stride, no format) or typed
    /// (a format). A typed view is bound from its first byte rounded down to the back end's storage
    /// offset alignment, its format, first element's byte in that range and count beside. `needs` is
    /// the bind flag the view's kind needs.
    fn buffer_span(
        &self,
        buffer: &Buffer<B>,
        needs: u32,
        format: u32,
        first: u32,
        count: u32,
        raw: bool,
    ) -> Result<(u64, u64, Option<TexelPlace>)> {
        const RAW_WORD_BYTES: u32 = size_of::<u32>() as u32;
        let d = &buffer.desc;
        if d.bind_flags & needs == 0 {
            return Err(Error::InvalidArg(
                "the resource lacks the bind flag the view needs",
            ));
        }
        let (stride, typed) = if raw {
            if format != format::R32_TYPELESS || d.misc_flags & MISC_ALLOW_RAW_VIEWS == 0 {
                return Err(Error::InvalidArg("a raw view of a buffer not made for one"));
            }
            (RAW_WORD_BYTES, None)
        } else if d.misc_flags & MISC_STRUCTURED != 0 {
            if format != format::UNKNOWN {
                return Err(Error::InvalidArg("a structured view with a format"));
            }
            (d.structure_byte_stride, None)
        } else {
            if format == format::UNKNOWN {
                return Err(Error::InvalidArg("a typed buffer view without a format"));
            }
            let f = format::texture_format_of(format)?;
            match f.block() {
                (1, 1, bytes) if !f.is_depth_stencil() => (bytes, Some(f)),
                _ => {
                    return Err(Error::InvalidArg(
                        "a buffer view of a block or depth format",
                    ));
                }
            }
        };
        let (offset, size) = (
            u64::from(first) * u64::from(stride),
            u64::from(count) * u64::from(stride),
        );
        if count == 0 || offset + size > u64::from(d.byte_width) {
            return Err(Error::InvalidArg("the view reaches past the resource"));
        }
        let align = u64::from(
            self.backend
                .caps()
                .limits
                .min_storage_offset_alignment
                .max(4),
        );
        if let Some(f) = typed {
            // A texture copy holds the whole buffer, its first element at its first texel.
            if self.typed_buffers_textured() {
                if !self
                    .backend
                    .caps()
                    .formats
                    .get(f)
                    .contains(FormatUse::SAMPLE)
                {
                    return not_implemented(TYPED_BUFFER_FORMAT_UNHELD);
                }
                let within = u32::try_from(offset).map_err(|_| Error::OutOfMemory)?;
                return Ok((0, u64::from(d.byte_width), Some((f, within, count))));
            }
            let start = offset / align * align;
            let end = (offset + size).next_multiple_of(4);
            let within = u32::try_from(offset - start).expect("below the alignment");
            return Ok((start, end - start, Some((f, within, count))));
        }
        if !offset.is_multiple_of(align) {
            return not_implemented(
                "a buffer view at an offset the back end does not bind at (StorageOffset)",
            );
        }
        Ok((offset, size, None))
    }

    pub fn create_sampler_state(&mut self, d: &SamplerDesc) -> Result<Rc<Sampler<B>>> {
        let made = gpu_sampler(d, self.backend.caps())?;
        self.live()?;
        self.bury();
        let gpu = self.backend.create_sampler(&made);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(Sampler {
            desc: *d,
            gpu: Kept::new(gpu, id, Object::Sampler, &self.graveyard),
            made,
            twin: RefCell::new(None),
            clamped: RefCell::new(Vec::new()),
        }))
    }

    pub fn create_blend_state(&mut self, d: &BlendDesc) -> Result<Rc<Blend<B>>> {
        // Targets past the first count only when they blend independently.
        let used = if d.independent { d.targets.len() } else { 1 };
        for t in d.targets[..used].iter().filter(|t| t.enable) {
            check_blend_factor(t.src, false)?;
            check_blend_factor(t.dst, false)?;
            check_blend_factor(t.src_alpha, true)?;
            check_blend_factor(t.dst_alpha, true)?;
            let known = blend_operation::ADD..=blend_operation::MAX;
            if !known.contains(&t.op) || !known.contains(&t.op_alpha) {
                return Err(Error::InvalidArg("unknown blend operation"));
            }
        }
        // Below 10_1 the targets that blend share their factors and operations (Direct3D 10's
        // blend), each with its own enable and write mask.
        let shared = |t: &TargetBlend| (t.src, t.dst, t.op, t.src_alpha, t.dst_alpha, t.op_alpha);
        let mut blending = d.targets.iter().filter(|t| t.enable).map(shared);
        if d.independent
            && self.feature_level() < level::L10_1
            && blending
                .next()
                .is_some_and(|first| blending.any(|t| t != first))
        {
            return Err(Error::InvalidArg(
                "blend factors or operations per target below feature level 10_1",
            ));
        }
        let block = blend_block(d);
        self.live()?;
        self.bury();
        // Without the back end's per-target blends, a block per group of targets that agree,
        // each drawn by a run of its own (`TargetsSplit`).
        let mut groups = Vec::new();
        if !block.same_for_all && !self.backend.caps().has(Features::INDEPENDENT_BLEND) {
            let mut left = u8::MAX;
            while left != 0 {
                let first = left.trailing_zeros() as usize;
                let members = (first..8)
                    .filter(|&i| block.targets[i] == block.targets[first])
                    .fold(0u8, |m, i| m | 1 << i);
                left &= !members;
                let desc = BlendDesc {
                    independent: false,
                    targets: [d.targets[first]; 8],
                    ..*d
                };
                let gpu = self.backend.create_blend(&blend_block(&desc));
                let gpu = self.gpu(gpu)?;
                let id = self.id();
                let group = Rc::new(Blend {
                    desc,
                    gpu: Kept::new(gpu, id, Object::Blend, &self.graveyard),
                    groups: Vec::new(),
                });
                groups.push((members, group));
            }
        }
        // Where the targets bound fall in one group, the draw takes that group's block; the
        // state's own is then never drawn with, and is the first group's.
        let own = match groups.first() {
            Some((_, g)) => blend_block(&g.desc),
            None => block,
        };
        let gpu = self.backend.create_blend(&own);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(Blend {
            desc: *d,
            gpu: Kept::new(gpu, id, Object::Blend, &self.graveyard),
            groups,
        }))
    }

    pub fn create_depth_stencil_state(
        &mut self,
        d: &DepthStencilDesc,
    ) -> Result<Rc<DepthStencil<B>>> {
        if d.depth_enable {
            compare_fn(d.depth_func)?;
        }
        for f in [d.front, d.back].iter().filter(|_| d.stencil_enable) {
            compare_fn(f.func)?;
            for op in [f.fail, f.depth_fail, f.pass] {
                stencil_op(op)?;
            }
        }
        let block = depth_stencil_block(d);
        self.live()?;
        self.bury();
        let gpu = self.backend.create_depth_stencil(&block);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(DepthStencil {
            desc: *d,
            gpu: Kept::new(gpu, id, Object::DepthStencil, &self.graveyard),
            read_only: RefCell::new(Vec::new()),
        }))
    }

    pub fn create_rasterizer_state(&mut self, d: &RasterizerDesc) -> Result<Rc<Rasterizer<B>>> {
        if !(fill_mode::WIREFRAME..=fill_mode::SOLID).contains(&d.fill)
            || !(cull_mode::NONE..=cull_mode::BACK).contains(&d.cull)
        {
            return Err(Error::InvalidArg("unknown fill or cull mode"));
        }
        let caps = self.backend.caps();
        if d.fill == fill_mode::WIREFRAME && !caps.has(Features::POLYGON_MODE) {
            return not_implemented("wireframe without the back end's polygon mode");
        }
        self.live()?;
        self.bury();
        let gpu = self
            .backend
            .create_raster(&raster_block(d, self.backend.caps()));
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(Rasterizer {
            desc: *d,
            gpu: Kept::new(gpu, id, Object::Raster, &self.graveyard),
            masked: RefCell::new(Vec::new()),
        }))
    }

    /// A vertex or pixel shader from its bytecode; `stage` is the one the API call makes.
    pub fn create_shader(
        &mut self,
        bytecode: &[u8],
        stage: wcontainers_gpu::SourceStage,
    ) -> Result<Rc<Shader<B>>> {
        shader::check_model(bytecode, self.level)?;
        if matches!(
            stage,
            wcontainers_gpu::SourceStage::Vertex | wcontainers_gpu::SourceStage::Domain
        ) && shader::writes_layer_or_viewport(bytecode)?
        {
            return Err(Error::InvalidArg(
                "a layer or viewport index written before the geometry stage",
            ));
        }
        let mut typed = shader::typed_views(bytecode)?;
        if self.typed_buffers_textured() {
            if typed.iter().any(|t| t.buffer && t.storage) {
                return not_implemented(TYPED_STORAGE_BUFFERS_UNSERVED);
            }
            for t in typed.iter_mut().filter(|t| t.buffer) {
                t.textured = true;
            }
        }
        let layered = shader::multisampled_arrays(bytecode)?;
        let texture_info = !self.backend.caps().has(Features::TEXTURE_LEVEL_QUERY);
        let (mut converted, reads_samples) =
            shader::convert(bytecode, stage, &mut typed, layered, texture_info)?;
        let samplers = shader::sampler_slots(&converted);
        let textures = shader::texture_slots(&converted);
        let mirror_filtered = shader::mirror_filtered_slots(&converted);
        let helpers = Rc::new(shader::Helpers::of(&converted));
        let mut module = converted.modules.pop().expect("one module");
        let constant_sizes = constant_sizes(&module.module);
        let stand_ins = stand_ins(&module.module);
        // Constant buffers past the back end's uniform buffers are read through textures.
        let set = crate::context::set_of(if stage == wcontainers_gpu::SourceStage::Pixel {
            crate::PIXEL
        } else {
            crate::VERTEX
        });
        let past_limit = if matches!(
            stage,
            wcontainers_gpu::SourceStage::Vertex | wcontainers_gpu::SourceStage::Pixel
        ) {
            shader::constants_past_limit(
                &converted.reflection,
                set,
                0,
                helpers.size > 0 || texture_info,
                self.backend.caps().limits.uniform_buffers_per_stage,
            )
        } else {
            0
        };
        if past_limit != 0 {
            let key = StageVariant {
                texture_info,
                constants_as_texture: past_limit,
                ..StageVariant::default()
            }
            .key(stage, &typed, layered, &[]);
            let mut c = shader::convert_variant(bytecode, stage, &key)?;
            module = c.modules.pop().expect("one module");
        }
        // A vertex or domain program writing clip distances the back end lacks passes them on for
        // the pixel program to apply, at the location past its outputs.
        let mut clip = None;
        if wcontainers_shaders::capabilities(&module.module)
            .contains(naga::valid::Capabilities::CLIP_DISTANCES)
            && !self.backend.caps().has(Features::CLIP_DISTANCE)
        {
            let outputs = &converted.reflection.outputs;
            let at = (
                shader::linked_location(outputs),
                shader::clip_distances(outputs),
            );
            let key = StageVariant {
                linked: at.0,
                clip_distances: at.1,
                constants_as_texture: past_limit,
                ..StageVariant::default()
            }
            .key(stage, &typed, layered, &[]);
            let mut c = shader::convert_variant(bytecode, stage, &key)?;
            module = c.modules.pop().expect("one module");
            clip = Some(at);
        }
        // A vertex program writing cull distances the back end lacks is drawn through the stage
        // helper, which drops the primitives they cull, and the back end holds its pass-through.
        let culled = wcontainers_shaders::capabilities(&module.module)
            .contains(naga::valid::Capabilities::CULL_DISTANCE)
            && !self.backend.caps().has(Features::CULL_DISTANCE);
        if culled {
            if stage != wcontainers_gpu::SourceStage::Vertex
                || !self.backend.caps().has(Features::COMPUTE)
            {
                return not_implemented(CULL_DISTANCES_UNSERVED);
            }
            let (linked, clip_distances) = clip.unwrap_or_default();
            let key = StageVariant {
                texture_info,
                linked,
                clip_distances,
                constants_as_texture: past_limit,
                ..StageVariant::default()
            }
            .key(stage, &typed, layered, &[]);
            module = shader::cull_passthrough(bytecode, key, &converted.reflection.inputs)?;
        }
        if wcontainers_shaders::capabilities(&module.module)
            .contains(naga::valid::Capabilities::PRIMITIVE_INDEX)
            && !self.backend.caps().has(Features::PRIMITIVE_INDEX)
        {
            return not_implemented(
                "the primitive index without the back end's (PrimitiveIdAsGeometry)",
            );
        }
        self.live()?;
        self.bury();
        let gpu = self.backend.create_shader(&module);
        let gpu = self.gpu(gpu)?;
        let indexed_constants = shader::indexed_constants(bytecode)?;
        let id = self.id();
        let vertex = stage == wcontainers_gpu::SourceStage::Vertex;
        let shader = Rc::new(Shader {
            indexed_constants,
            clip,
            culled,
            gpu: Kept::new(gpu, id, Object::Shader, &self.graveyard),
            key: module.key,
            reflection: converted.reflection,
            constant_sizes,
            stand_ins,
            bytecode: bytecode.to_vec(),
            samplers,
            textures,
            mirror_filtered,
            reads_samples,
            helpers,
            typed,
            layered,
        });
        // A culled vertex shader's draws are the stage helper's, which nothing builds ahead.
        if !culled
            && matches!(
                stage,
                wcontainers_gpu::SourceStage::Vertex | wcontainers_gpu::SourceStage::Pixel
            )
        {
            self.build_ahead(&shader, vertex);
        }
        Ok(shader)
    }

    /// A geometry shader, or a vertex shader's bytecode with a stream output declaration `so`.
    pub fn create_geometry_shader(
        &mut self,
        bytecode: &[u8],
        so: Option<crate::stream::Declaration<'_>>,
    ) -> Result<Rc<Geometry<B>>> {
        use wcontainers_gpu::SourceStage;
        shader::check_model(bytecode, self.level)?;
        let vertex = match shader::stage(bytecode)? {
            SourceStage::Geometry => false,
            SourceStage::Vertex | SourceStage::Domain if so.is_some() => true,
            _ => {
                return Err(Error::InvalidArg(
                    "the bytecode is a shader of another stage",
                ));
            }
        };
        let outputs = shader::output_signature(bytecode)?;
        let (entries, strides, rasterized) = so.unwrap_or((None, None, 0));
        let (stream, drawn) =
            crate::stream::declaration(entries, strides, rasterized, self.level, &outputs, vertex)?;
        if rasterized != 0 && outputs.iter().any(|o| o.stream == rasterized) {
            return not_implemented(GEOMETRY_STREAM_DRAWN_PAST_ZERO);
        }
        // Without compute programs a geometry shader, or a vertex shader's stream output, is
        // replayed through transform feedback (`GeometryAsReplay`, `StreamOutAfterReplay`).
        let caps = self.backend.caps();
        let replayed = !caps.has(Features::COMPUTE);
        if replayed && !caps.has(Features::TRANSFORM_FEEDBACK) {
            return not_implemented(GEOMETRY_STAGE_UNSERVED);
        }
        let stage = if vertex {
            None
        } else {
            let typed = shader::typed_views(bytecode)?;
            let layered = shader::multisampled_arrays(bytecode)?;
            let key = shader::geometry_key(&typed, layered, None, replayed);
            let mut converted = shader::convert_geometry(bytecode, &key)?;
            // A pass-through writing clip distances the back end lacks passes them on for the
            // pixel program to apply, at the location past the stage's outputs.
            let clip = (wcontainers_shaders::capabilities(&converted.modules[1].module)
                .contains(naga::valid::Capabilities::CLIP_DISTANCES)
                && !self.backend.caps().has(Features::CLIP_DISTANCE))
            .then(|| {
                let outputs = &converted.reflection.outputs;
                (
                    shader::linked_location(outputs),
                    shader::clip_distances(outputs),
                )
            });
            if clip.is_some() {
                let key = shader::geometry_key(&typed, layered, clip, replayed);
                converted = shader::convert_geometry(bytecode, &key)?;
            }
            let stage = self.geometry_stage(bytecode, &converted, typed, layered, clip)?;
            Some((stage, converted))
        };
        let id = self.id();
        Ok(Rc::new(Geometry {
            id,
            stage,
            stream,
            drawn,
        }))
    }

    /// The geometry stage of `converted`: the slots of its compute form, and its pass-through
    /// vertex program handed to the back end.
    pub(crate) fn geometry_stage(
        &mut self,
        bytecode: &[u8],
        converted: &wcontainers_shaders::Converted,
        typed: Vec<shader::TypedView>,
        layered: u128,
        clip: Option<(u32, u8)>,
    ) -> Result<Rc<Shader<B>>> {
        let compute = &converted.modules[0];
        self.live()?;
        self.bury();
        let gpu = self.backend.create_shader(&converted.modules[1]);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(Shader {
            gpu: Kept::new(gpu, id, Object::Shader, &self.graveyard),
            key: converted.modules[1].key,
            reflection: converted.reflection.clone(),
            constant_sizes: constant_sizes(&compute.module),
            stand_ins: stand_ins(&compute.module),
            bytecode: bytecode.to_vec(),
            samplers: shader::sampler_slots(converted),
            textures: shader::texture_slots(converted),
            mirror_filtered: shader::mirror_filtered_slots(converted),
            reads_samples: false,
            helpers: Rc::new(shader::Helpers::of(converted)),
            indexed_constants: shader::indexed_constants(bytecode)?,
            clip,
            culled: false,
            typed,
            layered,
        }))
    }

    pub fn create_input_layout(
        &mut self,
        elements: &[InputElement],
        vs_bytecode: &[u8],
    ) -> Result<Rc<InputLayout>> {
        // No elements is a layout for a vertex shader without inputs; one with inputs is refused
        // below, as each of its inputs needs an element.
        if elements.len() > 32 {
            return Err(Error::InvalidArg("an input layout has at most 32 elements"));
        }
        // Direct3D matches the elements against the bytecode's input signature, whatever its
        // stage; each input the input assembler does not generate needs an element.
        let inputs = shader::input_signature(vs_bytecode)?;
        let mut next_offset = [0u32; 32];
        let mut instance_steps = [None; 32];
        let mut resolved = Vec::new();
        let mut pulled = Vec::new();
        let mut pulled_paths = Vec::new();
        let steps_natively = self.backend.caps().has(Features::INSTANCE_STEP_RATE);
        for e in elements {
            if e.slot >= 32 {
                return Err(Error::InvalidArg("input slot past 31"));
            }
            let vf = format::vertex_format(e.format)
                .ok_or(Error::InvalidArg("a format vertex input cannot have"))?;
            let slot = e.slot as usize;
            let align = format::vertex_alignment(vf);
            let offset = if e.offset == APPEND_ALIGNED {
                next_offset[slot].next_multiple_of(align)
            } else if e.offset % align != 0 {
                return Err(Error::InvalidArg(
                    "a vertex element at an offset not a multiple of its component size",
                ));
            } else {
                e.offset
            };
            next_offset[slot] = offset + format::vertex_size(vf);
            let step = e.per_instance.then_some(e.step_rate);
            if resolved
                .iter()
                .chain(&pulled)
                .any(|r: &LayoutElement| r.slot == e.slot)
                && instance_steps[slot] != step
            {
                return Err(Error::InvalidArg("elements of one slot step differently"));
            }
            instance_steps[slot] = step;
            let input = inputs.iter().find(|i| {
                !i.generated()
                    && i.semantic.eq_ignore_ascii_case(&e.semantic)
                    && i.index == e.semantic_index
            });
            if let Some(input) = input {
                let element = LayoutElement {
                    location: input.register,
                    slot: e.slot,
                    offset,
                    format: vf,
                };
                // Rates other than one where the back end steps by one only are fetched in the
                // vertex program, and so are small integers read with the other signedness, whose
                // bits Direct3D hands over and WebGPU's formats would extend by their own.
                let scalar = format::vertex_scalar(vf);
                let read = match input.component {
                    1 => ScalarKind::Uint,
                    2 => ScalarKind::Sint,
                    _ => ScalarKind::Float,
                };
                let resigned = scalar != ScalarKind::Float
                    && read != ScalarKind::Float
                    && scalar != read
                    && format::vertex_format_read_as(vf, read) == vf;
                let path = if step.is_some_and(|r| r != 1) && !steps_natively {
                    Some(SlowPath::InstanceRateInShader)
                } else if resigned {
                    Some(SlowPath::VertexFormatConverted)
                } else {
                    None
                };
                if let Some(path) = path {
                    if !pulled_paths.contains(&path) {
                        pulled_paths.push(path);
                    }
                    pulled.push(element);
                } else {
                    resolved.push(element);
                }
            }
        }
        for input in inputs.iter().filter(|i| !i.generated()) {
            if !resolved
                .iter()
                .chain(&pulled)
                .any(|r| r.location == input.register)
            {
                return Err(Error::InvalidArg(
                    "the layout lacks an input the shader reads",
                ));
            }
        }
        let id = self.id();
        let all = resolved.iter().chain(&pulled).copied().collect();
        Ok(Rc::new(InputLayout {
            id,
            elements: resolved,
            pulled,
            pulled_paths,
            all,
            instance_steps,
        }))
    }
}

/// The back end's description of a buffer of `size` bytes for `d`.
pub(crate) fn gpu_buffer(d: &BufferDesc, size: u64) -> GpuBuffer {
    let mut usage = BufferUsage::COPY_SRC | BufferUsage::COPY_DST;
    for (flag, u) in [
        (bind::VERTEX_BUFFER, BufferUsage::VERTEX),
        (bind::INDEX_BUFFER, BufferUsage::INDEX),
        // Also storage, for programs reading past their declared array (`ConstantsAsStorage`).
        (
            bind::CONSTANT_BUFFER,
            BufferUsage::UNIFORM.union(BufferUsage::STORAGE),
        ),
        (bind::SHADER_RESOURCE, BufferUsage::STORAGE),
        (bind::UNORDERED_ACCESS, BufferUsage::STORAGE),
    ] {
        if d.bind_flags & flag != 0 {
            usage |= u;
        }
    }
    if d.misc_flags & MISC_DRAWINDIRECT_ARGS != 0 {
        // Read as storage by vertex programs that take the draw's bases from the arguments.
        usage |= BufferUsage::INDIRECT | BufferUsage::STORAGE;
    }
    if d.usage == usage::STAGING {
        usage |= BufferUsage::READ_BACK;
    }
    let intent = if d.usage == usage::DYNAMIC {
        MapIntent::Often
    } else {
        MapIntent::Rarely
    };
    GpuBuffer {
        size,
        usage,
        intent,
    }
}

/// The declared bytes of each uniform buffer `module` reads, by binding number.
fn constant_sizes(module: &naga::Module) -> Vec<(u32, u64)> {
    module
        .global_variables
        .iter()
        .filter(|(_, v)| v.space == naga::AddressSpace::Uniform)
        .filter_map(|(_, v)| {
            let size = module.types[v.ty].inner.size(module.to_ctx());
            v.binding.as_ref().map(|b| (b.binding, u64::from(size)))
        })
        .collect()
}

/// What stands in for an empty slot of each texture, buffer view and sampler `module` reads, by
/// binding number; `None` for multisampled and depth textures and comparison samplers. Converted DXBC reads a
/// one-dimensional texture as a two-dimensional one, so the 1D arm is for modules that keep 1D.
fn stand_ins(module: &naga::Module) -> Vec<(u32, Option<StandIn>)> {
    use naga::{ImageClass, ImageDimension, ScalarKind, TypeInner};
    module
        .global_variables
        .iter()
        .filter_map(|(_, v)| {
            let binding = v.binding.as_ref()?.binding;
            let (dim, arrayed, class) = match module.types[v.ty].inner {
                TypeInner::Image {
                    dim,
                    arrayed,
                    class,
                } => (dim, arrayed, class),
                TypeInner::Sampler { comparison } => {
                    return Some((binding, (!comparison).then_some(StandIn::DefaultSampler)));
                }
                _ => match v.space {
                    naga::AddressSpace::Storage { access }
                        if !access.contains(naga::StorageAccess::STORE) =>
                    {
                        return Some((binding, Some(StandIn::ZeroBuffer)));
                    }
                    _ => return None,
                },
            };
            let dimension = match (dim, arrayed) {
                (ImageDimension::D2, false) => Some(ViewDimension::D2),
                (ImageDimension::D2, true) => Some(ViewDimension::D2Array),
                (ImageDimension::Cube, false) => Some(ViewDimension::Cube),
                (ImageDimension::Cube, true) => Some(ViewDimension::CubeArray),
                (ImageDimension::D3, _) => Some(ViewDimension::D3),
                (ImageDimension::D1, _) => None,
            };
            let (kind, multi) = match class {
                ImageClass::Sampled { kind, multi } => (
                    match kind {
                        ScalarKind::Uint => Some(FormatKind::Uint),
                        ScalarKind::Sint => Some(FormatKind::Sint),
                        _ => Some(FormatKind::Float),
                    },
                    multi,
                ),
                _ => (None, false),
            };
            let dimension = dimension.filter(|&d| !multi || d == ViewDimension::D2);
            Some((
                binding,
                dimension
                    .zip(kind)
                    .map(|(d, k)| StandIn::Zeros(d, k, multi)),
            ))
        })
        .collect()
}

/// Whether multisampled textures of `format` resolve (averaging: not integer or depth).
pub(crate) fn resolvable(format: Format) -> bool {
    !matches!(
        format.kind(),
        FormatKind::Uint | FormatKind::Sint | FormatKind::DepthStencil
    )
}

fn check_buffer(d: &BufferDesc, has_init: bool) -> Result<()> {
    if d.byte_width == 0 {
        return Err(Error::InvalidArg("a buffer needs a size"));
    }
    if d.misc_flags & (MISC_GENERATE_MIPS | MISC_TEXTURECUBE) != 0 {
        return Err(Error::InvalidArg(
            "mip generation and cubes are for textures",
        ));
    }
    if d.bind_flags & bind::CONSTANT_BUFFER != 0 {
        if !d.byte_width.is_multiple_of(16) {
            return Err(Error::InvalidArg(
                "constant buffer sizes are multiples of 16",
            ));
        }
        if d.bind_flags != bind::CONSTANT_BUFFER {
            return Err(Error::InvalidArg(
                "a constant buffer has no other bind flags",
            ));
        }
    }
    // Buffers bound for views, render targets and stream output are made; the views and the
    // stream output binding are what is not served.
    if d.bind_flags & bind::DEPTH_STENCIL != 0 {
        return Err(Error::InvalidArg("buffers cannot hold depth"));
    }
    let views = bind::SHADER_RESOURCE | bind::UNORDERED_ACCESS;
    if d.misc_flags & MISC_STRUCTURED != 0 {
        let s = d.structure_byte_stride;
        if s == 0
            || !s.is_multiple_of(4)
            || s > 2048
            || !d.byte_width.is_multiple_of(s)
            || d.misc_flags & MISC_ALLOW_RAW_VIEWS != 0
            || d.bind_flags & (bind::VERTEX_BUFFER | bind::INDEX_BUFFER) != 0
        {
            return Err(Error::InvalidArg(
                "a structured buffer's stride and bindings",
            ));
        }
    }
    if d.misc_flags & MISC_ALLOW_RAW_VIEWS != 0 && d.bind_flags & views == 0 {
        return Err(Error::InvalidArg("a raw buffer is bound for views"));
    }
    // What the GPU does not write, it reads through a binding.
    let written = bind::STREAM_OUTPUT | bind::RENDER_TARGET | bind::UNORDERED_ACCESS;
    if matches!(d.usage, usage::IMMUTABLE | usage::DYNAMIC)
        && (d.bind_flags == 0 || d.bind_flags & written != 0)
    {
        return Err(Error::InvalidArg(
            "an immutable or dynamic buffer is only read, through a binding",
        ));
    }
    match d.usage {
        usage::DEFAULT if d.cpu_access_flags == 0 => Ok(()),
        usage::IMMUTABLE if !has_init => Err(Error::InvalidArg("an immutable buffer needs data")),
        usage::IMMUTABLE if d.cpu_access_flags == 0 => Ok(()),
        usage::DYNAMIC if d.cpu_access_flags == cpu::WRITE => Ok(()),
        usage::STAGING if d.bind_flags == 0 && d.cpu_access_flags != 0 => Ok(()),
        _ => Err(Error::InvalidArg(
            "usage, bind flags and CPU access do not agree",
        )),
    }
}

pub(crate) fn compare_fn(f: u32) -> Result<CompareFn> {
    Ok(match f {
        comparison::NEVER => CompareFn::Never,
        comparison::LESS => CompareFn::Less,
        comparison::EQUAL => CompareFn::Equal,
        comparison::LESS_EQUAL => CompareFn::LessEqual,
        comparison::GREATER => CompareFn::Greater,
        comparison::NOT_EQUAL => CompareFn::NotEqual,
        comparison::GREATER_EQUAL => CompareFn::GreaterEqual,
        comparison::ALWAYS => CompareFn::Always,
        _ => return Err(Error::InvalidArg("unknown comparison")),
    })
}

pub(crate) fn stencil_op(op: u32) -> Result<StencilOp> {
    Ok(match op {
        stencil_operation::KEEP => StencilOp::Keep,
        stencil_operation::ZERO => StencilOp::Zero,
        stencil_operation::REPLACE => StencilOp::Replace,
        stencil_operation::INCR_SAT => StencilOp::IncrementClamp,
        stencil_operation::DECR_SAT => StencilOp::DecrementClamp,
        stencil_operation::INVERT => StencilOp::Invert,
        stencil_operation::INCR => StencilOp::IncrementWrap,
        stencil_operation::DECR => StencilOp::DecrementWrap,
        _ => return Err(Error::InvalidArg("unknown stencil operation")),
    })
}

fn check_blend_factor(f: u32, alpha: bool) -> Result<()> {
    use blend::*;
    let colour_only = matches!(
        f,
        SRC_COLOR | INV_SRC_COLOR | DEST_COLOR | INV_DEST_COLOR | SRC1_COLOR | INV_SRC1_COLOR
    );
    if !matches!(f, ZERO..=SRC_ALPHA_SAT | BLEND_FACTOR..=INV_SRC1_ALPHA) || (alpha && colour_only)
    {
        return Err(Error::InvalidArg("a blend factor the slot cannot take"));
    }
    Ok(())
}

fn blend_factor(f: u32) -> BlendFactor {
    use BlendFactor::*;
    match f {
        blend::ZERO => Zero,
        blend::ONE => One,
        blend::SRC_COLOR => Src,
        blend::INV_SRC_COLOR => OneMinusSrc,
        blend::SRC_ALPHA => SrcAlpha,
        blend::INV_SRC_ALPHA => OneMinusSrcAlpha,
        blend::DEST_ALPHA => DstAlpha,
        blend::INV_DEST_ALPHA => OneMinusDstAlpha,
        blend::DEST_COLOR => Dst,
        blend::INV_DEST_COLOR => OneMinusDst,
        blend::SRC_ALPHA_SAT => SrcAlphaSaturated,
        blend::BLEND_FACTOR => Constant,
        blend::INV_BLEND_FACTOR => OneMinusConstant,
        blend::SRC1_COLOR => Src1,
        blend::INV_SRC1_COLOR => OneMinusSrc1,
        blend::SRC1_ALPHA => Src1Alpha,
        _ => OneMinusSrc1Alpha,
    }
}

fn blend_op(op: u32) -> BlendOp {
    match op {
        blend_operation::ADD => BlendOp::Add,
        blend_operation::SUBTRACT => BlendOp::Subtract,
        blend_operation::REV_SUBTRACT => BlendOp::ReverseSubtract,
        blend_operation::MIN => BlendOp::Min,
        _ => BlendOp::Max,
    }
}

pub(crate) fn blend_block(d: &BlendDesc) -> wcontainers_gpu::BlendDesc {
    let target = |t: &TargetBlend| RenderTargetBlend {
        blend: t.enable.then(|| {
            (
                BlendComponent {
                    src: blend_factor(t.src),
                    dst: blend_factor(t.dst),
                    op: blend_op(t.op),
                },
                BlendComponent {
                    src: blend_factor(t.src_alpha),
                    dst: blend_factor(t.dst_alpha),
                    op: blend_op(t.op_alpha),
                },
            )
        }),
        writes: ColorWrites::from_bits_truncate(t.write_mask),
    };
    let first = target(&d.targets[0]);
    let targets: [RenderTargetBlend; MAX_COLOR_TARGETS] = std::array::from_fn(|i| {
        if d.independent {
            target(&d.targets[i])
        } else {
            first
        }
    });
    let same_for_all = targets.iter().all(|t| *t == targets[0]);
    wcontainers_gpu::BlendDesc {
        targets,
        same_for_all,
        alpha_to_coverage: d.alpha_to_coverage,
        logic_op: None,
    }
}

pub(crate) fn depth_stencil_block(d: &DepthStencilDesc) -> GpuDepthStencil {
    let face = |f: &StencilFaceDesc| StencilFace {
        compare: compare_fn(f.func).unwrap_or(CompareFn::Always),
        fail: stencil_op(f.fail).unwrap_or(StencilOp::Keep),
        depth_fail: stencil_op(f.depth_fail).unwrap_or(StencilOp::Keep),
        pass: stencil_op(f.pass).unwrap_or(StencilOp::Keep),
    };
    GpuDepthStencil {
        depth_compare: d
            .depth_enable
            .then(|| compare_fn(d.depth_func).unwrap_or(CompareFn::Always)),
        depth_write: d.depth_enable && d.depth_write,
        stencil: d.stencil_enable.then(|| (face(&d.front), face(&d.back))),
        stencil_read_mask: [d.stencil_read_mask; 2],
        stencil_write_mask: [d.stencil_write_mask; 2],
        depth_bounds: None,
    }
}

/// The back end's raster block for Direct3D's description. A bias clamp the back end lacks is left
/// out: draws with depth to bias add the bias in the pixel program instead (`DepthBiasInShader`).
pub(crate) fn raster_block(d: &RasterizerDesc, caps: &Caps) -> RasterDesc {
    let clamp = if caps.has(Features::DEPTH_BIAS_CLAMP) {
        d.depth_bias_clamp
    } else {
        0.0
    };
    RasterDesc {
        fill: if d.fill == fill_mode::WIREFRAME {
            FillMode::Line
        } else {
            FillMode::Solid
        },
        cull: match d.cull {
            cull_mode::FRONT => CullMode::Front,
            cull_mode::BACK => CullMode::Back,
            _ => CullMode::None,
        },
        front: if d.front_counter_clockwise {
            FrontFace::CounterClockwise
        } else {
            FrontFace::Clockwise
        },
        depth_bias: (d.depth_bias as f32).to_bits(),
        depth_bias_slope: d.slope_scaled_depth_bias.to_bits(),
        depth_bias_clamp: clamp.to_bits(),
        // Where the back end clips only, the programs clamp (`DepthClampInShader`).
        depth_clip: d.depth_clip || !caps.has(Features::DEPTH_CLAMP),
        scissor: d.scissor,
        line_width: 1.0f32.to_bits(),
        provoking: ProvokingVertex::First,
        depth_minus_one: false,
        sample_mask: u32::MAX,
    }
}

/// Whether clear values of `format` are integers rather than floats.
pub(crate) fn integer_format(format: Format) -> bool {
    matches!(format.kind(), FormatKind::Uint | FormatKind::Sint)
}

/// The back end's sampler for Direct3D's description. Border and mirror-once addressing the back end
/// lacks clamp; the program applies them (`BorderInShader`, `MirrorOnceInShader`).
fn gpu_sampler(d: &SamplerDesc, caps: &Caps) -> Result<GpuSampler> {
    let filter = |bit: u32| {
        if d.filter & bit != 0 {
            Filter::Linear
        } else {
            Filter::Nearest
        }
    };
    if d.filter & !0xd5 != 0 {
        return not_implemented("minimum and maximum filtering");
    }
    let anisotropic = d.filter & 0x40 != 0;
    // Direct3D takes 0 as 1.
    if anisotropic && d.max_anisotropy > 16 {
        return Err(Error::InvalidArg("anisotropy above 16"));
    }
    let mut address = [AddressMode::Repeat; 3];
    for (a, &mode) in address.iter_mut().zip(&d.address) {
        *a = match mode {
            1 => AddressMode::Repeat,
            2 => AddressMode::Mirror,
            3 => AddressMode::Clamp,
            4 if caps.has(Features::BORDER_COLOR) => AddressMode::Border,
            5 if caps.has(Features::MIRROR_ONCE) => AddressMode::MirrorOnce,
            4 | 5 => AddressMode::Clamp,
            _ => return Err(Error::InvalidArg("unknown address mode")),
        };
    }
    let compare = if d.filter & 0x80 != 0 {
        Some(compare_fn(d.comparison)?)
    } else {
        None
    };
    let border = match d.border {
        [0.0, 0.0, 0.0, 0.0] => BorderColor::TransparentBlack,
        [0.0, 0.0, 0.0, 1.0] => BorderColor::OpaqueBlack,
        [1.0, 1.0, 1.0, 1.0] => BorderColor::OpaqueWhite,
        c => BorderColor::Custom(c.map(f32::to_bits)),
    };
    Ok(GpuSampler {
        min: if anisotropic {
            Filter::Linear
        } else {
            filter(0x10)
        },
        mag: if anisotropic {
            Filter::Linear
        } else {
            filter(0x4)
        },
        mip: if anisotropic {
            Filter::Linear
        } else {
            filter(0x1)
        },
        address,
        // WebGPU takes no minimum above the maximum: such a range holds every level at the
        // minimum.
        lod_range: [
            d.min_lod.max(0.0).to_bits(),
            d.max_lod.clamp(0.0, 32.0).max(d.min_lod.max(0.0)).to_bits(),
        ],
        anisotropy: if anisotropic {
            d.max_anisotropy.max(1) as u16
        } else {
            1
        },
        compare,
        border,
    })
}

/// The sampler's state as programs that filter or address in the program read it
/// (`HelperValue::SamplerState`): filters and address modes as codes in the order of the interface's
/// enums, the anisotropy, then the level range.
pub(crate) fn sampler_state(s: &Sampler<impl Backend>) -> [u32; 9] {
    let m = &s.made;
    let mut words = [0; 9];
    for (w, f) in words.iter_mut().zip([m.min, m.mag, m.mip]) {
        *w = f as u32;
    }
    for (w, &mode) in words[3..].iter_mut().zip(&s.desc.address) {
        *w = match mode {
            1 => AddressMode::Repeat,
            2 => AddressMode::Mirror,
            4 => AddressMode::Border,
            5 => AddressMode::MirrorOnce,
            _ => AddressMode::Clamp,
        } as u32;
    }
    words[6] = u32::from(m.anisotropy);
    words[7..].copy_from_slice(&m.lod_range);
    words
}
