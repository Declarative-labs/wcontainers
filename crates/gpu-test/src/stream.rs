//! The recording back end's call stream: every base call it received, with its arguments and its
//! answer, as plain data that [`replay`] runs again on a new recording back end.

use crate::recorder::{Id, Recorder};
use std::rc::Rc;
use wcontainers_gpu::{draw::*, error::*, format::*, resource::*, shader::*, state::*, *};

/// A shader in the stream, compared by its content key, entry point and stage.
#[derive(Clone)]
pub struct Shader(pub Rc<ShaderModule>);

impl PartialEq for Shader {
    fn eq(&self, other: &Self) -> bool {
        (self.0.key, &self.0.entry_point, self.0.stage)
            == (other.0.key, &other.0.entry_point, other.0.stage)
    }
}

impl core::fmt::Debug for Shader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Shader({:?} {})", self.0.stage, self.0.entry_point)
    }
}

/// A texture region: texture, level, layer, origin, stencil aspect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region(pub Id, pub u32, pub u32, pub Origin, pub bool);

impl Region {
    pub fn of(r: &TextureRegion<'_, Recorder>) -> Region {
        Region(*r.texture, r.mip, r.layer, r.origin, r.stencil)
    }
    pub fn get(&self) -> TextureRegion<'_, Recorder> {
        TextureRegion {
            texture: &self.0,
            mip: self.1,
            layer: self.2,
            origin: self.3,
            stencil: self.4,
        }
    }
}

/// The buffer side of a copy: buffer, offset, row pitch, rows per layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BufferSide(pub Id, pub u64, pub u32, pub u32);

impl BufferSide {
    pub fn of(c: &CopyBufferTexture<'_, Recorder>) -> BufferSide {
        BufferSide(*c.buffer, c.offset, c.row_pitch, c.rows_per_layer)
    }
    pub fn get(&self) -> CopyBufferTexture<'_, Recorder> {
        CopyBufferTexture {
            buffer: &self.0,
            offset: self.1,
            row_pitch: self.2,
            rows_per_layer: self.3,
        }
    }
}

/// What a binding entry binds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bind {
    Uniform(Id, u64, u64),
    Storage(Id, u64, u64, bool),
    Texture(Id),
    StorageTexture(Id, bool),
    Sampler(Id),
    TexelBuffer(Id, u64, u64, Format, bool),
    Table(Id),
}

/// A pass: colour attachments (view, slice, resolve, load, store), the depth attachment and its
/// aspects' operations, the size of an empty pass, the occlusion set.
#[derive(Clone, Debug, PartialEq)]
pub struct Pass {
    pub colors: Vec<Option<ColorOps>>,
    pub depth: Option<DepthOps>,
    pub empty: Option<EmptyPass>,
    pub occlusion: Option<Id>,
}

/// A colour attachment: view, slice, resolve target, load, store.
pub type ColorOps = (Id, u32, Option<Id>, LoadOp<[u32; 4]>, StoreOp);
/// A depth-stencil attachment: view, and each aspect's operations.
pub type DepthOps = (Id, Option<AspectOps<u32>>, Option<AspectOps<u8>>);

/// What a draw covers; indirect ones name their argument buffer and offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Range {
    Vertices(u32, u32, u32, u32),
    Indexed(u32, u32, i32, u32, u32),
    Indirect(Id, u64),
    IndexedIndirect(Id, u64),
}

/// A draw with everything it names.
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    pub blocks: [Id; 5],
    pub bindings: [Option<Id>; MAX_BINDING_SETS],
    pub vertex_buffers: Vec<Option<(Id, u64)>>,
    pub index: Option<(Id, u64, IndexFormat)>,
    pub topology: Topology,
    pub restart: bool,
    pub viewports: Vec<Viewport>,
    pub scissors: Vec<ScissorRect>,
    pub blend_constant: [u32; 4],
    pub stencil_reference: [u8; 2],
    pub immediates: Vec<u8>,
    pub range: Range,
}

/// A dispatch: program, binding sets, immediates, and its group counts or its argument buffer and
/// offset.
#[derive(Clone, Debug, PartialEq)]
pub struct Dispatch {
    pub program: Id,
    pub bindings: [Option<Id>; MAX_BINDING_SETS],
    pub immediates: Vec<u8>,
    pub groups: Result<[u32; 3], (Id, u64)>,
}

impl Dispatch {
    pub fn of(c: &DispatchCall<'_, Recorder>) -> Dispatch {
        Dispatch {
            program: *c.program,
            bindings: c.bindings.map(|b| b.copied()),
            immediates: c.immediates.to_vec(),
            groups: match c.groups {
                DispatchGroups::Direct(g) => Ok(g),
                DispatchGroups::Indirect { args, offset } => Err((*args, offset)),
            },
        }
    }
}

