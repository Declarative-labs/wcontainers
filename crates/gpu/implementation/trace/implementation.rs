//! A back end that records every call of the graphics interface into a stream file while it draws
//! through the back end it wraps ([`Traced`]). A real program's frames, recorded natively through a
//! front, are replayed in the browser on each back end to time them on the same calls.

mod stream;

use std::io::Write;
use stream::*;
pub use stream::{Frame, Replay};
use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, helpers::*, resource::*, shader::*, slow::*, state::*, *,
};

/// What a recording back end is made from: the config of the back end it wraps, and the file the
/// stream goes into (none: the calls run unrecorded).
#[derive(Debug)]
pub struct TracedConfig<C> {
    pub inner: C,
    pub file: Option<std::path::PathBuf>,
}

impl<B: StandaloneGpu> StandaloneGpu for Traced<B> {
    type Config = TracedConfig<B::Config>;
    /// Why the wrapped back end or the file could not be opened.
    type NewError = String;

    fn new(config: TracedConfig<B::Config>) -> Result<Self, String> {
        let inner = B::new(config.inner).map_err(|e| format!("{e:?}"))?;
        let file = match config.file {
            Some(path) => {
                Some(std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?)
            }
            None => None,
        };
        Ok(traced(inner, file))
    }
}

/// A recording back end over `inner`, writing into `file`.
fn traced<B: Gpu>(inner: B, file: Option<std::fs::File>) -> Traced<B> {
    Traced {
        inner,
        table: Table::default(),
        out: Out {
            file,
            frame: Vec::new(),
        },
        next: 0,
        scratch: Vec::new(),
        helpers: Helpers::default(),
    }
}

/// A back end that records every call into a stream file while running it on the back end it
/// wraps. Objects are the stream's numbers.
pub struct Traced<B: Gpu> {
    inner: B,
    table: Table<B>,
    out: Out,
    next: u32,
    scratch: Vec<u8>,
    helpers: Helpers<Traced<B>>,
}

impl<B: Gpu> core::fmt::Debug for Traced<B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Traced({} objects)", self.next)
    }
}

/// Where the calls go: the file, and the calls of the frame not yet written.
struct Out {
    /// None when the config names no file: the calls run unrecorded.
    file: Option<std::fs::File>,
    /// The calls since the last present, written whole at the next, so a program stopped
    /// mid-frame leaves whole frames.
    frame: Vec<u8>,
}

impl Out {
    fn write(&mut self, call: &Call) {
        if self.file.is_none() {
            return;
        }
        if let Err(e) = postcard::to_io(call, &mut self.frame) {
            eprintln!("gpu trace: {} not written: {e}", call.name());
        }
    }

    /// Writes the calls held so far into the file.
    fn flush(&mut self) {
        if let Some(file) = &mut self.file
            && let Err(e) = file.write_all(&self.frame)
        {
            eprintln!("gpu trace: a frame not written: {e}");
        }
        self.frame.clear();
    }
}

impl Drop for Out {
    fn drop(&mut self) {
        self.flush();
    }
}

fn id<B: Gpu>(t: &mut Traced<B>) -> H {
    t.next += 1;
    H(t.next)
}

/// Records the call and runs it.
fn run<B: Gpu>(t: &mut Traced<B>, call: Call) -> Result<Answer, GpuError> {
    t.out.write(&call);
    let mut timer = Timer {
        clock: None,
        spent: 0.0,
    };
    apply(
        &mut t.inner,
        &mut t.table,
        &call,
        &mut t.scratch,
        &mut timer,
    )
}

/// Records a call that makes an object and answers its number.
fn make<B: Gpu>(t: &mut Traced<B>, f: impl FnOnce(H) -> Call) -> Result<H, GpuError> {
    let h = id(t);
    run(t, f(h)).map(|_| h)
}

fn void<B: Gpu>(t: &mut Traced<B>, call: Call) {
    if let Err(e) = run(t, call) {
        eprintln!("gpu trace: a call the stream holds failed: {e:?}");
    }
}

fn region(r: &TextureRegion<'_, Traced<impl Gpu>>) -> Region {
    Region {
        texture: *r.texture,
        mip: r.mip,
        layer: r.layer,
        origin: r.origin,
        stencil: r.stencil,
    }
}

