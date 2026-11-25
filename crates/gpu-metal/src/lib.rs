//! The graphics interface drawn with Metal, through any implementation of the safe Metal trait.
//!
//! Copies are batched into blit passes, a pass is one render pass, and `submit` commits the command buffer.
//!
//! A buffer write no work can see lands in place; others go through a staging copy.

mod bytes;
mod convert;
mod objects;
mod record;
mod ring;
mod vertex;

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use wcontainers_gpu::{
    AdapterInfo, BindingEntry, BindingLayoutKey, BindingResource, BlendDesc, BufferDesc, Caps,
    CopyBufferTexture, DepthStencilDesc, DeviceStatus, DispatchCall, DispatchGroups, DrawCall,
    Extent, FORMAT_FACTS, Features, Format, FormatKind, Gpu, GpuError, Helpers, Limits, LossReason,
    Object, PassDesc, Program, QueryKind, RasterDesc, ReadSource, ReadState, SamplerDesc,
    ShaderModule, ShaderStage, SlowPaths, StepMode, TextureDesc, TextureDimension, TextureRegion,
    TextureUsage, TextureViewDesc, VertexLayoutDesc, ViewDimension, WriteMode,
};
use wcontainers_metal::{
    BlitCommand, BlitOption, CommandBufferStatus, ComputeCommand, DepthStencilDescriptor, Metal,
    Origin, SamplerDescriptor, SamplerMipFilter, Size, TextureDescriptor, TextureType,
    TextureViewDescriptor,
};

use bytes::{ByteRange, rounded};
pub use objects::{
    BlendBlock, BufferObj, DepthStencilBlock, MetalFence, Never, ProgramObj, QueryObj, RasterBlock,
    ReadbackObj, SamplerObj, SetObj, ShaderObj, Shared, SurfaceObj, TextureObj, VertexLayoutBlock,
    ViewObj,
};
use objects::{Frame, ReadLayout, SetResource, SlotKind};
use record::OpenPass;
use std::cell::RefCell;

/// The buffer slots each stage's resources take, from zero, of Metal's 31.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const RESOURCE_BUFFER_SLOTS: u32 = 20;

/// The first buffer slot of the vertex streams, stream n at this plus n.
const FIRST_STREAM_SLOT: u32 = RESOURCE_BUFFER_SLOTS;

/// The vertex streams a draw reads, each in a buffer slot of its own after the resources'.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const VERTEX_STREAMS: u32 = 8;

/// The buffer slot of a draw's immediates, the last but one of Metal's 31.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const IMMEDIATES_SLOT: u32 = 29;

/// The buffer slot of the lengths the bounds checks read, the last of Metal's 31.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const SIZES_SLOT: u32 = 30;

/// The texture slots each stage's resources take, of Metal's 128.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const TEXTURE_SLOTS: u32 = 32;

/// The bytes of one query result.
///
/// https://developer.apple.com/documentation/metal/mtlvisibilityresultmode
const QUERY_BYTES: u64 = 8;

/// The largest level of detail Metal clamps to.
///
/// https://developer.apple.com/documentation/metal/mtlsamplerdescriptor/lodmaxclamp
const LOD_MAX: f32 = f32::MAX;

/// The largest 24-bit unorm value, which reads as one.
///
/// https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html#fundamentals-fixedconv
const DEPTH24_MAX: f64 = 16_777_215.0;

/// The low 24 bits of a word, where 24-bit depth sits in a buffer.
///
/// https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html#copies-buffers-images-depth-stencil
const DEPTH24_BITS: u32 = 0x00ff_ffff;

/// What this implementation's devices take, from Apple's feature set tables for the Apple7 family on.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const LIMITS: Limits = Limits {
    max_texture_2d: 16384,
    max_texture_cube: 16384,
    max_texture_3d: 2048,
    max_texture_layers: 2048,
    max_buffer_size: 1 << 30,
    max_uniform_buffer_size: 1 << 16,
    max_storage_buffer_size: 1 << 30,
    uniform_buffers_per_stage: 12,
    textures_per_stage: 16,
    samplers_per_stage: wcontainers_metal::MAX_SAMPLER_SLOTS,
    storage_buffers_per_stage: 8,
    storage_textures_per_stage: 4,
    vertex_streams: VERTEX_STREAMS,
    vertex_attributes: 16,
    max_vertex_stride: 2048,
    color_targets: 8,
    color_bytes_per_sample: 32,
    inter_stage_slots: 16,
    min_uniform_offset_alignment: 256,
    min_storage_offset_alignment: 256,
    buffer_copy_alignment: 1,
    immediate_bytes: 256,
    compute_workgroup_storage: 32768,
    compute_invocations: 1024,
    compute_size: [1024, 1024, 64],
    max_dispatch: 65535,
    max_point_size: 511.0,
    max_line_width: 1.0,
    max_anisotropy: wcontainers_metal::MAX_ANISOTROPY as u16,
    capture_buffers: 0,
    capture_components: 0,
};

/// The interface drawn with Metal.
pub struct MetalGpu<M: Metal> {
    metal: M,
    caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<MetalGpu<M>>,
    next_id: u64,
    lost: Option<LossReason>,
    /// The command buffer being recorded, made at its first use.
    recording: Option<M::CommandBuffer>,
    /// The serial of the command buffer being recorded; every lower one was committed.
    serial: u64,
    /// The highest serial known completed.
    completed: u64,
    in_flight: VecDeque<(u64, M::CommandBuffer)>,
    /// Copies waiting to be encoded in one blit pass.
    blits: Vec<BlitCommand<M>>,
    pass: Option<OpenPass<M>>,
    vertex_functions: HashMap<(u64, u64, bool), M::Function>,
    fragment_functions: HashMap<u64, M::Function>,
    render_pipelines: HashMap<record::PipelineKey, M::RenderPipelineState>,
    compute_pipelines: HashMap<u64, M::ComputePipelineState>,
    depth_stencil_states: HashMap<u64, M::DepthStencilState>,
    /// The layers of the windows the host added, by the number its graphics hook names each by.
    windows: HashMap<u32, M::Layer>,
    /// The widened layout of each vertex layout that holds a format naga lacks.
    widened_layouts: HashMap<u64, Shared<VertexLayoutBlock>>,
    /// What draws a frame into a drawable, made at the first present.
    present: Option<record::PresentDraw<M>>,
    /// The programs that copy and fill bytes where Metal's blits take only multiples of four.
    byte_programs: bytes::BytePrograms<M>,
    /// How many times work was encoded into a command buffer, which ends a ring chunk's writes.
    encodes: u64,
    /// The rings of indices and of other per-draw data.
    rings: [ring::Ring<M>; 2],
}

impl<M: Metal> MetalGpu<M> {
    /// Makes the interface's device over a Metal device.
    pub fn new(metal: M) -> MetalGpu<M> {
        let caps = Caps {
            adapter: AdapterInfo {
                name: metal.name(),
                vendor: 0,
                device: 0,
                software: false,
            },
            features: Features::COMPUTE
                | Features::VERTEX_STORAGE_READ
                | Features::FRAGMENT_STORAGE_WRITE
                | Features::PROVOKING_VERTEX_FIRST
                | Features::DEPTH_CLAMP
                | Features::CLIP_DEPTH_ZERO_ONE
                | Features::DUAL_SOURCE_BLEND
                | Features::INDEPENDENT_BLEND
                | Features::SAMPLE_SHADING
                | Features::MULTISAMPLE_READ
                | Features::BASE_VERTEX_INSTANCE
                | Features::TEXTURE_VIEWS
                | Features::SEPARATE_SAMPLERS
                | Features::POLYGON_MODE
                | Features::BLOCK_COMPRESSION
                | Features::BLEND_CONSTANT_ALPHA
                | Features::DEPTH_BIAS_CLAMP
                | Features::READ_BACK_NOW
                | Features::FORMAT_REINTERPRET
                | Features::MIRROR_ONCE
                | Features::STENCIL_PER_FACE
                | Features::FRACTIONAL_DEPTH_BIAS
                | Features::OCCLUSION_COUNT,
            limits: LIMITS,
            formats: convert::format_caps(),
        };
        MetalGpu {
            metal,
            caps,
            slow: wcontainers_gpu::NO_SLOW_PATHS,
            helpers: Helpers::default(),
            next_id: u64::MIN,
            lost: None,
            recording: None,
            serial: 1,
            completed: u64::MIN,
            in_flight: VecDeque::new(),
            blits: Vec::new(),
            pass: None,
            vertex_functions: HashMap::new(),
            fragment_functions: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
            depth_stencil_states: HashMap::new(),
            windows: HashMap::new(),
            widened_layouts: HashMap::new(),
            present: None,
            byte_programs: bytes::BytePrograms::default(),
            encodes: u64::MIN,
            rings: Default::default(),
        }
    }