/// The kinds of [`Object`], for [`Call::Destroy`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Buffer,
    Texture,
    TextureView,
    Sampler,
    Shader,
    Program,
    Blend,
    DepthStencil,
    Raster,
    VertexLayout,
    BindingSet,
    QuerySet,
    ResourceTable,
    Surface,
}

type Made = Result<Id, GpuError>;

/// One base call and its answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    CreateBuffer(BufferDesc, Made),
    CreateTexture(TextureDesc, Made),
    CreateView(Id, TextureViewDesc, Made),
    CreateSampler(SamplerDesc, Made),
    CreateShader(Shader, Made),
    /// Vertex and fragment shaders, or a compute shader alone.
    CreateProgram(Id, Option<Id>, bool, Made),
    CreateBlend(Box<BlendDesc>, Made),
    CreateDepthStencil(DepthStencilDesc, Made),
    CreateRaster(RasterDesc, Made),
    CreateVertexLayout(Box<VertexLayoutDesc>, Made),
    CreateBindingSet(Vec<(u32, StageSet, Bind)>, Made),
    CreateQuerySet(QueryKind, u32, Made),
    Destroy(Kind, Id),
    WriteBuffer(Id, u64, Vec<u8>, WriteMode),
    WriteTexture(Region, Extent, Vec<u8>, u32, u32, WriteMode),
    BeginPass(Pass),
    EndPass,
    Draw(Box<Draw>),
    Dispatch(Box<Dispatch>, Result<(), GpuError>),
    /// The draw, its targets (buffer, offset, size) and whether it also rasterised.
    CaptureVertices(Box<Draw>, Vec<(Id, u64, u64)>, bool, Result<(), GpuError>),
    WriteTimestamp(Id, u32, Result<(), GpuError>),
    TimestampsDisjoint,
    BeginQuery(u32),
    EndQuery,
    CopyBuffer(Id, u64, Id, u64, u64),
    CopyTexture(Region, Region, Extent),
    CopyBufferToTexture(BufferSide, Region, Extent),
    CopyTextureToBuffer(Region, BufferSide, Extent),
    ClearBuffer(Id, u64, u64, Vec<u8>),
    Resolve(Region, Region, Format),
    Submit(Id),
    FenceDone(Id),
    WaitFence(Id),
    WaitRead(Id),
    /// A buffer range, or a texture region and extent.
    ReadBack(Result<(Id, u64, u64), (Region, Extent)>, Made),
    ReadQueries(Id, u32, u32, Made),
    /// The read back and the bytes it gave.
    FinishRead(Id, Vec<u8>),
    CreateSurface(u32, Made),
    ConfigureSurface(Id, Format, u32, u32, bool, Result<(), GpuError>),
    CurrentTexture(Id, Made),
    Present(Id, Result<(), GpuError>),
}

impl Call {
    /// The interface method the call was.
    pub fn name(&self) -> &'static str {
        match self {
            Call::CreateBuffer(..) => "create_buffer",
            Call::CreateTexture(..) => "create_texture",
            Call::CreateView(..) => "create_view",
            Call::CreateSampler(..) => "create_sampler",
            Call::CreateShader(..) => "create_shader",
            Call::CreateProgram(..) => "create_program",
            Call::CreateBlend(..) => "create_blend",
            Call::CreateDepthStencil(..) => "create_depth_stencil",
            Call::CreateRaster(..) => "create_raster",
            Call::CreateVertexLayout(..) => "create_vertex_layout",
            Call::CreateBindingSet(..) => "create_binding_set",
            Call::CreateQuerySet(..) => "create_query_set",
            Call::Destroy(..) => "destroy",
            Call::WriteBuffer(..) => "write_buffer",
            Call::WriteTexture(..) => "write_texture",
            Call::BeginPass(..) => "begin_pass",
            Call::EndPass => "end_pass",
            Call::Draw(..) => "draw",
            Call::Dispatch(..) => "dispatch",
            Call::CaptureVertices(..) => "capture_vertices",
            Call::WriteTimestamp(..) => "write_timestamp",
            Call::TimestampsDisjoint => "timestamps_disjoint",
            Call::BeginQuery(..) => "begin_query",
            Call::EndQuery => "end_query",
            Call::CopyBuffer(..) => "copy_buffer",
            Call::CopyTexture(..) => "copy_texture",
            Call::CopyBufferToTexture(..) => "copy_buffer_to_texture",
            Call::CopyTextureToBuffer(..) => "copy_texture_to_buffer",
            Call::ClearBuffer(..) => "clear_buffer",
            Call::Resolve(..) => "resolve",
            Call::Submit(..) => "submit",
            Call::FenceDone(..) => "fence_done",
            Call::WaitFence(..) => "wait_fence",
            Call::WaitRead(..) => "wait_read",
            Call::ReadBack(..) => "read_back",
            Call::ReadQueries(..) => "read_queries",
            Call::FinishRead(..) => "finish_read",
            Call::CreateSurface(..) => "create_surface",
            Call::ConfigureSurface(..) => "configure_surface",
            Call::CurrentTexture(..) => "current_texture",
            Call::Present(..) => "present",
        }
    }
}

