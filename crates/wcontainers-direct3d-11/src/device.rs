//! The device: object creation, each preceded by the checks Direct3D 11 makes at creation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use wcontainers_gpu::{
    AddressMode, Backend, BlendComponent, BlendFactor, BlendOp, BorderColor,
    BufferDesc as GpuBuffer, BufferUsage, ColorWrites, CompareFn, CullMode,
    DepthStencilDesc as GpuDepthStencil, DeviceStatus, Extent, Features, FillMode, Filter, Format,
    FormatKind, FormatUse, FrontFace, GpuError, LossReason, MAX_COLOR_TARGETS, MapIntent, Object,
    Origin, ProvokingVertex, RasterDesc, RenderTargetBlend, SamplerDesc as GpuSampler, SlowPath,
    StageValue, StencilFace, StencilOp, TextureDesc as GpuTexture, TextureDimension, TextureRegion,
    TextureUsage, TextureViewDesc, ViewDimension, WriteMode,
};

use crate::context::ContextState;
use crate::desc::*;
use crate::error::{Error, Result, not_implemented};
use crate::format;
use crate::object::*;
use crate::shader;

/// A Direct3D 11 device and its immediate context on back end `B`.
pub struct Device<B: Backend> {
    pub backend: B,
    pub(crate) graveyard: SharedGraveyard<B>,
    next_id: u64,
    level: u32,
    pub(crate) defaults: Defaults<B>,
    pub(crate) ctx: ContextState<B>,
    pub(crate) queries: crate::query::Queries<B>,
    /// Programs by (vertex shader, pixel shader) id.
    pub(crate) programs: HashMap<(u64, u64), Kept<B, B::Program>>,
    /// The vertex shaders converted for one pixel shader's interpolation, by the program's ids.
    pub(crate) interpolated: HashMap<(u64, u64), Kept<B, B::Shader>>,
    /// Vertex layout blocks by (input layout id, stride of each slot it reads).
    pub(crate) layouts: HashMap<(u64, [u32; 32]), Kept<B, B::VertexLayout>>,
    /// Vertex shaders' input signatures by the blake3 of their bytecode, for input layouts.
    vertex_inputs: HashMap<[u8; 32], Vec<StageValue>>,
    /// What empty slots read.
    pub(crate) zeros: Zeros<B>,
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
    pub textures: HashMap<(ViewDimension, FormatKind), ZeroView<B>>,
    pub sampler: Option<Rc<Sampler<B>>>,
}

/// A texture of zeros and its view.
pub(crate) struct ZeroView<B: Backend> {
    _texture: Kept<B, B::Texture>,
    view: Kept<B, B::TextureView>,
}

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
        GpuError::Absent(_) => Error::NotImplemented("a feature the back end lacks"),
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
const DEFAULT_LEVELS: [u32; 4] = [level::L11_0, level::L10_1, level::L10_0, 0x9300];

/// The highest feature level `backend` serves (plan section 9): 11_0 with compute, 10_0 without.
pub fn top_level(caps: &wcontainers_gpu::Caps) -> u32 {
    if caps.has(Features::COMPUTE) {
        level::L11_0
    } else {
        level::L10_0
    }
}

