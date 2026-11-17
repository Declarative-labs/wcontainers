//! The recording back end: a CPU device that keeps every object's bytes, runs every call when it is
//! recorded, checks the interface's rules on the way, and keeps a log of the calls. Every back end's
//! answers in the shared suite are checked against what the APIs promise; this one gives them without
//! a GPU, so the suite and the shared helpers can be tested anywhere.

use crate::program::Value;
use crate::stream::{self, BufferSide, Call, Draw, Region, Shader, bind_of, kind_of};
use crate::texel;
use std::rc::Rc;
use wcontainers_gpu::*;

/// A handle of the recording back end: an index into its objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(pub(crate) u32);

/// A frame shown by [`Backend::present`].
#[derive(Clone, Debug)]
pub struct Frame {
    pub window: u32,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    /// Rows of texels in `format`, top row first, tightly packed.
    pub bytes: Vec<u8>,
}

pub(crate) struct BufferObj {
    pub bytes: Vec<u8>,
    pub usage: BufferUsage,
}

pub(crate) struct TextureObj {
    pub desc: TextureDesc,
    /// One per mip level and layer (index mip * layers + layer); a 3D level holds all its slices.
    pub subresources: Vec<Vec<u8>>,
}

pub(crate) struct ViewObj {
    pub texture: Id,
    pub desc: TextureViewDesc,
}

/// A resource bound in a binding set.
#[derive(Clone, Debug)]
pub(crate) enum Bound {
    Buffer {
        buffer: Id,
        offset: u64,
        size: u64,
        writable: bool,
    },
    Texture(Id),
    Sampler(Id),
}

pub(crate) struct ProgramObj {
    pub vertex: Rc<ShaderModule>,
    pub fragment: Option<Rc<ShaderModule>>,
}

pub(crate) enum Obj {
    Buffer(BufferObj),
    Texture(TextureObj),
    View(ViewObj),
    Sampler(SamplerDesc),
    Shader(Rc<ShaderModule>),
    Program(ProgramObj),
    Blend(Box<BlendDesc>),
    DepthStencil(DepthStencilDesc),
    Raster(RasterDesc),
    VertexLayout(Box<VertexLayoutDesc>),
    BindingSet(Vec<(u32, Bound)>),
    QuerySet { kind: QueryKind, results: Vec<u64> },
    Readback { bytes: Vec<u8>, ready_at: u64 },
    Fence,
    Surface(SurfaceObj),
}

pub(crate) struct SurfaceObj {
    pub window: u32,
    pub config: Option<(Format, u32, u32)>,
    pub current: Option<(Id, Id)>,
}

/// A depth-stencil attachment: its view and what the pass does with each aspect.
pub(crate) type DepthOps = (Id, Option<AspectOps<u32>>, Option<AspectOps<u8>>);

/// The open pass: its attachments, as (view, 3D slice).
pub(crate) struct Pass {
    pub colors: [Option<(Id, u32, StoreOp)>; MAX_COLOR_TARGETS],
    pub depth: Option<DepthOps>,
    pub width: u32,
    pub height: u32,
    pub occlusion: Option<Id>,
    /// The running occlusion query and whether any sample passed.
    pub query: Option<(u32, u64)>,
    /// Buffers the pass's draws bound as writable storage, and those they used any other way.
    pub written: Vec<Id>,
    pub other: Vec<Id>,
}

/// The recording back end.
pub struct Recorder {
    caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<Recorder>,
    lost: Option<LossReason>,
    pub(crate) objects: Vec<Option<Obj>>,
    pub(crate) pass: Option<Pass>,
    submits: u64,
    /// Every base call, in order, with its answer.
    pub calls: Vec<Call>,
    /// How deep the calls being made are inside another base call, whose own entry is the one kept.
    nested: u32,
    /// Buffer ranges commands recorded since the last submit read: what no `NoOverwrite` write may
    /// touch.
    pub(crate) reads: Vec<(Id, u64, u64)>,
    /// Every rule of the interface a caller broke, and every call this back end could not carry out.
    /// Either loses the device.
    pub violations: Vec<String>,
    /// Every frame presented.
    pub shown: Vec<Frame>,
}

/// Bytes the recording back end fills discarded and uninitialised-by-`DontCare` attachments with, so a
/// caller relying on them sees nonsense.
const GARBAGE: u8 = 0xcd;