    /// Adds a window the host shows, by the number its graphics hook names it by; called on the main thread.
    pub fn add_window(
        &mut self,
        number: u32,
        window: &dyn raw_window_handle::HasWindowHandle,
    ) -> Result<(), GpuError> {
        let layer = self
            .metal
            .make_layer(window)
            .map_err(|_| GpuError::Invalid("a window Metal makes no layer of"))?;
        self.windows.insert(number, layer);
        Ok(())
    }

    fn id(&mut self) -> u64 {
        count_up(&mut self.next_id);
        self.next_id
    }

    /// Notes a refusal that is a bug here or in a front, which loses the device.
    fn fail(&mut self, why: std::fmt::Arguments<'_>) {
        eprintln!("metal: {why}");
        self.lost = Some(LossReason::Error);
    }

    /// Returns the command buffer being recorded, made at its first use.
    fn command_buffer(&mut self) -> Option<M::CommandBuffer> {
        if self.recording.is_none() {
            match self.metal.make_command_buffer() {
                Ok(command_buffer) => self.recording = Some(command_buffer),
                Err(error) => {
                    self.fail(format_args!("no command buffer: {error:?}"));
                    return None;
                }
            }
        }
        self.recording.clone()
    }

    /// Encodes the waiting copies into the command buffer being recorded.
    fn flush_blits(&mut self) {
        if self.blits.is_empty() {
            return;
        }
        let blits = std::mem::take(&mut self.blits);
        let Some(command_buffer) = self.command_buffer() else {
            return;
        };
        count_up(&mut self.encodes);
        if let Err(error) = self.metal.encode_blit_commands(&command_buffer, &blits) {
            self.fail(format_args!("copies refused: {error:?}"));
        }
    }

    /// Queues a copy, ending the open pass for it and opening it again after (`WriteSplitPass`).
    fn queue_blit(&mut self, command: BlitCommand<M>) {
        if self.pass.is_some() {
            self.record_slow_path(wcontainers_gpu::SlowPath::WriteSplitPass);
            let resumed = self.end_open_pass();
            self.blits.push(command);
            self.flush_blits();
            self.pass = resumed;
        } else {
            self.blits.push(command);
        }
    }

    /// Queues copies, or notes the refusal of a call whose places overflow.
    fn queue_copies(&mut self, copies: Option<Vec<BlitCommand<M>>>) {
        let Some(copies) = copies else {
            self.fail(format_args!(
                "a copy outside its texture, or whose places overflow"
            ));
            return;
        };
        for copy in copies {
            self.queue_blit(copy);
        }
    }

    /// Notes that the command buffer being recorded uses a buffer's current storage.
    fn mark_used(&self, buffer: &BufferObj<M>) {
        buffer.last_use.set(self.serial);
    }

    /// Learns which submitted command buffers have completed.
    fn poll(&mut self) {
        while let Some((serial, command_buffer)) = self.in_flight.front() {
            match self.metal.status(command_buffer) {
                CommandBufferStatus::Completed => {}
                CommandBufferStatus::Error => self.lost = Some(LossReason::Error),
                _ => break,
            }
            self.completed = *serial;
            self.in_flight.pop_front();
        }
    }

    /// Returns whether recorded or running work may still use a buffer's current storage.
    fn busy(&mut self, buffer: &BufferObj<M>) -> bool {
        if buffer.last_use.get() > self.completed {
            self.poll();
        }
        buffer.last_use.get() > self.completed
    }

    /// Makes a buffer holding `bytes`, which no work uses yet, as a range from its start.
    fn staging(&mut self, bytes: &[u8]) -> Option<ByteRange<M>> {
        let length = rounded(bytes.len() as u64)?;
        let buffer = self.metal.make_buffer(length).ok()?;
        self.metal.write_contents(&buffer, 0, bytes).ok()?;
        Some(ByteRange {
            buffer,
            length,
            offset: u64::MIN,
        })
    }

    /// Returns a range of a buffer's current storage.
    fn range(buffer: &BufferObj<M>, offset: u64) -> ByteRange<M> {
        // Every buffer's size was rounded when it was made, so this rounds without overflow; a failure would refuse
        // every copy of it.
        ByteRange {
            buffer: buffer.raw.borrow().clone(),
            length: rounded(buffer.size).unwrap_or(u64::MIN),
            offset,
        }
    }

    /// Commits the command buffer being recorded, and returns its serial.
    fn commit(&mut self) -> u64 {
        if self.pass.is_some() {
            self.fail(format_args!("submit inside a pass"));
            self.pass = None;
        }
        self.flush_blits();
        let serial = self.serial;
        if let Some(command_buffer) = self.command_buffer() {
            if let Err(error) = self.metal.commit(&command_buffer) {
                self.fail(format_args!("commit refused: {error:?}"));
            }
            self.in_flight.push_back((serial, command_buffer));
        }
        self.recording = None;
        count_up(&mut self.serial);
        serial
    }

    /// Blocks until the command buffer of `serial` has completed, committing it first if it is being recorded.
    fn wait_serial(&mut self, serial: u64) {
        if serial >= self.serial {
            self.commit();
        }
        while self.completed < serial {
            let Some((_, command_buffer)) = self.in_flight.front().cloned() else {
                break;
            };
            if let Err(error) = self.metal.wait_until_completed(&command_buffer) {
                self.fail(format_args!("wait refused: {error:?}"));
                break;
            }
            self.poll();
        }
    }

    /// Records a read back of `length` bytes that `copy` writes into a new buffer.
    fn read_into(
        &mut self,
        length: u64,
        layout: ReadLayout,
        copy: impl FnOnce(&M::Buffer) -> Vec<BlitCommand<M>>,
    ) -> Result<Shared<ReadbackObj<M>>, GpuError> {
        let buffer = self
            .metal
            .make_buffer(rounded(length).ok_or(GpuError::OutOfMemory)?)
            .map_err(|_| GpuError::OutOfMemory)?;
        for command in copy(&buffer) {
            self.queue_blit(command);
        }
        Ok(Shared(Rc::new(ReadbackObj {
            serial: self.serial,
            buffer,
            length,
            layout,
        })))
    }