pub(crate) fn kind_of(object: &Object<Recorder>) -> (Kind, Id) {
    match *object {
        Object::Buffer(i) => (Kind::Buffer, i),
        Object::Texture(i) => (Kind::Texture, i),
        Object::TextureView(i) => (Kind::TextureView, i),
        Object::Sampler(i) => (Kind::Sampler, i),
        Object::Shader(i) => (Kind::Shader, i),
        Object::Program(i) => (Kind::Program, i),
        Object::Blend(i) => (Kind::Blend, i),
        Object::DepthStencil(i) => (Kind::DepthStencil, i),
        Object::Raster(i) => (Kind::Raster, i),
        Object::VertexLayout(i) => (Kind::VertexLayout, i),
        Object::BindingSet(i) => (Kind::BindingSet, i),
        Object::QuerySet(i) => (Kind::QuerySet, i),
        Object::ResourceTable(i) => (Kind::ResourceTable, i),
        Object::Surface(i) => (Kind::Surface, i),
    }
}

fn object(kind: Kind, i: Id) -> Object<Recorder> {
    match kind {
        Kind::Buffer => Object::Buffer(i),
        Kind::Texture => Object::Texture(i),
        Kind::TextureView => Object::TextureView(i),
        Kind::Sampler => Object::Sampler(i),
        Kind::Shader => Object::Shader(i),
        Kind::Program => Object::Program(i),
        Kind::Blend => Object::Blend(i),
        Kind::DepthStencil => Object::DepthStencil(i),
        Kind::Raster => Object::Raster(i),
        Kind::VertexLayout => Object::VertexLayout(i),
        Kind::BindingSet => Object::BindingSet(i),
        Kind::QuerySet => Object::QuerySet(i),
        Kind::ResourceTable => Object::ResourceTable(i),
        Kind::Surface => Object::Surface(i),
    }
}

pub(crate) fn bind_of(r: &BindingResource<'_, Recorder>) -> Bind {
    let range = |r: &BufferRange<&Id>| (*r.buffer, r.offset, r.size);
    match r {
        BindingResource::UniformBuffer(b) => {
            let (i, o, s) = range(b);
            Bind::Uniform(i, o, s)
        }
        BindingResource::StorageBuffer { range: b, writable } => {
            let (i, o, s) = range(b);
            Bind::Storage(i, o, s, *writable)
        }
        BindingResource::Texture(v) => Bind::Texture(**v),
        BindingResource::StorageTexture { view, writable } => {
            Bind::StorageTexture(**view, *writable)
        }
        BindingResource::Sampler(s) => Bind::Sampler(**s),
        BindingResource::TexelBuffer {
            range: b,
            format,
            writable,
        } => {
            let (i, o, s) = range(b);
            Bind::TexelBuffer(i, o, s, *format, *writable)
        }
        BindingResource::Table(t) => Bind::Table(**t),
    }
}

fn resource(b: &Bind) -> BindingResource<'_, Recorder> {
    let range = |buffer, offset, size| BufferRange {
        buffer,
        offset,
        size,
    };
    match b {
        Bind::Uniform(i, o, s) => BindingResource::UniformBuffer(range(i, *o, *s)),
        Bind::Storage(i, o, s, w) => BindingResource::StorageBuffer {
            range: range(i, *o, *s),
            writable: *w,
        },
        Bind::Texture(v) => BindingResource::Texture(v),
        Bind::StorageTexture(v, w) => BindingResource::StorageTexture {
            view: v,
            writable: *w,
        },
        Bind::Sampler(s) => BindingResource::Sampler(s),
        Bind::TexelBuffer(i, o, s, f, w) => BindingResource::TexelBuffer {
            range: range(i, *o, *s),
            format: *f,
            writable: *w,
        },
        Bind::Table(t) => BindingResource::Table(t),
    }
}