impl core::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Recorder ({} calls)", self.calls.len())
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    pub fn new() -> Self {
        let mut uses = [FormatUse::empty(); Format::COUNT];
        let mut samples = [0u8; Format::COUNT];
        // Positions without a format the back end holds keep no uses and no sample counts.
        for ((u, n), f) in uses.iter_mut().zip(&mut samples).zip(every_format()) {
            if !texel::held(f) {
                continue;
            }
            *n = 1;
            *u = FormatUse::SAMPLE | FormatUse::COPY_OUT;
            if f.is_depth_stencil() {
                *u |= FormatUse::DEPTH_STENCIL;
            } else {
                *u |= FormatUse::RENDER;
                if !texel::integer(f) {
                    *u |= FormatUse::FILTER | FormatUse::BLEND;
                }
            }
            if matches!(f, Format::Rgba8Unorm | Format::Bgra8Unorm) {
                *u |= FormatUse::PRESENT;
            }
        }
        let limits = Limits {
            max_texture_2d: 16384,
            max_texture_cube: 16384,
            max_texture_3d: 2048,
            max_texture_layers: 2048,
            max_buffer_size: 1 << 30,
            max_uniform_buffer_size: 1 << 16,
            max_storage_buffer_size: 1 << 27,
            uniform_buffers_per_stage: 14,
            textures_per_stage: 128,
            samplers_per_stage: 16,
            storage_buffers_per_stage: 8,
            storage_textures_per_stage: 8,
            vertex_streams: 32,
            vertex_attributes: 32,
            max_vertex_stride: 2048,
            color_targets: 8,
            color_bytes_per_sample: 128,
            inter_stage_slots: 32,
            min_uniform_offset_alignment: 256,
            min_storage_offset_alignment: 256,
            buffer_copy_alignment: 1,
            immediate_bytes: 0,
            compute_workgroup_storage: 0,
            compute_invocations: 0,
            compute_size: [0; 3],
            max_dispatch: 0,
            max_point_size: 1.0,
            max_line_width: 1.0,
            max_anisotropy: 1,
            capture_buffers: 0,
        };
        Recorder {
            caps: Caps {
                adapter: AdapterInfo {
                    name: "wcontainers recording back end".into(),
                    vendor: 0,
                    device: 0,
                    software: true,
                },
                features: Features::INDEPENDENT_BLEND
                    | Features::PROVOKING_VERTEX_FIRST
                    | Features::PROVOKING_VERTEX_LAST
                    | Features::SAMPLE_SHADING
                    | Features::TEXTURE_VIEWS
                    | Features::FRACTIONAL_VIEWPORT
                    | Features::FRACTIONAL_DEPTH_BIAS
                    | Features::DEPTH_BIAS_CLAMP
                    | Features::BLEND_CONSTANT_ALPHA
                    | Features::INDEX_U8
                    | Features::DEPTH24_UNORM
                    | Features::BASE_VERTEX_INSTANCE
                    | Features::CLIP_DEPTH_ZERO_ONE
                    | Features::FRAGMENT_STORAGE_WRITE,
                limits,
                formats: FormatCaps { uses, samples },
            },
            slow: SlowPaths::default(),
            helpers: Helpers::default(),
            lost: None,
            objects: Vec::new(),
            pass: None,
            submits: 0,
            calls: Vec::new(),
            nested: 0,
            reads: Vec::new(),
            violations: Vec::new(),
            shown: Vec::new(),
        }
    }

    /// The bytes every live buffer and texture holds, by object, and the frames shown: what two
    /// recordings of one stream must agree on.
    pub fn contents(&self) -> (Vec<Option<Vec<u8>>>, Vec<Vec<u8>>) {
        let objects = self
            .objects
            .iter()
            .map(|o| match o {
                Some(Obj::Buffer(b)) => Some(b.bytes.clone()),
                Some(Obj::Texture(t)) => Some(t.subresources.concat()),
                _ => None,
            })
            .collect();
        (
            objects,
            self.shown.iter().map(|f| f.bytes.clone()).collect(),
        )
    }

    fn add(&mut self, obj: Obj) -> Id {
        self.objects.push(Some(obj));
        Id(self.objects.len() as u32 - 1)
    }

    /// Records a broken rule or a call this back end cannot carry out, and loses the device.
    pub(crate) fn violate(&mut self, what: impl Into<String>) {
        let what = what.into();
        self.violations.push(what);
        self.lost = Some(LossReason::Error);
    }

    fn log(&mut self, call: Call) {
        if self.nested == 0 {
            self.calls.push(call);
        }
    }

    pub(crate) fn obj(&self, id: Id) -> Result<&Obj, String> {
        self.objects
            .get(id.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| format!("object {} used after it was destroyed", id.0))
    }

    pub(crate) fn buffer(&self, id: Id) -> Result<&BufferObj, String> {
        match self.obj(id)? {
            Obj::Buffer(b) => Ok(b),
            _ => Err(format!("object {} is not a buffer", id.0)),
        }
    }

    fn buffer_mut(&mut self, id: Id) -> Result<&mut BufferObj, String> {
        match self.objects.get_mut(id.0 as usize).and_then(Option::as_mut) {
            Some(Obj::Buffer(b)) => Ok(b),
            _ => Err(format!("object {} is not a live buffer", id.0)),
        }
    }

    /// Checks a buffer was made with `usage` for what `what` does with it.
    pub(crate) fn buffer_for(&self, id: Id, usage: BufferUsage, what: &str) -> Result<(), String> {
        if self.buffer(id)?.usage.contains(usage) {
            Ok(())
        } else {
            Err(format!("{what} of buffer {} made without {usage:?}", id.0))
        }
    }

    /// Checks a texture was made with `usage` for what `what` does with it.
    pub(crate) fn texture_for(
        &self,
        id: Id,
        usage: TextureUsage,
        what: &str,
    ) -> Result<(), String> {
        if self.texture(id)?.desc.usage.contains(usage) {
            Ok(())
        } else {
            Err(format!("{what} of texture {} made without {usage:?}", id.0))
        }
    }

    pub(crate) fn texture(&self, id: Id) -> Result<&TextureObj, String> {
        match self.obj(id)? {
            Obj::Texture(t) => Ok(t),
            _ => Err(format!("object {} is not a texture", id.0)),
        }
    }

    pub(crate) fn texture_mut(&mut self, id: Id) -> Result<&mut TextureObj, String> {
        match self.objects.get_mut(id.0 as usize).and_then(Option::as_mut) {
            Some(Obj::Texture(t)) => Ok(t),
            _ => Err(format!("object {} is not a live texture", id.0)),
        }
    }

    pub(crate) fn view(&self, id: Id) -> Result<&ViewObj, String> {
        match self.obj(id)? {
            Obj::View(v) => Ok(v),
            _ => Err(format!("object {} is not a view", id.0)),
        }
    }

    /// Runs `f`, turning its error into a violation.
    fn check(&mut self, f: impl FnOnce(&mut Self) -> Result<(), String>) {
        if let Err(e) = f(self) {
            self.violate(e);
        }
    }

    fn outside_pass(&self, what: &str) -> Result<(), String> {
        match self.pass {
            Some(_) => Err(format!("{what} inside a pass")),
            None => Ok(()),
        }
    }

    /// The bytes of a texture region, rows of blocks tightly packed, layer after layer.
    pub(crate) fn read_region(
        &self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> Result<Vec<u8>, String> {
        let t = self.texture(*region.texture)?;
        let mut out = Vec::new();
        for_rows(&t.desc, region, extent, |sub, at, len| {
            out.extend_from_slice(&t.subresources[sub][at..at + len]);
        })?;
        Ok(out)
    }

    /// Writes `data` (rows `row_pitch` apart, layers `rows_per_layer` rows apart) into a region.
    pub(crate) fn write_region(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
    ) -> Result<(), String> {
        let t = self.texture_mut(*region.texture)?;
        let (bw, bh, _) = t.desc.format.block();
        let rows = extent.height.div_ceil(bh);
        let rows_per_layer = if rows_per_layer == 0 {
            rows
        } else {
            rows_per_layer
        };
        if rows_per_layer < rows || row_pitch < extent.width.div_ceil(bw) * t.desc.format.block().2
        {
            return Err("write or copy pitch smaller than the region".into());
        }
        let mut n = 0usize;
        let desc = t.desc;
        let subresources = &mut t.subresources;
        let mut failed = None;
        for_rows(&desc, region, extent, |sub, at, len| {
            let (layer, row) = (n / rows as usize, n % rows as usize);
            let from =
                layer * rows_per_layer as usize * row_pitch as usize + row * row_pitch as usize;
            match data.get(from..from + len) {
                Some(src) => subresources[sub][at..at + len].copy_from_slice(src),
                None => failed = Some("data shorter than the region"),
            }
            n += 1;
        })?;
        failed.map_or(Ok(()), |e| Err(e.into()))
    }

    /// The (texture, subresource index, slice) a view attaches, for a pass.
    pub(crate) fn attachment(&self, view: Id, slice: u32) -> Result<(Id, usize, u32), String> {
        let v = self.view(view)?;
        let t = self.texture(v.texture)?;
        if v.desc.mip_count != 1
            || (t.desc.dimension == TextureDimension::D2 && v.desc.layer_count != 1)
        {
            return Err("an attachment view must be one level and one layer".into());
        }
        if !t.desc.usage.contains(TextureUsage::RENDER) {
            return Err("an attachment without render usage".into());
        }
        Ok((
            v.texture,
            subresource(&t.desc, v.desc.base_mip, v.desc.base_layer),
            slice,
        ))
    }

    /// The size of a view's first level.
    pub(crate) fn view_size(&self, view: Id) -> Result<(u32, u32), String> {
        let v = self.view(view)?;
        let t = self.texture(v.texture)?;
        let (w, h, _) = level_size(&t.desc, v.desc.base_mip);
        Ok((w, h))
    }

    /// Fills one attachment's level and layer (or slice) with `f` applied to each texel's bytes.
    fn fill_attachment(
        &mut self,
        view: Id,
        slice: u32,
        f: impl Fn(&mut [u8]),
    ) -> Result<(), String> {
        let (texture, sub, slice) = self.attachment(view, slice)?;
        let format = self.view(view)?.desc.format;
        let t = self.texture_mut(texture)?;
        let mip = (sub as u32) / layers(&t.desc);
        let (w, h, _) = level_size(&t.desc, mip);
        let size = format.block().2 as usize;
        let plane = (w * h) as usize * size;
        let bytes = &mut t.subresources[sub][slice as usize * plane..(slice as usize + 1) * plane];
        for texel in bytes.chunks_exact_mut(size) {
            f(texel);
        }
        Ok(())
    }

    fn start_pass(&mut self, pass: &PassDesc<'_, Self>) -> Result<(), String> {
        self.outside_pass("begin_pass")?;
        let mut size = None;
        let mut same = |w: u32, h: u32| -> Result<(), String> {
            match size {
                Some(s) if s != (w, h) => Err("attachments of different sizes".to_string()),
                _ => {
                    size = Some((w, h));
                    Ok(())
                }
            }
        };
        let mut colors = [None; MAX_COLOR_TARGETS];
        for (i, c) in pass.colors.iter().enumerate() {
            let Some(c) = c else { continue };
            if c.resolve.is_some() {
                return Err("a resolve target, but no texture here is multisampled".into());
            }
            let (w, h) = self.view_size(*c.view)?;
            same(w, h)?;
            let format = self.view(*c.view)?.desc.format;
            if format.is_depth_stencil() {
                return Err("a depth format as a colour attachment".into());
            }
            self.attachment(*c.view, c.depth_slice)?;
            self.texture_for(
                self.view(*c.view)?.texture,
                TextureUsage::RENDER,
                "an attachment",
            )?;
            colors[i] = Some((*c.view, c.depth_slice, c.store));
        }
        let mut depth = None;
        if let Some(d) = &pass.depth {
            let (w, h) = self.view_size(*d.view)?;
            same(w, h)?;
            let format = self.view(*d.view)?.desc.format;
            let has_depth = !matches!(format.kind(), FormatKind::Stencil);
            let has_stencil = texel::stencil_byte(format).is_some();
            if !format.is_depth_stencil() {
                return Err("a colour format as the depth attachment".into());
            }
            if d.depth.is_some() != has_depth || d.stencil.is_some() != has_stencil {
                return Err("depth attachment aspects differ from the format's".into());
            }
            self.attachment(*d.view, 0)?;
            self.texture_for(
                self.view(*d.view)?.texture,
                TextureUsage::RENDER,
                "an attachment",
            )?;
            depth = Some((*d.view, d.depth, d.stencil));
        }
        let (width, height) = match (size, pass.empty) {
            (Some(s), None) => s,
            (None, Some(e)) => (e.width, e.height),
            (Some(_), Some(_)) => return Err("a pass with attachments and `empty`".into()),
            (None, None) => return Err("a pass with no attachment and no `empty`".into()),
        };
        if let Some(q) = pass.occlusion {
            match self.obj(*q)? {
                Obj::QuerySet {
                    kind: QueryKind::OcclusionAny,
                    ..
                } => {}
                _ => return Err("the pass's occlusion set is not an occlusion set".into()),
            }
        }
        // Loads.
        for c in pass.colors.iter().flatten() {
            let format = self.view(*c.view)?.desc.format;
            match c.load {
                LoadOp::Load => {}
                LoadOp::Clear(v) => {
                    let mut texel = vec![0u8; format.block().2 as usize];
                    texel::encode(format, clear_values(format, v), &mut texel);
                    self.fill_attachment(*c.view, c.depth_slice, |t| t.copy_from_slice(&texel))?;
                }
                LoadOp::DontCare => {
                    self.fill_attachment(*c.view, c.depth_slice, |t| t.fill(GARBAGE))?
                }
            }
        }
        if let Some(d) = &pass.depth {
            let format = self.view(*d.view)?.desc.format;
            let stencil_at = texel::stencil_byte(format);
            if let Some(a) = d.depth {
                match a.load {
                    LoadOp::Load => {}
                    LoadOp::Clear(bits) => self.fill_attachment(*d.view, 0, |t| {
                        texel::encode_depth(format, f64::from(f32::from_bits(bits)), t)
                    })?,
                    LoadOp::DontCare => {
                        self.fill_attachment(*d.view, 0, |t| texel::encode_depth(format, 0.123, t))?
                    }
                }
            }
            if let (Some(a), Some(at)) = (d.stencil, stencil_at) {
                match a.load {
                    LoadOp::Load => {}
                    LoadOp::Clear(v) => self.fill_attachment(*d.view, 0, |t| t[at] = v)?,
                    LoadOp::DontCare => self.fill_attachment(*d.view, 0, |t| t[at] = GARBAGE)?,
                }
            }
        }
        self.pass = Some(Pass {
            colors,
            depth,
            width,
            height,
            occlusion: pass.occlusion.copied(),
            query: None,
            written: Vec::new(),
            other: Vec::new(),
        });
        Ok(())
    }

    fn finish_pass(&mut self) -> Result<(), String> {
        let pass = self.pass.take().ok_or("end_pass without a pass")?;
        if pass.query.is_some() {
            return Err("a pass ended with its query running".into());
        }
        for (view, slice, store) in pass.colors.into_iter().flatten() {
            if store == StoreOp::Discard {
                self.fill_attachment(view, slice, |t| t.fill(GARBAGE))?;
            }
        }
        if let Some((view, d, s)) = pass.depth {
            let format = self.view(view)?.desc.format;
            if d.is_some_and(|a| a.store == StoreOp::Discard) {
                self.fill_attachment(view, 0, |t| texel::encode_depth(format, 0.123, t))?;
            }
            if let (Some(a), Some(at)) = (s, texel::stencil_byte(format))
                && a.store == StoreOp::Discard
            {
                self.fill_attachment(view, 0, |t| t[at] = GARBAGE)?;
            }
        }
        Ok(())
    }

    fn snapshot(&mut self, src: &ReadSource<'_, Self>) -> Result<Vec<u8>, String> {
        self.outside_pass("read_back")?;
        match src {
            ReadSource::Buffer {
                buffer,
                offset,
                size,
            } => {
                self.buffer_for(**buffer, BufferUsage::COPY_SRC, "a read back")?;
                self.reads.push((**buffer, *offset, *offset + *size));
                let b = self.buffer(**buffer)?;
                let range = *offset as usize..(*offset + *size) as usize;
                b.bytes
                    .get(range)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "read back past the buffer's end".into())
            }
            ReadSource::Texture { region, extent } => {
                self.texture_for(*region.texture, TextureUsage::COPY_SRC, "a read back")?;
                self.read_region(region, *extent)
            }
        }
    }

    fn readback(&mut self, bytes: Vec<u8>) -> Id {
        let ready_at = self.submits + 1;
        self.add(Obj::Readback { bytes, ready_at })
    }
}