    /// Returns the copies that move one texture region and a buffer's bytes, one per layer; none when a place overflows.
    #[allow(clippy::too_many_arguments)]
    fn buffer_texture_copies(
        &self,
        texture: &TextureObj<M>,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        buffer: &M::Buffer,
        offset: u64,
        row_pitch: u64,
        rows_per_layer: u64,
        into_texture: bool,
    ) -> Option<Vec<BlitCommand<M>>> {
        // A region outside the texture is refused first, so that the layers below are bounded by the texture's.
        if !texture_region_inside(region, extent) {
            return None;
        }
        let three_d = texture.desc.dimension == TextureDimension::D3;
        let options = match (
            FORMAT_FACTS[texture.desc.format as usize].kind,
            region.stencil,
        ) {
            (FormatKind::DepthStencil, true) => BlitOption::StencilFromDepthStencil,
            (FormatKind::DepthStencil, false) => BlitOption::DepthFromDepthStencil,
            _ => BlitOption::None,
        };
        let image = row_pitch.checked_mul(rows_per_layer)?;
        let (layers, depth) = if three_d {
            (1, extent.depth_or_layers)
        } else {
            (extent.depth_or_layers, 1)
        };
        (0..layers)
            .map(|layer| {
                let slice = if three_d {
                    0
                } else {
                    region.layer.checked_add(layer)?
                };
                let origin = Origin {
                    x: region.origin.x,
                    y: region.origin.y,
                    z: if three_d { region.origin.z } else { 0 },
                };
                let size = Size {
                    width: extent.width,
                    height: extent.height,
                    depth,
                };
                let at = u64::from(layer)
                    .checked_mul(image)
                    .and_then(|bytes| offset.checked_add(bytes))?;
                Some(if into_texture {
                    BlitCommand::CopyFromBufferToTexture {
                        source_buffer: buffer.clone(),
                        source_offset: at,
                        source_bytes_per_row: row_pitch,
                        source_bytes_per_image: image,
                        source_size: size,
                        destination_texture: texture.raw.clone(),
                        destination_slice: slice,
                        destination_level: region.mip,
                        destination_origin: origin,
                        options,
                    }
                } else {
                    BlitCommand::CopyFromTextureToBuffer {
                        source_texture: texture.raw.clone(),
                        source_slice: slice,
                        source_level: region.mip,
                        source_origin: origin,
                        source_size: size,
                        destination_buffer: buffer.clone(),
                        destination_offset: at,
                        destination_bytes_per_row: row_pitch,
                        destination_bytes_per_image: image,
                        options,
                    }
                })
            })
            .collect()
    }

    /// Returns the bytes of one row of blocks and the rows of blocks of an extent, for the aspect a region moves.
    fn tight_rows(&self, format: Format, stencil: bool, extent: Extent) -> Option<(u64, u64)> {
        let facts = FORMAT_FACTS[format as usize];
        let (bw, bh, bytes) = facts.block;
        let bytes = match facts.kind {
            FormatKind::DepthStencil if stencil => 1,
            FormatKind::DepthStencil => 4,
            _ => bytes,
        };
        Some((
            u64::from(extent.width.div_ceil(bw)).checked_mul(u64::from(bytes))?,
            u64::from(extent.height.div_ceil(bh)),
        ))
    }

    /// Returns the bytes of a region's tight rows over its layers, and the row's bytes and the rows.
    fn tight_bytes(
        &self,
        format: Format,
        stencil: bool,
        extent: Extent,
    ) -> Option<(u64, u64, u64)> {
        let (row, rows) = self.tight_rows(format, stencil, extent)?;
        let length = row
            .checked_mul(rows)?
            .checked_mul(u64::from(extent.depth_or_layers))?;
        Some((length, row, rows))
    }

    /// Reads the depth words of a buffer's region on the host, tightly packed; none when a place overflows.
    fn depth_words_in(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        raw: &M::Buffer,
        extent: Extent,
        format: Format,
    ) -> Option<Vec<u8>> {
        let (row, rows) = self.tight_rows(format, false, extent)?;
        let pitch = u64::from(src.row_pitch);
        let layer_bytes = pitch.checked_mul(u64::from(src.rows_per_layer))?;
        let mut packed = Vec::new();
        for layer in 0..u64::from(extent.depth_or_layers) {
            for y in 0..rows {
                let at = layer
                    .checked_mul(layer_bytes)?
                    .checked_add(y.checked_mul(pitch)?)?
                    .checked_add(src.offset)?;
                packed.extend(self.read_buffer_now(raw, at, row));
            }
        }
        Some(packed)
    }

    /// Copies tightly packed depth words into a buffer's region at its pitches; none when a place overflows.
    fn depth_words_out(
        &mut self,
        words: &[u8],
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
        format: Format,
    ) -> Option<()> {
        let (row, rows) = self.tight_rows(format, false, extent)?;
        let pitch = u64::from(dst.row_pitch);
        let layer_bytes = pitch.checked_mul(u64::from(dst.rows_per_layer))?;
        let row_bytes = usize::try_from(row).ok()?;
        for layer in 0..u64::from(extent.depth_or_layers) {
            for y in 0..rows {
                let from = layer.checked_mul(rows)?.checked_add(y)?.checked_mul(row)?;
                let from = usize::try_from(from).ok()?;
                let part = words.get(from..from.checked_add(row_bytes)?)?;
                let at = layer
                    .checked_mul(layer_bytes)?
                    .checked_add(y.checked_mul(pitch)?)?
                    .checked_add(dst.offset)?;
                let Some(staging) = self.staging(part) else {
                    self.fail(format_args!("no staging buffer"));
                    return Some(());
                };
                self.copy_bytes(staging, Self::range(dst.buffer, at), row);
            }
        }
        Some(())
    }

    /// Returns the bytes a host write or a buffer copy into a texture holds, as Metal's copy takes them.
    fn converted_in(format: Format, stencil: bool, bytes: &[u8]) -> Vec<u8> {
        if convert::depth_converted(format) && !stencil {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|w| {
                    let value = f64::from(u32::from_le_bytes(*w) & DEPTH24_BITS) / DEPTH24_MAX;
                    (value as f32).to_le_bytes()
                })
                .collect()
        } else {
            bytes.to_vec()
        }
    }

    /// Reads bytes of a buffer now, waiting for the work before.
    fn read_buffer_now(&mut self, buffer: &M::Buffer, offset: u64, length: u64) -> Vec<u8> {
        let serial = self.serial;
        self.wait_serial(serial);
        // Every caller's length is within the buffer, checked before.
        let mut bytes = vec![0; usize::try_from(length).unwrap_or(usize::MIN)];
        if let Err(error) = self.metal.read_contents(buffer, offset, &mut bytes) {
            self.fail(format_args!("read refused: {error:?}"));
        }
        bytes
    }

    /// Returns the Metal slots of every resource binding a shader's entry point reads.
    fn slots(module: &ShaderModule) -> Result<objects::Slots, GpuError> {
        let stage = match module.stage {
            ShaderStage::Vertex => naga::ShaderStage::Vertex,
            ShaderStage::Fragment => naga::ShaderStage::Fragment,
            ShaderStage::Compute => naga::ShaderStage::Compute,
        };
        let index = module
            .module
            .entry_points
            .iter()
            .position(|e| e.name == module.entry_point && e.stage == stage)
            .ok_or(GpuError::Invalid("the module has no such entry point"))?;
        let info = module.info.get_entry_point(index);
        let mut slots = std::collections::BTreeMap::new();
        let mut next = [0u32; 3];
        for (handle, var) in module.module.global_variables.iter() {
            let Some(binding) = var.binding else {
                continue;
            };
            if info[handle].is_empty() {
                continue;
            }
            let kind = match (var.space, &module.module.types[var.ty].inner) {
                (naga::AddressSpace::Uniform | naga::AddressSpace::Storage { .. }, _) => {
                    SlotKind::Buffer
                }
                (_, naga::TypeInner::Image { .. }) => SlotKind::Texture,
                (_, naga::TypeInner::Sampler { .. }) => SlotKind::Sampler,
                _ => {
                    return Err(GpuError::Invalid(
                        "a binding of a kind this implementation does not bind",
                    ));
                }
            };
            let (count, most) = match kind {
                SlotKind::Buffer => (&mut next[0], RESOURCE_BUFFER_SLOTS),
                SlotKind::Texture => (&mut next[1], TEXTURE_SLOTS),
                SlotKind::Sampler => (&mut next[2], wcontainers_metal::MAX_SAMPLER_SLOTS),
            };
            if *count >= most {
                return Err(GpuError::Invalid(
                    "more bindings of a kind than a stage takes",
                ));
            }
            slots.insert((binding.group, binding.binding), (kind, *count));
            count_up_u32(count);
        }
        Ok(slots)
    }
}