impl Pass {
    pub fn of(p: &PassDesc<'_, Recorder>) -> Pass {
        Pass {
            colors: p
                .colors
                .iter()
                .map(|c| {
                    c.as_ref()
                        .map(|c| (*c.view, c.depth_slice, c.resolve.copied(), c.load, c.store))
                })
                .collect(),
            depth: p.depth.as_ref().map(|d| (*d.view, d.depth, d.stencil)),
            empty: p.empty,
            occlusion: p.occlusion.copied(),
        }
    }
}

impl Draw {
    pub fn of(c: &DrawCall<'_, Recorder>) -> Draw {
        Draw {
            blocks: [
                *c.program,
                *c.blend,
                *c.depth_stencil,
                *c.raster,
                *c.vertex_layout,
            ],
            bindings: c.bindings.map(|b| b.copied()),
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
                } => Range::Vertices(first, count, first_instance, instances),
                DrawRange::Indexed {
                    first,
                    count,
                    base_vertex,
                    first_instance,
                    instances,
                } => Range::Indexed(first, count, base_vertex, first_instance, instances),
                DrawRange::Indirect { args, offset } => Range::Indirect(*args, offset),
                DrawRange::IndexedIndirect { args, offset } => {
                    Range::IndexedIndirect(*args, offset)
                }
            },
        }
    }
}