/// Every format, in the order of [`Caps::formats`].
fn every_format() -> Vec<Format> {
    use Format::*;
    let all = vec![
        R8Unorm,
        R8Snorm,
        R8Uint,
        R8Sint,
        R16Unorm,
        R16Snorm,
        R16Uint,
        R16Sint,
        R16Float,
        Rg8Unorm,
        Rg8Snorm,
        Rg8Uint,
        Rg8Sint,
        R32Uint,
        R32Sint,
        R32Float,
        Rg16Unorm,
        Rg16Snorm,
        Rg16Uint,
        Rg16Sint,
        Rg16Float,
        Rgba8Unorm,
        Rgba8UnormSrgb,
        Rgba8Snorm,
        Rgba8Uint,
        Rgba8Sint,
        Bgra8Unorm,
        Bgra8UnormSrgb,
        Bgrx8Unorm,
        Bgrx8UnormSrgb,
        Rgb10a2Unorm,
        Rgb10a2Uint,
        Rg11b10Float,
        Rgb9e5Float,
        B5g6r5Unorm,
        Bgr5a1Unorm,
        Bgra4Unorm,
        Rgbg8Unorm,
        Grgb8Unorm,
        Rg32Uint,
        Rg32Sint,
        Rg32Float,
        Rgba16Unorm,
        Rgba16Snorm,
        Rgba16Uint,
        Rgba16Sint,
        Rgba16Float,
        Rgb32Uint,
        Rgb32Sint,
        Rgb32Float,
        Rgba32Uint,
        Rgba32Sint,
        Rgba32Float,
        A8Unorm,
        Stencil8,
        Depth16Unorm,
        Depth24Plus,
        Depth24PlusStencil8,
        Depth32Float,
        Depth32FloatStencil8,
        Bc1RgbaUnorm,
        Bc1RgbaUnormSrgb,
        Bc2RgbaUnorm,
        Bc2RgbaUnormSrgb,
        Bc3RgbaUnorm,
        Bc3RgbaUnormSrgb,
        Bc4RUnorm,
        Bc4RSnorm,
        Bc5RgUnorm,
        Bc5RgSnorm,
        Bc6hRgbUfloat,
        Bc6hRgbFloat,
        Bc7RgbaUnorm,
        Bc7RgbaUnormSrgb,
        Etc2Rgb8Unorm,
        Etc2Rgb8UnormSrgb,
        Etc2Rgba8Unorm,
        Etc2Rgba8UnormSrgb,
        Astc4x4Unorm,
        Astc4x4UnormSrgb,
    ];
    assert_eq!(all.len(), Format::COUNT);
    assert!(all.iter().enumerate().all(|(i, f)| f.index() == i));
    all
}