impl<M: Metal> Gpu for MetalGpu<M> {
    type Buffer = Shared<BufferObj<M>>;
    type Texture = Shared<TextureObj<M>>;
    type TextureView = Shared<ViewObj<M>>;
    type Sampler = Shared<SamplerObj<M>>;
    type Shader = Shared<ShaderObj>;
    type Program = Shared<ProgramObj>;
    type Blend = Shared<BlendBlock>;
    type DepthStencil = Shared<DepthStencilBlock>;
    type Raster = Shared<RasterBlock>;
    type VertexLayout = Shared<VertexLayoutBlock>;
    type BindingSet = Shared<SetObj<M>>;
    type QuerySet = Shared<QueryObj<M>>;
    type Readback = Shared<ReadbackObj<M>>;
    type Fence = MetalFence;
    type ResourceTable = Never;
    type Surface = Shared<SurfaceObj<M>>;

    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn slow_paths(&mut self) -> &mut SlowPaths {
        &mut self.slow
    }

    fn status(&mut self) -> DeviceStatus {
        self.poll();
        match self.lost {
            Some(reason) => DeviceStatus::Lost(reason),
            None => DeviceStatus::Ok,
        }
    }

    fn helpers(&mut self) -> &mut Helpers<Self> {
        &mut self.helpers
    }

    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Self::Buffer, GpuError> {
        if desc.size > self.caps.limits.max_buffer_size {
            return Err(GpuError::Invalid("buffer larger than max_buffer_size"));
        }
        let raw = self
            .metal
            .make_buffer(rounded(desc.size).ok_or(GpuError::OutOfMemory)?)
            .map_err(|_| GpuError::OutOfMemory)?;
        Ok(Shared(Rc::new(BufferObj {
            size: desc.size,
            raw: raw.into(),
            last_use: 0.into(),
        })))
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Self::Texture, GpuError> {
        let pixel_format = convert::pixel_format(desc.format)
            .filter(|_| !self.caps.formats.uses[desc.format as usize].is_empty())
            .ok_or(GpuError::Invalid(
                "texture format this implementation does not hold",
            ))?;
        let (texture_type, depth, array_length) =
            match (desc.dimension, desc.samples > 1, desc.cube) {
                (TextureDimension::D3, _, _) => (TextureType::Type3D, desc.depth_or_layers, 1),
                (TextureDimension::D2, true, _) => (TextureType::Type2DMultisample, 1, 1),
                (TextureDimension::D2, false, true)
                    if desc.depth_or_layers == wcontainers_gpu::CUBE_FACES =>
                {
                    (TextureType::TypeCube, 1, 1)
                }
                (TextureDimension::D2, false, true) => (
                    TextureType::TypeCubeArray,
                    1,
                    desc.depth_or_layers / wcontainers_gpu::CUBE_FACES,
                ),
                (TextureDimension::D2, false, false) if desc.depth_or_layers == 1 => {
                    (TextureType::Type2D, 1, 1)
                }
                (TextureDimension::D2, false, false) => {
                    (TextureType::Type2DArray, 1, desc.depth_or_layers)
                }
            };
        let raw = self
            .metal
            .make_texture(&TextureDescriptor {
                texture_type,
                pixel_format,
                width: desc.width,
                height: desc.height,
                depth,
                mipmap_level_count: desc.mip_levels,
                sample_count: desc.samples,
                array_length,
                usage: wcontainers_metal::TextureUsage {
                    shader_read: desc.usage.intersects(
                        TextureUsage::SAMPLE | TextureUsage::STORAGE | TextureUsage::RENDER,
                    ),
                    shader_write: desc.usage.contains(TextureUsage::STORAGE),
                    render_target: desc
                        .usage
                        .intersects(TextureUsage::RENDER | TextureUsage::PRESENT),
                    pixel_format_view: desc.reinterpret || has_srgb_twin(desc.format),
                },
            })
            .map_err(|error| match error {
                wcontainers_metal::MakeTextureError::Invalid(why) => GpuError::Invalid(why),
                wcontainers_metal::MakeTextureError::Refused => GpuError::OutOfMemory,
            })?;
        Ok(Shared(Rc::new(TextureObj { desc: *desc, raw })))
    }

    fn create_view(
        &mut self,
        texture: &Self::Texture,
        desc: &TextureViewDesc,
    ) -> Result<Self::TextureView, GpuError> {
        let pixel_format = objects::choose_view_format(desc.format, desc.stencil).ok_or(
            GpuError::Invalid("view format this implementation does not hold"),
        )?;
        let multisampled = texture.desc.samples > 1;
        let texture_type = match (desc.dimension, multisampled) {
            (ViewDimension::D2, true) => TextureType::Type2DMultisample,
            (ViewDimension::D2, false) => TextureType::Type2D,
            (ViewDimension::D2Array, _) => TextureType::Type2DArray,
            (ViewDimension::Cube, _) => TextureType::TypeCube,
            (ViewDimension::CubeArray, _) => TextureType::TypeCubeArray,
            (ViewDimension::D3, _) => TextureType::Type3D,
        };
        let three_d = texture.desc.dimension == TextureDimension::D3;
        let raw = self
            .metal
            .make_texture_view(
                &texture.raw,
                &TextureViewDescriptor {
                    pixel_format,
                    texture_type,
                    first_level: desc.base_mip,
                    level_count: desc.mip_count,
                    first_slice: if three_d { 0 } else { desc.base_layer },
                    slice_count: if three_d { 1 } else { desc.layer_count },
                },
            )
            .map_err(|_| {
                GpuError::Invalid("a view this implementation does not make of its texture")
            })?;
        let size = |n: u32| (n >> desc.base_mip).max(1);
        Ok(Shared(Rc::new(ViewObj {
            raw,
            format: desc.format,
            width: size(texture.desc.width),
            height: size(texture.desc.height),
            samples: texture.desc.samples,
        })))
    }

    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Self::Sampler, GpuError> {
        let [u, v, w] = desc.address.map(|mode| convert::address(mode, desc.border));
        let (s, border) = u?;
        let (t, _) = v?;
        let (r, _) = w?;
        let linear_only =
            ![desc.min, desc.mag, desc.mip].contains(&wcontainers_gpu::Filter::Nearest);
        let raw = self
            .metal
            .make_sampler_state(&SamplerDescriptor {
                min_filter: convert::filter(desc.min),
                mag_filter: convert::filter(desc.mag),
                mip_filter: match desc.mip {
                    wcontainers_gpu::Filter::Nearest => SamplerMipFilter::Nearest,
                    wcontainers_gpu::Filter::Linear => SamplerMipFilter::Linear,
                },
                s_address_mode: s,
                t_address_mode: t,
                r_address_mode: r,
                border_color: border,
                lod_min_clamp: f32::from_bits(desc.lod_range[0]).max(0.0),
                lod_max_clamp: f32::from_bits(desc.lod_range[1]).min(LOD_MAX),
                max_anisotropy: if linear_only {
                    u32::from(
                        desc.anisotropy
                            .clamp(1, wcontainers_metal::MAX_ANISOTROPY as u16),
                    )
                } else {
                    1
                },
                compare_function: desc.compare.map(convert::compare),
            })
            .map_err(|_| GpuError::Invalid("a sampler Metal refuses"))?;
        Ok(Shared(Rc::new(SamplerObj { raw })))
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<Self::Shader, GpuError> {
        let slots = Self::slots(module)?;
        Ok(Shared(Rc::new(ShaderObj {
            id: self.id(),
            module: module.module.clone(),
            info: module.info.clone(),
            entry_point: module.entry_point.clone(),
            stage: module.stage,
            slots,
        })))
    }

    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<Self::Program, GpuError> {
        let id = self.id();
        Ok(Shared(Rc::new(match program {
            Program::Render { vertex, fragment } => ProgramObj::Render {
                id,
                vertex: (*vertex).clone(),
                fragment: fragment.cloned(),
            },
            Program::Compute(shader) => ProgramObj::Compute {
                id,
                shader: (*shader).clone(),
            },
        })))
    }

    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Self::Blend, GpuError> {
        if desc.logic_op.is_some() {
            return Err(GpuError::Absent(Features::LOGIC_OP));
        }
        Ok(Shared(Rc::new(BlendBlock {
            id: self.id(),
            desc: *desc,
        })))
    }

    fn create_depth_stencil(
        &mut self,
        desc: &DepthStencilDesc,
    ) -> Result<Self::DepthStencil, GpuError> {
        if desc.depth_bounds.is_some() {
            return Err(GpuError::Absent(Features::DEPTH_BOUNDS));
        }
        let id = self.id();
        let raw = self
            .metal
            .make_depth_stencil_state(&DepthStencilDescriptor {
                depth_compare_function: desc
                    .depth_compare
                    .map_or(wcontainers_metal::CompareFunction::Always, convert::compare),
                depth_write_enabled: desc.depth_write,
                front_face_stencil: desc.stencil.map(|(front, _)| {
                    convert::stencil(
                        &front,
                        desc.stencil_read_mask[0],
                        desc.stencil_write_mask[0],
                    )
                }),
                back_face_stencil: desc.stencil.map(|(_, back)| {
                    convert::stencil(&back, desc.stencil_read_mask[1], desc.stencil_write_mask[1])
                }),
            })
            .map_err(|_| GpuError::OutOfMemory)?;
        self.depth_stencil_states.insert(id, raw);
        Ok(Shared(Rc::new(DepthStencilBlock { id, desc: *desc })))
    }

    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Self::Raster, GpuError> {
        let absent = [
            (
                desc.fill == wcontainers_gpu::FillMode::Point,
                Features::POLYGON_MODE_POINT,
            ),
            (
                desc.provoking == wcontainers_gpu::ProvokingVertex::Last,
                Features::PROVOKING_VERTEX_LAST,
            ),
            (desc.depth_minus_one, Features::CLIP_DEPTH_MINUS_ONE),
            (f32::from_bits(desc.line_width) != 1.0, Features::WIDE_LINES),
            (desc.sample_mask != !0, Features::SAMPLE_MASK),
        ];
        if let Some((_, feature)) = absent.iter().find(|(missing, _)| *missing) {
            return Err(GpuError::Absent(*feature));
        }
        Ok(Shared(Rc::new(RasterBlock {
            id: self.id(),
            desc: *desc,
        })))
    }

    fn create_vertex_layout(
        &mut self,
        desc: &VertexLayoutDesc,
    ) -> Result<Self::VertexLayout, GpuError> {
        for (stream, s) in desc.streams.iter().enumerate() {
            let Some(s) = s else {
                continue;
            };
            if !matches!(s.step, StepMode::Vertex | StepMode::Instance(1)) {
                return Err(GpuError::Absent(Features::INSTANCE_STEP_RATE));
            }
            if stream as u32 >= VERTEX_STREAMS {
                return Err(GpuError::Invalid(
                    "a stream past the back end's vertex_streams",
                ));
            }
        }
        for a in desc.attributes.iter().flatten() {
            if desc
                .streams
                .get(a.stream as usize)
                .copied()
                .flatten()
                .is_none()
            {
                return Err(GpuError::Invalid("attribute of a stream the layout lacks"));
            }
        }
        Ok(Shared(Rc::new(VertexLayoutBlock {
            id: self.id(),
            desc: *desc,
        })))
    }

    fn create_binding_set(
        &mut self,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<Self::BindingSet, GpuError> {
        let mut out = Vec::with_capacity(entries.len());
        for entry in entries {
            let resource = match &entry.resource {
                BindingResource::UniformBuffer(range)
                | BindingResource::StorageBuffer { range, .. } => SetResource::Buffer {
                    buffer: range.buffer.clone(),
                    offset: range.offset,
                    size: range.size,
                },
                BindingResource::Texture(view) | BindingResource::StorageTexture { view, .. } => {
                    SetResource::Texture((*view).clone())
                }
                BindingResource::Sampler(sampler) => SetResource::Sampler((*sampler).clone()),
                BindingResource::TexelBuffer { .. } => {
                    return Err(GpuError::Absent(Features::TEXEL_BUFFERS));
                }
                BindingResource::Table(table) => match **table {},
            };
            out.push((entry.binding, resource));
        }
        Ok(Shared(Rc::new(SetObj { entries: out })))
    }

    fn binding_layout_key(&self, _set: &Self::BindingSet) -> BindingLayoutKey {
        BindingLayoutKey::default()
    }

    fn create_query_set(
        &mut self,
        kind: QueryKind,
        count: u32,
    ) -> Result<Self::QuerySet, GpuError> {
        let counting = match kind {
            QueryKind::OcclusionAny => false,
            QueryKind::Occlusion => true,
            QueryKind::Timestamp => return Err(GpuError::Absent(Features::TIMESTAMPS)),
        };
        let buffer = self
            .metal
            .make_buffer(query_bytes(count.max(1)))
            .map_err(|_| GpuError::OutOfMemory)?;
        Ok(Shared(Rc::new(QueryObj {
            count,
            buffer,
            counting,
        })))
    }

    fn destroy(&mut self, object: Object<Self>) {
        // Recorded work keeps what it uses: Metal's command buffers hold their resources.
        drop(object);
    }

    fn write_buffer(&mut self, buffer: &Self::Buffer, offset: u64, data: &[u8], mode: WriteMode) {
        if data.is_empty() {
            return;
        }
        if offset
            .checked_add(data.len() as u64)
            .is_none_or(|end| end > buffer.size)
        {
            self.fail(format_args!("a write past its buffer's end"));
            return;
        }
        if self.busy(buffer) && mode == WriteMode::Discard {
            match self
                .metal
                .make_buffer(rounded(buffer.size).unwrap_or(u64::MAX))
            {
                Ok(fresh) => {
                    *buffer.raw.borrow_mut() = fresh;
                    buffer.last_use.set(0);
                }
                Err(_) => {
                    self.fail(format_args!("no buffer to rename into"));
                    return;
                }
            }
        }
        if !self.busy(buffer) {
            let raw = buffer.raw.borrow().clone();
            if let Err(error) = self.metal.write_contents(&raw, offset, data) {
                self.fail(format_args!("write refused: {error:?}"));
            }
            return;
        }
        let Some(staging) = self.staging(data) else {
            self.fail(format_args!("no staging buffer"));
            return;
        };
        self.mark_used(buffer);
        self.copy_bytes(staging, Self::range(buffer, offset), data.len() as u64);
    }

    fn write_texture(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        _mode: WriteMode,
    ) {
        let texture = region.texture.clone();
        let bytes = Self::converted_in(texture.desc.format, region.stencil, data);
        let Some(staging) = self.staging(&bytes) else {
            self.fail(format_args!("no staging buffer"));
            return;
        };
        let copies = self.buffer_texture_copies(
            &texture,
            region,
            extent,
            &staging.buffer,
            0,
            u64::from(row_pitch),
            u64::from(rows_per_layer),
            true,
        );
        self.queue_copies(copies);
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        self.flush_blits();
        self.pass = Some(OpenPass::new(pass));
    }

    fn end_pass(&mut self) {
        self.end_open_pass();
    }

    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        if let Err(why) = self.record_draw(call) {
            self.fail(format_args!("{why}"));
        }
    }

    fn begin_query(&mut self, index: u32) {
        self.record_query(Some(index));
    }

    fn end_query(&mut self) {
        self.record_query(None);
    }

    fn copy_buffer(
        &mut self,
        src: &Self::Buffer,
        src_offset: u64,
        dst: &Self::Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        self.mark_used(src);
        self.mark_used(dst);
        self.copy_bytes(
            Self::range(src, src_offset),
            Self::range(dst, dst_offset),
            size,
        );
    }

    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let (s, d) = (src.texture.clone(), dst.texture.clone());
        let (s3, d3) = (
            s.desc.dimension == TextureDimension::D3,
            d.desc.dimension == TextureDimension::D3,
        );
        // Both regions are checked first, so the layers below, and each sum of them, are bounded by the textures'.
        if !texture_region_inside(src, extent) || !texture_region_inside(dst, extent) {
            self.fail(format_args!("a copy outside its textures"));
            return;
        }
        let layers = if s3 { 1 } else { extent.depth_or_layers };
        for layer in 0..layers {
            let (Some(source_layer), Some(destination_layer)) =
                (src.layer.checked_add(layer), dst.layer.checked_add(layer))
            else {
                self.fail(format_args!("a copy whose layers overflow"));
                return;
            };
            self.queue_blit(BlitCommand::CopyFromTextureToTexture {
                source_texture: s.raw.clone(),
                source_slice: if s3 { 0 } else { source_layer },
                source_level: src.mip,
                source_origin: Origin {
                    x: src.origin.x,
                    y: src.origin.y,
                    z: if s3 { src.origin.z } else { 0 },
                },
                source_size: Size {
                    width: extent.width,
                    height: extent.height,
                    depth: if s3 { extent.depth_or_layers } else { 1 },
                },
                destination_texture: d.raw.clone(),
                destination_slice: if d3 { 0 } else { destination_layer },
                destination_level: dst.mip,
                destination_origin: Origin {
                    x: dst.origin.x,
                    y: dst.origin.y,
                    z: if d3 { dst.origin.z } else { 0 },
                },
            });
        }
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let texture = dst.texture.clone();
        self.mark_used(src.buffer);
        let raw = src.buffer.raw.borrow().clone();
        if convert::depth_converted(texture.desc.format) && !dst.stencil {
            // Metal holds this depth as float: the words are read on the host and converted.
            if !texture_region_inside(dst, extent) {
                self.fail(format_args!("a depth copy outside its texture"));
                return;
            }
            let Some(packed) = self.depth_words_in(src, &raw, extent, texture.desc.format) else {
                self.fail(format_args!("a depth copy whose places overflow"));
                return;
            };
            let rows = self.tight_rows(texture.desc.format, false, extent);
            let Some((Ok(row), Ok(rows))) =
                rows.map(|(row, rows)| (u32::try_from(row), u32::try_from(rows)))
            else {
                self.fail(format_args!("a depth copy whose rows overflow"));
                return;
            };
            self.write_texture(dst, extent, &packed, row, rows, WriteMode::Ordered);
            return;
        }
        let copies = self.buffer_texture_copies(
            &texture,
            dst,
            extent,
            &raw,
            src.offset,
            u64::from(src.row_pitch),
            u64::from(src.rows_per_layer),
            true,
        );
        self.queue_copies(copies);
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        let texture = src.texture.clone();
        self.mark_used(dst.buffer);
        let raw = dst.buffer.raw.borrow().clone();
        if convert::depth_converted(texture.desc.format) && !src.stencil {
            // `read_back` refuses a region outside the texture, so the words below are bounded by its bytes.
            let Ok(readback) = self.read_back(&ReadSource::Texture {
                region: TextureRegion { ..*src },
                extent,
            }) else {
                self.fail(format_args!("no read back for a depth copy"));
                return;
            };
            self.wait_read(&readback);
            let mut words = vec![0; usize::try_from(readback.length).unwrap_or(usize::MIN)];
            self.finish_read(readback, &mut words);
            if self
                .depth_words_out(&words, dst, extent, texture.desc.format)
                .is_none()
            {
                self.fail(format_args!("a depth copy whose places overflow"));
            }
            return;
        }
        let copies = self.buffer_texture_copies(
            &texture,
            src,
            extent,
            &raw,
            dst.offset,
            u64::from(dst.row_pitch),
            u64::from(dst.rows_per_layer),
            false,
        );
        self.queue_copies(copies);
    }

    fn clear_buffer(&mut self, buffer: &Self::Buffer, offset: u64, size: u64, pattern: &[u8]) {
        if size == 0 {
            return;
        }
        // The range and the pattern are checked before anything is sized by them.
        if offset.checked_add(size).is_none_or(|end| end > buffer.size) {
            self.fail(format_args!("a clear past its buffer's end"));
            return;
        }
        let Some(&first) = pattern.first() else {
            self.fail(format_args!("a clear with an empty pattern"));
            return;
        };
        if !size.is_multiple_of(pattern.len() as u64) {
            self.fail(format_args!("a clear of a part of a pattern"));
            return;
        }
        self.mark_used(buffer);
        if pattern.iter().all(|b| *b == first) {
            self.fill_bytes(Self::range(buffer, offset), size, first);
            return;
        }
        self.record_slow_path(wcontainers_gpu::SlowPath::PatternClearCopied);
        // The size is within the buffer, which is within `Limits::max_buffer_size`.
        let Ok(length) = usize::try_from(size) else {
            self.fail(format_args!("a clear longer than the host holds"));
            return;
        };
        let bytes: Vec<u8> = pattern.iter().copied().cycle().take(length).collect();
        let Some(staging) = self.staging(&bytes) else {
            self.fail(format_args!("no staging buffer"));
            return;
        };
        self.copy_bytes(staging, Self::range(buffer, offset), size);
    }

    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) {
        if let Err(why) = self.record_resolve(src, dst, format) {
            self.fail(format_args!("{why}"));
        }
    }