fn side(c: &CopyBufferTexture<'_, Traced<impl Gpu>>) -> Side {
    Side(*c.buffer, c.offset, c.row_pitch, c.rows_per_layer)
}

fn source(s: &ReadSource<'_, Traced<impl Gpu>>) -> Source {
    match s {
        ReadSource::Buffer {
            buffer,
            offset,
            size,
        } => Source::Buffer(**buffer, *offset, *size),
        ReadSource::Texture { region: r, extent } => Source::Texture(region(r), *extent),
    }
}

fn sets(s: &[Option<&H>; MAX_BINDING_SETS]) -> [Option<H>; MAX_BINDING_SETS] {
    s.map(|s| s.copied())
}

fn draw(c: &DrawCall<'_, Traced<impl Gpu>>) -> Box<Draw> {
    Box::new(Draw {
        program: *c.program,
        blend: *c.blend,
        depth_stencil: *c.depth_stencil,
        raster: *c.raster,
        vertex_layout: *c.vertex_layout,
        bindings: sets(&c.bindings),
        vertex_buffers: c
            .vertex_buffers
            .iter()
            .map(|v| v.as_ref().map(|v| (*v.buffer, v.offset)))
            .collect(),
        index: c.index.as_ref().map(|i| (*i.buffer, i.offset, i.format)),
        topology: c.topology,
        restart: c.restart,
        viewports: c.viewports.to_vec(),
        scissors: c.scissors.to_vec(),
        blend_constant: c.blend_constant,
        stencil_reference: c.stencil_reference,
        immediates: c.immediates.to_vec(),
        range: match c.range {
            DrawRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            } => Range::Vertices([first, count, first_instance, instances]),
            DrawRange::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            } => Range::Indexed(first, count, base_vertex, first_instance, instances),
            DrawRange::Indirect { args, offset } => Range::Indirect(*args, offset),
            DrawRange::IndexedIndirect { args, offset } => Range::IndexedIndirect(*args, offset),
        },
    })
}

const NO_TABLES: GpuError = GpuError::Invalid("trace: a traced device keeps no resource tables");