/// A clear value's bits as the values of `format`.
pub(crate) fn clear_values(format: Format, v: [u32; 4]) -> [f64; 4] {
    match format.kind() {
        FormatKind::Uint => v.map(f64::from),
        FormatKind::Sint => v.map(|x| f64::from(x as i32)),
        _ => v.map(|x| f64::from(f32::from_bits(x))),
    }
}

/// Layers per level: array layers, or one for a 3D texture.
pub(crate) fn layers(desc: &TextureDesc) -> u32 {
    match desc.dimension {
        TextureDimension::D2 => desc.depth_or_layers,
        TextureDimension::D3 => 1,
    }
}

pub(crate) fn subresource(desc: &TextureDesc, mip: u32, layer: u32) -> usize {
    (mip * layers(desc) + layer) as usize
}

/// Width, height and depth (3D slices, else one) of a level.
pub(crate) fn level_size(desc: &TextureDesc, mip: u32) -> (u32, u32, u32) {
    let s = |n: u32| (n >> mip).max(1);
    let depth = match desc.dimension {
        TextureDimension::D3 => s(desc.depth_or_layers),
        TextureDimension::D2 => 1,
    };
    (s(desc.width), s(desc.height), depth)
}

/// Bytes of one row of blocks and the number of rows of a level.
fn level_rows(desc: &TextureDesc, mip: u32) -> (usize, usize) {
    let (w, h, _) = level_size(desc, mip);
    let (bw, bh, size) = desc.format.block();
    ((w.div_ceil(bw) * size) as usize, h.div_ceil(bh) as usize)
}

/// Calls `f(subresource, byte offset, length)` for each row of blocks of a region, layer by layer.
fn for_rows(
    desc: &TextureDesc,
    region: &TextureRegion<'_, Recorder>,
    extent: Extent,
    mut f: impl FnMut(usize, usize, usize),
) -> Result<(), String> {
    let (bw, bh, size) = desc.format.block();
    if region.mip >= desc.mip_levels {
        return Err("a region past the last level".into());
    }
    let (w, h, d) = level_size(desc, region.mip);
    let o = region.origin;
    if !o.x.is_multiple_of(bw)
        || !o.y.is_multiple_of(bh)
        || o.x + extent.width > w.next_multiple_of(bw)
        || o.y + extent.height > h.next_multiple_of(bh)
    {
        return Err("a region outside its level or off the block grid".into());
    }
    let (row_bytes, rows) = level_rows(desc, region.mip);
    let len = (extent.width.div_ceil(bw) * size) as usize;
    let x = (o.x / bw * size) as usize;
    let first_row = (o.y / bh) as usize;
    let region_rows = extent.height.div_ceil(bh) as usize;
    let three_d = desc.dimension == TextureDimension::D3;
    if three_d {
        if o.z + extent.depth_or_layers > d {
            return Err("a region past the last slice".into());
        }
    } else if region.layer + extent.depth_or_layers > desc.depth_or_layers {
        return Err("a region past the last layer".into());
    }
    for k in 0..extent.depth_or_layers {
        let (sub, slice) = if three_d {
            (subresource(desc, region.mip, 0), (o.z + k) as usize)
        } else {
            (subresource(desc, region.mip, region.layer + k), 0)
        };
        for r in 0..region_rows {
            f(sub, (slice * rows + first_row + r) * row_bytes + x, len);
        }
    }
    Ok(())
}