    fn submit(&mut self) -> Self::Fence {
        MetalFence(self.commit())
    }

    fn fence_done(&mut self, fence: &Self::Fence) -> bool {
        self.poll();
        fence.0 <= self.completed
    }

    fn wait_fence(&mut self, fence: &Self::Fence) {
        self.wait_serial(fence.0);
    }

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Self::Readback, GpuError> {
        // A range outside its resource is refused before anything is sized by it, as the recorder refuses it.
        match src {
            ReadSource::Buffer {
                buffer,
                offset,
                size,
            } => {
                if offset
                    .checked_add(*size)
                    .is_none_or(|end| end > buffer.size)
                {
                    return Err(GpuError::Invalid("a read past its buffer's end"));
                }
                self.mark_used(buffer);
                let length = rounded(*size).ok_or(GpuError::OutOfMemory)?;
                let into = self
                    .metal
                    .make_buffer(length)
                    .map_err(|_| GpuError::OutOfMemory)?;
                // The read back buffer is this implementation's own, so the copy may run to its end.
                let copied = if offset.is_multiple_of(bytes::COPY_GRANULE) {
                    length
                } else {
                    *size
                };
                let destination = ByteRange {
                    buffer: into.clone(),
                    length,
                    offset: u64::MIN,
                };
                self.copy_bytes(Self::range(buffer, *offset), destination, copied);
                Ok(Shared(Rc::new(ReadbackObj {
                    serial: self.serial,
                    buffer: into,
                    length: *size,
                    layout: ReadLayout::Bytes,
                })))
            }
            ReadSource::Texture { region, extent } => {
                if !texture_region_inside(region, *extent) {
                    return Err(GpuError::Invalid("a read outside its texture"));
                }
                let texture = region.texture.clone();
                let (length, row, rows) = self
                    .tight_bytes(texture.desc.format, region.stencil, *extent)
                    .ok_or(GpuError::OutOfMemory)?;
                let layout = if convert::depth_converted(texture.desc.format) && !region.stencil {
                    ReadLayout::Depth24
                } else {
                    ReadLayout::Bytes
                };
                let buffer = self
                    .metal
                    .make_buffer(rounded(length).ok_or(GpuError::OutOfMemory)?)
                    .map_err(|_| GpuError::OutOfMemory)?;
                let commands = self
                    .buffer_texture_copies(&texture, region, *extent, &buffer, 0, row, rows, false);
                self.queue_copies(commands);
                Ok(Shared(Rc::new(ReadbackObj {
                    serial: self.serial,
                    buffer,
                    length,
                    layout,
                })))
            }
        }
    }

