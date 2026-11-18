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

/// A binding set's entries: binding number, the stages named, and what is bound.
pub(crate) type Entries = Vec<(u32, StageSet, Bound)>;

pub(crate) enum Obj {
    Buffer(BufferObj),
    Texture(TextureObj),
    View(ViewObj),
    Sampler(SamplerDesc),
    Shader(Rc<ShaderModule>),
    Program(ProgramObj),
    ComputeProgram(Rc<ShaderModule>),
    Blend(Box<BlendDesc>),
    DepthStencil(DepthStencilDesc),
    Raster(RasterDesc),
    VertexLayout(Box<VertexLayoutDesc>),
    BindingSet(Entries, StageCounts),
    QuerySet {
        kind: QueryKind,
        results: Vec<u64>,
    },
    Readback {
        bytes: Vec<u8>,
        ready_at: u64,
    },
    /// A fence, and the submit it ends.
    Fence(u64),
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
    /// Multisampled attachments and the views they resolve into when the pass ends.
    pub resolves: Vec<(Id, Id)>,
    pub depth: Option<DepthOps>,
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub occlusion: Option<Id>,
    /// The running occlusion query and whether any sample passed.
    pub query: Option<(u32, u64)>,
    /// The query indices begun in this pass.
    pub begun: Vec<u32>,
    /// Buffers the pass's draws bound as writable storage, and those they used any other way.
    pub written: Vec<Id>,
    pub other: Vec<Id>,
}

