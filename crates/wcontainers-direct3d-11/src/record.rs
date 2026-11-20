//! A back end that draws nothing: it writes every call into a log, and keeps the bytes host writes and
//! copies put into buffers and textures, so read backs return them. Draws and clears change no bytes.
//! It runs the front without a GPU, for checking the calls the front makes.

use std::collections::HashMap;
use std::fmt::Write;

use wcontainers_gpu::*;

use crate::format::row_bytes;

/// The recording back end's handles: a number per object.
pub type Id = u32;

struct Texture {
    desc: TextureDesc,
    /// Per subresource (mip + layer * mips), tightly packed rows of blocks.
    subresources: Vec<Vec<u8>>,
}

/// The refusal of a texture of more than one cube where [`Recorder::cube_arrays`] is cleared.
const NO_CUBE_ARRAYS: &str = "recorder: no cube arrays";

pub struct Recorder {
    /// What it declares; tests take uses away to reach a front's slow paths.
    pub caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<Recorder>,
    next: Id,
    /// One line per call.
    pub log: Vec<String>,
    /// Where each line is also written as it is recorded, if anywhere.
    echo: Option<fn(&str)>,
    buffers: HashMap<Id, Vec<u8>>,
    textures: HashMap<Id, Texture>,
    views: HashMap<Id, (Id, TextureViewDesc)>,
    reads: HashMap<Id, Vec<u8>>,
    /// Set to lose the device: what `status` reports.
    pub lost: Option<LossReason>,
    /// Set to make read backs pending until a submit (or a wait) after them, as on a GPU.
    pub reads_need_submit: bool,
    /// Cleared to refuse textures of more than one cube, as WebGL 2 does.
    pub cube_arrays: bool,
    /// The last id submitted.
    submitted: Id,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recorder({} calls)", self.log.len())
    }
}

impl Recorder {
    /// The bytes buffer `id` holds now.
    pub fn buffer_bytes(&self, id: Id) -> Option<&[u8]> {
        self.buffers.get(&id).map(Vec::as_slice)
    }

    /// A recorder declaring the abilities WebGPU in Chrome declares on the development desktop (plan
    /// 4.11), with every format able to do everything; `echo` also receives each line.
    pub fn new(echo: Option<fn(&str)>) -> Self {
        let limits = Limits {
            max_texture_2d: 16384,
            max_texture_cube: 16384,
            max_texture_3d: 2048,
            max_texture_layers: 2048,
            max_buffer_size: 1 << 30,
            max_uniform_buffer_size: 1 << 16,
            max_storage_buffer_size: 1 << 30,
            uniform_buffers_per_stage: 12,
            textures_per_stage: 48,
            samplers_per_stage: 16,
            storage_buffers_per_stage: 10,
            storage_textures_per_stage: 8,
            vertex_streams: 8,
            vertex_attributes: 30,
            max_vertex_stride: 2048,
            color_targets: 8,
            color_bytes_per_sample: 128,
            inter_stage_slots: 28,
            min_uniform_offset_alignment: 256,
            min_storage_offset_alignment: 256,
            buffer_copy_alignment: 4,
            immediate_bytes: 64,
            compute_workgroup_storage: 32768,
            compute_invocations: 1024,
            compute_size: [1024, 1024, 64],
            max_dispatch: 65535,
            max_point_size: 1.0,
            max_line_width: 1.0,
            max_anisotropy: 16,
            capture_buffers: 0,
            capture_components: 0,
        };
        Recorder {
            caps: Caps {
                features: Features::COMPUTE
                    | Features::INDEPENDENT_BLEND
                    | Features::DEPTH_CLAMP
                    | Features::BASE_VERTEX_INSTANCE
                    | Features::TEXTURE_VIEWS
                    | Features::FRACTIONAL_VIEWPORT
                    | Features::SAMPLE_MASK
                    | Features::TEXTURE_LEVEL_QUERY,
                limits,
                formats: FormatCaps {
                    uses: [FormatUse::all(); Format::COUNT],
                    samples: [0b101; Format::COUNT],
                },
                adapter: AdapterInfo {
                    name: "wcontainers recording back end (draws nothing)".into(),
                    ..Default::default()
                },
            },
            slow: SlowPaths::default(),
            helpers: Helpers::default(),
            next: 0,
            log: Vec::new(),
            echo,
            buffers: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new(),
            reads: HashMap::new(),
            lost: None,
            reads_need_submit: false,
            cube_arrays: true,
            submitted: 0,
        }
    }