    fn read_queries(
        &mut self,
        set: &Self::QuerySet,
        first: u32,
        count: u32,
    ) -> Result<Self::Readback, GpuError> {
        if first.checked_add(count).is_none_or(|end| end > set.count) {
            return Err(GpuError::Invalid("queries past the set's end"));
        }
        let raw = set.buffer.clone();
        let length = query_bytes(count);
        // No queries read nothing, with no copy, which Metal refuses of no bytes.
        self.read_into(length, ReadLayout::Bytes, |into| {
            if length == 0 {
                return Vec::new();
            }
            vec![BlitCommand::CopyFromBufferToBuffer {
                source_buffer: raw,
                source_offset: query_bytes(first),
                destination_buffer: into.clone(),
                destination_offset: u64::MIN,
                size: length,
            }]
        })
    }

    fn read_state(&mut self, readback: &Self::Readback) -> ReadState {
        self.poll();
        if readback.serial <= self.completed {
            ReadState::Ready
        } else {
            ReadState::Pending
        }
    }

    fn wait_read(&mut self, readback: &Self::Readback) {
        self.wait_serial(readback.serial);
    }

    fn finish_read(&mut self, readback: Self::Readback, dst: &mut [u8]) {
        self.wait_serial(readback.serial);
        // The length was checked against its buffer or texture when the read back began.
        let mut bytes = vec![0; usize::try_from(readback.length).unwrap_or(usize::MIN)];
        if let Err(error) = self.metal.read_contents(&readback.buffer, 0, &mut bytes) {
            self.fail(format_args!("read refused: {error:?}"));
            return;
        }
        if let ReadLayout::Depth24 = readback.layout {
            for word in bytes.as_chunks_mut::<4>().0 {
                let depth = f64::from(f32::from_le_bytes(*word)).clamp(0.0, 1.0);
                *word = ((depth * DEPTH24_MAX).round_ties_even() as u32).to_le_bytes();
            }
        }
        let n = dst.len().min(bytes.len());
        dst[..n].copy_from_slice(&bytes[..n]);
    }

