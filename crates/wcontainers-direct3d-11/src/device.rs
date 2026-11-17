//! The device: object creation, each preceded by the checks Direct3D 11 makes at creation.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use wcontainers_gpu::{
    AddressMode, Backend, BlendComponent, BlendFactor, BlendOp, BorderColor,
    BufferDesc as GpuBuffer, BufferUsage, ColorWrites, CompareFn, CullMode,
    DepthStencilDesc as GpuDepthStencil, DeviceStatus, Extent, Features, FillMode, Filter, Format,
    FormatKind, FormatUse, FrontFace, LossReason, MAX_COLOR_TARGETS, MapIntent, Object, Origin,
    ProvokingVertex, RasterDesc, RenderTargetBlend, SamplerDesc as GpuSampler, SlowPath,
    StageValue, StencilFace, StencilOp, TextureDesc, TextureDimension, TextureRegion, TextureUsage,
    TextureViewDesc, ViewDimension, WriteMode,
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
    /// Programs by (vertex shader, pixel shader) id.
    pub(crate) programs: HashMap<(u64, u64), Kept<B, B::Program>>,
    /// Vertex layout blocks by (input layout id, stride of each slot it reads).
    pub(crate) layouts: HashMap<(u64, [u32; 32]), Kept<B, B::VertexLayout>>,
    /// Vertex shaders' input signatures by the blake3 of their bytecode, for input layouts.
    vertex_inputs: HashMap<[u8; 32], Vec<StageValue>>,
    /// Set once the device is removed; later work is dropped and the API reports the removal.
    pub removed: Option<Removal>,
}