    fn id(&mut self) -> Id {
        self.next += 1;
        self.next
    }

    fn record(&mut self, line: String) {
        if let Some(echo) = self.echo {
            echo(&line);
        }
        self.log.push(line);
    }

    /// Byte ranges of each block row a region of `extent` at `origin` of `mip` covers, in the
    /// subresource's tight storage: (offset in storage, bytes per row, row count).
    fn rows(t: &Texture, mip: u32, origin: Origin, extent: Extent) -> (usize, usize, usize, usize) {
        let (_, bh, _) = t.desc.format.block();
        let width = (t.desc.width >> mip).max(1);
        let (pitch, _) = row_bytes(t.desc.format, width, 1);
        let (row, rows) = row_bytes(t.desc.format, extent.width, extent.height);
        let (x, _) = row_bytes(t.desc.format, origin.x, 1);
        let start = (origin.y / bh) as usize * pitch as usize + x as usize;
        (start, pitch as usize, row as usize, rows as usize)
    }

    /// For a format with depth and stencil, which the recorder keeps in Direct3D's layout, the aspect
    /// a region names: bytes per stored texel, the aspect's bytes within one, and its bytes per texel
    /// as the interface hands it over (depth in four, stencil in one).
    fn aspect(t: &Texture, stencil: bool) -> Option<(usize, core::ops::Range<usize>, usize)> {
        let texel = t.desc.format.block().2 as usize;
        let aspect = match (texel, stencil) {
            (4, false) => (4, 0..3, 4),
            (4, true) => (4, 3..4, 1),
            (_, false) => (texel, 0..4, 4),
            (_, true) => (texel, 4..5, 1),
        };
        (t.desc.format.kind() == FormatKind::DepthStencil).then_some(aspect)
    }

    fn subresource(t: &Texture, mip: u32, layer: u32) -> usize {
        (mip + layer * t.desc.mip_levels) as usize
    }
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
        let id = self.id();
        self.buffers.insert(id, vec![0; desc.size as usize]);
        self.record(format!(
            "create_buffer {id} size {} {:?} {:?}",
            desc.size, desc.usage, desc.intent
        ));
        Ok(id)
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Id, GpuError> {
        if !self.cube_arrays && desc.own_view() == ViewDimension::CubeArray {
            return Err(GpuError::Invalid(NO_CUBE_ARRAYS));
        }
        let id = self.id();
        let mut subresources = Vec::new();
        for _layer in 0..desc.depth_or_layers {
            for mip in 0..desc.mip_levels {
                let (w, h) = ((desc.width >> mip).max(1), (desc.height >> mip).max(1));
                let (row, rows) = row_bytes(desc.format, w, h);
                subresources.push(vec![0; (row * rows) as usize]);
            }
        }
        self.textures.insert(
            id,
            Texture {
                desc: *desc,
                subresources,
            },
        );
        self.record(format!(
            "create_texture {id} {:?} {}x{}x{} mips {} {:?}",
            desc.format, desc.width, desc.height, desc.depth_or_layers, desc.mip_levels, desc.usage
        ));
        Ok(id)
    }