fn entries_of(entries: &[BindingEntry<'_, Traced<impl Gpu>>]) -> Result<Vec<Entry>, GpuError> {
    entries
        .iter()
        .map(|e| {
            Ok(Entry {
                binding: e.binding,
                stages: e.stages,
                resource: match &e.resource {
                    BindingResource::UniformBuffer(r) => {
                        Resource::Uniform(*r.buffer, r.offset, r.size)
                    }
                    BindingResource::StorageBuffer { range: r, writable } => {
                        Resource::Storage(*r.buffer, r.offset, r.size, *writable)
                    }
                    BindingResource::Texture(v) => Resource::Texture(**v),
                    BindingResource::StorageTexture { view, writable } => {
                        Resource::StorageTexture(**view, *writable)
                    }
                    BindingResource::Sampler(s) => Resource::Sampler(**s),
                    BindingResource::TexelBuffer {
                        range: r,
                        format,
                        writable,
                    } => Resource::TexelBuffer(*r.buffer, r.offset, r.size, *format, *writable),
                    BindingResource::Table(_) => return Err(NO_TABLES),
                },
            })
        })
        .collect()
}

impl<B: Gpu> Gpu for Traced<B> {
    type Buffer = H;
    type Texture = H;
    type TextureView = H;
    type Sampler = H;
    type Shader = H;
    type Program = H;
    type Blend = H;
    type DepthStencil = H;
    type Raster = H;
    type VertexLayout = H;
    type BindingSet = H;
    type QuerySet = H;
    type Readback = H;
    type Fence = H;
    type ResourceTable = H;
    type Surface = H;

    fn caps(&self) -> &Caps {
        self.inner.caps()
    }

    fn slow_paths(&mut self) -> &mut SlowPaths {
        self.inner.slow_paths()
    }

    fn status(&mut self) -> DeviceStatus {
        self.inner.status()
    }

    fn helpers(&mut self) -> &mut Helpers<Self> {
        &mut self.helpers
    }

    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<H, GpuError> {
        make(self, |h| Call::CreateBuffer(h, *desc))
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<H, GpuError> {
        make(self, |h| Call::CreateTexture(h, *desc))
    }

    fn create_view(&mut self, texture: &H, desc: &TextureViewDesc) -> Result<H, GpuError> {
        make(self, |h| Call::CreateView(h, *texture, *desc))
    }

    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<H, GpuError> {
        make(self, |h| Call::CreateSampler(h, *desc))
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<H, GpuError> {
        let h = id(self);
        self.out
            .write(&Call::CreateShader(h, Box::new(Shader::of(module))));
        let s = self.inner.create_shader(module)?;
        self.table.shaders.insert(h, s);
        Ok(h)
    }

    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<H, GpuError> {
        let (first, fragment, compute) = match program {
            Program::Render { vertex, fragment } => (**vertex, fragment.copied(), false),
            Program::Compute(s) => (**s, None, true),
        };
        make(self, |h| Call::CreateProgram(h, first, fragment, compute))
    }

    fn create_blend(&mut self, desc: &BlendDesc) -> Result<H, GpuError> {
        make(self, |h| Call::CreateBlend(h, Box::new(*desc)))
    }

    fn create_depth_stencil(&mut self, desc: &DepthStencilDesc) -> Result<H, GpuError> {
        make(self, |h| Call::CreateDepthStencil(h, *desc))
    }

    fn create_raster(&mut self, desc: &RasterDesc) -> Result<H, GpuError> {
        make(self, |h| Call::CreateRaster(h, *desc))
    }

    fn create_vertex_layout(&mut self, desc: &VertexLayoutDesc) -> Result<H, GpuError> {
        make(self, |h| Call::CreateVertexLayout(h, Box::new(*desc)))
    }

    fn create_binding_set(&mut self, entries: &[BindingEntry<'_, Self>]) -> Result<H, GpuError> {
        let entries = entries_of(entries)?;
        make(self, |h| Call::CreateBindingSet(h, entries))
    }

    fn binding_layout_key(&self, set: &H) -> BindingLayoutKey {
        match self.table.sets.get(set) {
            Some(s) => self.inner.binding_layout_key(s),
            None => BindingLayoutKey::default(),
        }
    }

    fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<H, GpuError> {
        make(self, |h| Call::CreateQuerySet(h, kind, count))
    }

    fn destroy(&mut self, object: Object<Self>) {
        let (kind, h) = match object {
            Object::Buffer(h) => (Kind::Buffer, h),
            Object::Texture(h) => (Kind::Texture, h),
            Object::TextureView(h) => (Kind::TextureView, h),
            Object::Sampler(h) => (Kind::Sampler, h),
            Object::Shader(h) => (Kind::Shader, h),
            Object::Program(h) => (Kind::Program, h),
            Object::Blend(h) => (Kind::Blend, h),
            Object::DepthStencil(h) => (Kind::DepthStencil, h),
            Object::Raster(h) => (Kind::Raster, h),
            Object::VertexLayout(h) => (Kind::VertexLayout, h),
            Object::BindingSet(h) => (Kind::BindingSet, h),
            Object::QuerySet(h) => (Kind::QuerySet, h),
            Object::Surface(h) => (Kind::Surface, h),
            Object::ResourceTable(_) => return,
        };
        void(self, Call::Destroy(kind, h));
    }

    fn write_buffer(&mut self, buffer: &H, offset: u64, data: &[u8], mode: WriteMode) {
        void(
            self,
            Call::WriteBuffer(*buffer, offset, data.to_vec(), mode),
        );
    }

    fn write_texture(
        &mut self,
        r: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    ) {
        void(
            self,
            Call::WriteTexture(
                region(r),
                extent,
                data.to_vec(),
                row_pitch,
                rows_per_layer,
                mode,
            ),
        );
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        let colors = pass.colors.each_ref().map(|c| {
            c.as_ref().map(|c| Color {
                view: *c.view,
                depth_slice: c.depth_slice,
                resolve: c.resolve.copied(),
                load: c.load,
                store: c.store,
            })
        });
        void(
            self,
            Call::BeginPass(Box::new(Pass {
                colors,
                depth: pass.depth.as_ref().map(|d| Depth {
                    view: *d.view,
                    depth: d.depth,
                    stencil: d.stencil,
                }),
                empty: pass.empty,
                occlusion: pass.occlusion.copied(),
            })),
        );
    }

    fn end_pass(&mut self) {
        void(self, Call::EndPass);
    }

    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        void(self, Call::Draw(draw(call)));
    }

    fn begin_query(&mut self, index: u32) {
        void(self, Call::BeginQuery(index));
    }

    fn end_query(&mut self) {
        void(self, Call::EndQuery);
    }

    fn copy_buffer(&mut self, src: &H, src_offset: u64, dst: &H, dst_offset: u64, size: u64) {
        void(
            self,
            Call::CopyBuffer(*src, src_offset, *dst, dst_offset, size),
        );
    }

    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        void(self, Call::CopyTexture(region(src), region(dst), extent));
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        void(
            self,
            Call::CopyBufferToTexture(side(src), region(dst), extent),
        );
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        void(
            self,
            Call::CopyTextureToBuffer(region(src), side(dst), extent),
        );
    }

    fn clear_buffer(&mut self, buffer: &H, offset: u64, size: u64, pattern: &[u8]) {
        void(
            self,
            Call::ClearBuffer(*buffer, offset, size, pattern.to_vec()),
        );
    }

    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) {
        void(self, Call::Resolve(region(src), region(dst), format));
    }

    fn submit(&mut self) -> H {
        let h = id(self);
        void(self, Call::Submit(h));
        h
    }

    fn fence_done(&mut self, fence: &H) -> bool {
        matches!(run(self, Call::FenceDone(*fence)), Ok(Answer::Bool(true)))
    }

    fn wait_fence(&mut self, fence: &H) {
        void(self, Call::WaitFence(*fence));
    }

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<H, GpuError> {
        let s = source(src);
        make(self, |h| Call::ReadBack(h, s))
    }

    fn read_queries(&mut self, set: &H, first: u32, count: u32) -> Result<H, GpuError> {
        make(self, |h| Call::ReadQueries(h, *set, first, count))
    }

    fn read_state(&mut self, readback: &H) -> ReadState {
        match run(self, Call::ReadState(*readback)) {
            Ok(Answer::State(s)) => s,
            _ => ReadState::Ready,
        }
    }

    fn wait_read(&mut self, readback: &H) {
        void(self, Call::WaitRead(*readback));
    }

    fn finish_read(&mut self, readback: H, dst: &mut [u8]) {
        self.out
            .write(&Call::FinishRead(readback, dst.len() as u64));
        if let Some(r) = self.table.readbacks.remove(&readback) {
            self.inner.finish_read(r, dst);
        }
    }

    fn create_surface(&mut self, window: u32) -> Result<H, GpuError> {
        make(self, |h| Call::CreateSurface(h, window))
    }

    fn configure_surface(
        &mut self,
        surface: &H,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError> {
        run(
            self,
            Call::ConfigureSurface(*surface, format, width, height, vsync),
        )
        .map(|_| ())
    }

    fn current_texture(&mut self, surface: &H) -> Result<H, GpuError> {
        make(self, |h| Call::CurrentTexture(h, *surface))
    }

    fn present(&mut self, surface: &H) -> Result<(), GpuError> {
        let r = run(self, Call::Present(*surface)).map(|_| ());
        self.out.flush();
        r
    }

    fn read_back_now(
        &mut self,
        src: &ReadSource<'_, Self>,
        dst: &mut [u8],
    ) -> Result<(), GpuError> {
        let s = source(src);
        self.out.write(&Call::ReadBackNow(s, dst.len() as u64));
        let s = self.table.source(&s)?;
        self.inner.read_back_now(&s, dst)
    }

    fn draw_fan_or_loop(
        &mut self,
        call: &DrawCall<'_, Self>,
        kind: FanOrLoop,
        source: IndexSource<'_>,
        provoking: ProvokingVertex,
        first_instance: u32,
        instances: u32,
    ) -> Result<(), GpuError> {
        let indices = match source {
            IndexSource::Sequential { first, count } => Indices::Sequential(first, count),
            IndexSource::Guest {
                indices,
                format,
                base_vertex,
            } => Indices::Guest(indices.to_vec(), format, base_vertex),
        };
        run(
            self,
            Call::DrawFanOrLoop(
                draw(call),
                kind,
                indices,
                provoking,
                first_instance,
                instances,
            ),
        )
        .map(|_| ())
    }

    fn prepare(&mut self, desc: &PrepareDesc<'_, Self>) {
        void(
            self,
            Call::Prepare(Box::new(Prepare {
                program: *desc.program,
                blend: *desc.blend,
                depth_stencil: *desc.depth_stencil,
                raster: *desc.raster,
                vertex_layout: *desc.vertex_layout,
                targets: desc.targets,
                topology: desc.topology,
                layouts: desc.layouts,
            })),
        );
    }

    fn clear_texture(
        &mut self,
        texture: &H,
        desc: &TextureDesc,
        format: Format,
        mips: std::ops::Range<u32>,
        layers: std::ops::Range<u32>,
        value: ClearValue,
    ) -> Result<(), GpuError> {
        run(
            self,
            Call::ClearTexture(
                *texture,
                *desc,
                format,
                [mips.start, mips.end],
                [layers.start, layers.end],
                value,
            ),
        )
        .map(|_| ())
    }

    fn clear_rects(
        &mut self,
        view: &H,
        format: Format,
        value: ClearValue,
        rects: &[ScissorRect],
    ) -> Result<(), GpuError> {
        run(self, Call::ClearRects(*view, format, value, rects.to_vec())).map(|_| ())
    }

    fn generate_mips(
        &mut self,
        texture: &H,
        desc: &TextureDesc,
        values: Format,
        mips: std::ops::Range<u32>,
        layers: std::ops::Range<u32>,
    ) -> Result<(), GpuError> {
        run(
            self,
            Call::GenerateMips(
                *texture,
                *desc,
                values,
                [mips.start, mips.end],
                [layers.start, layers.end],
            ),
        )
        .map(|_| ())
    }

    fn write_buffer_ring(
        &mut self,
        kind: RingKind,
        parts: &[&[u8]],
        align: u32,
        offsets: &mut [u64],
    ) -> Result<H, GpuError> {
        let h = id(self);
        let owned = parts.iter().map(|p| p.to_vec()).collect();
        self.out
            .write(&Call::WriteBufferRing(h, kind, owned, align));
        let b = self.inner.write_buffer_ring(kind, parts, align, offsets)?;
        self.table.buffers.insert(h, b);
        Ok(h)
    }

    fn resolve_queries(
        &mut self,
        set: &H,
        first: u32,
        count: u32,
        dst: &H,
        dst_offset: u64,
    ) -> Result<(), GpuError> {
        run(
            self,
            Call::ResolveQueries(*set, first, count, *dst, dst_offset),
        )
        .map(|_| ())
    }

    fn dispatch(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
        run(
            self,
            Call::Dispatch(Box::new(Dispatch {
                program: *call.program,
                bindings: sets(&call.bindings),
                immediates: call.immediates.to_vec(),
                groups: match call.groups {
                    DispatchGroups::Direct(g) => Ok(g),
                    DispatchGroups::Indirect { args, offset } => Err((*args, offset)),
                },
            })),
        )
        .map(|_| ())
    }

    fn capture_vertices(
        &mut self,
        call: &DrawCall<'_, Self>,
        targets: CaptureTargets<'_, Self>,
        rasterize: bool,
    ) -> Result<(), GpuError> {
        let targets = targets
            .iter()
            .map(|r| (*r.buffer, r.offset, r.size))
            .collect();
        run(self, Call::CaptureVertices(draw(call), targets, rasterize)).map(|_| ())
    }

    fn create_resource_table(&mut self, size: u32) -> Result<H, GpuError> {
        self.inner.create_resource_table(size)?;
        Err(NO_TABLES)
    }

    fn write_resource_table(
        &mut self,
        _table: &H,
        _first: u32,
        _entries: &[BindingEntry<'_, Self>],
    ) -> Result<(), GpuError> {
        Err(NO_TABLES)
    }

    fn write_timestamp(&mut self, set: &H, index: u32) -> Result<(), GpuError> {
        run(self, Call::WriteTimestamp(*set, index)).map(|_| ())
    }

    fn timestamps_disjoint(&mut self) -> Result<bool, GpuError> {
        run(self, Call::TimestampsDisjoint).map(|a| a == Answer::Bool(true))
    }
}