/// Why the device was removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// The program asked for something the front cannot serve and that has no result to refuse it by.
    Unserved,
    /// The back end lost its device.
    Lost(LossReason),
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
        let blend = backend.create_blend(&blend_block(&blend_desc))?;
        let ds = backend.create_depth_stencil(&depth_stencil_block(&ds_desc))?;
        let raster = backend.create_raster(&raster_block(&raster_desc))?;
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
            programs: HashMap::new(),
            layouts: HashMap::new(),
            vertex_inputs: HashMap::new(),
            removed: None,
        })
    }

    pub fn feature_level(&self) -> u32 {
        self.level
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
        if self.poll_removed().is_none() {
            eprintln!("wcontainers direct3d 11: device removed: {e:?}");
            self.removed = Some(Removal::Unserved);
        }
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
        } else {
            Err(Error::InvalidArg(
                "the format does not support the bind flags",
            ))
        }
    }

    pub fn create_buffer(&mut self, d: &BufferDesc, init: Option<&[u8]>) -> Result<Rc<Buffer<B>>> {
        check_buffer(d, init.is_some())?;
        if d.misc_flags != 0 {
            return not_implemented("buffers with misc flags (structured, raw, indirect, shared)");
        }
        self.live()?;
        self.bury();
        // The buffer is made whole granules of the back end's copies long, so writing the initial data
        // (padded with zeros past the program's bytes) never needs an edge merge.
        let granule = u64::from(self.backend.caps().limits.buffer_copy_alignment.max(1));
        let size = u64::from(d.byte_width).div_ceil(granule) * granule;
        let gpu = self.backend.create_buffer(&gpu_buffer(d, size))?;
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
        }))
    }

    /// The checks of `CreateTexture2D` before its initial data is read: the format and the number of
    /// mip levels, or why the description is refused.
    pub fn check_texture_2d(&self, d: &Texture2DDesc, has_init: bool) -> Result<(Format, u32)> {
        let limits = &self.backend.caps().limits;
        if d.width == 0 || d.height == 0 || d.array_size == 0 {
            return Err(Error::InvalidArg("a texture needs a size"));
        }
        if d.width > limits.max_texture_2d
            || d.height > limits.max_texture_2d
            || d.array_size > limits.max_texture_layers
        {
            return Err(Error::InvalidArg(
                "the texture is larger than the feature level allows",
            ));
        }
        let full = 32 - d.width.max(d.height).leading_zeros();
        let mips = if d.mip_levels == 0 {
            full
        } else {
            d.mip_levels
        };
        if mips > full {
            return Err(Error::InvalidArg("more mip levels than the size has"));
        }
        let format = format::texture_format_of(d.format)?;
        if d.sample_count != 1 || d.sample_quality != 0 {
            return not_implemented("multisampled textures");
        }
        let cube = d.misc_flags & MISC_TEXTURECUBE != 0;
        if d.misc_flags & !MISC_TEXTURECUBE != 0 {
            return not_implemented("textures with misc flags other than a cube");
        }
        if cube && (!d.array_size.is_multiple_of(6) || d.width != d.height) {
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
            usage::DYNAMIC => return not_implemented("dynamic textures"),
            usage::DEFAULT | usage::IMMUTABLE | usage::STAGING => {}
            _ => return Err(Error::InvalidArg("unknown usage")),
        }
        if d.usage != usage::STAGING && d.cpu_access_flags != 0 {
            return Err(Error::InvalidArg(
                "only staging textures here have CPU access",
            ));
        }
        let mut needs = FormatUse::empty();
        for (flag, n) in [
            (bind::SHADER_RESOURCE, FormatUse::SAMPLE),
            (bind::RENDER_TARGET, FormatUse::RENDER),
            (bind::DEPTH_STENCIL, FormatUse::DEPTH_STENCIL),
        ] {
            if d.bind_flags & flag != 0 {
                needs |= n;
            }
        }
        if d.bind_flags & bind::UNORDERED_ACCESS != 0 {
            return not_implemented("textures with unordered access");
        }
        if d.bind_flags & !(bind::SHADER_RESOURCE | bind::RENDER_TARGET | bind::DEPTH_STENCIL) != 0
        {
            return Err(Error::InvalidArg("bind flags a texture cannot have"));
        }
        self.check_format(format, needs)?;
        Ok((format, mips))
    }

    pub fn create_texture_2d(
        &mut self,
        d: &Texture2DDesc,
        init: &[SubresourceData<'_>],
    ) -> Result<Rc<Texture2D<B>>> {
        let (format, mips) = self.check_texture_2d(d, !init.is_empty())?;
        let cube = d.misc_flags & MISC_TEXTURECUBE != 0;
        let mut usage = TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
        for (flag, u) in [
            (bind::SHADER_RESOURCE, TextureUsage::SAMPLE),
            (bind::RENDER_TARGET, TextureUsage::RENDER),
            (bind::DEPTH_STENCIL, TextureUsage::RENDER),
        ] {
            if d.bind_flags & flag != 0 {
                usage |= u;
            }
        }
        let subresources = (mips * d.array_size) as usize;
        if !init.is_empty() && init.len() < subresources {
            return Err(Error::InvalidArg("initial data for every subresource"));
        }
        self.live()?;
        self.bury();
        let gpu_desc = TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: d.width,
            height: d.height,
            depth_or_layers: d.array_size,
            mip_levels: mips,
            samples: 1,
            usage,
            cube,
            reinterpret: false,
        };
        let gpu = self.backend.create_texture(&gpu_desc)?;
        let resolved = Texture2DDesc {
            mip_levels: mips,
            ..*d
        };
        for (i, data) in init.iter().take(subresources).enumerate() {
            let (mip, layer) = (i as u32 % mips, i as u32 / mips);
            let (w, h) = ((d.width >> mip).max(1), (d.height >> mip).max(1));
            let (_, rows) = format::row_bytes(format, w, h);
            self.backend.write_texture(
                &TextureRegion {
                    texture: &gpu,
                    mip,
                    layer,
                    origin: Origin { x: 0, y: 0, z: 0 },
                    stencil: false,
                },
                Extent {
                    width: w,
                    height: h,
                    depth_or_layers: 1,
                },
                data.bytes,
                data.row_pitch,
                rows,
                WriteMode::Ordered,
            );
        }
        let shadow = if d.usage == usage::STAGING {
            (0..subresources)
                .map(|i| {
                    let mip = i as u32 % mips;
                    let (w, h) = ((d.width >> mip).max(1), (d.height >> mip).max(1));
                    let (row, rows) = format::row_bytes(format, w, h);
                    vec![0; (row * rows) as usize]
                })
                .collect()
        } else {
            Vec::new()
        };
        let id = self.id();
        Ok(Rc::new(Texture2D {
            desc: resolved,
            format,
            gpu_desc,
            gpu: Kept::new(gpu, id, Object::Texture, &self.graveyard),
            shadow: RefCell::new(shadow),
        }))
    }

    /// A view of `texture`; `kind` is the bind flag the view needs (render target, depth stencil or
    /// shader resource), and `None` views the whole texture as Direct3D's defaults say.
    pub fn create_view(
        &mut self,
        texture: &Rc<Texture2D<B>>,
        desc: Option<&ViewDesc>,
        kind: u32,
    ) -> Result<Rc<View<B>>> {
        let t = &texture.desc;
        if t.bind_flags & kind == 0 {
            return Err(Error::InvalidArg(
                "the resource lacks the bind flag the view needs",
            ));
        }
        let whole = ViewDesc {
            format: t.format,
            dimension: if t.array_size > 1 {
                ViewDim::Texture2DArray
            } else {
                ViewDim::Texture2D
            },
            first_mip: 0,
            mip_count: if kind == bind::SHADER_RESOURCE {
                t.mip_levels
            } else {
                1
            },
            first_layer: 0,
            layer_count: t.array_size,
        };
        let mut v = desc.copied().unwrap_or(whole);
        if v.format == format::UNKNOWN {
            v.format = t.format;
        }
        if v.mip_count == u32::MAX {
            v.mip_count = t.mip_levels - v.first_mip.min(t.mip_levels);
        }
        if v.dimension == ViewDim::Texture2D {
            v.first_layer = 0;
            v.layer_count = 1;
        }
        if v.layer_count == u32::MAX {
            v.layer_count = t.array_size - v.first_layer.min(t.array_size);
        }
        if v.first_mip + v.mip_count > t.mip_levels
            || v.mip_count == 0
            || v.first_layer + v.layer_count > t.array_size
            || v.layer_count == 0
            || (kind != bind::SHADER_RESOURCE && v.mip_count != 1)
        {
            return Err(Error::InvalidArg("the view reaches past the resource"));
        }
        let format = format::texture_format_of(v.format)?;
        if format != texture.format {
            return not_implemented("views in another format of the typeless group");
        }
        let dimension = match v.dimension {
            ViewDim::Texture2D => ViewDimension::D2,
            ViewDim::Texture2DArray => ViewDimension::D2Array,
            ViewDim::TextureCube => {
                if !texture.gpu_desc.cube || v.layer_count != 6 {
                    return Err(Error::InvalidArg(
                        "a cube view needs six faces of a cube texture",
                    ));
                }
                ViewDimension::Cube
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
                base_layer: v.first_layer,
                layer_count: v.layer_count,
                stencil: false,
            },
        )?;
        let id = self.id();
        Ok(Rc::new(View {
            texture: texture.clone(),
            desc: v,
            format,
            gpu: Kept::new(gpu, id, Object::TextureView, &self.graveyard),
        }))
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
        if anisotropic && !(1..=16).contains(&d.max_anisotropy) {
            return Err(Error::InvalidArg("anisotropy must be 1 to 16"));
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
                d.max_anisotropy as u16
            } else {
                1
            },
            compare,
            border,
        })?;
        let id = self.id();
        Ok(Rc::new(Sampler {
            desc: *d,
            gpu: Kept::new(gpu, id, Object::Sampler, &self.graveyard),
        }))
    }

    pub fn create_blend_state(&mut self, d: &BlendDesc) -> Result<Rc<Blend<B>>> {
        // Targets past the first count only when they blend independently.
        let used = if d.independent { d.targets.len() } else { 1 };
        for t in &d.targets[..used] {
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
        let gpu = self.backend.create_blend(&block)?;
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
        let gpu = self.backend.create_depth_stencil(&block)?;
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
        if d.depth_bias != 0 || d.slope_scaled_depth_bias != 0.0 || d.depth_bias_clamp != 0.0 {
            return not_implemented("depth bias");
        }
        if !d.depth_clip && !caps.has(Features::DEPTH_CLAMP) {
            return not_implemented("depth clamping without the back end's depth clamp");
        }
        self.live()?;
        self.bury();
        let gpu = self.backend.create_raster(&raster_block(d))?;
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
        let gpu = self.backend.create_shader(&module)?;
        if stage == wcontainers_gpu::SourceStage::Vertex {
            self.vertex_inputs.insert(
                *blake3::hash(bytecode).as_bytes(),
                converted.reflection.inputs.clone(),
            );
        }
        let constant_sizes = constant_sizes(&module.module);
        let id = self.id();
        Ok(Rc::new(Shader {
            gpu: Kept::new(gpu, id, Object::Shader, &self.graveyard),
            reflection: converted.reflection,
            constant_sizes,
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
            let offset = if e.offset == APPEND_ALIGNED {
                next_offset[slot]
            } else {
                e.offset
            };
            next_offset[slot] = offset + format::vertex_size(vf);
            let step = e.per_instance.then_some(e.step_rate);
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
    if d.bind_flags & (bind::RENDER_TARGET | bind::DEPTH_STENCIL) != 0 {
        return Err(Error::InvalidArg("buffers cannot be targets"));
    }
    if d.bind_flags & bind::STREAM_OUTPUT != 0 {
        return not_implemented("stream output buffers");
    }
    if d.bind_flags & (bind::SHADER_RESOURCE | bind::UNORDERED_ACCESS) != 0 {
        return not_implemented("buffer views");
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
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
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