/// A range of bytes of a buffer (`sub` 0), or of one subresource of a texture as held (whole texels),
/// and the submit the use belongs to.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Touched {
    pub object: Id,
    pub sub: usize,
    pub start: u64,
    pub end: u64,
    pub submit: u64,
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
    /// The time timestamps give, in nanoseconds: one for each call run, nested ones too.
    pub(crate) clock: u64,
    /// Every base call, in order, with its answer.
    pub calls: Vec<Call>,
    /// How deep the calls being made are inside another base call, whose own entry is the one kept.
    nested: u32,
    /// Bytes commands read or wrote, and `Ordered` writes wrote, until the caller waits for their
    /// submit: what no `NoOverwrite` write may touch.
    pub(crate) touched: Vec<Touched>,
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
            // One sample, and four for every format drawn into (bit n for 2^n samples).
            *n = 0b101;
            *u = FormatUse::SAMPLE | FormatUse::COPY_OUT;
            if f.is_depth_stencil() {
                *u |= FormatUse::DEPTH_STENCIL;
            } else {
                *u |= FormatUse::RENDER;
                if !texel::integer(f) {
                    *u |= FormatUse::FILTER | FormatUse::BLEND | FormatUse::RESOLVE;
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
            // Workgroup variables are not run: programs run one invocation at a time.
            compute_workgroup_storage: 0,
            compute_invocations: 256,
            compute_size: [256, 256, 64],
            max_dispatch: 65535,
            max_point_size: 1.0,
            max_line_width: 1.0,
            max_anisotropy: 1,
            capture_buffers: 4,
            capture_components: 64,
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
                    | Features::SAMPLE_MASK
                    | Features::MULTISAMPLE_READ
                    | Features::TEXTURE_VIEWS
                    | Features::FORMAT_REINTERPRET
                    | Features::SEPARATE_SAMPLERS
                    | Features::FRACTIONAL_VIEWPORT
                    | Features::FRACTIONAL_DEPTH_BIAS
                    | Features::DEPTH_BIAS_CLAMP
                    | Features::BLEND_CONSTANT_ALPHA
                    | Features::INDEX_U8
                    | Features::DEPTH24_UNORM
                    | Features::BASE_VERTEX_INSTANCE
                    | Features::INSTANCE_STEP_RATE
                    | Features::CLIP_DEPTH_ZERO_ONE
                    | Features::FRAGMENT_STORAGE_WRITE
                    | Features::VERTEX_STORAGE_READ
                    | Features::COMPUTE
                    | Features::TRANSFORM_FEEDBACK
                    | Features::TIMESTAMPS,
                limits,
                formats: FormatCaps { uses, samples },
            },
            slow: SlowPaths::default(),
            helpers: Helpers::default(),
            lost: None,
            objects: Vec::new(),
            pass: None,
            submits: 0,
            clock: 0,
            calls: Vec::new(),
            nested: 0,
            touched: Vec::new(),
            violations: Vec::new(),
            shown: Vec::new(),
        }
    }

    /// A recording back end whose capabilities `change` alters (a feature or a format's uses taken
    /// away), so a test drives the route a helper takes on back ends without them.
    pub fn with_caps(change: impl FnOnce(&mut Caps)) -> Self {
        let mut r = Self::new();
        change(&mut r.caps);
        r
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

    /// A live texture's bytes as held: its subresources in order, a multisampled one's samples as
    /// consecutive planes.
    pub fn texture_bytes(&self, texture: Id) -> Option<Vec<u8>> {
        self.texture(texture).ok().map(|t| t.subresources.concat())
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

    /// The answer of a call that may be refused: a broken rule (`Err(Ok(why))`) loses the device and
    /// answers [`GpuError::Invalid`] with `what`; an absent feature (`Err(Err(e))`) answers `e`.
    fn refused(
        &mut self,
        done: Result<(), Result<String, GpuError>>,
        what: &'static str,
    ) -> Result<(), GpuError> {
        match done {
            Ok(()) => Ok(()),
            Err(Ok(why)) => {
                self.violate(why);
                Err(GpuError::Invalid(what))
            }
            Err(Err(e)) => Err(e),
        }
    }

    fn log(&mut self, call: Call) {
        self.clock += 1;
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

    /// Checks a texture's format copies out into buffers.
    fn copies_out(&self, id: Id) -> Result<(), String> {
        let format = self.texture(id)?.desc.format;
        if self.caps.formats.get(format).contains(FormatUse::COPY_OUT) {
            Ok(())
        } else {
            Err(format!(
                "{format:?} copied out, which this back end does not declare"
            ))
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

    pub(crate) fn outside_pass(&self, what: &str) -> Result<(), String> {
        match self.pass {
            Some(_) => Err(format!("{what} inside a pass")),
            None => Ok(()),
        }
    }

    /// The whole texels of a texture region, rows of blocks tightly packed, layer after layer.
    fn read_texels(
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

    /// The bytes of a texture region as a buffer copy or read back moves them: whole texels, or of
    /// a format with depth and stencil the aspect the region names.
    pub(crate) fn read_region(
        &self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> Result<Vec<u8>, String> {
        let format = self.texture(*region.texture)?.desc.format;
        let texels = self.read_texels(region, extent)?;
        if aspect_size(format, region.stencil)?.is_none() {
            return Ok(texels);
        }
        let size = format.block().2 as usize;
        let mut out = Vec::new();
        for t in texels.chunks_exact(size) {
            match (region.stencil, texel::stencil_byte(format)) {
                (true, Some(at)) => out.push(t[at]),
                // 24-bit depth in the low bits of a word.
                (false, Some(3)) => out.extend_from_slice(&[t[0], t[1], t[2], 0]),
                _ => out.extend_from_slice(&t[..4]),
            }
        }
        Ok(out)
    }

    /// Writes `data` (rows `row_pitch` apart, layers `rows_per_layer` rows apart) into a region: whole
    /// texels, or of a format with depth and stencil the aspect the region names.
    pub(crate) fn write_region(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
    ) -> Result<(), String> {
        let format = self.texture(*region.texture)?.desc.format;
        let Some(n) = aspect_size(format, region.stencil)? else {
            return self.write_texels(region, extent, data, row_pitch, rows_per_layer);
        };
        let row = extent.width as usize * n;
        let rows = extent.height as usize;
        let rows_per_layer = if rows_per_layer == 0 {
            rows
        } else {
            rows_per_layer as usize
        };
        if rows_per_layer < rows || (row_pitch as usize) < row {
            return Err("write or copy pitch smaller than the region".into());
        }
        let mut texels = self.read_texels(region, extent)?;
        let size = format.block().2 as usize;
        for (i, t) in texels.chunks_exact_mut(size).enumerate() {
            let (layer, y, x) = (
                i / (rows * extent.width as usize),
                i / extent.width as usize % rows,
                i % extent.width as usize,
            );
            let from = (layer * rows_per_layer + y) * row_pitch as usize + x * n;
            let src = data
                .get(from..from + n)
                .ok_or("data shorter than the region")?;
            match (region.stencil, texel::stencil_byte(format)) {
                (true, Some(at)) => t[at] = src[0],
                (false, Some(3)) => t[..3].copy_from_slice(&src[..3]),
                _ => t[..4].copy_from_slice(src),
            }
        }
        let pitch = extent.width * size as u32;
        self.write_texels(region, extent, &texels, pitch, extent.height)
    }

    /// Writes whole texels, `data` rows `row_pitch` apart and layers `rows_per_layer` rows apart.
    fn write_texels(
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
        if slice >= level_size(&t.desc, v.desc.base_mip).2 {
            return Err("an attachment's slice outside its level".into());
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
        let plane = (w * h * t.desc.samples) as usize * size;
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
        let mut count = None;
        let mut same_samples = |n: u32| -> Result<(), String> {
            match count {
                Some(c) if c != n => Err("attachments of different sample counts".to_string()),
                _ => {
                    count = Some(n);
                    Ok(())
                }
            }
        };
        let mut colors = [None; MAX_COLOR_TARGETS];
        let mut resolves = Vec::new();
        // Every (texture, subresource, slice) attached, to find one attached twice.
        let mut attached = Vec::new();
        let mut formats = Vec::new();
        for (i, c) in pass.colors.iter().enumerate() {
            let Some(c) = c else { continue };
            if i >= self.caps.limits.color_targets as usize {
                return Err("a colour attachment past the limit".into());
            }
            let (w, h) = self.view_size(*c.view)?;
            same(w, h)?;
            let format = self.view(*c.view)?.desc.format;
            if format.is_depth_stencil() {
                return Err("a depth format as a colour attachment".into());
            }
            formats.push(format);
            let (texture, sub, slice) = self.attachment(*c.view, c.depth_slice)?;
            attached.push((texture, sub, slice));
            let samples = self.texture(texture)?.desc.samples;
            same_samples(samples)?;
            if let Some(r) = c.resolve {
                let (rw, rh) = self.view_size(*r)?;
                same(rw, rh)?;
                let (rt, sub, slice) = self.attachment(*r, 0)?;
                attached.push((rt, sub, slice));
                if samples == 1 || self.texture(rt)?.desc.samples != 1 {
                    return Err(
                        "a resolve from a single-sampled attachment or into a multisampled one"
                            .into(),
                    );
                }
                if self.view(*r)?.desc.format != format {
                    return Err("a resolve target of another format than its attachment".into());
                }
                if !self.caps.formats.get(format).contains(FormatUse::RESOLVE) {
                    return Err("a resolve of a format that cannot be averaged".into());
                }
                resolves.push((*c.view, *r));
            }
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
            let (texture, sub, slice) = self.attachment(*d.view, 0)?;
            attached.push((texture, sub, slice));
            same_samples(self.texture(texture)?.desc.samples)?;
            depth = Some((*d.view, d.depth, d.stencil));
        }
        let bytes = color_target_bytes(formats)
            .ok_or("a colour attachment of a format that cannot be a colour target")?;
        if bytes > self.caps.limits.color_bytes_per_sample {
            return Err("colour attachments of more bytes a sample than the limit".into());
        }
        if (1..attached.len()).any(|i| attached[..i].contains(&attached[i])) {
            return Err("one subresource attached twice".into());
        }
        for &(texture, sub, slice) in &attached {
            self.touch_slice(texture, sub, slice)?;
        }
        let (width, height, samples) = match (size, pass.empty) {
            (Some(s), None) => (s.0, s.1, count.unwrap_or(1)),
            (None, Some(e)) => {
                let most = self.caps.limits.max_texture_2d;
                if e.width == 0 || e.height == 0 || e.width > most || e.height > most {
                    return Err(
                        "an empty pass of no size or wider or taller than max_texture_2d".into(),
                    );
                }
                if !self.caps.formats.takes_samples(Format::R8Unorm, e.samples) {
                    return Err("an empty pass of a sample count r8unorm does not take".into());
                }
                (e.width, e.height, e.samples)
            }
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
            resolves,
            depth,
            width,
            height,
            samples,
            occlusion: pass.occlusion.copied(),
            query: None,
            begun: Vec::new(),
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
        for (from, to) in pass.resolves {
            let format = self.view(from)?.desc.format;
            let (src, ..) = self.attachment(from, 0)?;
            let (dst, sub, _) = self.attachment(to, 0)?;
            let src_sub = subresource(
                &self.texture(src)?.desc,
                self.view(from)?.desc.base_mip,
                self.view(from)?.desc.base_layer,
            );
            self.average(src, src_sub, dst, sub, format)?;
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
                let last = end(*offset, *size, "read back")?;
                self.touch(**buffer, *offset, last);
                let b = self.buffer(**buffer)?;
                let range = *offset as usize..last as usize;
                b.bytes
                    .get(range)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "read back past the buffer's end".into())
            }
            ReadSource::Texture { region, extent } => {
                self.texture_for(*region.texture, TextureUsage::COPY_SRC, "a read back")?;
                self.copies_out(*region.texture)?;
                self.touch_region(region, *extent)?;
                self.read_region(region, *extent)
            }
        }
    }

    /// Writes into subresource `dst_sub` of `dst` the average of the samples of subresource `src_sub`
    /// of `src`, as values of `format`.
    fn average(
        &mut self,
        src: Id,
        src_sub: usize,
        dst: Id,
        dst_sub: usize,
        format: Format,
    ) -> Result<(), String> {
        let s = self.texture(src)?;
        let n = s.desc.samples as usize;
        let size = format.block().2 as usize;
        let bytes = &s.subresources[src_sub];
        let plane = bytes.len() / n;
        let mut out = vec![0u8; plane];
        for (i, texel) in out.chunks_exact_mut(size).enumerate() {
            let mut sum = [0.0; 4];
            for k in 0..n {
                let at = k * plane + i * size;
                let v = texel::decode(format, &bytes[at..at + size]);
                for c in 0..4 {
                    sum[c] += v[c];
                }
            }
            texel::encode(format, sum.map(|x| x / n as f64), texel);
        }
        self.texture_mut(dst)?.subresources[dst_sub] = out;
        Ok(())
    }

    /// Notes bytes of a buffer a command recorded now reads or writes, part of the next submit.
    pub(crate) fn touch(&mut self, buffer: Id, start: u64, end: u64) {
        self.touch_in(buffer, 0, start, end);
    }

    pub(crate) fn touch_in(&mut self, object: Id, sub: usize, start: u64, end: u64) {
        self.touched.push(Touched {
            object,
            sub,
            start,
            end,
            submit: self.submits + 1,
        });
    }

    /// Whether an unfinished use touches any of `ranges` of `object`.
    fn in_use(&self, object: Id, ranges: &[(usize, u64, u64)]) -> bool {
        self.touched.iter().any(|t| {
            t.object == object
                && ranges
                    .iter()
                    .any(|&(sub, s, e)| t.sub == sub && t.start < e && s < t.end)
        })
    }

    /// The bytes of a texture region in its subresources as held, rows that meet made one range.
    fn region_ranges(
        &self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> Result<Vec<(usize, u64, u64)>, String> {
        let t = self.texture(*region.texture)?;
        let mut out: Vec<(usize, u64, u64)> = Vec::new();
        for_rows(&t.desc, region, extent, |sub, at, len| {
            let (at, end) = (at as u64, (at + len) as u64);
            match out.last_mut() {
                Some(last) if last.0 == sub && last.2 == at => last.2 = end,
                _ => out.push((sub, at, end)),
            }
        })?;
        Ok(out)
    }

    /// Notes a texture region a command recorded now reads or writes.
    fn touch_region(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> Result<(), String> {
        for (sub, s, e) in self.region_ranges(region, extent)? {
            self.touch_in(*region.texture, sub, s, e);
        }
        Ok(())
    }

    /// Notes one slice of a texture's subresource (all of it but for 3D) a command uses.
    fn touch_slice(&mut self, texture: Id, sub: usize, slice: u32) -> Result<(), String> {
        let t = self.texture(texture)?;
        let mip = sub as u32 / layers(&t.desc);
        let plane = t.subresources[sub].len() as u64 / u64::from(level_size(&t.desc, mip).2);
        let at = u64::from(slice) * plane;
        self.touch_in(texture, sub, at, at + plane);
        Ok(())
    }

    fn readback(&mut self, bytes: Vec<u8>) -> Id {
        let ready_at = self.submits + 1;
        self.add(Obj::Readback { bytes, ready_at })
    }
}

/// Bytes per texel of the aspect a buffer copy or read back of a region of `format` moves, where the
/// format has both depth and stencil: one for stencil, four for depth (32-bit float, or 24-bit unorm
/// in the low bits of a word). None where the region moves whole texels.
/// The end of `size` bytes from `offset`, refused as past the buffer where it passes the largest offset.
fn end(offset: u64, size: u64, what: &str) -> Result<u64, String> {
    offset
        .checked_add(size)
        .ok_or_else(|| format!("{what} past the buffer's end"))
}

fn aspect_size(format: Format, stencil: bool) -> Result<Option<usize>, String> {
    match texel::stencil_byte(format) {
        Some(at) if at > 0 => Ok(Some(if stencil { 1 } else { 4 })),
        Some(_) => Ok(None),
        None if stencil => Err("a stencil region of a format without stencil".into()),
        None => Ok(None),
    }
}

/// The buffer side of a copy with a texture: its row pitch and offset whole numbers of `block`, the
/// bytes of a block of the aspect moved.
fn blocks_whole(side: &CopyBufferTexture<'_, Recorder>, block: u32) -> Result<(), String> {
    if !side.row_pitch.is_multiple_of(block) {
        return Err("a buffer-texture copy's row pitch is not a whole number of blocks".into());
    }
    if !side.offset.is_multiple_of(u64::from(block)) {
        return Err("a buffer-texture copy's offset is not a whole number of blocks".into());
    }
    Ok(())
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

/// Bindings of a set the vertex and the fragment stage see: uniform buffers, sampled textures,
/// samplers and storage buffers, against the limits per stage.
/// Bindings of each kind (uniform, texture, sampler, storage) for the vertex, fragment and compute
/// stages.
pub(crate) type StageCounts = [[u32; 4]; 3];

/// The components of each value a vertex program passes on to the fragment stage, each value one
/// inter-stage slot.
pub(crate) fn varyings(s: &ShaderModule) -> Vec<u32> {
    let Some(ep) = s
        .module
        .entry_points
        .iter()
        .find(|e| e.name == s.entry_point)
    else {
        return Vec::new();
    };
    let Some(r) = &ep.function.result else {
        return Vec::new();
    };
    let components = |ty: naga::Handle<naga::Type>| match s.module.types[ty].inner {
        naga::TypeInner::Vector { size, .. } => size as u32,
        _ => 1,
    };
    match (&r.binding, &s.module.types[r.ty].inner) {
        (Some(naga::Binding::Location { .. }), _) => vec![components(r.ty)],
        (None, naga::TypeInner::Struct { members, .. }) => members
            .iter()
            .filter(|m| matches!(m.binding, Some(naga::Binding::Location { .. })))
            .map(|m| components(m.ty))
            .collect(),
        _ => Vec::new(),
    }
}

/// A value passed between stages: its location, type, and interpolation and sampling, unstated ones
/// taken as WGSL defaults them (a float perspective, an integer flat; flat sampled at the first
/// vertex, perspective and linear at the centre).
type StageValueAt = (
    u32,
    naga::TypeInner,
    Option<naga::Interpolation>,
    Option<naga::Sampling>,
);

/// The location values a shader's entry point writes (`output`) or reads.
fn stage_values(s: &ShaderModule, output: bool) -> Vec<StageValueAt> {
    let Some(ep) = s
        .module
        .entry_points
        .iter()
        .find(|e| e.name == s.entry_point)
    else {
        return Vec::new();
    };
    let value = |binding: &Option<naga::Binding>, ty: naga::Handle<naga::Type>| {
        let Some(naga::Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        }) = *binding
        else {
            return None;
        };
        let inner = &s.module.types[ty].inner;
        let float = inner.scalar_kind() == Some(naga::ScalarKind::Float);
        let interpolation = interpolation.or(if float {
            Some(naga::Interpolation::Perspective)
        } else {
            Some(naga::Interpolation::Flat)
        });
        let sampling = sampling.or(match interpolation {
            Some(naga::Interpolation::Flat) => Some(naga::Sampling::First),
            _ => Some(naga::Sampling::Center),
        });
        Some((location, inner.clone(), interpolation, sampling))
    };
    // Each binding, or a struct's members.
    let each = |binding: &Option<naga::Binding>, ty: naga::Handle<naga::Type>| {
        let members: Vec<_> = match (binding, &s.module.types[ty].inner) {
            (None, naga::TypeInner::Struct { members, .. }) => {
                members.iter().map(|m| (m.binding.clone(), m.ty)).collect()
            }
            _ => vec![(binding.clone(), ty)],
        };
        members
            .into_iter()
            .filter_map(|(b, t)| value(&b, t))
            .collect::<Vec<_>>()
    };
    if output {
        ep.function
            .result
            .as_ref()
            .map(|r| each(&r.binding, r.ty))
            .unwrap_or_default()
    } else {
        ep.function
            .arguments
            .iter()
            .flat_map(|a| each(&a.binding, a.ty))
            .collect()
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
    let s = |n: u32| n.checked_shr(mip).unwrap_or(0).max(1);
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
    if desc.samples > 1 {
        return Err(
            "a multisampled texture's bytes moved other than by a draw, a resolve or a whole copy"
                .into(),
        );
    }
    if region.mip >= desc.mip_levels {
        return Err("a region past the last level".into());
    }
    let (w, h, d) = level_size(desc, region.mip);
    let o = region.origin;
    if !o.x.is_multiple_of(bw)
        || !o.y.is_multiple_of(bh)
        || u64::from(o.x) + u64::from(extent.width) > u64::from(w.next_multiple_of(bw))
        || u64::from(o.y) + u64::from(extent.height) > u64::from(h.next_multiple_of(bh))
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
        if u64::from(o.z) + u64::from(extent.depth_or_layers) > u64::from(d) {
            return Err("a region past the last slice".into());
        }
    } else if u64::from(region.layer) + u64::from(extent.depth_or_layers)
        > u64::from(desc.depth_or_layers)
    {
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
            if desc.samples > 1
                && (desc.dimension != TextureDimension::D2
                    || desc.mip_levels != 1
                    || desc.depth_or_layers != 1
                    || desc.usage.contains(TextureUsage::STORAGE))
            {
                return Err(GpuError::Invalid(
                    "a multisampled texture that is not 2D with one level and one layer, or is storage",
                ));
            }
            let l = self.caps.limits;
            let fits = match desc.dimension {
                TextureDimension::D3 => {
                    desc.width.max(desc.height).max(desc.depth_or_layers) <= l.max_texture_3d
                }
                TextureDimension::D2 => {
                    let most = if desc.cube {
                        l.max_texture_cube
                    } else {
                        l.max_texture_2d
                    };
                    desc.width.max(desc.height) <= most
                        && desc.depth_or_layers <= l.max_texture_layers
                }
            };
            if !fits {
                return Err(GpuError::Invalid("a texture past the size limits"));
            }
            let render = FormatUse::RENDER | FormatUse::DEPTH_STENCIL;
            if (desc.usage.contains(TextureUsage::RENDER) && !uses.intersects(render))
                || (desc.usage.contains(TextureUsage::SAMPLE) && !uses.contains(FormatUse::SAMPLE))
                || (desc.usage.contains(TextureUsage::STORAGE)
                    && !uses.intersects(FormatUse::STORAGE_READ | FormatUse::STORAGE_WRITE))
            {
                return Err(GpuError::Invalid(
                    "a texture usage its format does not take",
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
            if desc.cube
                && (desc.dimension != TextureDimension::D2
                    || desc.width != desc.height
                    || !desc.depth_or_layers.is_multiple_of(6))
            {
                return Err(GpuError::Invalid(
                    "cube texture that is not 2D and square with whole cubes of layers",
                ));
            }
            let (w, h, d) = level_size(desc, 0);
            if desc.mip_levels > 32 - w.max(h).max(d).leading_zeros() {
                return Err(GpuError::Invalid("more levels than the full chain"));
            }
            // A multisampled level holds its samples as consecutive planes.
            let mut subresources = Vec::new();
            for mip in 0..desc.mip_levels {
                let (row, rows) = level_rows(desc, mip);
                let (_, _, slices) = level_size(desc, mip);
                for _ in 0..layers(desc) {
                    subresources.push(vec![0; row * rows * (slices * desc.samples) as usize]);
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
            // Another colour format of the same block size, which draws read and write by the
            // view's format.
            let same_size = !desc.format.is_depth_stencil()
                && !t.desc.format.is_depth_stencil()
                && desc.format.block() == t.desc.format.block()
                && !self.caps.formats.get(desc.format).is_empty();
            if desc.format != t.desc.format && !(twin && t.desc.reinterpret) {
                if !(same_size && t.desc.reinterpret) {
                    return Err(GpuError::Invalid(
                        "view format neither the texture's nor one of its size it may take",
                    ));
                }
                if !self.caps.has(Features::FORMAT_REINTERPRET) {
                    return Err(GpuError::Absent(Features::FORMAT_REINTERPRET));
                }
            }
            if u64::from(desc.base_mip) + u64::from(desc.mip_count) > u64::from(t.desc.mip_levels)
                || u64::from(desc.base_layer) + u64::from(desc.layer_count)
                    > u64::from(layers(&t.desc))
                || desc.mip_count == 0
                || desc.layer_count == 0
            {
                return Err(GpuError::Invalid("view range outside the texture"));
            }
            let d2 = t.desc.dimension == TextureDimension::D2;
            let fits = match desc.dimension {
                ViewDimension::D2 => d2 && desc.layer_count == 1,
                ViewDimension::D2Array => d2,
                ViewDimension::Cube => t.desc.cube && desc.layer_count == 6,
                ViewDimension::CubeArray => t.desc.cube && desc.layer_count.is_multiple_of(6),
                ViewDimension::D3 => !d2,
            };
            if !fits {
                return Err(GpuError::Invalid(
                    "view dimension that does not fit its texture and layers",
                ));
            }
            // Without TEXTURE_VIEWS: views to draw into of one level and one layer or all layers,
            // and views of all layers in the texture's own dimension.
            let all_layers = desc.base_layer == 0 && desc.layer_count == layers(&t.desc);
            let to_draw = desc.mip_count == 1 && (desc.layer_count == 1 || all_layers);
            if !self.caps.features.contains(Features::TEXTURE_VIEWS)
                && !to_draw
                && !(all_layers && desc.dimension == t.desc.own_view())
            {
                return Err(GpuError::Absent(Features::TEXTURE_VIEWS));
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
            // The features the sampler asks for, each named only when one of its modes needs it.
            let asked = desc.address.iter().fold(Features::empty(), |f, a| match a {
                AddressMode::Border => f | Features::BORDER_COLOR,
                AddressMode::MirrorOnce => f | Features::MIRROR_ONCE,
                _ => f,
            });
            if !asked.is_empty() {
                return Err(GpuError::Absent(asked));
            }
            if desc.anisotropy > self.caps.limits.max_anisotropy {
                return Err(GpuError::Invalid("a sampler's anisotropy past the limit"));
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
            let stage = match module.stage {
                ShaderStage::Vertex => naga::ShaderStage::Vertex,
                ShaderStage::Fragment => naga::ShaderStage::Fragment,
                ShaderStage::Compute => naga::ShaderStage::Compute,
            };
            let Some(at) = module
                .module
                .entry_points
                .iter()
                .position(|e| e.name == module.entry_point && e.stage == stage)
            else {
                return Err(GpuError::Invalid(
                    "a shader naming an entry point of its stage its module lacks",
                ));
            };
            let textures = SlotKind::Texture.first_binding()
                ..SlotKind::Texture.first_binding() + SlotKind::Texture.slots();
            let outside = |image: naga::Handle<naga::GlobalVariable>| {
                module.module.global_variables[image]
                    .binding
                    .as_ref()
                    .is_some_and(|b| !textures.contains(&b.binding))
            };
            let paired = &module.info.get_entry_point(at).sampling_set;
            if paired.iter().any(|k| outside(k.image)) {
                return Err(GpuError::Invalid(
                    "a texture paired with a sampler outside the sampled-texture bindings",
                ));
            }
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
                Program::Compute(_) if !self.caps.has(Features::COMPUTE) => {
                    return Err(GpuError::Absent(Features::COMPUTE));
                }
                Program::Compute(c) => {
                    let c = shader(self, c)?;
                    if c.stage != ShaderStage::Compute {
                        return Err(GpuError::Invalid("a program of shaders of other stages"));
                    }
                    self.workgroup_limits(&c).map_err(GpuError::Invalid)?;
                    let id = self.add(Obj::ComputeProgram(c));
                    return Ok(id);
                }
            };
            if obj.vertex.stage != ShaderStage::Vertex
                || obj
                    .fragment
                    .as_ref()
                    .is_some_and(|f| f.stage != ShaderStage::Fragment)
            {
                return Err(GpuError::Invalid("a program of shaders of other stages"));
            }
            if let Some(f) = &obj.fragment {
                let outputs = stage_values(&obj.vertex, true);
                for input in stage_values(f, false) {
                    match outputs.iter().find(|o| o.0 == input.0) {
                        None => {
                            return Err(GpuError::Invalid(
                                "a fragment input the vertex shader does not write",
                            ));
                        }
                        Some(o) if *o != input => {
                            return Err(GpuError::Invalid(
                                "a fragment input of another type, interpolation or sampling than the vertex output",
                            ));
                        }
                        Some(_) => {}
                    }
                }
            }
            if varyings(&obj.vertex).len() > self.caps.limits.inter_stage_slots as usize {
                return Err(GpuError::Invalid(
                    "vertex outputs past the inter-stage limit",
                ));
            }
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
            let has = self.caps.features;
            if desc.logic_op.is_some() {
                return Err(GpuError::Absent(Features::LOGIC_OP));
            }
            if !desc.same_for_all && !has.contains(Features::INDEPENDENT_BLEND) {
                return Err(GpuError::Absent(Features::INDEPENDENT_BLEND));
            }
            let factors = || {
                desc.targets
                    .iter()
                    .filter_map(|t| t.blend)
                    .flat_map(|(c, a)| [c.src, c.dst, a.src, a.dst])
            };
            let second_source = |f: BlendFactor| {
                use BlendFactor::*;
                matches!(f, Src1 | OneMinusSrc1 | Src1Alpha | OneMinusSrc1Alpha)
            };
            if factors().any(second_source) {
                return Err(GpuError::Absent(Features::DUAL_SOURCE_BLEND));
            }
            if !has.contains(Features::BLEND_CONSTANT_ALPHA)
                && factors().any(|f| {
                    matches!(
                        f,
                        BlendFactor::ConstantAlpha | BlendFactor::OneMinusConstantAlpha
                    )
                })
            {
                return Err(GpuError::Absent(Features::BLEND_CONSTANT_ALPHA));
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
            if (desc.stencil_read_mask[0] != desc.stencil_read_mask[1]
                || desc.stencil_write_mask[0] != desc.stencil_write_mask[1])
                && !self.caps.features.contains(Features::STENCIL_PER_FACE)
            {
                return Err(GpuError::Absent(Features::STENCIL_PER_FACE));
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
            // Lines and points are captured, not drawn, so neither fill mode is declared.
            match desc.fill {
                FillMode::Solid => {}
                FillMode::Line => return Err(GpuError::Absent(Features::POLYGON_MODE)),
                FillMode::Point => return Err(GpuError::Absent(Features::POLYGON_MODE_POINT)),
            }
            if !desc.depth_clip {
                return Err(GpuError::Absent(Features::DEPTH_CLAMP));
            }
            if desc.depth_minus_one {
                return Err(GpuError::Absent(Features::CLIP_DEPTH_MINUS_ONE));
            }
            let has = self.caps.features;
            let needs = [
                (
                    !has.contains(Features::CLIP_DEPTH_ZERO_ONE),
                    Features::CLIP_DEPTH_ZERO_ONE,
                ),
                (
                    desc.provoking == ProvokingVertex::First
                        && !has.contains(Features::PROVOKING_VERTEX_FIRST),
                    Features::PROVOKING_VERTEX_FIRST,
                ),
                (
                    desc.provoking == ProvokingVertex::Last
                        && !has.contains(Features::PROVOKING_VERTEX_LAST),
                    Features::PROVOKING_VERTEX_LAST,
                ),
                (
                    f32::from_bits(desc.line_width) != 1.0 && !has.contains(Features::WIDE_LINES),
                    Features::WIDE_LINES,
                ),
                (
                    f32::from_bits(desc.depth_bias_clamp) != 0.0
                        && !has.contains(Features::DEPTH_BIAS_CLAMP),
                    Features::DEPTH_BIAS_CLAMP,
                ),
                (
                    f32::from_bits(desc.depth_bias).fract() != 0.0
                        && !has.contains(Features::FRACTIONAL_DEPTH_BIAS),
                    Features::FRACTIONAL_DEPTH_BIAS,
                ),
                (
                    desc.sample_mask != !0 && !has.contains(Features::SAMPLE_MASK),
                    Features::SAMPLE_MASK,
                ),
            ];
            if let Some(&(_, f)) = needs.iter().find(|(missing, _)| *missing) {
                return Err(GpuError::Absent(f));
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
            let l = self.caps.limits;
            let streams = desc
                .streams
                .iter()
                .rposition(Option::is_some)
                .map_or(0, |i| i + 1);
            let attributes = desc.attributes.iter().flatten().count();
            if streams > l.vertex_streams as usize || attributes > l.vertex_attributes as usize {
                return Err(GpuError::Invalid(
                    "a vertex layout of more streams or attributes than the limits",
                ));
            }
            if desc
                .streams
                .iter()
                .flatten()
                .any(|s| s.stride > l.max_vertex_stride)
            {
                return Err(GpuError::Invalid("a vertex stride past the limit"));
            }
            if !self.caps.has(Features::INSTANCE_STEP_RATE)
                && desc
                    .streams
                    .iter()
                    .flatten()
                    .any(|s| matches!(s.step, StepMode::Instance(n) if n != 1))
            {
                return Err(GpuError::Absent(Features::INSTANCE_STEP_RATE));
            }
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
            let mut counts = StageCounts::default();
            let (limits, has) = (self.caps.limits, self.caps.features);
            for e in entries {
                let range = match e.resource {
                    BindingResource::UniformBuffer(r) => Some((
                        r,
                        limits.min_uniform_offset_alignment,
                        u64::from(limits.max_uniform_buffer_size),
                    )),
                    BindingResource::StorageBuffer { range: r, .. } => Some((
                        r,
                        limits.min_storage_offset_alignment,
                        limits.max_storage_buffer_size,
                    )),
                    _ => None,
                };
                if let Some((r, align, max)) = range {
                    if !r.offset.is_multiple_of(u64::from(align)) {
                        return Err(GpuError::Invalid(
                            "a buffer binding at an offset off the limit's alignment",
                        ));
                    }
                    if r.size > max {
                        return Err(GpuError::Invalid("a buffer binding larger than the limit"));
                    }
                    if self.buffer(*r.buffer).is_ok_and(|b| {
                        r.offset
                            .checked_add(r.size)
                            .is_none_or(|e| e > b.bytes.len() as u64)
                    }) {
                        return Err(GpuError::Invalid("a buffer binding past the buffer's end"));
                    }
                }
                if let BindingResource::StorageBuffer { writable: true, .. } = e.resource
                    && e.stages.contains(StageSet::FRAGMENT)
                    && !has.contains(Features::FRAGMENT_STORAGE_WRITE)
                {
                    return Err(GpuError::Absent(Features::FRAGMENT_STORAGE_WRITE));
                }
                if let BindingResource::StorageBuffer {
                    writable: false, ..
                } = e.resource
                    && e.stages.contains(StageSet::VERTEX)
                    && !has.contains(Features::VERTEX_STORAGE_READ)
                {
                    return Err(GpuError::Absent(Features::VERTEX_STORAGE_READ));
                }
                if let BindingResource::Texture(v) = e.resource
                    && !has.contains(Features::TEXTURE_VIEWS)
                    && self.view(*v).is_ok_and(|v| {
                        self.texture(v.texture).is_ok_and(|t| {
                            v.desc.dimension != t.desc.own_view()
                                || v.desc.base_layer != 0
                                || v.desc.layer_count != layers(&t.desc)
                        })
                    })
                {
                    return Err(GpuError::Absent(Features::TEXTURE_VIEWS));
                }
                if let BindingResource::Texture(v) = e.resource
                    && self
                        .view(*v)
                        .and_then(|v| self.texture(v.texture))
                        .is_ok_and(|t| t.desc.samples > 1)
                    && !has.contains(Features::MULTISAMPLE_READ)
                {
                    return Err(GpuError::Absent(Features::MULTISAMPLE_READ));
                }
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
                        return Err(GpuError::Absent(Features::TEXEL_BUFFERS));
                    }
                    BindingResource::Table(_) => {
                        return Err(GpuError::Absent(Features::RESOURCE_TABLES));
                    }
                };
                if set.iter().any(|(b, _, _)| *b == e.binding) {
                    return Err(GpuError::Invalid("two entries at one binding"));
                }
                let kind = match e.resource {
                    BindingResource::UniformBuffer(_) => 0,
                    BindingResource::Texture(_) => 1,
                    BindingResource::Sampler(_) => 2,
                    _ => 3,
                };
                for (stage, c) in [StageSet::VERTEX, StageSet::FRAGMENT, StageSet::COMPUTE]
                    .iter()
                    .zip(&mut counts)
                {
                    if e.stages.contains(*stage) {
                        c[kind] += 1;
                    }
                }
                set.push((e.binding, e.stages, bound));
            }
            let id = self.add(Obj::BindingSet(set, counts));
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
                QueryKind::Timestamp if !self.caps.has(Features::TIMESTAMPS) => {
                    return Err(GpuError::Absent(Features::TIMESTAMPS));
                }
                QueryKind::Timestamp => {}
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
            let end = end(offset, data.len() as u64, "write")?;
            if mode == WriteMode::NoOverwrite && r.in_use(*buffer, &[(0, offset, end)]) {
                return Err(format!(
                    "a NoOverwrite write over bytes of buffer {} an unfinished command or an \
                     ordered write touches",
                    buffer.0
                ));
            }
            match mode {
                WriteMode::Ordered => r.touch(*buffer, offset, end),
                // Fresh storage: earlier commands keep the old bytes, and the rest is undefined.
                WriteMode::Discard => {
                    r.touched.retain(|t| t.object != *buffer);
                    if end <= r.buffer(*buffer)?.bytes.len() as u64 {
                        r.buffer_mut(*buffer)?.bytes.fill(GARBAGE);
                    }
                }
                WriteMode::NoOverwrite => {}
            }
            let b = r.buffer_mut(*buffer)?;
            b.bytes
                .get_mut(offset as usize..end as usize)
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
            let texture = *region.texture;
            r.texture_for(texture, TextureUsage::COPY_DST, "write_texture")?;
            let ranges = r.region_ranges(region, extent)?;
            if mode == WriteMode::NoOverwrite && r.in_use(texture, &ranges) {
                return Err(format!(
                    "a NoOverwrite write over texels of texture {} an unfinished command or an \
                     ordered write touches",
                    texture.0
                ));
            }
            match mode {
                WriteMode::Ordered => {
                    for (sub, s, e) in ranges {
                        r.touch_in(texture, sub, s, e);
                    }
                }
                // Fresh storage: earlier commands keep the old texels, and the rest is undefined.
                WriteMode::Discard => {
                    r.touched.retain(|t| t.object != texture);
                    for s in &mut r.texture_mut(texture)?.subresources {
                        s.fill(GARBAGE);
                    }
                }
                WriteMode::NoOverwrite => {}
            }
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
            if pass.begun.contains(&index) {
                return Err(format!("query {index} begun twice in one pass"));
            }
            pass.begun.push(index);
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
            let src_end = end(src_offset, size, "copy from")?;
            let dst_end = end(dst_offset, size, "copy to")?;
            r.touch(*src, src_offset, src_end);
            r.touch(*dst, dst_offset, dst_end);
            r.buffer_for(*dst, BufferUsage::COPY_DST, "copy_buffer")?;
            if src == dst && src_offset < dst_end && dst_offset < src_end {
                return Err("copy between overlapping ranges of one buffer".into());
            }
            let range = src_offset as usize..src_end as usize;
            let bytes = r
                .buffer(*src)?
                .bytes
                .get(range)
                .ok_or("copy from past the buffer's end")?
                .to_vec();
            let d = r.buffer_mut(*dst)?;
            d.bytes
                .get_mut(dst_offset as usize..dst_end as usize)
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
            let (sd, dd) = (r.texture(*src.texture)?.desc, r.texture(*dst.texture)?.desc);
            // Regions of one texture: the levels, the layers or slices, the rows and the columns
            // all meet where the regions overlap.
            let span = |a: u32, b: u32, n: u32| {
                u64::from(a) < u64::from(b) + u64::from(n)
                    && u64::from(b) < u64::from(a) + u64::from(n)
            };
            let (s, d) = (src.origin, dst.origin);
            let (sz, dz) = if sd.dimension == TextureDimension::D3 {
                (s.z, d.z)
            } else {
                (src.layer, dst.layer)
            };
            if src.texture == dst.texture
                && src.mip == dst.mip
                && span(sz, dz, extent.depth_or_layers)
                && span(s.y, d.y, extent.height)
                && span(s.x, d.x, extent.width)
            {
                return Err("a copy between overlapping regions of one texture".into());
            }
            if sd.samples > 1 || dd.samples > 1 {
                let whole = |d: &TextureDesc, at: &TextureRegion<'_, Self>| {
                    at.origin == (Origin { x: 0, y: 0, z: 0 })
                        && (extent.width, extent.height, extent.depth_or_layers)
                            == (d.width, d.height, 1)
                };
                if sd.samples != dd.samples || !whole(&sd, src) || !whole(&dd, dst) {
                    return Err(
                        "a multisampled copy that is not of whole subresources of one sample count"
                            .into(),
                    );
                }
                let bytes = r.texture(*src.texture)?.subresources[0].clone();
                r.texture_mut(*dst.texture)?.subresources[0] = bytes;
                return Ok(());
            }
            if (src.stencil || dst.stencil) && aspect_size(sf, true)?.is_some() {
                return Err(
                    "a copy between textures naming one aspect of depth and stencil".into(),
                );
            }
            let bytes = r.read_texels(src, extent)?;
            let (bw, bh, size) = sf.block();
            let pitch = extent.width.div_ceil(bw) * size;
            // The destination's extent in its own blocks.
            let (dbw, dbh, _) = df.block();
            let dst_extent = Extent {
                width: extent.width.div_ceil(bw) * dbw,
                height: extent.height.div_ceil(bh) * dbh,
                depth_or_layers: extent.depth_or_layers,
            };
            r.touch_region(src, extent)?;
            r.touch_region(dst, dst_extent)?;
            r.write_texels(dst, dst_extent, &bytes, pitch, extent.height.div_ceil(bh))
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
            let format = r.texture(*dst.texture)?.desc.format;
            let (bw, bh, size) = format.block();
            let size = aspect_size(format, dst.stencil)?.map_or(size, |n| n as u32);
            blocks_whole(src, size)?;
            let rows = u64::from(extent.height.div_ceil(bh));
            let pitch = u64::from(src.row_pitch);
            let rows_per_layer = u64::from(src.rows_per_layer).max(rows);
            let span = (u64::from(extent.depth_or_layers.saturating_sub(1)) * rows_per_layer
                + rows.saturating_sub(1))
                * pitch
                + u64::from(extent.width.div_ceil(bw) * size);
            let last = end(src.offset, span, "copy from")?;
            r.touch(*src.buffer, src.offset, last);
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
            r.touch_region(dst, extent)?;
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
            r.copies_out(*src.texture)?;
            let bytes = r.read_region(src, extent)?;
            r.touch_region(src, extent)?;
            let format = r.texture(*src.texture)?.desc.format;
            let (bw, bh, size) = format.block();
            let size = aspect_size(format, src.stencil)?.map_or(size, |n| n as u32);
            blocks_whole(dst, size)?;
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
            let span = bytes.len().div_ceil(len.max(1)).saturating_sub(1);
            let last = end(
                dst.offset,
                ((span / rows * rows_per_layer + span % rows) * dst.row_pitch as usize + len)
                    as u64,
                "copy to",
            )?;
            r.touch(*dst.buffer, dst.offset, last);
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
            let last = end(offset, size, "clear")?;
            r.touch(*buffer, offset, last);
            if !matches!(pattern.len(), 1 | 2 | 4 | 8 | 16)
                || !size.is_multiple_of(pattern.len() as u64)
            {
                return Err("clear pattern of a size the interface does not take".into());
            }
            let b = r.buffer_mut(*buffer)?;
            let range = b
                .bytes
                .get_mut(offset as usize..last as usize)
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
        self.check(|r| {
            r.outside_pass("resolve")?;
            r.texture_for(*src.texture, TextureUsage::RENDER, "resolve")?;
            r.texture_for(*dst.texture, TextureUsage::RENDER, "resolve")?;
            let (s, d) = (r.texture(*src.texture)?.desc, r.texture(*dst.texture)?.desc);
            if s.samples == 1 || d.samples != 1 {
                return Err(
                    "a resolve from a single-sampled texture or into a multisampled one".into(),
                );
            }
            if format.linear() != s.format.linear() || d.format.linear() != s.format.linear() {
                return Err(
                    "a resolve between formats other than one format and its sRGB twin".into(),
                );
            }
            if !r.caps.formats.get(format).contains(FormatUse::RESOLVE) {
                return Err("a resolve of a format that cannot be averaged".into());
            }
            if dst.mip >= d.mip_levels
                || dst.layer >= layers(&d)
                || level_size(&d, dst.mip) != level_size(&s, 0)
                || src.mip != 0
                || src.layer != 0
            {
                return Err("a resolve between subresources of different sizes".into());
            }
            let sub = subresource(&d, dst.mip, dst.layer);
            r.touch_slice(*dst.texture, sub, 0)?;
            r.average(*src.texture, 0, *dst.texture, sub, format)
        });
    }

    fn submit(&mut self) -> Id {
        if self.pass.is_some() {
            self.violate("submit inside a pass");
        }
        self.submits += 1;
        let id = self.add(Obj::Fence(self.submits));
        self.log(Call::Submit(id));
        id
    }

    /// The recording back end finishes all work at a submit, so every fence is done; the caller now
    /// knows the work before it finished.
    fn fence_done(&mut self, fence: &Id) -> bool {
        self.log(Call::FenceDone(*fence));
        match self.obj(*fence) {
            Ok(&Obj::Fence(n)) => self.touched.retain(|t| t.submit > n),
            _ => self.violate("fence_done of an object that is not a fence"),
        }
        true
    }

    fn wait_fence(&mut self, fence: &Id) {
        self.log(Call::WaitFence(*fence));
        self.nested += 1;
        self.fence_done(fence);
        self.nested -= 1;
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
                    .get(first as usize..first as usize + count as usize)
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
        if let Ok(&Obj::Readback { ready_at, .. }) = self.obj(*readback) {
            self.touched.retain(|t| t.submit > ready_at);
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

    fn dispatch(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
        self.nested += 1;
        let done = self.dispatch_call(call);
        self.nested -= 1;
        let done = self.refused(done, "dispatch refused");
        self.log(Call::Dispatch(Box::new(stream::Dispatch::of(call)), done));
        done
    }

    fn capture_vertices(
        &mut self,
        call: &DrawCall<'_, Self>,
        targets: CaptureTargets<'_, Self>,
        rasterize: bool,
    ) -> Result<(), GpuError> {
        self.nested += 1;
        let done = self.capture_call(call, targets, rasterize);
        self.nested -= 1;
        let done = self.refused(done, "capture_vertices refused");
        self.log(Call::CaptureVertices(
            Box::new(Draw::of(call)),
            targets
                .iter()
                .map(|t| (*t.buffer, t.offset, t.size))
                .collect(),
            rasterize,
            done,
        ));
        done
    }

    fn write_timestamp(&mut self, set: &Id, index: u32) -> Result<(), GpuError> {
        self.nested += 1;
        let done = self.timestamp_call(*set, index);
        self.nested -= 1;
        let done = self.refused(done, "write_timestamp refused");
        self.log(Call::WriteTimestamp(*set, index, done));
        done
    }

    fn timestamps_disjoint(&mut self) -> Result<bool, GpuError> {
        self.log(Call::TimestampsDisjoint);
        if !self.caps.has(Features::TIMESTAMPS) {
            return Err(GpuError::Absent(Features::TIMESTAMPS));
        }
        Ok(false)
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