impl Backend for Recorder {
    type Buffer = Id;
    type Texture = Id;
    type TextureView = Id;
    type Sampler = Id;
    type Shader = Id;
    type Program = Id;
    type Blend = Id;
    type DepthStencil = Id;
    type Raster = Id;
    type VertexLayout = Id;
    type BindingSet = Id;
    type QuerySet = Id;
    type Readback = Id;
    type Fence = Id;
    type ResourceTable = Id;
    type Surface = Id;

    fn caps(&self) -> &Caps {
        &self.caps
    }
    fn slow_paths(&mut self) -> &mut SlowPaths {
        &mut self.slow
    }
    fn status(&mut self) -> DeviceStatus {
        match self.lost {
            Some(reason) => DeviceStatus::Lost(reason),
            None => DeviceStatus::Ok,
        }
    }
    fn helpers(&mut self) -> &mut Helpers<Self> {
        &mut self.helpers
    }

    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            if desc.size > self.caps.limits.max_buffer_size {
                return Err(GpuError::Invalid("buffer larger than the limit"));
            }
            let id = self.add(Obj::Buffer(BufferObj {
                bytes: vec![0; desc.size as usize],
                usage: desc.usage,
            }));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateBuffer(*desc, made));
        made
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let uses = self.caps.formats.get(desc.format);
            if uses.is_empty() {
                return Err(GpuError::Invalid(
                    "texture of a format this back end does not hold",
                ));
            }
            if !self.caps.formats.takes_samples(desc.format, desc.samples) {
                return Err(GpuError::Invalid(
                    "texture sample count the format does not take",
                ));
            }
            if desc.mip_levels == 0
                || desc.width == 0
                || desc.height == 0
                || desc.depth_or_layers == 0
            {
                return Err(GpuError::Invalid("empty texture"));
            }
            if desc.cube && !desc.depth_or_layers.is_multiple_of(6) {
                return Err(GpuError::Invalid(
                    "cube texture whose layers are not whole cubes",
                ));
            }
            let mut subresources = Vec::new();
            for mip in 0..desc.mip_levels {
                let (row, rows) = level_rows(desc, mip);
                let (_, _, slices) = level_size(desc, mip);
                for _ in 0..layers(desc) {
                    subresources.push(vec![0; row * rows * slices as usize]);
                }
            }
            let id = self.add(Obj::Texture(TextureObj {
                desc: *desc,
                subresources,
            }));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateTexture(*desc, made));
        made
    }

    fn create_view(&mut self, texture: &Id, desc: &TextureViewDesc) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let t = self
                .texture(*texture)
                .map_err(|_| GpuError::Invalid("view of an object that is not a live texture"))?;
            let twin = desc.format.linear() == t.desc.format.linear();
            if desc.format != t.desc.format && !(twin && t.desc.reinterpret) {
                return Err(GpuError::Invalid(
                    "view format neither the texture's nor its twin",
                ));
            }
            if desc.base_mip + desc.mip_count > t.desc.mip_levels
                || desc.base_layer + desc.layer_count > layers(&t.desc)
                || desc.mip_count == 0
                || desc.layer_count == 0
            {
                return Err(GpuError::Invalid("view range outside the texture"));
            }
            let id = self.add(Obj::View(ViewObj {
                texture: *texture,
                desc: *desc,
            }));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateView(*texture, *desc, made));
        made
    }

    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            if desc
                .address
                .iter()
                .any(|a| matches!(a, AddressMode::Border | AddressMode::MirrorOnce))
            {
                return Err(GpuError::Absent(
                    Features::BORDER_COLOR | Features::MIRROR_ONCE,
                ));
            }
            let id = self.add(Obj::Sampler(*desc));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateSampler(*desc, made));
        made
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let id = self.add(Obj::Shader(Rc::new(module.clone())));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateShader(Shader(Rc::new(module.clone())), made));
        made
    }

    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let shader = |r: &Self, id: &Id| match r.obj(*id) {
                Ok(Obj::Shader(s)) => Ok(s.clone()),
                _ => Err(GpuError::Invalid(
                    "program of an object that is not a live shader",
                )),
            };
            let obj = match program {
                Program::Render { vertex, fragment } => ProgramObj {
                    vertex: shader(self, vertex)?,
                    fragment: fragment.map(|f| shader(self, f)).transpose()?,
                },
                Program::Compute(_) => return Err(GpuError::Absent(Features::COMPUTE)),
            };
            let id = self.add(Obj::Program(obj));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(match program {
            Program::Render { vertex, fragment } => {
                Call::CreateProgram(**vertex, fragment.copied(), false, made)
            }
            Program::Compute(c) => Call::CreateProgram(**c, None, true, made),
        });
        made
    }

    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            if desc.logic_op.is_some() {
                return Err(GpuError::Absent(Features::LOGIC_OP));
            }
            let second_source = |f: BlendFactor| {
                use BlendFactor::*;
                matches!(f, Src1 | OneMinusSrc1 | Src1Alpha | OneMinusSrc1Alpha)
            };
            if desc
                .targets
                .iter()
                .filter_map(|t| t.blend)
                .flat_map(|(c, a)| [c.src, c.dst, a.src, a.dst])
                .any(second_source)
            {
                return Err(GpuError::Absent(Features::DUAL_SOURCE_BLEND));
            }
            let id = self.add(Obj::Blend(Box::new(*desc)));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateBlend(Box::new(*desc), made));
        made
    }

    fn create_depth_stencil(&mut self, desc: &DepthStencilDesc) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            if desc.depth_bounds.is_some() {
                return Err(GpuError::Absent(Features::DEPTH_BOUNDS));
            }
            let id = self.add(Obj::DepthStencil(*desc));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateDepthStencil(*desc, made));
        made
    }

    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            if desc.fill != FillMode::Solid {
                return Err(GpuError::Absent(Features::POLYGON_MODE));
            }
            if !desc.depth_clip {
                return Err(GpuError::Absent(Features::DEPTH_CLAMP));
            }
            if desc.depth_minus_one {
                return Err(GpuError::Absent(Features::CLIP_DEPTH_MINUS_ONE));
            }
            let id = self.add(Obj::Raster(*desc));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateRaster(*desc, made));
        made
    }

    fn create_vertex_layout(&mut self, desc: &VertexLayoutDesc) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let id = self.add(Obj::VertexLayout(Box::new(*desc)));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateVertexLayout(Box::new(*desc), made));
        made
    }

    fn create_binding_set(&mut self, entries: &[BindingEntry<'_, Self>]) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let mut set = Vec::new();
            for e in entries {
                let bound = match e.resource {
                    BindingResource::UniformBuffer(r)
                        if !self
                            .buffer(*r.buffer)
                            .is_ok_and(|b| b.usage.contains(BufferUsage::UNIFORM)) =>
                    {
                        return Err(GpuError::Invalid(
                            "a uniform binding of a buffer without UNIFORM",
                        ));
                    }
                    BindingResource::StorageBuffer { range: r, .. }
                        if !self
                            .buffer(*r.buffer)
                            .is_ok_and(|b| b.usage.contains(BufferUsage::STORAGE)) =>
                    {
                        return Err(GpuError::Invalid(
                            "a storage binding of a buffer without STORAGE",
                        ));
                    }
                    BindingResource::Texture(v)
                        if !self
                            .view(*v)
                            .and_then(|v| self.texture(v.texture))
                            .is_ok_and(|t| t.desc.usage.contains(TextureUsage::SAMPLE)) =>
                    {
                        return Err(GpuError::Invalid(
                            "a texture binding of a texture without SAMPLE",
                        ));
                    }
                    BindingResource::UniformBuffer(r) => Bound::Buffer {
                        buffer: *r.buffer,
                        offset: r.offset,
                        size: r.size,
                        writable: false,
                    },
                    BindingResource::StorageBuffer { writable: true, .. }
                        if e.stages.contains(StageSet::VERTEX) =>
                    {
                        return Err(GpuError::Absent(Features::VERTEX_STORAGE_WRITE));
                    }
                    BindingResource::StorageBuffer { range: r, writable } => Bound::Buffer {
                        buffer: *r.buffer,
                        offset: r.offset,
                        size: r.size,
                        writable,
                    },
                    BindingResource::Texture(v) => Bound::Texture(*v),
                    BindingResource::Sampler(s) => Bound::Sampler(*s),
                    // No format this back end holds has storage uses.
                    BindingResource::StorageTexture { .. } => {
                        return Err(GpuError::Invalid(
                            "a storage texture of a format without storage uses",
                        ));
                    }
                    BindingResource::TexelBuffer { .. } => {
                        return Err(GpuError::Invalid("typed texel buffers are fronts' routes"));
                    }
                    BindingResource::Table(_) => {
                        return Err(GpuError::Absent(Features::RESOURCE_TABLES));
                    }
                };
                if set.iter().any(|(b, _)| *b == e.binding) {
                    return Err(GpuError::Invalid("two entries at one binding"));
                }
                set.push((e.binding, bound));
            }
            let id = self.add(Obj::BindingSet(set));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateBindingSet(
            entries
                .iter()
                .map(|e| (e.binding, e.stages, bind_of(&e.resource)))
                .collect(),
            made,
        ));
        made
    }

    fn binding_layout_key(&self, _: &Id) -> BindingLayoutKey {
        BindingLayoutKey::default()
    }

    fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            match kind {
                QueryKind::OcclusionAny => {}
                QueryKind::Occlusion => return Err(GpuError::Absent(Features::OCCLUSION_COUNT)),
                QueryKind::Timestamp => return Err(GpuError::Absent(Features::TIMESTAMPS)),
            }
            let id = self.add(Obj::QuerySet {
                kind,
                results: vec![0; count as usize],
            });
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateQuerySet(kind, count, made));
        made
    }

    fn destroy(&mut self, object: Object<Self>) {
        let kind = kind_of(&object);
        let id = match object {
            Object::Buffer(i)
            | Object::Texture(i)
            | Object::TextureView(i)
            | Object::Sampler(i)
            | Object::Shader(i)
            | Object::Program(i)
            | Object::Blend(i)
            | Object::DepthStencil(i)
            | Object::Raster(i)
            | Object::VertexLayout(i)
            | Object::BindingSet(i)
            | Object::QuerySet(i)
            | Object::ResourceTable(i)
            | Object::Surface(i) => i,
        };
        self.log(Call::Destroy(kind.0, kind.1));
        match self.objects.get_mut(id.0 as usize) {
            Some(slot @ Some(_)) => *slot = None,
            _ => self.violate(format!("object {} destroyed twice", id.0)),
        }
    }

    fn write_buffer(&mut self, buffer: &Id, offset: u64, data: &[u8], mode: WriteMode) {
        self.log(Call::WriteBuffer(*buffer, offset, data.to_vec(), mode));
        self.check(|r| {
            r.buffer_for(*buffer, BufferUsage::COPY_DST, "write_buffer")?;
            let end = offset + data.len() as u64;
            if mode == WriteMode::NoOverwrite
                && r.reads
                    .iter()
                    .any(|&(b, s, e)| b == *buffer && s < end && offset < e)
            {
                return Err(format!(
                    "a NoOverwrite write over bytes of buffer {} an unfinished command reads",
                    buffer.0
                ));
            }
            let b = r.buffer_mut(*buffer)?;
            let end = offset as usize + data.len();
            b.bytes
                .get_mut(offset as usize..end)
                .ok_or("write past the buffer's end")?
                .copy_from_slice(data);
            Ok(())
        });
    }

    fn write_texture(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    ) {
        self.log(Call::WriteTexture(
            Region::of(region),
            extent,
            data.to_vec(),
            row_pitch,
            rows_per_layer,
            mode,
        ));
        self.check(|r| {
            r.texture_for(*region.texture, TextureUsage::COPY_DST, "write_texture")?;
            r.write_region(region, extent, data, row_pitch, rows_per_layer)
        });
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        self.log(Call::BeginPass(stream::Pass::of(pass)));
        if let Err(e) = self.start_pass(pass) {
            self.violate(e);
        }
    }

    fn end_pass(&mut self) {
        self.log(Call::EndPass);
        self.check(Self::finish_pass);
    }

    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        self.log(Call::Draw(Box::new(Draw::of(call))));
        self.check(|r| r.draw_call(call));
    }

    fn begin_query(&mut self, index: u32) {
        self.log(Call::BeginQuery(index));
        self.check(|r| {
            let pass = r.pass.as_mut().ok_or("begin_query outside a pass")?;
            if pass.occlusion.is_none() {
                return Err("begin_query in a pass without an occlusion set".into());
            }
            if pass.query.replace((index, 0)).is_some() {
                return Err("begin_query while a query runs".into());
            }
            Ok(())
        });
    }

    fn end_query(&mut self) {
        self.log(Call::EndQuery);
        self.check(|r| {
            let pass = r.pass.as_mut().ok_or("end_query outside a pass")?;
            let (index, passed) = pass
                .query
                .take()
                .ok_or("end_query without a running query")?;
            let set = pass.occlusion.ok_or("no occlusion set")?;
            match r.objects.get_mut(set.0 as usize).and_then(Option::as_mut) {
                Some(Obj::QuerySet { results, .. }) => {
                    *results
                        .get_mut(index as usize)
                        .ok_or("query index past the set")? = u64::from(passed > 0);
                    Ok(())
                }
                _ => Err("the occlusion set was destroyed".into()),
            }
        });
    }

    fn copy_buffer(&mut self, src: &Id, src_offset: u64, dst: &Id, dst_offset: u64, size: u64) {
        self.log(Call::CopyBuffer(*src, src_offset, *dst, dst_offset, size));
        self.check(|r| {
            r.outside_pass("copy_buffer")?;
            r.buffer_for(*src, BufferUsage::COPY_SRC, "copy_buffer")?;
            r.reads.push((*src, src_offset, src_offset + size));
            r.buffer_for(*dst, BufferUsage::COPY_DST, "copy_buffer")?;
            if src == dst && src_offset < dst_offset + size && dst_offset < src_offset + size {
                return Err("copy between overlapping ranges of one buffer".into());
            }
            let range = src_offset as usize..(src_offset + size) as usize;
            let bytes = r
                .buffer(*src)?
                .bytes
                .get(range)
                .ok_or("copy from past the buffer's end")?
                .to_vec();
            let d = r.buffer_mut(*dst)?;
            d.bytes
                .get_mut(dst_offset as usize..(dst_offset + size) as usize)
                .ok_or("copy to past the buffer's end")?
                .copy_from_slice(&bytes);
            Ok(())
        });
    }

    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.log(Call::CopyTexture(Region::of(src), Region::of(dst), extent));
        self.check(|r| {
            r.outside_pass("copy_texture")?;
            r.texture_for(*src.texture, TextureUsage::COPY_SRC, "copy_texture")?;
            r.texture_for(*dst.texture, TextureUsage::COPY_DST, "copy_texture")?;
            let (sf, df) = (
                r.texture(*src.texture)?.desc.format,
                r.texture(*dst.texture)?.desc.format,
            );
            if texel::bits_group(sf) != texel::bits_group(df) {
                return Err("copy between formats of different typeless groups".into());
            }
            let bytes = r.read_region(src, extent)?;
            let (bw, bh, size) = sf.block();
            let pitch = extent.width.div_ceil(bw) * size;
            // The destination's extent in its own blocks.
            let (dbw, dbh, _) = df.block();
            let dst_extent = Extent {
                width: extent.width.div_ceil(bw) * dbw,
                height: extent.height.div_ceil(bh) * dbh,
                depth_or_layers: extent.depth_or_layers,
            };
            r.write_region(dst, dst_extent, &bytes, pitch, extent.height.div_ceil(bh))
        });
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.log(Call::CopyBufferToTexture(
            BufferSide::of(src),
            Region::of(dst),
            extent,
        ));
        self.check(|r| {
            r.outside_pass("copy_buffer_to_texture")?;
            r.buffer_for(*src.buffer, BufferUsage::COPY_SRC, "copy_buffer_to_texture")?;
            let (bw, bh, size) = r.texture(*dst.texture)?.desc.format.block();
            let rows = u64::from(extent.height.div_ceil(bh));
            let pitch = u64::from(src.row_pitch);
            let rows_per_layer = u64::from(src.rows_per_layer).max(rows);
            let end = src.offset
                + (u64::from(extent.depth_or_layers - 1) * rows_per_layer + rows - 1) * pitch
                + u64::from(extent.width.div_ceil(bw) * size);
            r.reads.push((*src.buffer, src.offset, end));
            r.texture_for(
                *dst.texture,
                TextureUsage::COPY_DST,
                "copy_buffer_to_texture",
            )?;
            let bytes = r
                .buffer(*src.buffer)?
                .bytes
                .get(src.offset as usize..)
                .ok_or("copy from past the buffer's end")?
                .to_vec();
            r.write_region(dst, extent, &bytes, src.row_pitch, src.rows_per_layer)
        });
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        self.log(Call::CopyTextureToBuffer(
            Region::of(src),
            BufferSide::of(dst),
            extent,
        ));
        self.check(|r| {
            r.outside_pass("copy_texture_to_buffer")?;
            r.texture_for(
                *src.texture,
                TextureUsage::COPY_SRC,
                "copy_texture_to_buffer",
            )?;
            r.buffer_for(*dst.buffer, BufferUsage::COPY_DST, "copy_texture_to_buffer")?;
            let bytes = r.read_region(src, extent)?;
            let format = r.texture(*src.texture)?.desc.format;
            let (bw, bh, size) = format.block();
            let len = (extent.width.div_ceil(bw) * size) as usize;
            let rows = extent.height.div_ceil(bh) as usize;
            let rows_per_layer = if dst.rows_per_layer == 0 {
                rows
            } else {
                dst.rows_per_layer as usize
            };
            if (dst.row_pitch as usize) < len || rows_per_layer < rows {
                return Err("copy pitch smaller than the region".into());
            }
            let b = r.buffer_mut(*dst.buffer)?;
            for (n, row) in bytes.chunks_exact(len.max(1)).enumerate() {
                let (layer, y) = (n / rows, n % rows);
                let at = dst.offset as usize
                    + layer * rows_per_layer * dst.row_pitch as usize
                    + y * dst.row_pitch as usize;
                b.bytes
                    .get_mut(at..at + len)
                    .ok_or("copy to past the buffer's end")?
                    .copy_from_slice(row);
            }
            Ok(())
        });
    }

    fn clear_buffer(&mut self, buffer: &Id, offset: u64, size: u64, pattern: &[u8]) {
        self.log(Call::ClearBuffer(*buffer, offset, size, pattern.to_vec()));
        self.check(|r| {
            r.outside_pass("clear_buffer")?;
            r.buffer_for(*buffer, BufferUsage::COPY_DST, "clear_buffer")?;
            if !matches!(pattern.len(), 1 | 2 | 4 | 8 | 16)
                || !size.is_multiple_of(pattern.len() as u64)
            {
                return Err("clear pattern of a size the interface does not take".into());
            }
            let b = r.buffer_mut(*buffer)?;
            let range = b
                .bytes
                .get_mut(offset as usize..(offset + size) as usize)
                .ok_or("clear past the buffer's end")?;
            for chunk in range.chunks_exact_mut(pattern.len()) {
                chunk.copy_from_slice(pattern);
            }
            Ok(())
        });
    }

    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) {
        self.log(Call::Resolve(Region::of(src), Region::of(dst), format));
        self.violate("resolve: this back end makes no multisampled texture to resolve");
    }

    fn submit(&mut self) -> Id {
        if self.pass.is_some() {
            self.violate("submit inside a pass");
        }
        self.submits += 1;
        // The recording back end finishes all work at a submit.
        self.reads.clear();
        let id = self.add(Obj::Fence);
        self.log(Call::Submit(id));
        id
    }

    fn fence_done(&mut self, fence: &Id) -> bool {
        if !matches!(self.obj(*fence), Ok(Obj::Fence)) {
            self.violate("fence_done of an object that is not a fence");
        }
        true
    }

    fn wait_fence(&mut self, fence: &Id) {
        self.fence_done(fence);
    }

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            match self.snapshot(src) {
                Ok(bytes) => Ok(self.readback(bytes)),
                Err(e) => {
                    self.violate(e);
                    Err(GpuError::Invalid(
                        "read back the recording back end refused",
                    ))
                }
            }
        })();
        self.nested -= 1;
        self.log(Call::ReadBack(
            match src {
                ReadSource::Buffer {
                    buffer,
                    offset,
                    size,
                } => Ok((**buffer, *offset, *size)),
                ReadSource::Texture { region, extent } => Err((Region::of(region), *extent)),
            },
            made,
        ));
        made
    }

    fn read_queries(&mut self, set: &Id, first: u32, count: u32) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let results = match self.obj(*set) {
                Ok(Obj::QuerySet { results, .. }) => results
                    .get(first as usize..(first + count) as usize)
                    .map(|r| r.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>()),
                _ => None,
            };
            match results {
                Some(bytes) if self.pass.is_none() => Ok(self.readback(bytes)),
                _ => {
                    self.violate("read_queries of a range outside a live set, or inside a pass");
                    Err(GpuError::Invalid("read_queries refused"))
                }
            }
        })();
        self.nested -= 1;
        self.log(Call::ReadQueries(*set, first, count, made));
        made
    }

    fn read_state(&mut self, readback: &Id) -> ReadState {
        match self.obj(*readback) {
            Ok(Obj::Readback { ready_at, .. }) if *ready_at <= self.submits => ReadState::Ready,
            Ok(Obj::Readback { .. }) => ReadState::Pending,
            _ => {
                self.violate("read_state of an object that is not a read back");
                ReadState::Pending
            }
        }
    }

    fn wait_read(&mut self, readback: &Id) {
        self.log(Call::WaitRead(*readback));
        if self.read_state(readback) == ReadState::Pending {
            self.nested += 1;
            self.submit();
            self.nested -= 1;
        }
    }

    fn finish_read(&mut self, readback: Id, dst: &mut [u8]) {
        #[allow(clippy::redundant_closure_call)]
        (|| {
            if self.read_state(&readback) == ReadState::Pending {
                return self.violate("finish_read of a read back that is not ready");
            }
            let Some(Some(Obj::Readback { bytes, .. })) =
                self.objects.get_mut(readback.0 as usize).map(Option::take)
            else {
                return self.violate("finish_read of an object that is not a read back");
            };
            if dst.len() != bytes.len() {
                return self.violate(format!(
                    "finish_read into {} bytes of a {}-byte read back",
                    dst.len(),
                    bytes.len()
                ));
            }
            dst.copy_from_slice(&bytes);
        })();
        self.log(Call::FinishRead(readback, dst.to_vec()));
    }

    fn create_surface(&mut self, window: u32) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let id = self.add(Obj::Surface(SurfaceObj {
                window,
                config: None,
                current: None,
            }));
            Ok(id)
        })();
        self.nested -= 1;
        self.log(Call::CreateSurface(window, made));
        made
    }

    fn configure_surface(
        &mut self,
        surface: &Id,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError> {
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<(), GpuError> {
            if !self.caps.formats.get(format).contains(FormatUse::PRESENT) {
                return Err(GpuError::Invalid("surface format without PRESENT"));
            }
            match self
                .objects
                .get_mut(surface.0 as usize)
                .and_then(Option::as_mut)
            {
                Some(Obj::Surface(s)) => {
                    s.config = Some((format, width, height));
                    Ok(())
                }
                _ => Err(GpuError::Invalid(
                    "configure_surface of an object that is not a surface",
                )),
            }
        })();
        self.log(Call::ConfigureSurface(
            *surface, format, width, height, vsync, made,
        ));
        made
    }

    fn current_texture(&mut self, surface: &Id) -> Result<Id, GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<Id, GpuError> {
            let config = match self.obj(*surface) {
                Ok(Obj::Surface(s)) => s.config,
                _ => {
                    return Err(GpuError::Invalid(
                        "current_texture of an object that is not a surface",
                    ));
                }
            };
            let (format, width, height) = config.ok_or(GpuError::Invalid(
                "current_texture of an unconfigured surface",
            ))?;
            let desc = TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width,
                height,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::RENDER | TextureUsage::COPY_DST | TextureUsage::PRESENT,
                cube: false,
                reinterpret: false,
            };
            let texture = self.create_texture(&desc)?;
            let view = self.create_view(
                &texture,
                &TextureViewDesc {
                    format,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: false,
                },
            )?;
            if let Some(Some(Obj::Surface(s))) = self.objects.get_mut(surface.0 as usize) {
                s.current = Some((texture, view));
            }
            Ok(view)
        })();
        self.nested -= 1;
        self.log(Call::CurrentTexture(*surface, made));
        made
    }

    fn present(&mut self, surface: &Id) -> Result<(), GpuError> {
        self.nested += 1;
        #[allow(clippy::redundant_closure_call)]
        let made = (|| -> Result<(), GpuError> {
            let (window, config, current) = match self
                .objects
                .get_mut(surface.0 as usize)
                .and_then(Option::as_mut)
            {
                Some(Obj::Surface(s)) => (s.window, s.config, s.current.take()),
                _ => {
                    return Err(GpuError::Invalid(
                        "present of an object that is not a surface",
                    ));
                }
            };
            let (Some((format, width, height)), Some((texture, view))) = (config, current) else {
                self.violate("present without a current texture");
                return Err(GpuError::Invalid("present without a current texture"));
            };
            if self.pass.is_some() {
                self.violate("present inside a pass");
            }
            let bytes = self
                .texture(texture)
                .map(|t| t.subresources[0].clone())
                .unwrap_or_default();
            self.shown.push(Frame {
                window,
                format,
                width,
                height,
                bytes,
            });
            self.objects[view.0 as usize] = None;
            self.objects[texture.0 as usize] = None;
            self.submit();
            Ok(())
        })();
        self.nested -= 1;
        self.log(Call::Present(*surface, made));
        made
    }
}

/// A value as the scalar kind a program input asks for.
pub(crate) fn as_kind(v: f64, kind: naga::ScalarKind) -> Value {
    match kind {
        naga::ScalarKind::Sint => Value::I32(v as i32),
        naga::ScalarKind::Uint => Value::U32(v as u32),
        naga::ScalarKind::Bool => Value::Bool(v != 0.0),
        _ => Value::F32(v as f32),
    }
}