    fn create_surface(&mut self, window: u32) -> Result<Self::Surface, GpuError> {
        let layer = self
            .windows
            .get(&window)
            .cloned()
            .ok_or(GpuError::Invalid("a window the host did not add"))?;
        Ok(Shared(Rc::new(SurfaceObj {
            layer,
            frame: RefCell::new(None),
        })))
    }

    fn configure_surface(
        &mut self,
        surface: &Self::Surface,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError> {
        if !self.caps.formats.uses[format as usize].contains(wcontainers_gpu::FormatUse::PRESENT) {
            return Err(GpuError::Invalid("a surface format without PRESENT"));
        }
        self.metal
            .configure_layer(
                &surface.layer,
                &wcontainers_metal::LayerDescriptor {
                    pixel_format: wcontainers_metal::PixelFormat::Bgra8Unorm,
                    width,
                    height,
                    display_sync_enabled: vsync,
                },
            )
            .map_err(|_| GpuError::Invalid("a layer Metal does not configure"))?;
        // The frame is drawn into a texture of the surface's format, then into the layer's drawable.
        let texture = self.create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width,
            height,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::SAMPLE,
            cube: false,
            reinterpret: false,
        })?;
        let whole = |format| TextureViewDesc {
            format,
            dimension: ViewDimension::D2,
            base_mip: u32::MIN,
            mip_count: 1,
            base_layer: u32::MIN,
            layer_count: 1,
            stencil: false,
        };
        let drawn = self.create_view(&texture, &whole(format))?;
        let read = self.create_view(&texture, &whole(FORMAT_FACTS[format as usize].linear))?;
        *surface.frame.borrow_mut() = Some(Frame {
            format,
            drawn,
            read,
        });
        Ok(())
    }

    fn current_texture(&mut self, surface: &Self::Surface) -> Result<Self::TextureView, GpuError> {
        surface
            .frame
            .borrow()
            .as_ref()
            .map(|frame| frame.drawn.clone())
            .ok_or(GpuError::Invalid("a surface not configured"))
    }

    fn present(&mut self, surface: &Self::Surface) -> Result<(), GpuError> {
        let (format, read) = match &*surface.frame.borrow() {
            Some(frame) => (frame.format, frame.read.clone()),
            None => return Err(GpuError::Invalid("a surface not configured")),
        };
        if format != Format::Bgra8Unorm {
            self.record_slow_path(wcontainers_gpu::SlowPath::PresentConverted);
        }
        let drawable = self
            .metal
            .next_drawable(&surface.layer)
            .map_err(|_| GpuError::Invalid("no drawable to present"))?;
        let texture = self.metal.drawable_texture(&drawable);
        let (width, height) = (read.width, read.height);
        let view = Shared(Rc::new(ViewObj {
            raw: texture,
            format: Format::Bgra8Unorm,
            width,
            height,
            samples: 1,
        }));
        self.draw_frame(&view, &read)?;
        let command_buffer = self.command_buffer().ok_or(GpuError::DeviceLost)?;
        self.metal
            .present_drawable(&command_buffer, &drawable)
            .map_err(|_| GpuError::Invalid("a drawable Metal does not present"))?;
        self.commit();
        Ok(())
    }

    fn generate_mips(
        &mut self,
        texture: &Self::Texture,
        desc: &TextureDesc,
        values: Format,
        mips: std::ops::Range<u32>,
        layers: std::ops::Range<u32>,
    ) -> Result<(), GpuError> {
        // Metal fills every level of every layer at once, in the texture's own format.
        let whole = mips == (1..desc.mip_levels) && layers == (0..desc.depth_or_layers);
        let filtered = self.caps.formats.uses[values as usize]
            .contains(wcontainers_gpu::FormatUse::RENDER | wcontainers_gpu::FormatUse::FILTER);
        if !whole || values != desc.format || !filtered || desc.samples > 1 {
            return wcontainers_gpu::generate_mips_by_draws(
                self, texture, desc, values, mips, layers,
            );
        }
        self.queue_blit(BlitCommand::GenerateMipmaps {
            texture: texture.raw.clone(),
        });
        Ok(())
    }

    fn prepare(&mut self, desc: &wcontainers_gpu::PrepareDesc<'_, Self>) {
        // Metal builds the pipeline now, so the first draw finds it.
        self.prepare_pipeline(desc);
    }

    fn read_back_now(
        &mut self,
        src: &ReadSource<'_, Self>,
        dst: &mut [u8],
    ) -> Result<(), GpuError> {
        // A buffer is read straight from the memory the CPU shares with the GPU once the work that uses it is done. A
        // range past the buffer's end is the error `read_back` gives, so this agrees with the default on every input.
        if let ReadSource::Buffer {
            buffer,
            offset,
            size,
        } = src
        {
            if offset
                .checked_add(*size)
                .is_none_or(|end| end > buffer.size)
            {
                return Err(GpuError::Invalid("a read past its buffer's end"));
            }
            if self.busy(buffer) {
                self.record_slow_path(wcontainers_gpu::SlowPath::ReadBackWait);
                self.wait_serial(buffer.last_use.get());
            }
            let raw = buffer.raw.borrow().clone();
            let length = usize::try_from(*size).unwrap_or(usize::MAX).min(dst.len());
            if let Err(error) = self.metal.read_contents(&raw, *offset, &mut dst[..length]) {
                self.fail(format_args!("read refused: {error:?}"));
            }
            return Ok(());
        }
        let readback = self.read_back(src)?;
        if self.read_state(&readback) == ReadState::Pending {
            self.record_slow_path(wcontainers_gpu::SlowPath::ReadBackWait);
            self.wait_read(&readback);
        }
        self.finish_read(readback, dst);
        Ok(())
    }

    fn write_buffer_ring(
        &mut self,
        kind: wcontainers_gpu::RingKind,
        parts: &[&[u8]],
        align: u32,
        offsets: &mut [u64],
    ) -> Result<Self::Buffer, GpuError> {
        self.write_ring(kind, parts, align, offsets)
    }

    fn resolve_queries(
        &mut self,
        set: &Self::QuerySet,
        first: u32,
        count: u32,
        dst: &Self::Buffer,
        dst_offset: u64,
    ) -> Result<(), GpuError> {
        if !dst_offset.is_multiple_of(QUERY_BYTES) {
            return Err(GpuError::Invalid(
                "query results go to multiples of 8 bytes",
            ));
        }
        if first.checked_add(count).is_none_or(|end| end > set.count) {
            return Err(GpuError::Invalid("queries past the set's end"));
        }
        // As the default: no queries write nothing, wherever, and results past the buffer's end are the refusal its
        // `write_buffer` notes, with the call itself answered.
        let bytes = query_bytes(count);
        if bytes == 0 {
            return Ok(());
        }
        if dst_offset
            .checked_add(bytes)
            .is_none_or(|end| end > dst.size)
        {
            self.fail(format_args!("a write past its buffer's end"));
            return Ok(());
        }
        // The results are copied on the GPU, in order with the work around them, and never wait.
        self.mark_used(dst);
        let source = ByteRange {
            buffer: set.buffer.clone(),
            length: query_bytes(set.count.max(1)),
            offset: query_bytes(first),
        };
        self.copy_bytes(source, Self::range(dst, dst_offset), bytes);
        Ok(())
    }

    fn dispatch(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
        let DispatchGroups::Direct(groups) = call.groups else {
            return Err(GpuError::Absent(Features::INDIRECT));
        };
        if groups.contains(&0) {
            return Ok(());
        }
        let ProgramObj::Compute { id, shader } = &*call.program.0 else {
            return Err(GpuError::Invalid("dispatch of a render program"));
        };
        let pipeline = match self.compute_pipelines.get(id) {
            Some(pipeline) => pipeline.clone(),
            None => {
                let function = self
                    .make_function(shader, &[], false)
                    .map_err(GpuError::Invalid)?;
                let pipeline = self
                    .metal
                    .make_compute_pipeline_state(&function)
                    .map_err(|_| GpuError::Invalid("a compute pipeline Metal refuses"))?;
                self.compute_pipelines.insert(*id, pipeline.clone());
                pipeline
            }
        };
        let mut commands = vec![ComputeCommand::SetComputePipelineState(pipeline)];
        for (group, set) in call.bindings.iter().enumerate() {
            let Some(set) = set else {
                continue;
            };
            for (binding, resource) in &set.entries {
                let Some(&(_, index)) = shader.slots.get(&(group as u32, *binding)) else {
                    continue;
                };
                commands.push(match resource {
                    SetResource::Buffer {
                        buffer,
                        offset,
                        size,
                    } => {
                        self.mark_used(buffer);
                        ComputeCommand::SetBuffer {
                            buffer: buffer.raw.borrow().clone(),
                            offset: *offset,
                            length: *size,
                            index,
                        }
                    }
                    SetResource::Texture(view) => ComputeCommand::SetTexture {
                        texture: view.raw.clone(),
                        index,
                    },
                    SetResource::Sampler(sampler) => ComputeCommand::SetSamplerState {
                        sampler: sampler.raw.clone(),
                        index,
                    },
                });
            }
        }
        if !call.immediates.is_empty() {
            commands.push(ComputeCommand::SetBytes {
                bytes: call.immediates.to_vec(),
                index: IMMEDIATES_SLOT,
            });
        }
        commands.push(ComputeCommand::DispatchThreadgroups(Size {
            width: groups[0],
            height: groups[1],
            depth: groups[2],
        }));
        self.flush_blits();
        let command_buffer = self.command_buffer().ok_or(GpuError::DeviceLost)?;
        count_up(&mut self.encodes);
        self.metal
            .encode_compute_commands(&command_buffer, &commands)
            .map_err(|error| {
                eprintln!("metal: dispatch refused: {error:?}");
                GpuError::Invalid("a dispatch Metal's checks refuse")
            })
    }
}