    fn create_view(&mut self, texture: &Id, desc: &TextureViewDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.views.insert(id, (*texture, *desc));
        self.record(format!(
            "create_view {id} of {texture} {:?} {:?} mips {}+{} layers {}+{}{}",
            desc.format,
            desc.dimension,
            desc.base_mip,
            desc.mip_count,
            desc.base_layer,
            desc.layer_count,
            if desc.stencil { " stencil" } else { "" }
        ));
        Ok(id)
    }

    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.record(format!(
            "create_sampler {id} {:?} {:?} {:?} {:?} lod {}..{} compare {:?}",
            desc.min,
            desc.mag,
            desc.mip,
            desc.address,
            f32::from_bits(desc.lod_range[0]),
            f32::from_bits(desc.lod_range[1]),
            desc.compare
        ));
        Ok(id)
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<Id, GpuError> {
        let id = self.id();
        self.record(format!(
            "create_shader {id} {:?} {}",
            module.stage, module.entry_point
        ));
        Ok(id)
    }

    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<Id, GpuError> {
        let id = self.id();
        let line = match program {
            Program::Render { vertex, fragment } => {
                format!("create_program {id} vertex {vertex} fragment {fragment:?}")
            }
            Program::Compute(c) => format!("create_program {id} compute {c}"),
        };
        self.record(line);
        Ok(id)
    }

    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.record(format!(
            "create_blend {id} {:?} same {}",
            desc.targets[0], desc.same_for_all
        ));
        Ok(id)
    }

    fn create_depth_stencil(&mut self, desc: &DepthStencilDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.record(format!(
            "create_depth_stencil {id} {:?} write {} stencil {}",
            desc.depth_compare,
            desc.depth_write,
            desc.stencil.is_some()
        ));
        Ok(id)
    }

    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Id, GpuError> {
        let id = self.id();
        self.record(format!(
            "create_raster {id} {:?} {:?} {:?} clip {} mask {:#x} bias {} {} {}",
            desc.fill,
            desc.cull,
            desc.front,
            desc.depth_clip,
            desc.sample_mask,
            f32::from_bits(desc.depth_bias),
            f32::from_bits(desc.depth_bias_slope),
            f32::from_bits(desc.depth_bias_clamp)
        ));
        Ok(id)
    }

    fn create_vertex_layout(&mut self, desc: &VertexLayoutDesc) -> Result<Id, GpuError> {
        let id = self.id();
        let mut line = format!("create_vertex_layout {id}");
        for (i, s) in desc.streams.iter().enumerate() {
            if let Some(s) = s {
                let _ = write!(line, " stream {i} stride {} {:?}", s.stride, s.step);
            }
        }
        for a in desc.attributes.iter().flatten() {
            let _ = write!(
                line,
                " attribute {} at {}+{} {:?}",
                a.location, a.stream, a.offset, a.format
            );
        }
        self.record(line);
        Ok(id)
    }

    fn create_binding_set(&mut self, entries: &[BindingEntry<'_, Self>]) -> Result<Id, GpuError> {
        let id = self.id();
        let mut line = format!("create_binding_set {id}");
        for e in entries {
            let what = match &e.resource {
                BindingResource::UniformBuffer(r) => {
                    format!("uniform {}+{}:{}", r.buffer, r.offset, r.size)
                }
                BindingResource::Texture(v) => format!("texture {v}"),
                BindingResource::Sampler(s) => format!("sampler {s}"),
                other => format!("{other:?}"),
            };
            let _ = write!(line, " {}={what}", e.binding);
        }
        self.record(line);
        Ok(id)
    }

    fn binding_layout_key(&self, _set: &Id) -> BindingLayoutKey {
        BindingLayoutKey {
            unfilterable_textures: 0,
        }
    }

    fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<Id, GpuError> {
        let id = self.id();
        self.record(format!("create_query_set {id} {kind:?} {count}"));
        Ok(id)
    }

    fn destroy(&mut self, object: Object<Self>) {
        if let Object::Buffer(id) = object {
            self.buffers.remove(&id);
        }
        if let Object::Texture(id) = object {
            self.textures.remove(&id);
        }
        self.record(format!("destroy {object:?}"));
    }

    fn write_buffer(&mut self, buffer: &Id, offset: u64, data: &[u8], mode: WriteMode) {
        if let Some(b) = self.buffers.get_mut(buffer) {
            b[offset as usize..offset as usize + data.len()].copy_from_slice(data);
        }
        self.record(format!(
            "write_buffer {buffer} at {offset} {} bytes {mode:?}",
            data.len()
        ));
    }

    fn write_texture(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        _rows_per_layer: u32,
        mode: WriteMode,
    ) {
        if let Some(t) = self.textures.get_mut(region.texture) {
            let (start, pitch, row, rows) = Self::rows(t, region.mip, region.origin, extent);
            let sub = Self::subresource(t, region.mip, region.layer);
            let aspect = Self::aspect(t, region.stencil);
            for r in 0..rows {
                let src = r * row_pitch as usize;
                let dst = start + r * pitch;
                let out = &mut t.subresources[sub][dst..dst + row];
                match &aspect {
                    None => out.copy_from_slice(&data[src..src + row]),
                    Some((texel, part, given)) => {
                        for (i, o) in out.chunks_exact_mut(*texel).enumerate() {
                            let at = src + i * given;
                            o[part.clone()].copy_from_slice(&data[at..at + part.len()]);
                        }
                    }
                }
            }
        }
        self.record(format!(
            "write_texture {} mip {} layer {} at {},{} {}x{} pitch {row_pitch} {mode:?}",
            region.texture,
            region.mip,
            region.layer,
            region.origin.x,
            region.origin.y,
            extent.width,
            extent.height
        ));
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        let mut line = String::from("begin_pass");
        for (i, c) in pass.colors.iter().enumerate() {
            if let Some(c) = c {
                let _ = write!(
                    line,
                    " color {i} view {} {:?} {:?}",
                    c.view, c.load, c.store
                );
            }
        }
        if let Some(d) = &pass.depth {
            let _ = write!(
                line,
                " depth view {} {:?} stencil {:?}",
                d.view,
                d.depth.as_ref().map(|a| a.load),
                d.stencil.as_ref().map(|a| a.load)
            );
            for (name, read_only) in [
                ("depth", d.depth.as_ref().is_some_and(|a| a.read_only)),
                ("stencil", d.stencil.as_ref().is_some_and(|a| a.read_only)),
            ] {
                if read_only {
                    let _ = write!(line, " {name} read-only");
                }
            }
        }
        if let Some(e) = &pass.empty {
            let _ = write!(
                line,
                " empty {}x{} samples {}",
                e.width, e.height, e.samples
            );
        }
        self.record(line);
    }

    fn end_pass(&mut self) {
        self.record("end_pass".into());
    }

    fn prepare(&mut self, desc: &PrepareDesc<'_, Self>) {
        self.record(format!(
            "prepare program {} blend {} depth {} raster {} layout {} {:?} {:?}",
            desc.program,
            desc.blend,
            desc.depth_stencil,
            desc.raster,
            desc.vertex_layout,
            desc.targets,
            desc.topology,
        ));
    }

    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        let streams: Vec<_> = call
            .vertex_buffers
            .iter()
            .map(|v| v.as_ref().map(|v| (*v.buffer, v.offset)))
            .collect();
        let index = call.index.as_ref().map(|i| (*i.buffer, i.offset, i.format));
        let viewports: Vec<_> = call
            .viewports
            .iter()
            .map(|v| [v.x, v.y, v.width, v.height].map(f32::from_bits))
            .collect();
        let scissors: Vec<_> = call
            .scissors
            .iter()
            .map(|s| [s.x, s.y, s.width, s.height])
            .collect();
        self.record(format!(
            "draw program {} blend {} depth {} raster {} layout {} sets {:?} streams {streams:?} index {index:?} {:?} restart {} viewports {viewports:?} scissors {scissors:?} {:?}",
            call.program,
            call.blend,
            call.depth_stencil,
            call.raster,
            call.vertex_layout,
            call.bindings,
            call.topology,
            call.restart,
            call.range
        ));
    }

    fn dispatch(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
        let groups = match call.groups {
            DispatchGroups::Direct(g) => format!("{g:?}"),
            DispatchGroups::Indirect { args, offset } => format!("indirect {args} {offset}"),
        };
        self.record(format!(
            "dispatch program {} sets {:?} groups {groups}",
            call.program, call.bindings
        ));
        Ok(())
    }

    fn begin_query(&mut self, index: u32) {
        self.record(format!("begin_query {index}"));
    }

    fn end_query(&mut self) {
        self.record("end_query".into());
    }

    fn copy_buffer(&mut self, src: &Id, src_offset: u64, dst: &Id, dst_offset: u64, size: u64) {
        let (s, d, n) = (src_offset as usize, dst_offset as usize, size as usize);
        if let Some(bytes) = self.buffers.get(src).map(|b| b[s..s + n].to_vec())
            && let Some(b) = self.buffers.get_mut(dst)
        {
            b[d..d + n].copy_from_slice(&bytes);
        }
        self.record(format!(
            "copy_buffer {src}+{src_offset} to {dst}+{dst_offset} {size} bytes"
        ));
    }

    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        assert!(
            !src.stencil && !dst.stencil,
            "copy_texture copies both aspects and names none"
        );
        let rows = self.textures.get(src.texture).map(|t| {
            let (start, pitch, row, rows) = Self::rows(t, src.mip, src.origin, extent);
            let sub = &t.subresources[Self::subresource(t, src.mip, src.layer)];
            (0..rows)
                .map(|r| sub[start + r * pitch..start + r * pitch + row].to_vec())
                .collect::<Vec<_>>()
        });
        if let (Some(rows), Some(t)) = (rows, self.textures.get_mut(dst.texture)) {
            let (start, pitch, row, _) = Self::rows(t, dst.mip, dst.origin, extent);
            let sub = Self::subresource(t, dst.mip, dst.layer);
            for (r, bytes) in rows.iter().enumerate() {
                t.subresources[sub][start + r * pitch..start + r * pitch + row]
                    .copy_from_slice(bytes);
            }
        }
        self.record(format!(
            "copy_texture {} mip {} layer {} at {},{} to {} mip {} layer {} at {},{} {}x{}",
            src.texture,
            src.mip,
            src.layer,
            src.origin.x,
            src.origin.y,
            dst.texture,
            dst.mip,
            dst.layer,
            dst.origin.x,
            dst.origin.y,
            extent.width,
            extent.height
        ));
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.record(format!(
            "copy_buffer_to_texture {} to {} {}x{}",
            src.buffer, dst.texture, extent.width, extent.height
        ));
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        self.record(format!(
            "copy_texture_to_buffer {} to {} {}x{}",
            src.texture, dst.buffer, extent.width, extent.height
        ));
    }

    fn clear_buffer(&mut self, buffer: &Id, offset: u64, size: u64, pattern: &[u8]) {
        self.record(format!(
            "clear_buffer {buffer} at {offset} {size} bytes pattern {pattern:?}"
        ));
    }

    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) {
        self.record(format!(
            "resolve {} to {} as {format:?}",
            src.texture, dst.texture
        ));
    }

    fn submit(&mut self) -> Id {
        let id = self.id();
        self.record(format!("submit {id}"));
        self.submitted = id;
        id
    }

    fn fence_done(&mut self, _fence: &Id) -> bool {
        true
    }

    fn wait_fence(&mut self, _fence: &Id) {}

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Id, GpuError> {
        let id = self.id();
        let (bytes, line) = match src {
            ReadSource::Buffer {
                buffer,
                offset,
                size,
            } => {
                let b = &self.buffers[buffer];
                (
                    b[*offset as usize..(*offset + *size) as usize].to_vec(),
                    format!("read_back {id} buffer {buffer}+{offset} {size} bytes"),
                )
            }
            ReadSource::Texture { region, extent } => {
                let t = &self.textures[region.texture];
                let (start, pitch, row, rows) = Self::rows(t, region.mip, region.origin, *extent);
                let sub = &t.subresources[Self::subresource(t, region.mip, region.layer)];
                let rows = (0..rows).map(|r| &sub[start + r * pitch..start + r * pitch + row]);
                let bytes = match Self::aspect(t, region.stencil) {
                    None => rows.flatten().copied().collect(),
                    Some((texel, part, given)) => rows
                        .flat_map(|r| r.chunks_exact(texel))
                        .flat_map(|c| {
                            let mut out = [0u8; 4];
                            out[..part.len()].copy_from_slice(&c[part.clone()]);
                            out.into_iter().take(given)
                        })
                        .collect(),
                };
                (
                    bytes,
                    format!(
                        "read_back {id} texture {} mip {} layer {} {}x{}",
                        region.texture, region.mip, region.layer, extent.width, extent.height
                    ),
                )
            }
        };
        self.reads.insert(id, bytes);
        self.record(line);
        Ok(id)
    }

    fn read_queries(&mut self, set: &Id, first: u32, count: u32) -> Result<Id, GpuError> {
        let id = self.id();
        self.reads.insert(id, vec![0; count as usize * 8]);
        self.record(format!("read_queries {id} of {set} {first}+{count}"));
        Ok(id)
    }

    fn read_state(&mut self, readback: &Id) -> ReadState {
        if self.reads_need_submit && *readback > self.submitted {
            ReadState::Pending
        } else {
            ReadState::Ready
        }
    }

    fn wait_read(&mut self, readback: &Id) {
        self.record(format!("wait_read {readback}"));
        self.submitted = self.submitted.max(*readback);
    }

    fn finish_read(&mut self, readback: Id, dst: &mut [u8]) {
        if let Some(bytes) = self.reads.remove(&readback) {
            let n = bytes.len().min(dst.len());
            dst[..n].copy_from_slice(&bytes[..n]);
        }
    }

    fn create_surface(&mut self, window: u32) -> Result<Id, GpuError> {
        let id = self.id();
        self.record(format!("create_surface {id} window {window}"));
        Ok(id)
    }

    fn configure_surface(
        &mut self,
        surface: &Id,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError> {
        self.record(format!(
            "configure_surface {surface} {format:?} {width}x{height} vsync {vsync}"
        ));
        Ok(())
    }

    fn current_texture(&mut self, surface: &Id) -> Result<Id, GpuError> {
        let id = self.id();
        self.record(format!("current_texture {surface} view {id}"));
        Ok(id)
    }

    fn present(&mut self, surface: &Id) -> Result<(), GpuError> {
        self.record(format!("present {surface}"));
        Ok(())
    }
}