impl<B: Backend> Device<B> {
    /// Makes the device at the first of `levels` (or of Direct3D's default list) the back end serves.
    pub fn new(mut backend: B, levels: &[u32]) -> Result<Self> {
        let top = top_level(backend.caps());
        let asked: &[u32] = if levels.is_empty() {
            &DEFAULT_LEVELS
        } else {
            levels
        };
        let level = *asked
            .iter()
            .find(|&&l| l <= top && l >= level::L10_0)
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
            .create_raster(&raster_block(&raster_desc))
            .map_err(gpu_error)?;
        let defaults = Defaults {
            blend: Rc::new(Blend {
                desc: blend_desc,
                gpu: Kept::new(blend, id(), Object::Blend, &graveyard),
            }),
            depth_stencil: Rc::new(DepthStencil {
                desc: ds_desc,
                gpu: Kept::new(ds, id(), Object::DepthStencil, &graveyard),
            }),
            raster: Rc::new(Rasterizer {
                desc: raster_desc,
                gpu: Kept::new(raster, id(), Object::Raster, &graveyard),
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
            layouts: HashMap::new(),
            vertex_inputs: HashMap::new(),
            zeros: Zeros {
                buffer: None,
                storage: None,
                textures: HashMap::new(),
                sampler: None,
            },
            removed: None,
        })
    }

    pub fn feature_level(&self) -> u32 {
        self.level
    }

    /// What `CheckFormatSupport` answers for DXGI format `dxgi`: the uses this front serves on its
    /// back end (1D and 3D textures and typed buffers are not reported).
    pub fn format_support(&self, dxgi: u32) -> u32 {
        use crate::desc::support::*;
        let mut bits = 0;
        if format::vertex_format(dxgi).is_some() {
            bits |= IA_VERTEX_BUFFER;
        }
        if format::index_format(dxgi).is_some() {
            bits |= IA_INDEX_BUFFER;
        }
        // A typeless format is reported as its storage: textures of it, not its uses (its members').
        if format::is_typeless(dxgi) {
            let stored = format::storage_format(dxgi, false).ok();
            if stored.is_some_and(|f| !self.backend.caps().formats.get(f).is_empty()) {
                bits |= TEXTURE2D | TEXTURECUBE | MIP;
            }
            return bits;
        }
        let Some(f) = format::texture_format(dxgi) else {
            return bits;
        };
        let uses = self.backend.caps().formats.get(f);
        if uses.is_empty() {
            return bits;
        }
        bits |= TEXTURE2D | TEXTURECUBE | MIP;
        for (has, bit) in [
            (FormatUse::SAMPLE, SHADER_LOAD),
            (FormatUse::FILTER, SHADER_SAMPLE),
            (FormatUse::RENDER, RENDER_TARGET),
            (FormatUse::BLEND, BLENDABLE),
            (FormatUse::DEPTH_STENCIL, DEPTH_STENCIL),
            (FormatUse::COPY_OUT, CPU_LOCKABLE),
        ] {
            if uses.contains(has) {
                bits |= bit;
            }
        }
        if [2, 4, 8, 16].iter().any(|&n| self.takes_samples(f, n)) {
            bits |= MULTISAMPLE_RENDERTARGET;
            if uses.contains(FormatUse::SAMPLE) {
                bits |= MULTISAMPLE_LOAD;
            }
            if resolvable(f) {
                bits |= MULTISAMPLE_RESOLVE;
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
            return u32::from(self.format_support(dxgi) & crate::desc::support::TEXTURE2D != 0);
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
            eprintln!("wcontainers direct3d 11: device removed: the back end lost it ({reason:?})");
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
            eprintln!("wcontainers direct3d 11: device removed: {why}");
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
                    eprintln!("wcontainers direct3d 11: device removed: the back end lost it");
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

    /// A view of zeros of `kind` in `dimension`, made at its first use.
    pub(crate) fn zero_view(
        &mut self,
        dimension: ViewDimension,
        kind: FormatKind,
    ) -> Result<B::TextureView> {
        if let Some(z) = self.zeros.textures.get(&(dimension, kind)) {
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
            ViewDimension::Cube | ViewDimension::CubeArray => (TextureDimension::D2, 6),
            ViewDimension::D3 => (TextureDimension::D3, 1),
        };
        let desc = GpuTexture {
            dimension: texture_dimension,
            format,
            width: 1,
            height: 1,
            depth_or_layers: layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            cube: layers == 6,
            reinterpret: false,
        };
        let texture = self.backend.create_texture(&desc);
        let texture = self.gpu(texture)?;
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
            (dimension, kind),
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
            .retain(|(v, p), _| !ids.contains(v) && !ids.contains(p));
        self.interpolated
            .retain(|(v, p), _| !ids.contains(v) && !ids.contains(p));
        self.layouts.retain(|(l, _), _| !ids.contains(l));
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
        if d.misc_flags & !(MISC_DRAWINDIRECT_ARGS | MISC_ALLOW_RAW_VIEWS | MISC_STRUCTURED) != 0 {
            return not_implemented(
                "buffers with misc flags other than structured, raw and indirect",
            );
        }
        self.live()?;
        self.bury();
        // The buffer is made whole granules of the back end's copies long, so writing the initial data
        // (padded with zeros past the program's bytes) never needs an edge merge.
        let granule = u64::from(self.backend.caps().limits.buffer_copy_alignment.max(1));
        let size = u64::from(d.byte_width).div_ceil(granule) * granule;
        let gpu = self.backend.create_buffer(&gpu_buffer(d, size));
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
        }))
    }

    /// The checks of `CreateTexture1D`, `2D` and `3D` before their initial data is read: the format
    /// and the number of mip levels, or why the description is refused.
    pub fn check_texture(&self, d: &TextureDesc, has_init: bool) -> Result<(Format, u32)> {
        let limits = &self.backend.caps().limits;
        if d.width == 0 || d.height == 0 || d.depth == 0 || d.array_size == 0 {
            return Err(Error::InvalidArg("a texture needs a size"));
        }
        let (most, layers) = match d.dimension {
            TexDim::D3 => (limits.max_texture_3d, 1),
            _ => (limits.max_texture_2d, limits.max_texture_layers),
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
        let format = format::storage_format(d.format, d.bind_flags & bind::DEPTH_STENCIL != 0)?;
        // A count and quality the device reports no level for is invalid, as in Direct3D.
        if d.sample_quality >= self.multisample_quality_levels(d.format, d.sample_count) {
            return Err(Error::InvalidArg(
                "a sample count and quality the device reports no level for",
            ));
        }
        if d.sample_count > 1
            && (d.dimension != TexDim::D2
                || mips != 1
                || d.usage != usage::DEFAULT
                || d.bind_flags & (bind::RENDER_TARGET | bind::DEPTH_STENCIL) == 0
                || d.bind_flags & bind::UNORDERED_ACCESS != 0)
        {
            return Err(Error::InvalidArg(
                "a multisampled texture is 2D, of one level and default usage, drawn into",
            ));
        }
        let cube = d.misc_flags & MISC_TEXTURECUBE != 0;
        if d.misc_flags & !(MISC_TEXTURECUBE | MISC_GENERATE_MIPS) != 0 {
            return not_implemented(
                "textures with misc flags other than a cube and mip generation",
            );
        }
        let both = bind::SHADER_RESOURCE | bind::RENDER_TARGET;
        if d.misc_flags & MISC_GENERATE_MIPS != 0 && d.bind_flags & both != both {
            return Err(Error::InvalidArg(
                "mip generation needs shader resource and render target bindings",
            ));
        }
        if cube
            && (!d.array_size.is_multiple_of(6) || d.width != d.height || d.dimension != TexDim::D2)
        {
            return Err(Error::InvalidArg(
                "a cube texture needs square faces in sixes",
            ));
        }
        match d.usage {
            usage::IMMUTABLE if !has_init => {
                return Err(Error::InvalidArg("an immutable texture needs initial data"));
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
            (bind::UNORDERED_ACCESS, FormatUse::STORAGE_WRITE),
        ] {
            if d.bind_flags & flag != 0 {
                needs |= n;
            }
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
        let cube = d.misc_flags & MISC_TEXTURECUBE != 0;
        let mut usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
        for (flag, u) in [
            (bind::SHADER_RESOURCE, TextureUsage::SAMPLE),
            (bind::RENDER_TARGET, TextureUsage::RENDER),
            (bind::DEPTH_STENCIL, TextureUsage::RENDER),
            (bind::UNORDERED_ACCESS, TextureUsage::STORAGE),
        ] {
            if d.bind_flags & flag != 0 {
                usage |= u;
            }
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
        let (dimension, depth_or_layers) = match d.dimension {
            TexDim::D3 => (TextureDimension::D3, d.depth),
            TexDim::D1 | TexDim::D2 => (TextureDimension::D2, d.array_size),
        };
        let gpu_desc = GpuTexture {
            dimension,
            format,
            width: d.width,
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
        };
        for (i, data) in init.iter().take(subresources).enumerate() {
            let (region, extent) = texture
                .whole(i as u32)
                .expect("a subresource of the texture");
            self.write_texels(format, &region, extent, data);
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
        let t = &texture.desc;
        if t.bind_flags & kind == 0 {
            return Err(Error::InvalidArg(
                "the resource lacks the bind flag the view needs",
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
                (TexDim::D2, _) if cube && t.array_size == 6 => ViewDim::TextureCube,
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
        if of != t.dimension || ms_view != multisampled {
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
            v.layer_count = layers - v.first_layer.min(layers);
        }
        if v.first_mip + v.mip_count > t.mip_levels
            || v.mip_count == 0
            || v.first_layer + v.layer_count > layers
            || v.layer_count == 0
            || (kind != bind::SHADER_RESOURCE && v.mip_count != 1)
        {
            return Err(Error::InvalidArg("the view reaches past the resource"));
        }
        let (format, stencil) = format::view_format(v.format, t.format, texture.format)?;
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
            && (!texture.gpu_desc.cube
                || !v.layer_count.is_multiple_of(6)
                || (v.dimension == ViewDim::TextureCube && v.layer_count != 6))
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
            let dimension = match v.dimension {
                ViewDim::Texture1D | ViewDim::Texture2D | ViewDim::Texture2DMS => ViewDimension::D2,
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
        self.live()?;
        self.bury();
        let gpu = self.backend.create_view(
            &texture.gpu.handle,
            &TextureViewDesc {
                format,
                dimension,
                base_mip: v.first_mip,
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
            gpu: Kept::new(gpu, id, Object::TextureView, &self.graveyard),
        }))
    }

    /// An unordered access view of one level of a texture.
    pub fn create_texture_uav(
        &mut self,
        texture: &Rc<Texture<B>>,
        desc: Option<&ViewDesc>,
    ) -> Result<Rc<ShaderView<B>>> {
        let v = self.create_view(texture, desc, bind::UNORDERED_ACCESS)?;
        let t = &texture.desc;
        if t.dimension == TexDim::D3
            && (v.desc.first_layer != 0
                || v.desc.layer_count != (t.depth >> v.desc.first_mip).max(1))
        {
            return not_implemented("an unordered access view of some slices of a 3D level");
        }
        self.check_format(v.format, FormatUse::STORAGE_WRITE)?;
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
        const RAW: u32 = 1;
        const APPEND: u32 = 2;
        const COUNTER: u32 = 4;
        let raw = flags & RAW != 0;
        let (offset, size) =
            self.buffer_span(buffer, bind::UNORDERED_ACCESS, format, first, count, raw)?;
        let structured = buffer.desc.misc_flags & MISC_STRUCTURED != 0;
        let counted = flags & (APPEND | COUNTER);
        if counted == APPEND | COUNTER || (counted != 0 && (!structured || raw)) {
            return Err(Error::InvalidArg(
                "append and counter views are structured, one or the other",
            ));
        }
        self.live()?;
        self.bury();
        let counter = if counted != 0 {
            let desc = BufferDesc {
                byte_width: 4,
                usage: usage::DEFAULT,
                bind_flags: bind::UNORDERED_ACCESS,
                cpu_access_flags: 0,
                misc_flags: 0,
                structure_byte_stride: 0,
            };
            let gpu = self.backend.create_buffer(&gpu_buffer(&desc, 4));
            let gpu = self.gpu(gpu)?;
            self.backend
                .write_buffer(&gpu, 0, &[0; 4], WriteMode::Discard);
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
        let (offset, size) =
            self.buffer_span(buffer, bind::SHADER_RESOURCE, format, first, count, raw)?;
        self.live()?;
        Ok(Rc::new(ShaderView::Buffer(BufferSpan {
            buffer: buffer.clone(),
            offset,
            size,
            counter: None,
        })))
    }

    /// The bytes (offset, size) that `count` elements from `first` of a buffer view cover: raw
    /// (32-bit words, format `R32_TYPELESS`) or structured (the buffer's stride, no format). `needs`
    /// is the bind flag the view's kind needs.
    fn buffer_span(
        &self,
        buffer: &Buffer<B>,
        needs: u32,
        format: u32,
        first: u32,
        count: u32,
        raw: bool,
    ) -> Result<(u64, u64)> {
        const R32_TYPELESS: u32 = 39;
        let d = &buffer.desc;
        if d.bind_flags & needs == 0 {
            return Err(Error::InvalidArg(
                "the resource lacks the bind flag the view needs",
            ));
        }
        let stride = if raw {
            if format != R32_TYPELESS || d.misc_flags & MISC_ALLOW_RAW_VIEWS == 0 {
                return Err(Error::InvalidArg("a raw view of a buffer not made for one"));
            }
            4
        } else if d.misc_flags & MISC_STRUCTURED != 0 {
            if format != format::UNKNOWN {
                return Err(Error::InvalidArg("a structured view with a format"));
            }
            d.structure_byte_stride
        } else {
            return not_implemented("typed buffer views (TypedBuffer)");
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
                .max(1),
        );
        if !offset.is_multiple_of(align) {
            return not_implemented(
                "a buffer view at an offset the back end does not bind at (StorageOffset)",
            );
        }
        Ok((offset, size))
    }

    pub fn create_sampler_state(&mut self, d: &SamplerDesc) -> Result<Rc<Sampler<B>>> {
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
        if d.mip_lod_bias != 0.0 {
            return not_implemented("a level-of-detail bias (LodBiasInShader)");
        }
        let mut address = [AddressMode::Repeat; 3];
        for (a, &mode) in address.iter_mut().zip(&d.address) {
            *a = match mode {
                1 => AddressMode::Repeat,
                2 => AddressMode::Mirror,
                3 => AddressMode::Clamp,
                4 if self.backend.caps().has(Features::BORDER_COLOR) => AddressMode::Border,
                5 if self.backend.caps().has(Features::MIRROR_ONCE) => AddressMode::MirrorOnce,
                4 | 5 => {
                    return not_implemented("border and mirror-once addressing in the program");
                }
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
        self.live()?;
        self.bury();
        let gpu = self.backend.create_sampler(&GpuSampler {
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
            lod_range: [
                d.min_lod.max(0.0).to_bits(),
                d.max_lod.clamp(0.0, 32.0).to_bits(),
            ],
            anisotropy: if anisotropic {
                d.max_anisotropy.max(1) as u16
            } else {
                1
            },
            compare,
            border,
        });
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(Sampler {
            desc: *d,
            gpu: Kept::new(gpu, id, Object::Sampler, &self.graveyard),
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
            if !(1..=5).contains(&t.op) || !(1..=5).contains(&t.op_alpha) {
                return Err(Error::InvalidArg("unknown blend operation"));
            }
        }
        let block = blend_block(d);
        if !block.same_for_all && !self.backend.caps().has(Features::INDEPENDENT_BLEND) {
            return not_implemented("targets blended differently (TargetsSplit)");
        }
        self.live()?;
        self.bury();
        let gpu = self.backend.create_blend(&block);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(Blend {
            desc: *d,
            gpu: Kept::new(gpu, id, Object::Blend, &self.graveyard),
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
        }))
    }

    pub fn create_rasterizer_state(&mut self, d: &RasterizerDesc) -> Result<Rc<Rasterizer<B>>> {
        if !(2..=3).contains(&d.fill) || !(1..=3).contains(&d.cull) {
            return Err(Error::InvalidArg("unknown fill or cull mode"));
        }
        let caps = self.backend.caps();
        if d.fill == 2 && !caps.has(Features::POLYGON_MODE) {
            return not_implemented("wireframe without the back end's polygon mode");
        }
        if d.depth_bias_clamp != 0.0 && !caps.has(Features::DEPTH_BIAS_CLAMP) {
            return not_implemented("a depth bias clamp without the back end's");
        }
        if !d.depth_clip && !caps.has(Features::DEPTH_CLAMP) {
            return not_implemented("depth clamping without the back end's depth clamp");
        }
        self.live()?;
        self.bury();
        let gpu = self.backend.create_raster(&raster_block(d));
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        Ok(Rc::new(Rasterizer {
            desc: *d,
            gpu: Kept::new(gpu, id, Object::Raster, &self.graveyard),
        }))
    }

    /// A vertex or pixel shader from its bytecode; `stage` is the one the API call makes.
    pub fn create_shader(
        &mut self,
        bytecode: &[u8],
        stage: wcontainers_gpu::SourceStage,
    ) -> Result<Rc<Shader<B>>> {
        let mut converted = shader::convert(bytecode, stage)?;
        let module = converted.modules.pop().expect("one module");
        self.live()?;
        self.bury();
        let gpu = self.backend.create_shader(&module);
        let gpu = self.gpu(gpu)?;
        if stage == wcontainers_gpu::SourceStage::Vertex {
            self.vertex_inputs.insert(
                *blake3::hash(bytecode).as_bytes(),
                converted.reflection.inputs.clone(),
            );
        }
        let constant_sizes = constant_sizes(&module.module);
        let stand_ins = stand_ins(&module.module);
        let id = self.id();
        let vertex = stage == wcontainers_gpu::SourceStage::Vertex;
        Ok(Rc::new(Shader {
            gpu: Kept::new(gpu, id, Object::Shader, &self.graveyard),
            reflection: converted.reflection,
            constant_sizes,
            stand_ins,
            bytecode: if vertex {
                bytecode.to_vec()
            } else {
                Vec::new()
            },
        }))
    }

    pub fn create_input_layout(
        &mut self,
        elements: &[InputElement],
        vs_bytecode: &[u8],
    ) -> Result<Rc<InputLayout>> {
        if elements.is_empty() || elements.len() > 32 {
            return Err(Error::InvalidArg("an input layout has 1 to 32 elements"));
        }
        let key = *blake3::hash(vs_bytecode).as_bytes();
        let inputs = match self.vertex_inputs.get(&key) {
            Some(inputs) => inputs.clone(),
            None => {
                let inputs = shader::convert(vs_bytecode, wcontainers_gpu::SourceStage::Vertex)?
                    .reflection
                    .inputs;
                self.vertex_inputs.insert(key, inputs.clone());
                inputs
            }
        };
        let mut next_offset = [0u32; 32];
        let mut instance_steps = [None; 32];
        let mut resolved = Vec::new();
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
            if step.is_some_and(|r| r != 1)
                && !self.backend.caps().has(Features::INSTANCE_STEP_RATE)
            {
                return not_implemented(
                    "instance step rates other than one without the back end's",
                );
            }
            if resolved.iter().any(|r: &LayoutElement| r.slot == e.slot)
                && instance_steps[slot] != step
            {
                return Err(Error::InvalidArg("elements of one slot step differently"));
            }
            instance_steps[slot] = step;
            let input = inputs.iter().find(|i| {
                i.system.is_none()
                    && i.semantic.eq_ignore_ascii_case(&e.semantic)
                    && i.semantic_index == e.semantic_index
            });
            if let Some(input) = input {
                resolved.push(LayoutElement {
                    location: input.location,
                    slot: e.slot,
                    offset,
                    format: vf,
                });
            }
        }
        for input in inputs.iter().filter(|i| i.system.is_none()) {
            if !resolved.iter().any(|r| r.location == input.location) {
                return Err(Error::InvalidArg(
                    "the layout lacks an input the shader reads",
                ));
            }
        }
        let id = self.id();
        Ok(Rc::new(InputLayout {
            id,
            elements: resolved,
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
        (bind::CONSTANT_BUFFER, BufferUsage::UNIFORM),
        (bind::SHADER_RESOURCE, BufferUsage::STORAGE),
        (bind::UNORDERED_ACCESS, BufferUsage::STORAGE),
    ] {
        if d.bind_flags & flag != 0 {
            usage |= u;
        }
    }
    if d.misc_flags & MISC_DRAWINDIRECT_ARGS != 0 {
        usage |= BufferUsage::INDIRECT;
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
            let kind = match class {
                ImageClass::Sampled { kind, multi: false } => match kind {
                    ScalarKind::Uint => Some(FormatKind::Uint),
                    ScalarKind::Sint => Some(FormatKind::Sint),
                    _ => Some(FormatKind::Float),
                },
                _ => None,
            };
            Some((
                binding,
                dimension.zip(kind).map(|(d, k)| StandIn::Zeros(d, k)),
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
        1 => CompareFn::Never,
        2 => CompareFn::Less,
        3 => CompareFn::Equal,
        4 => CompareFn::LessEqual,
        5 => CompareFn::Greater,
        6 => CompareFn::NotEqual,
        7 => CompareFn::GreaterEqual,
        8 => CompareFn::Always,
        _ => return Err(Error::InvalidArg("unknown comparison")),
    })
}

fn stencil_op(op: u32) -> Result<StencilOp> {
    Ok(match op {
        1 => StencilOp::Keep,
        2 => StencilOp::Zero,
        3 => StencilOp::Replace,
        4 => StencilOp::IncrementClamp,
        5 => StencilOp::DecrementClamp,
        6 => StencilOp::Invert,
        7 => StencilOp::IncrementWrap,
        8 => StencilOp::DecrementWrap,
        _ => return Err(Error::InvalidArg("unknown stencil operation")),
    })
}

fn check_blend_factor(f: u32, alpha: bool) -> Result<()> {
    let colour_only = matches!(f, 3 | 4 | 9 | 10 | 16 | 17);
    if !matches!(f, 1..=11 | 14..=19) || (alpha && colour_only) {
        return Err(Error::InvalidArg("a blend factor the slot cannot take"));
    }
    Ok(())
}

fn blend_factor(f: u32) -> BlendFactor {
    use BlendFactor::*;
    match f {
        1 => Zero,
        2 => One,
        3 => Src,
        4 => OneMinusSrc,
        5 => SrcAlpha,
        6 => OneMinusSrcAlpha,
        7 => DstAlpha,
        8 => OneMinusDstAlpha,
        9 => Dst,
        10 => OneMinusDst,
        11 => SrcAlphaSaturated,
        14 => Constant,
        15 => OneMinusConstant,
        16 => Src1,
        17 => OneMinusSrc1,
        18 => Src1Alpha,
        _ => OneMinusSrc1Alpha,
    }
}

fn blend_op(op: u32) -> BlendOp {
    match op {
        1 => BlendOp::Add,
        2 => BlendOp::Subtract,
        3 => BlendOp::ReverseSubtract,
        4 => BlendOp::Min,
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

pub(crate) fn raster_block(d: &RasterizerDesc) -> RasterDesc {
    RasterDesc {
        fill: if d.fill == 2 {
            FillMode::Line
        } else {
            FillMode::Solid
        },
        cull: match d.cull {
            2 => CullMode::Front,
            3 => CullMode::Back,
            _ => CullMode::None,
        },
        front: if d.front_counter_clockwise {
            FrontFace::CounterClockwise
        } else {
            FrontFace::Clockwise
        },
        depth_bias: (d.depth_bias as f32).to_bits(),
        depth_bias_slope: d.slope_scaled_depth_bias.to_bits(),
        depth_bias_clamp: d.depth_bias_clamp.to_bits(),
        depth_clip: d.depth_clip,
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
