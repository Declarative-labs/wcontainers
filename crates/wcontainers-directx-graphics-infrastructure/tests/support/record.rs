//! A test back end that draws nothing and records every base call as a line of text, so tests can
//! check what a front asked for. Read backs answer zeros. Used until the interface track's recording
//! back end is on the shared branch.

use wcontainers_gpu::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id(pub u32);

pub struct Recorder {
    pub calls: Vec<String>,
    caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<Recorder>,
    next: u32,
    fences: u32,
}

/// Capabilities like WebGPU's in Chrome on a desktop GPU, as far as the tests need them.
pub fn webgpu_like_caps() -> Caps {
    let mut uses = [FormatUse::empty(); Format::COUNT];
    let colour = FormatUse::SAMPLE
        | FormatUse::FILTER
        | FormatUse::RENDER
        | FormatUse::BLEND
        | FormatUse::RESOLVE
        | FormatUse::COPY_OUT;
    for f in [
        Format::Rgba8Unorm,
        Format::Rgba8UnormSrgb,
        Format::Bgra8Unorm,
        Format::Bgra8UnormSrgb,
        Format::Rgba16Float,
        Format::Rgb10a2Unorm,
        Format::R32Float,
    ] {
        uses[f.index()] = colour;
    }
    for f in [Format::Rgba8Unorm, Format::Bgra8Unorm, Format::Rgba16Float] {
        uses[f.index()] |= FormatUse::PRESENT;
    }
    for f in [
        Format::Depth32Float,
        Format::Depth24PlusStencil8,
        Format::Depth32FloatStencil8,
    ] {
        uses[f.index()] = FormatUse::SAMPLE | FormatUse::DEPTH_STENCIL;
    }
    uses[Format::R32Uint.index()] =
        FormatUse::SAMPLE | FormatUse::RENDER | FormatUse::STORAGE_WRITE;
    Caps {
        adapter: AdapterInfo {
            name: "recording back end".into(),
            vendor: 0x10de,
            device: 0x2206,
            software: false,
        },
        features: Features::COMPUTE
            | Features::INDEPENDENT_BLEND
            | Features::INDIRECT
            | Features::TIMESTAMPS
            | Features::TEXTURE_VIEWS,
        limits: Limits {
            max_texture_2d: 16384,
            max_texture_cube: 16384,
            max_texture_3d: 2048,
            max_texture_layers: 2048,
            max_buffer_size: 1 << 32,
            max_uniform_buffer_size: 65536,
            max_storage_buffer_size: 1 << 31,
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
        },
        formats: FormatCaps {
            uses,
            samples: [0b101; Format::COUNT],
        },
    }
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recorder")
    }
}

impl Recorder {
    pub fn new() -> Self {
        Recorder {
            calls: Vec::new(),
            caps: webgpu_like_caps(),
            slow: SlowPaths::default(),
            helpers: Helpers::default(),
            next: 0,
            fences: 0,
        }
    }

    fn id(&mut self, what: impl std::fmt::Debug) -> Result<Id, GpuError> {
        self.next += 1;
        self.calls.push(format!("create {} {what:?}", self.next));
        Ok(Id(self.next))
    }