impl Draw {
    /// Calls `f` with the draw as a [`DrawCall`].
    pub fn with<T>(&self, f: impl FnOnce(&DrawCall<'_, Recorder>) -> T) -> T {
        let d = self;
        let streams: Vec<_> = d
            .vertex_buffers
            .iter()
            .map(|v| {
                v.as_ref().map(|(buffer, offset)| VertexBinding {
                    buffer,
                    offset: *offset,
                })
            })
            .collect();
        f(&DrawCall {
            program: &d.blocks[0],
            blend: &d.blocks[1],
            depth_stencil: &d.blocks[2],
            raster: &d.blocks[3],
            vertex_layout: &d.blocks[4],
            bindings: [0, 1, 2, 3].map(|i| d.bindings[i].as_ref()),
            vertex_buffers: &streams,
            index: d
                .index
                .as_ref()
                .map(|(buffer, offset, format)| IndexBinding {
                    buffer,
                    offset: *offset,
                    format: *format,
                }),
            topology: d.topology,
            restart: d.restart,
            viewports: &d.viewports,
            scissors: &d.scissors,
            blend_constant: d.blend_constant,
            stencil_reference: d.stencil_reference,
            immediates: &d.immediates,
            range: match &d.range {
                Range::Vertices(first, count, first_instance, instances) => DrawRange::Vertices {
                    first: *first,
                    count: *count,
                    first_instance: *first_instance,
                    instances: *instances,
                },
                Range::Indexed(first, count, base_vertex, first_instance, instances) => {
                    DrawRange::Indexed {
                        first: *first,
                        count: *count,
                        base_vertex: *base_vertex,
                        first_instance: *first_instance,
                        instances: *instances,
                    }
                }
                Range::Indirect(args, offset) => DrawRange::Indirect {
                    args,
                    offset: *offset,
                },
                Range::IndexedIndirect(args, offset) => DrawRange::IndexedIndirect {
                    args,
                    offset: *offset,
                },
            },
        })
    }
}

/// Runs `calls` on a new recording back end, each as it was made, and gives the back end, whose
/// own stream then equals `calls` where the recording is deterministic.
pub fn replay(calls: &[Call]) -> Recorder {
    let mut r = Recorder::new();
    for call in calls {
        run(&mut r, call);
    }
    r
}

fn run(r: &mut Recorder, call: &Call) {
    match call {
        Call::CreateBuffer(d, _) => _ = r.create_buffer(d),
        Call::CreateTexture(d, _) => _ = r.create_texture(d),
        Call::CreateView(t, d, _) => _ = r.create_view(t, d),
        Call::CreateSampler(d, _) => _ = r.create_sampler(d),
        Call::CreateShader(s, _) => _ = r.create_shader(&s.0),
        Call::CreateProgram(v, f, compute, _) => {
            _ = r.create_program(&if *compute {
                Program::Compute(v)
            } else {
                Program::Render {
                    vertex: v,
                    fragment: f.as_ref(),
                }
            })
        }
        Call::CreateBlend(d, _) => _ = r.create_blend(d),
        Call::CreateDepthStencil(d, _) => _ = r.create_depth_stencil(d),
        Call::CreateRaster(d, _) => _ = r.create_raster(d),
        Call::CreateVertexLayout(d, _) => _ = r.create_vertex_layout(d),
        Call::CreateBindingSet(entries, _) => {
            let entries: Vec<_> = entries
                .iter()
                .map(|(binding, stages, b)| BindingEntry {
                    binding: *binding,
                    stages: *stages,
                    resource: resource(b),
                })
                .collect();
            _ = r.create_binding_set(&entries);
        }
        Call::CreateQuerySet(kind, count, _) => _ = r.create_query_set(*kind, *count),
        Call::Destroy(kind, i) => r.destroy(object(*kind, *i)),
        Call::WriteBuffer(b, offset, data, mode) => r.write_buffer(b, *offset, data, *mode),
        Call::WriteTexture(region, extent, data, pitch, rows, mode) => {
            r.write_texture(&region.get(), *extent, data, *pitch, *rows, *mode)
        }
        Call::BeginPass(p) => {
            let mut colors: [Option<ColorAttachment<'_, Recorder>>; MAX_COLOR_TARGETS] =
                Default::default();
            for (slot, c) in colors.iter_mut().zip(&p.colors) {
                *slot = c
                    .as_ref()
                    .map(|(view, slice, resolve, load, store)| ColorAttachment {
                        view,
                        depth_slice: *slice,
                        resolve: resolve.as_ref(),
                        load: *load,
                        store: *store,
                    });
            }
            r.begin_pass(&PassDesc {
                colors,
                depth: p
                    .depth
                    .as_ref()
                    .map(|(view, depth, stencil)| DepthAttachment {
                        view,
                        depth: *depth,
                        stencil: *stencil,
                    }),
                empty: p.empty,
                occlusion: p.occlusion.as_ref(),
            });
        }
        Call::EndPass => r.end_pass(),
        Call::Draw(d) => d.with(|c| r.draw(c)),
        Call::Dispatch(d, _) => {
            _ = r.dispatch(&DispatchCall {
                program: &d.program,
                bindings: [0, 1, 2, 3].map(|i| d.bindings[i].as_ref()),
                immediates: &d.immediates,
                groups: match &d.groups {
                    Ok(g) => DispatchGroups::Direct(*g),
                    Err((args, offset)) => DispatchGroups::Indirect {
                        args,
                        offset: *offset,
                    },
                },
            })
        }
        Call::CaptureVertices(d, targets, rasterize, _) => {
            let targets: Vec<_> = targets
                .iter()
                .map(|(buffer, offset, size)| BufferRange {
                    buffer,
                    offset: *offset,
                    size: *size,
                })
                .collect();
            _ = d.with(|c| r.capture_vertices(c, &targets, *rasterize));
        }
        Call::WriteTimestamp(set, index, _) => _ = r.write_timestamp(set, *index),
        Call::TimestampsDisjoint => _ = r.timestamps_disjoint(),
        Call::BeginQuery(i) => r.begin_query(*i),
        Call::EndQuery => r.end_query(),
        Call::CopyBuffer(s, so, d, doff, size) => r.copy_buffer(s, *so, d, *doff, *size),
        Call::CopyTexture(s, d, e) => r.copy_texture(&s.get(), &d.get(), *e),
        Call::CopyBufferToTexture(s, d, e) => r.copy_buffer_to_texture(&s.get(), &d.get(), *e),
        Call::CopyTextureToBuffer(s, d, e) => r.copy_texture_to_buffer(&s.get(), &d.get(), *e),
        Call::ClearBuffer(b, offset, size, pattern) => r.clear_buffer(b, *offset, *size, pattern),
        Call::Resolve(s, d, f) => r.resolve(&s.get(), &d.get(), *f),
        Call::Submit(_) => _ = r.submit(),
        Call::FenceDone(f) => _ = r.fence_done(f),
        Call::WaitFence(f) => r.wait_fence(f),
        Call::WaitRead(rb) => r.wait_read(rb),
        Call::ReadBack(src, _) => {
            _ = match src {
                Ok((buffer, offset, size)) => r.read_back(&ReadSource::Buffer {
                    buffer,
                    offset: *offset,
                    size: *size,
                }),
                Err((region, extent)) => r.read_back(&ReadSource::Texture {
                    region: region.get(),
                    extent: *extent,
                }),
            }
        }
        Call::ReadQueries(set, first, count, _) => _ = r.read_queries(set, *first, *count),
        Call::FinishRead(rb, bytes) => r.finish_read(*rb, &mut vec![0; bytes.len()]),
        Call::CreateSurface(window, _) => _ = r.create_surface(*window),
        Call::ConfigureSurface(s, f, w, h, vsync, _) => {
            _ = r.configure_surface(s, *f, *w, *h, *vsync);
        }
        Call::CurrentTexture(s, _) => _ = r.current_texture(s),
        Call::Present(s, _) => _ = r.present(s),
    }
}