/// Counts one more: a count of calls never reaches the end of a `u64`, and would stop there rather than wrap to reuse one.
fn count_up(count: &mut u64) {
    *count = count.saturating_add(1);
}

/// Counts one more binding of a kind, each below the slots of its kind, so far below the end of a `u32`.
fn count_up_u32(count: &mut u32) {
    *count = count.saturating_add(1);
}

/// Returns the bytes of `count` query results.
fn query_bytes(count: u32) -> u64 {
    u64::from(count).saturating_mul(QUERY_BYTES)
}

/// Returns whether a region of `extent` lies inside a level and the layers of a texture, which bound its lengths.
fn region_inside(
    desc: &TextureDesc,
    mip: u32,
    origin: [u32; 3],
    layer: u32,
    extent: Extent,
) -> bool {
    if mip >= desc.mip_levels {
        return false;
    }
    let side = |n: u32| n.checked_shr(mip).unwrap_or(u32::MIN).max(1);
    let (bw, bh, _) = FORMAT_FACTS[desc.format as usize].block;
    let fits = |at: u32, n: u32, most: Option<u32>| {
        at.checked_add(n)
            .is_some_and(|end| most.is_some_and(|most| end <= most))
    };
    let three_d = desc.dimension == TextureDimension::D3;
    let (depth, layers) = if three_d {
        (Some(side(desc.depth_or_layers)), Some(1))
    } else {
        (Some(1), Some(desc.depth_or_layers))
    };
    let (z, slices) = if three_d {
        ((origin[2], extent.depth_or_layers), (layer, 1))
    } else {
        ((origin[2], 1), (layer, extent.depth_or_layers))
    };
    fits(
        origin[0],
        extent.width,
        side(desc.width).checked_next_multiple_of(bw),
    ) && fits(
        origin[1],
        extent.height,
        side(desc.height).checked_next_multiple_of(bh),
    ) && fits(z.0, z.1, depth)
        && fits(slices.0, slices.1, layers)
}

/// Returns whether a texture region of `extent` lies inside its texture.
fn texture_region_inside<M: Metal>(
    region: &TextureRegion<'_, MetalGpu<M>>,
    extent: Extent,
) -> bool {
    let origin = [region.origin.x, region.origin.y, region.origin.z];
    region_inside(
        &region.texture.desc,
        region.mip,
        origin,
        region.layer,
        extent,
    )
}

/// Returns whether a format has an sRGB twin, whose views Metal makes only of textures made for them.
fn has_srgb_twin(format: Format) -> bool {
    use Format as F;
    matches!(
        format,
        F::Rgba8Unorm
            | F::Rgba8UnormSrgb
            | F::Bgra8Unorm
            | F::Bgra8UnormSrgb
            | F::Bc1RgbaUnorm
            | F::Bc1RgbaUnormSrgb
            | F::Bc2RgbaUnorm
            | F::Bc2RgbaUnormSrgb
            | F::Bc3RgbaUnorm
            | F::Bc3RgbaUnormSrgb
            | F::Bc7RgbaUnorm
            | F::Bc7RgbaUnormSrgb
            | F::Etc2Rgb8Unorm
            | F::Etc2Rgb8UnormSrgb
            | F::Etc2Rgba8Unorm
            | F::Etc2Rgba8UnormSrgb
            | F::Astc4x4Unorm
            | F::Astc4x4UnormSrgb
    )
}