    /// The recorded calls whose text starts with `prefix`.
    pub fn named(&self, prefix: &str) -> Vec<&str> {
        self.calls
            .iter()
            .filter(|c| c.starts_with(prefix))
            .map(String::as_str)
            .collect()
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
    type Fence = u32;
    type ResourceTable = Id;
    type Surface = Id;

    fn caps(&self) -> &Caps {
        &self.caps
    }
    fn slow_paths(&mut self) -> &mut SlowPaths {
        &mut self.slow
    }
    fn status(&mut self) -> DeviceStatus {
        DeviceStatus::Ok
    }
    fn helpers(&mut self) -> &mut Helpers<Self> {
        &mut self.helpers
    }
    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Id, GpuError> {
        self.id(("buffer", desc))
    }
    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Id, GpuError> {
        self.id(("texture", desc))
    }
    fn create_view(&mut self, texture: &Id, desc: &TextureViewDesc) -> Result<Id, GpuError> {
        self.id(("view", texture.0, desc))
    }
    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Id, GpuError> {
        self.id(("sampler", desc))
    }
    fn create_shader(&mut self, module: &ShaderModule) -> Result<Id, GpuError> {
        self.id(("shader", &module.entry_point, module.stage))
    }
    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<Id, GpuError> {
        self.id(("program", program))
    }
    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Id, GpuError> {
        self.id(("blend", desc))
    }
    fn create_depth_stencil(&mut self, desc: &DepthStencilDesc) -> Result<Id, GpuError> {
        self.id(("depth_stencil", desc))
    }
    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Id, GpuError> {
        self.id(("raster", desc))
    }
    fn create_vertex_layout(&mut self, desc: &VertexLayoutDesc) -> Result<Id, GpuError> {
        let streams: Vec<_> = desc.streams.iter().flatten().collect();
        let attributes: Vec<_> = desc.attributes.iter().flatten().collect();
        self.id(("vertex_layout", streams, attributes))
    }
    fn create_binding_set(&mut self, entries: &[BindingEntry<'_, Self>]) -> Result<Id, GpuError> {
        self.id(("binding_set", entries))
    }
    fn binding_layout_key(&self, _set: &Id) -> BindingLayoutKey {
        BindingLayoutKey {
            unfilterable_textures: 0,
        }
    }
    fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<Id, GpuError> {
        self.id(("query_set", kind, count))
    }
    fn destroy(&mut self, object: Object<Self>) {
        self.calls.push(format!("destroy {object:?}"));
    }
    fn write_buffer(&mut self, buffer: &Id, offset: u64, data: &[u8], mode: WriteMode) {
        self.calls.push(format!(
            "write_buffer {} {offset} {} {mode:?} {:?}",
            buffer.0,
            data.len(),
            &data[..data.len().min(16)]
        ));
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
        self.calls.push(format!(
            "write_texture {region:?} {extent:?} {} {row_pitch} {rows_per_layer} {mode:?}",
            data.len()
        ));
    }
    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        self.calls.push(format!("begin_pass {pass:?}"));
    }
    fn end_pass(&mut self) {
        self.calls.push("end_pass".into());
    }
    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        self.calls.push(format!("draw {call:?}"));
    }
    fn begin_query(&mut self, index: u32) {
        self.calls.push(format!("begin_query {index}"));
    }
    fn end_query(&mut self) {
        self.calls.push("end_query".into());
    }
    fn copy_buffer(&mut self, src: &Id, src_offset: u64, dst: &Id, dst_offset: u64, size: u64) {
        self.calls.push(format!(
            "copy_buffer {} {src_offset} {} {dst_offset} {size}",
            src.0, dst.0
        ));
    }
    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.calls
            .push(format!("copy_texture {src:?} {dst:?} {extent:?}"));
    }
    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.calls
            .push(format!("copy_buffer_to_texture {src:?} {dst:?} {extent:?}"));
    }
    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        self.calls
            .push(format!("copy_texture_to_buffer {src:?} {dst:?} {extent:?}"));
    }
    fn clear_buffer(&mut self, buffer: &Id, offset: u64, size: u64, pattern: &[u8]) {
        self.calls.push(format!(
            "clear_buffer {} {offset} {size} {pattern:?}",
            buffer.0
        ));
    }
    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) {
        self.calls
            .push(format!("resolve {src:?} {dst:?} {format:?}"));
    }
    fn submit(&mut self) -> u32 {
        self.fences += 1;
        self.calls.push(format!("submit {}", self.fences));
        self.fences
    }
    fn fence_done(&mut self, _fence: &u32) -> bool {
        true
    }
    fn wait_fence(&mut self, fence: &u32) {
        self.calls.push(format!("wait_fence {fence}"));
    }
    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Id, GpuError> {
        self.id(("read_back", src))
    }
    fn read_queries(&mut self, set: &Id, first: u32, count: u32) -> Result<Id, GpuError> {
        self.id(("read_queries", set.0, first, count))
    }
    fn read_state(&mut self, _readback: &Id) -> ReadState {
        ReadState::Ready
    }
    fn wait_read(&mut self, readback: &Id) {
        self.calls.push(format!("wait_read {}", readback.0));
    }
    fn finish_read(&mut self, readback: Id, dst: &mut [u8]) {
        self.calls
            .push(format!("finish_read {} {}", readback.0, dst.len()));
        dst.fill(0);
    }
    fn create_surface(&mut self, window: u32) -> Result<Id, GpuError> {
        self.id(("surface", window))
    }
    fn configure_surface(
        &mut self,
        surface: &Id,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError> {
        self.calls.push(format!(
            "configure_surface {} {format:?} {width} {height} {vsync}",
            surface.0
        ));
        Ok(())
    }
    fn current_texture(&mut self, surface: &Id) -> Result<Id, GpuError> {
        self.id(("current_texture", surface.0))
    }
    fn present(&mut self, surface: &Id) -> Result<(), GpuError> {
        self.calls.push(format!("present {}", surface.0));
        Ok(())
    }
}
