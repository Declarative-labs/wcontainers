//! Call streams of the interface: every call a front makes, with the objects it names by number, as
//! plain data that is saved (postcard, shaders as naga modules) and run again on any back end. A real
//! program's frames, recorded natively through a front, are replayed this way in the browser on both
//! back ends to time them on the same calls. With the `record` feature [`Traced`] records into a file
//! while it draws through the back end it wraps.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wcontainers_gpu::*;

/// An object of the stream, numbered in the order the stream made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct H(pub u32);

/// A shader in the common form.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shader {
    pub module: naga::Module,
    pub entry_point: String,
    pub stage: ShaderStage,
    pub key: [u8; 32],
}

impl Shader {
    pub fn of(m: &ShaderModule) -> Shader {
        Shader {
            module: m.module.clone(),
            entry_point: m.entry_point.clone(),
            stage: m.stage,
            key: m.key,
        }
    }

    /// The module validated again, as the front handed it over.
    pub fn module(&self) -> Result<ShaderModule, GpuError> {
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&self.module)
        .map_err(|_| GpuError::Invalid("trace: a recorded shader does not validate"))?;
        Ok(ShaderModule {
            module: self.module.clone(),
            info,
            entry_point: self.entry_point.clone(),
            stage: self.stage,
            key: self.key,
        })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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
    Surface,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Region {
    pub texture: H,
    pub mip: u32,
    pub layer: u32,
    pub origin: Origin,
    pub stencil: bool,
}

/// The buffer side of a copy: buffer, offset, row pitch, rows per layer.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Side(pub H, pub u64, pub u32, pub u32);

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Source {
    Buffer(H, u64, u64),
    Texture(Region, Extent),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Resource {
    Uniform(H, u64, u64),
    Storage(H, u64, u64, bool),
    Texture(H),
    StorageTexture(H, bool),
    Sampler(H),
    TexelBuffer(H, u64, u64, Format, bool),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub binding: u32,
    pub stages: StageSet,
    pub resource: Resource,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Color {
    pub view: H,
    pub depth_slice: u32,
    pub resolve: Option<H>,
    pub load: LoadOp<[u32; 4]>,
    pub store: StoreOp,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Depth {
    pub view: H,
    pub depth: Option<AspectOps<u32>>,
    pub stencil: Option<AspectOps<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pass {
    pub colors: [Option<Color>; MAX_COLOR_TARGETS],
    pub depth: Option<Depth>,
    pub empty: Option<EmptyPass>,
    pub occlusion: Option<H>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Range {
    Vertices([u32; 4]),
    /// First, count, base vertex, first instance, instances.
    Indexed(u32, u32, i32, u32, u32),
    Indirect(H, u64),
    IndexedIndirect(H, u64),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Draw {
    pub program: H,
    pub blend: H,
    pub depth_stencil: H,
    pub raster: H,
    pub vertex_layout: H,
    pub bindings: [Option<H>; MAX_BINDING_SETS],
    pub vertex_buffers: Vec<Option<(H, u64)>>,
    pub index: Option<(H, u64, IndexFormat)>,
    pub topology: Topology,
    pub restart: bool,
    pub viewports: Vec<Viewport>,
    pub scissors: Vec<ScissorRect>,
    pub blend_constant: [u32; 4],
    pub stencil_reference: [u8; 2],
    pub immediates: Vec<u8>,
    pub range: Range,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prepare {
    pub program: H,
    pub blend: H,
    pub depth_stencil: H,
    pub raster: H,
    pub vertex_layout: H,
    pub targets: TargetLayout,
    pub topology: Topology,
    pub layouts: [BindingLayoutKey; MAX_BINDING_SETS],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Indices {
    Sequential(u32, u32),
    Guest(Vec<u8>, IndexFormat, i32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dispatch {
    pub program: H,
    pub bindings: [Option<H>; MAX_BINDING_SETS],
    pub immediates: Vec<u8>,
    pub groups: Result<[u32; 3], (H, u64)>,
}

/// One call of the interface. A call that makes an object names the number it gets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Call {
    CreateBuffer(H, BufferDesc),
    CreateTexture(H, TextureDesc),
    CreateView(H, H, TextureViewDesc),
    CreateSampler(H, SamplerDesc),
    CreateShader(H, Box<Shader>),
    /// Vertex and fragment shaders, or a compute shader alone (`true`).
    CreateProgram(H, H, Option<H>, bool),
    CreateBlend(H, Box<BlendDesc>),
    CreateDepthStencil(H, DepthStencilDesc),
    CreateRaster(H, RasterDesc),
    CreateVertexLayout(H, Box<VertexLayoutDesc>),
    CreateBindingSet(H, Vec<Entry>),
    CreateQuerySet(H, QueryKind, u32),
    Destroy(Kind, H),
    WriteBuffer(H, u64, Vec<u8>, WriteMode),
    WriteTexture(Region, Extent, Vec<u8>, u32, u32, WriteMode),
    BeginPass(Box<Pass>),
    EndPass,
    Draw(Box<Draw>),
    BeginQuery(u32),
    EndQuery,
    CopyBuffer(H, u64, H, u64, u64),
    CopyTexture(Region, Region, Extent),
    CopyBufferToTexture(Side, Region, Extent),
    CopyTextureToBuffer(Region, Side, Extent),
    ClearBuffer(H, u64, u64, Vec<u8>),
    Resolve(Region, Region, Format),
    Submit(H),
    FenceDone(H),
    WaitFence(H),
    ReadBack(H, Source),
    ReadQueries(H, H, u32, u32),
    ReadState(H),
    WaitRead(H),
    /// The read back and how many bytes it gave.
    FinishRead(H, u64),
    CreateSurface(H, u32),
    ConfigureSurface(H, Format, u32, u32, bool),
    CurrentTexture(H, H),
    Present(H),
    ReadBackNow(Source, u64),
    DrawFanOrLoop(Box<Draw>, FanOrLoop, Indices, ProvokingVertex, u32, u32),
    Prepare(Box<Prepare>),
    /// Texture, its description, value format, mips, layers, value.
    ClearTexture(H, TextureDesc, Format, [u32; 2], [u32; 2], ClearValue),
    ClearRects(H, Format, ClearValue, Vec<ScissorRect>),
    GenerateMips(H, TextureDesc, Format, [u32; 2], [u32; 2]),
    /// The buffer it answered, the ring, the parts and their alignment.
    WriteBufferRing(H, RingKind, Vec<Vec<u8>>, u32),
    ResolveQueries(H, u32, u32, H, u64),
    Dispatch(Box<Dispatch>),
    /// The draw, its targets (buffer, offset, size) and whether it also rasterised.
    CaptureVertices(Box<Draw>, Vec<(H, u64, u64)>, bool),
    WriteTimestamp(H, u32),
    TimestampsDisjoint,
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
            Call::ReadBack(..) => "read_back",
            Call::ReadQueries(..) => "read_queries",
            Call::ReadState(..) => "read_state",
            Call::WaitRead(..) => "wait_read",
            Call::FinishRead(..) => "finish_read",
            Call::CreateSurface(..) => "create_surface",
            Call::ConfigureSurface(..) => "configure_surface",
            Call::CurrentTexture(..) => "current_texture",
            Call::Present(..) => "present",
            Call::ReadBackNow(..) => "read_back_now",
            Call::DrawFanOrLoop(..) => "draw_fan_or_loop",
            Call::Prepare(..) => "prepare",
            Call::ClearTexture(..) => "clear_texture",
            Call::ClearRects(..) => "clear_rects",
            Call::GenerateMips(..) => "generate_mips",
            Call::WriteBufferRing(..) => "write_buffer_ring",
            Call::ResolveQueries(..) => "resolve_queries",
            Call::Dispatch(..) => "dispatch",
            Call::CaptureVertices(..) => "capture_vertices",
            Call::WriteTimestamp(..) => "write_timestamp",
            Call::TimestampsDisjoint => "timestamps_disjoint",
        }
    }
}

/// A back end's objects by the stream's numbers.
pub struct Table<B: Backend> {
    buffers: HashMap<H, B::Buffer>,
    textures: HashMap<H, B::Texture>,
    views: HashMap<H, B::TextureView>,
    samplers: HashMap<H, B::Sampler>,
    shaders: HashMap<H, B::Shader>,
    programs: HashMap<H, B::Program>,
    blends: HashMap<H, B::Blend>,
    depth_stencils: HashMap<H, B::DepthStencil>,
    rasters: HashMap<H, B::Raster>,
    layouts: HashMap<H, B::VertexLayout>,
    sets: HashMap<H, B::BindingSet>,
    query_sets: HashMap<H, B::QuerySet>,
    readbacks: HashMap<H, B::Readback>,
    fences: HashMap<H, B::Fence>,
    surfaces: HashMap<H, B::Surface>,
}

impl<B: Backend> Default for Table<B> {
    fn default() -> Self {
        Table {
            buffers: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new(),
            samplers: HashMap::new(),
            shaders: HashMap::new(),
            programs: HashMap::new(),
            blends: HashMap::new(),
            depth_stencils: HashMap::new(),
            rasters: HashMap::new(),
            layouts: HashMap::new(),
            sets: HashMap::new(),
            query_sets: HashMap::new(),
            readbacks: HashMap::new(),
            fences: HashMap::new(),
            surfaces: HashMap::new(),
        }
    }
}

const UNKNOWN: GpuError = GpuError::Invalid("trace: the stream names an object it never made");

fn get<V>(m: &HashMap<H, V>, h: H) -> Result<&V, GpuError> {
    m.get(&h).ok_or(UNKNOWN)
}

fn made<V>(m: &mut HashMap<H, V>, h: H, r: Result<V, GpuError>) -> Result<Answer, GpuError> {
    m.insert(h, r?);
    Ok(Answer::Done)
}

impl<B: Backend> Table<B> {
    fn region(&self, r: &Region) -> Result<TextureRegion<'_, B>, GpuError> {
        Ok(TextureRegion {
            texture: get(&self.textures, r.texture)?,
            mip: r.mip,
            layer: r.layer,
            origin: r.origin,
            stencil: r.stencil,
        })
    }

    fn side(&self, s: &Side) -> Result<CopyBufferTexture<'_, B>, GpuError> {
        Ok(CopyBufferTexture {
            buffer: get(&self.buffers, s.0)?,
            offset: s.1,
            row_pitch: s.2,
            rows_per_layer: s.3,
        })
    }

    fn source(&self, s: &Source) -> Result<ReadSource<'_, B>, GpuError> {
        Ok(match s {
            Source::Buffer(b, offset, size) => ReadSource::Buffer {
                buffer: get(&self.buffers, *b)?,
                offset: *offset,
                size: *size,
            },
            Source::Texture(r, extent) => ReadSource::Texture {
                region: self.region(r)?,
                extent: *extent,
            },
        })
    }

    fn sets(
        &self,
        sets: &[Option<H>; MAX_BINDING_SETS],
    ) -> Result<[Option<&B::BindingSet>; MAX_BINDING_SETS], GpuError> {
        let mut out = [None; MAX_BINDING_SETS];
        for (o, s) in out.iter_mut().zip(sets) {
            if let Some(s) = s {
                *o = Some(get(&self.sets, *s)?);
            }
        }
        Ok(out)
    }

    fn vertex_buffers(&self, d: &Draw) -> Result<Vec<Option<VertexBinding<'_, B>>>, GpuError> {
        d.vertex_buffers
            .iter()
            .map(|v| {
                v.map(|(b, offset)| {
                    Ok(VertexBinding {
                        buffer: get(&self.buffers, b)?,
                        offset,
                    })
                })
                .transpose()
            })
            .collect()
    }

    fn draw<'a>(
        &'a self,
        d: &'a Draw,
        vertex_buffers: &'a [Option<VertexBinding<'a, B>>],
    ) -> Result<DrawCall<'a, B>, GpuError> {
        Ok(DrawCall {
            program: get(&self.programs, d.program)?,
            blend: get(&self.blends, d.blend)?,
            depth_stencil: get(&self.depth_stencils, d.depth_stencil)?,
            raster: get(&self.rasters, d.raster)?,
            vertex_layout: get(&self.layouts, d.vertex_layout)?,
            bindings: self.sets(&d.bindings)?,
            vertex_buffers,
            index: match d.index {
                Some((b, offset, format)) => Some(IndexBinding {
                    buffer: get(&self.buffers, b)?,
                    offset,
                    format,
                }),
                None => None,
            },
            topology: d.topology,
            restart: d.restart,
            viewports: &d.viewports,
            scissors: &d.scissors,
            blend_constant: d.blend_constant,
            stencil_reference: d.stencil_reference,
            immediates: &d.immediates,
            range: match d.range {
                Range::Vertices([first, count, first_instance, instances]) => DrawRange::Vertices {
                    first,
                    count,
                    first_instance,
                    instances,
                },
                Range::Indexed(first, count, base_vertex, first_instance, instances) => {
                    DrawRange::Indexed {
                        first,
                        count,
                        base_vertex,
                        first_instance,
                        instances,
                    }
                }
                Range::Indirect(b, offset) => DrawRange::Indirect {
                    args: get(&self.buffers, b)?,
                    offset,
                },
                Range::IndexedIndirect(b, offset) => DrawRange::IndexedIndirect {
                    args: get(&self.buffers, b)?,
                    offset,
                },
            },
        })
    }

    fn entries(&self, entries: &[Entry]) -> Result<Vec<BindingEntry<'_, B>>, GpuError> {
        let range = |b: H, offset: u64, size: u64| {
            Ok::<_, GpuError>(BufferRange {
                buffer: get(&self.buffers, b)?,
                offset,
                size,
            })
        };
        entries
            .iter()
            .map(|e| {
                Ok(BindingEntry {
                    binding: e.binding,
                    stages: e.stages,
                    resource: match e.resource {
                        Resource::Uniform(b, o, s) => {
                            BindingResource::UniformBuffer(range(b, o, s)?)
                        }
                        Resource::Storage(b, o, s, writable) => BindingResource::StorageBuffer {
                            range: range(b, o, s)?,
                            writable,
                        },
                        Resource::Texture(v) => BindingResource::Texture(get(&self.views, v)?),
                        Resource::StorageTexture(v, writable) => BindingResource::StorageTexture {
                            view: get(&self.views, v)?,
                            writable,
                        },
                        Resource::Sampler(s) => BindingResource::Sampler(get(&self.samplers, s)?),
                        Resource::TexelBuffer(b, o, s, format, writable) => {
                            BindingResource::TexelBuffer {
                                range: range(b, o, s)?,
                                format,
                                writable,
                            }
                        }
                    },
                })
            })
            .collect()
    }

    fn pass(&self, p: &Pass) -> Result<PassDesc<'_, B>, GpuError> {
        let mut colors = [const { None }; MAX_COLOR_TARGETS];
        for (o, c) in colors.iter_mut().zip(&p.colors) {
            if let Some(c) = c {
                *o = Some(ColorAttachment {
                    view: get(&self.views, c.view)?,
                    depth_slice: c.depth_slice,
                    resolve: c.resolve.map(|r| get(&self.views, r)).transpose()?,
                    load: c.load,
                    store: c.store,
                });
            }
        }
        Ok(PassDesc {
            colors,
            depth: match &p.depth {
                Some(d) => Some(DepthAttachment {
                    view: get(&self.views, d.view)?,
                    depth: d.depth,
                    stencil: d.stencil,
                }),
                None => None,
            },
            empty: p.empty,
            occlusion: p.occlusion.map(|q| get(&self.query_sets, q)).transpose()?,
        })
    }

    fn object(&mut self, kind: Kind, h: H) -> Result<Object<B>, GpuError> {
        Ok(match kind {
            Kind::Buffer => Object::Buffer(self.buffers.remove(&h).ok_or(UNKNOWN)?),
            Kind::Texture => Object::Texture(self.textures.remove(&h).ok_or(UNKNOWN)?),
            Kind::TextureView => Object::TextureView(self.views.remove(&h).ok_or(UNKNOWN)?),
            Kind::Sampler => Object::Sampler(self.samplers.remove(&h).ok_or(UNKNOWN)?),
            Kind::Shader => Object::Shader(self.shaders.remove(&h).ok_or(UNKNOWN)?),
            Kind::Program => Object::Program(self.programs.remove(&h).ok_or(UNKNOWN)?),
            Kind::Blend => Object::Blend(self.blends.remove(&h).ok_or(UNKNOWN)?),
            Kind::DepthStencil => {
                Object::DepthStencil(self.depth_stencils.remove(&h).ok_or(UNKNOWN)?)
            }
            Kind::Raster => Object::Raster(self.rasters.remove(&h).ok_or(UNKNOWN)?),
            Kind::VertexLayout => Object::VertexLayout(self.layouts.remove(&h).ok_or(UNKNOWN)?),
            Kind::BindingSet => Object::BindingSet(self.sets.remove(&h).ok_or(UNKNOWN)?),
            Kind::QuerySet => Object::QuerySet(self.query_sets.remove(&h).ok_or(UNKNOWN)?),
            Kind::Surface => Object::Surface(self.surfaces.remove(&h).ok_or(UNKNOWN)?),
        })
    }
}

/// What a call answered besides success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Done,
    Bool(bool),
    State(ReadState),
}

/// Times the back end's own part of a call: the clock's reading is taken just around it.
pub struct Timer<'c> {
    pub clock: Option<&'c dyn Fn() -> f64>,
    pub spent: f64,
}

impl Timer<'_> {
    fn run<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let Some(clock) = self.clock else {
            return f();
        };
        let start = clock();
        let out = f();
        self.spent += clock() - start;
        out
    }
}

/// Runs one call on `b`, its objects looked up in `t` (and objects it makes put there). Read backs'
/// bytes go into `scratch`.
pub fn apply<B: Backend>(
    b: &mut B,
    t: &mut Table<B>,
    call: &Call,
    scratch: &mut Vec<u8>,
    timer: &mut Timer<'_>,
) -> Result<Answer, GpuError> {
    let done = Ok(Answer::Done);
    match call {
        Call::CreateBuffer(h, d) => made(&mut t.buffers, *h, timer.run(|| b.create_buffer(d))),
        Call::CreateTexture(h, d) => made(&mut t.textures, *h, timer.run(|| b.create_texture(d))),
        Call::CreateView(h, tex, d) => {
            let r = {
                let tex = get(&t.textures, *tex)?;
                timer.run(|| b.create_view(tex, d))
            };
            made(&mut t.views, *h, r)
        }
        Call::CreateSampler(h, d) => made(&mut t.samplers, *h, timer.run(|| b.create_sampler(d))),
        Call::CreateShader(h, s) => {
            let m = s.module()?;
            made(&mut t.shaders, *h, timer.run(|| b.create_shader(&m)))
        }
        Call::CreateProgram(h, first, fragment, compute) => {
            let r = {
                let first = get(&t.shaders, *first)?;
                let program = if *compute {
                    Program::Compute(first)
                } else {
                    Program::Render {
                        vertex: first,
                        fragment: fragment.map(|f| get(&t.shaders, f)).transpose()?,
                    }
                };
                timer.run(|| b.create_program(&program))
            };
            made(&mut t.programs, *h, r)
        }
        Call::CreateBlend(h, d) => made(&mut t.blends, *h, timer.run(|| b.create_blend(d))),
        Call::CreateDepthStencil(h, d) => made(
            &mut t.depth_stencils,
            *h,
            timer.run(|| b.create_depth_stencil(d)),
        ),
        Call::CreateRaster(h, d) => made(&mut t.rasters, *h, timer.run(|| b.create_raster(d))),
        Call::CreateVertexLayout(h, d) => {
            made(&mut t.layouts, *h, timer.run(|| b.create_vertex_layout(d)))
        }
        Call::CreateBindingSet(h, entries) => {
            let r = {
                let entries = t.entries(entries)?;
                timer.run(|| b.create_binding_set(&entries))
            };
            made(&mut t.sets, *h, r)
        }
        Call::CreateQuerySet(h, kind, count) => made(
            &mut t.query_sets,
            *h,
            timer.run(|| b.create_query_set(*kind, *count)),
        ),
        Call::Destroy(kind, h) => {
            let object = t.object(*kind, *h)?;
            timer.run(|| b.destroy(object));
            done
        }
        Call::WriteBuffer(h, offset, data, mode) => {
            let buffer = get(&t.buffers, *h)?;
            timer.run(|| b.write_buffer(buffer, *offset, data, *mode));
            done
        }
        Call::WriteTexture(r, extent, data, pitch, rows, mode) => {
            let r = t.region(r)?;
            timer.run(|| b.write_texture(&r, *extent, data, *pitch, *rows, *mode));
            done
        }
        Call::BeginPass(p) => {
            let p = t.pass(p)?;
            timer.run(|| b.begin_pass(&p));
            done
        }
        Call::EndPass => {
            timer.run(|| b.end_pass());
            done
        }
        Call::Draw(d) => {
            let vb = t.vertex_buffers(d)?;
            let call = t.draw(d, &vb)?;
            timer.run(|| b.draw(&call));
            done
        }
        Call::BeginQuery(i) => {
            timer.run(|| b.begin_query(*i));
            done
        }
        Call::EndQuery => {
            timer.run(|| b.end_query());
            done
        }
        Call::CopyBuffer(s, so, d, dof, size) => {
            let (s, d) = (get(&t.buffers, *s)?, get(&t.buffers, *d)?);
            timer.run(|| b.copy_buffer(s, *so, d, *dof, *size));
            done
        }
        Call::CopyTexture(s, d, extent) => {
            let (s, d) = (t.region(s)?, t.region(d)?);
            timer.run(|| b.copy_texture(&s, &d, *extent));
            done
        }
        Call::CopyBufferToTexture(s, d, extent) => {
            let (s, d) = (t.side(s)?, t.region(d)?);
            timer.run(|| b.copy_buffer_to_texture(&s, &d, *extent));
            done
        }
        Call::CopyTextureToBuffer(s, d, extent) => {
            let (s, d) = (t.region(s)?, t.side(d)?);
            timer.run(|| b.copy_texture_to_buffer(&s, &d, *extent));
            done
        }
        Call::ClearBuffer(h, offset, size, pattern) => {
            let buffer = get(&t.buffers, *h)?;
            timer.run(|| b.clear_buffer(buffer, *offset, *size, pattern));
            done
        }
        Call::Resolve(s, d, format) => {
            let (s, d) = (t.region(s)?, t.region(d)?);
            timer.run(|| b.resolve(&s, &d, *format));
            done
        }
        Call::Submit(h) => {
            let fence = timer.run(|| b.submit());
            t.fences.insert(*h, fence);
            done
        }
        Call::FenceDone(h) => {
            let fence = get(&t.fences, *h)?;
            Ok(Answer::Bool(timer.run(|| b.fence_done(fence))))
        }
        Call::WaitFence(h) => {
            let fence = get(&t.fences, *h)?;
            timer.run(|| b.wait_fence(fence));
            done
        }
        Call::ReadBack(h, s) => {
            let r = {
                let s = t.source(s)?;
                timer.run(|| b.read_back(&s))
            };
            made(&mut t.readbacks, *h, r)
        }
        Call::ReadQueries(h, set, first, count) => {
            let r = {
                let set = get(&t.query_sets, *set)?;
                timer.run(|| b.read_queries(set, *first, *count))
            };
            made(&mut t.readbacks, *h, r)
        }
        Call::ReadState(h) => {
            let r = get(&t.readbacks, *h)?;
            Ok(Answer::State(timer.run(|| b.read_state(r))))
        }
        Call::WaitRead(h) => {
            let r = get(&t.readbacks, *h)?;
            timer.run(|| b.wait_read(r));
            done
        }
        Call::FinishRead(h, len) => {
            let r = t.readbacks.remove(h).ok_or(UNKNOWN)?;
            scratch.resize(*len as usize, 0);
            timer.run(|| b.finish_read(r, scratch));
            done
        }
        Call::CreateSurface(h, window) => {
            made(&mut t.surfaces, *h, timer.run(|| b.create_surface(*window)))
        }
        Call::ConfigureSurface(h, format, width, height, vsync) => {
            let s = get(&t.surfaces, *h)?;
            timer.run(|| b.configure_surface(s, *format, *width, *height, *vsync))?;
            done
        }
        Call::CurrentTexture(h, s) => {
            let r = {
                let s = get(&t.surfaces, *s)?;
                timer.run(|| b.current_texture(s))
            };
            made(&mut t.views, *h, r)
        }
        Call::Present(h) => {
            let s = get(&t.surfaces, *h)?;
            timer.run(|| b.present(s))?;
            done
        }
        Call::ReadBackNow(s, len) => {
            let s = t.source(s)?;
            scratch.resize(*len as usize, 0);
            timer.run(|| b.read_back_now(&s, scratch))?;
            done
        }
        Call::DrawFanOrLoop(d, kind, indices, provoking, first_instance, instances) => {
            let vb = t.vertex_buffers(d)?;
            let call = t.draw(d, &vb)?;
            let source = match indices {
                Indices::Sequential(first, count) => IndexSource::Sequential {
                    first: *first,
                    count: *count,
                },
                Indices::Guest(indices, format, base_vertex) => IndexSource::Guest {
                    indices,
                    format: *format,
                    base_vertex: *base_vertex,
                },
            };
            timer.run(|| {
                b.draw_fan_or_loop(
                    &call,
                    *kind,
                    source,
                    *provoking,
                    *first_instance,
                    *instances,
                )
            })?;
            done
        }
        Call::Prepare(p) => {
            let desc = PrepareDesc {
                program: get(&t.programs, p.program)?,
                blend: get(&t.blends, p.blend)?,
                depth_stencil: get(&t.depth_stencils, p.depth_stencil)?,
                raster: get(&t.rasters, p.raster)?,
                vertex_layout: get(&t.layouts, p.vertex_layout)?,
                targets: p.targets,
                topology: p.topology,
                layouts: p.layouts,
            };
            timer.run(|| b.prepare(&desc));
            done
        }
        Call::ClearTexture(h, desc, format, mips, layers, value) => {
            let tex = get(&t.textures, *h)?;
            timer.run(|| {
                b.clear_texture(
                    tex,
                    desc,
                    *format,
                    mips[0]..mips[1],
                    layers[0]..layers[1],
                    *value,
                )
            })?;
            done
        }
        Call::ClearRects(h, format, value, rects) => {
            let view = get(&t.views, *h)?;
            timer.run(|| b.clear_rects(view, *format, *value, rects))?;
            done
        }
        Call::GenerateMips(h, desc, format, mips, layers) => {
            let tex = get(&t.textures, *h)?;
            timer.run(|| {
                b.generate_mips(tex, desc, *format, mips[0]..mips[1], layers[0]..layers[1])
            })?;
            done
        }
        Call::WriteBufferRing(h, kind, parts, align) => {
            let parts: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
            let mut offsets = vec![0; parts.len()];
            let r = timer.run(|| b.write_buffer_ring(*kind, &parts, *align, &mut offsets));
            made(&mut t.buffers, *h, r)
        }
        Call::ResolveQueries(set, first, count, dst, offset) => {
            let (set, dst) = (get(&t.query_sets, *set)?, get(&t.buffers, *dst)?);
            timer.run(|| b.resolve_queries(set, *first, *count, dst, *offset))?;
            done
        }
        Call::Dispatch(d) => {
            let call = DispatchCall {
                program: get(&t.programs, d.program)?,
                bindings: t.sets(&d.bindings)?,
                immediates: &d.immediates,
                groups: match d.groups {
                    Ok(groups) => DispatchGroups::Direct(groups),
                    Err((args, offset)) => DispatchGroups::Indirect {
                        args: get(&t.buffers, args)?,
                        offset,
                    },
                },
            };
            timer.run(|| b.dispatch(&call))?;
            done
        }
        Call::CaptureVertices(d, targets, rasterize) => {
            let vb = t.vertex_buffers(d)?;
            let call = t.draw(d, &vb)?;
            let targets = targets
                .iter()
                .map(|&(h, offset, size)| {
                    Ok(BufferRange {
                        buffer: get(&t.buffers, h)?,
                        offset,
                        size,
                    })
                })
                .collect::<Result<Vec<_>, GpuError>>()?;
            timer.run(|| b.capture_vertices(&call, &targets, *rasterize))?;
            done
        }
        Call::WriteTimestamp(set, index) => {
            let set = get(&t.query_sets, *set)?;
            timer.run(|| b.write_timestamp(set, *index))?;
            done
        }
        Call::TimestampsDisjoint => Ok(Answer::Bool(timer.run(|| b.timestamps_disjoint())?)),
    }
}

/// The next call of a saved stream, and the bytes after it.
pub fn decode(bytes: &[u8]) -> Result<(Call, &[u8]), String> {
    postcard::take_from_bytes::<Call>(bytes)
        .map_err(|e| format!("trace: a call does not decode: {e}"))
}

/// What one frame of a replay cost: per call kind, how many calls and the milliseconds inside the
/// back end.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// Per call kind (copies between textures also by their formats): calls and milliseconds.
    pub kinds: Vec<(String, u32, f64)>,
    /// Calls the back end refused or failed, with the first one's reason.
    pub failed: u32,
    pub first_failure: Option<String>,
    pub presented: bool,
}

impl Frame {
    fn add(&mut self, name: &str, ms: f64) {
        match self.kinds.iter_mut().find(|k| k.0 == name) {
            Some(k) => {
                k.1 += 1;
                k.2 += ms;
            }
            None => self.kinds.push((name.to_string(), 1, ms)),
        }
    }
}

/// A stream replayed on one back end, frame by frame (a frame ends at each present).
pub struct Replay<B: Backend> {
    bytes: Vec<u8>,
    at: usize,
    table: Table<B>,
    scratch: Vec<u8>,
    /// Each texture's format, to name copies by theirs.
    formats: HashMap<H, Format>,
}

impl<B: Backend> Replay<B> {
    pub fn new(bytes: Vec<u8>) -> Self {
        Replay {
            bytes,
            at: 0,
            table: Table::default(),
            scratch: Vec::new(),
            formats: HashMap::new(),
        }
    }

    /// The call's kind as frames count it: its method, and for a copy between textures their
    /// formats.
    fn kind(&self, call: &Call) -> String {
        match call {
            Call::CopyTexture(s, d, _) => format!(
                "copy_texture({:?},{:?})",
                self.formats.get(&s.texture),
                self.formats.get(&d.texture)
            ),
            _ => call.name().to_string(),
        }
    }

    /// The bytes the last read back of the stream gave.
    pub fn read(&self) -> &[u8] {
        &self.scratch
    }

    /// Runs the calls up to and including the next present; none when the stream has ended.
    /// `window` is called before a surface is made on a host window, for hosts that must name it
    /// first. With a clock each call's time inside the back end is added to its kind.
    pub fn frame(
        &mut self,
        b: &mut B,
        clock: Option<&dyn Fn() -> f64>,
        window: &mut dyn FnMut(&mut B, u32),
    ) -> Option<Result<Frame, String>> {
        if self.at == self.bytes.len() {
            return None;
        }
        let mut frame = Frame::default();
        while self.at < self.bytes.len() {
            let call = match decode(&self.bytes[self.at..]) {
                Ok((call, rest)) => {
                    self.at = self.bytes.len() - rest.len();
                    call
                }
                Err(e) => {
                    self.at = self.bytes.len();
                    return Some(Err(e));
                }
            };
            match call {
                Call::CreateSurface(_, w) => window(b, w),
                Call::CreateTexture(h, d) => {
                    self.formats.insert(h, d.format);
                }
                _ => {}
            }
            let mut timer = Timer { clock, spent: 0.0 };
            let r = apply(b, &mut self.table, &call, &mut self.scratch, &mut timer);
            frame.add(&self.kind(&call), timer.spent);
            if let Err(e) = r {
                frame.failed += 1;
                frame
                    .first_failure
                    .get_or_insert_with(|| format!("{}: {e:?}", call.name()));
            }
            if let Call::Present(_) = call {
                frame.presented = true;
                break;
            }
        }
        Some(Ok(frame))
    }
}

#[cfg(feature = "record")]
pub use record::Traced;

#[cfg(feature = "record")]
mod record {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A back end that records every call into a stream file while running it on the back end it
    /// wraps. Objects are the stream's numbers.
    pub struct Traced<B: Backend> {
        inner: B,
        table: Table<B>,
        /// None without `WEBGL_TRACE`: the calls run unrecorded.
        file: Option<std::fs::File>,
        /// The calls since the last present, written whole at the next, so a program stopped
        /// mid-frame leaves whole frames.
        frame: Vec<u8>,
        next: u32,
        scratch: Vec<u8>,
        helpers: Helpers<Traced<B>>,
    }

    impl<B: Backend> core::fmt::Debug for Traced<B> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "Traced({} objects)", self.next)
        }
    }

    /// Devices made so far in the process, so the second writes beside the first.
    static DEVICES: AtomicU32 = AtomicU32::new(0);

    impl<B: Backend> Traced<B> {
        /// Records `inner`'s calls into the file `WEBGL_TRACE` names (the process's second device
        /// into `<name>.1`, and so on); without it, records nothing, so a build that has the
        /// feature only through another crate's runs as it would without.
        pub fn new(inner: B) -> Result<Self, String> {
            let file = match std::env::var("WEBGL_TRACE") {
                Ok(path) => {
                    let n = DEVICES.fetch_add(1, Ordering::Relaxed);
                    let path = if n == 0 { path } else { format!("{path}.{n}") };
                    Some(std::fs::File::create(&path).map_err(|e| format!("{path}: {e}"))?)
                }
                Err(_) => None,
            };
            Ok(Traced {
                inner,
                table: Table::default(),
                file,
                frame: Vec::new(),
                next: 0,
                scratch: Vec::new(),
                helpers: Helpers::default(),
            })
        }

        fn id(&mut self) -> H {
            self.next += 1;
            H(self.next)
        }

        fn write(&mut self, call: &Call) {
            if self.file.is_none() {
                return;
            }
            if let Err(e) = postcard::to_io(call, &mut self.frame) {
                eprintln!("webgl trace: {} not written: {e}", call.name());
            }
        }

        /// Writes the calls held so far into the file.
        fn flush(&mut self) {
            if let Some(file) = &mut self.file
                && let Err(e) = file.write_all(&self.frame)
            {
                eprintln!("webgl trace: a frame not written: {e}");
            }
            self.frame.clear();
        }

        /// Records the call and runs it.
        fn run(&mut self, call: Call) -> Result<Answer, GpuError> {
            self.write(&call);
            let mut timer = Timer {
                clock: None,
                spent: 0.0,
            };
            apply(
                &mut self.inner,
                &mut self.table,
                &call,
                &mut self.scratch,
                &mut timer,
            )
        }

        /// Records a call that makes an object and answers its number.
        fn make(&mut self, f: impl FnOnce(H) -> Call) -> Result<H, GpuError> {
            let h = self.id();
            self.run(f(h)).map(|_| h)
        }

        fn void(&mut self, call: Call) {
            if let Err(e) = self.run(call) {
                eprintln!("webgl trace: a call the stream holds failed: {e:?}");
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl Traced<crate::Gl> {
        /// [`crate::Gl::on_angle`], recorded.
        pub fn on_angle(dir: &std::path::Path, off: &[&str]) -> Result<Self, String> {
            Traced::new(crate::Gl::on_angle(dir, off)?)
        }

        /// [`crate::Gl::add_window`].
        ///
        /// # Safety
        /// As [`crate::Gl::add_window`].
        pub unsafe fn add_window(&mut self, name: u32, window: *mut core::ffi::c_void) {
            // SAFETY: the caller keeps the rule of the wrapped call.
            unsafe { self.inner.add_window(name, window) }
        }
    }

    impl<B: Backend> Drop for Traced<B> {
        fn drop(&mut self) {
            self.flush();
        }
    }

    fn region(r: &TextureRegion<'_, Traced<impl Backend>>) -> Region {
        Region {
            texture: *r.texture,
            mip: r.mip,
            layer: r.layer,
            origin: r.origin,
            stencil: r.stencil,
        }
    }

    fn side(c: &CopyBufferTexture<'_, Traced<impl Backend>>) -> Side {
        Side(*c.buffer, c.offset, c.row_pitch, c.rows_per_layer)
    }

    fn source(s: &ReadSource<'_, Traced<impl Backend>>) -> Source {
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

    fn draw(c: &DrawCall<'_, Traced<impl Backend>>) -> Box<Draw> {
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
                DrawRange::IndexedIndirect { args, offset } => {
                    Range::IndexedIndirect(*args, offset)
                }
            },
        })
    }

    const NO_TABLES: GpuError =
        GpuError::Invalid("trace: a traced device keeps no resource tables");

    fn entries_of(
        entries: &[BindingEntry<'_, Traced<impl Backend>>],
    ) -> Result<Vec<Entry>, GpuError> {
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

    impl<B: Backend> Backend for Traced<B> {
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
            self.make(|h| Call::CreateBuffer(h, *desc))
        }

        fn create_texture(&mut self, desc: &TextureDesc) -> Result<H, GpuError> {
            self.make(|h| Call::CreateTexture(h, *desc))
        }

        fn create_view(&mut self, texture: &H, desc: &TextureViewDesc) -> Result<H, GpuError> {
            self.make(|h| Call::CreateView(h, *texture, *desc))
        }

        fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<H, GpuError> {
            self.make(|h| Call::CreateSampler(h, *desc))
        }

        fn create_shader(&mut self, module: &ShaderModule) -> Result<H, GpuError> {
            let h = self.id();
            self.write(&Call::CreateShader(h, Box::new(Shader::of(module))));
            let s = self.inner.create_shader(module)?;
            self.table.shaders.insert(h, s);
            Ok(h)
        }

        fn create_program(&mut self, program: &Program<'_, Self>) -> Result<H, GpuError> {
            let (first, fragment, compute) = match program {
                Program::Render { vertex, fragment } => (**vertex, fragment.copied(), false),
                Program::Compute(s) => (**s, None, true),
            };
            self.make(|h| Call::CreateProgram(h, first, fragment, compute))
        }

        fn create_blend(&mut self, desc: &BlendDesc) -> Result<H, GpuError> {
            self.make(|h| Call::CreateBlend(h, Box::new(*desc)))
        }

        fn create_depth_stencil(&mut self, desc: &DepthStencilDesc) -> Result<H, GpuError> {
            self.make(|h| Call::CreateDepthStencil(h, *desc))
        }

        fn create_raster(&mut self, desc: &RasterDesc) -> Result<H, GpuError> {
            self.make(|h| Call::CreateRaster(h, *desc))
        }

        fn create_vertex_layout(&mut self, desc: &VertexLayoutDesc) -> Result<H, GpuError> {
            self.make(|h| Call::CreateVertexLayout(h, Box::new(*desc)))
        }

        fn create_binding_set(
            &mut self,
            entries: &[BindingEntry<'_, Self>],
        ) -> Result<H, GpuError> {
            let entries = entries_of(entries)?;
            self.make(|h| Call::CreateBindingSet(h, entries))
        }

        fn binding_layout_key(&self, set: &H) -> BindingLayoutKey {
            match self.table.sets.get(set) {
                Some(s) => self.inner.binding_layout_key(s),
                None => BindingLayoutKey::default(),
            }
        }

        fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<H, GpuError> {
            self.make(|h| Call::CreateQuerySet(h, kind, count))
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
            self.void(Call::Destroy(kind, h));
        }

        fn write_buffer(&mut self, buffer: &H, offset: u64, data: &[u8], mode: WriteMode) {
            self.void(Call::WriteBuffer(*buffer, offset, data.to_vec(), mode));
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
            self.void(Call::WriteTexture(
                region(r),
                extent,
                data.to_vec(),
                row_pitch,
                rows_per_layer,
                mode,
            ));
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
            self.void(Call::BeginPass(Box::new(Pass {
                colors,
                depth: pass.depth.as_ref().map(|d| Depth {
                    view: *d.view,
                    depth: d.depth,
                    stencil: d.stencil,
                }),
                empty: pass.empty,
                occlusion: pass.occlusion.copied(),
            })));
        }

        fn end_pass(&mut self) {
            self.void(Call::EndPass);
        }

        fn draw(&mut self, call: &DrawCall<'_, Self>) {
            self.void(Call::Draw(draw(call)));
        }

        fn begin_query(&mut self, index: u32) {
            self.void(Call::BeginQuery(index));
        }

        fn end_query(&mut self) {
            self.void(Call::EndQuery);
        }

        fn copy_buffer(&mut self, src: &H, src_offset: u64, dst: &H, dst_offset: u64, size: u64) {
            self.void(Call::CopyBuffer(*src, src_offset, *dst, dst_offset, size));
        }

        fn copy_texture(
            &mut self,
            src: &TextureRegion<'_, Self>,
            dst: &TextureRegion<'_, Self>,
            extent: Extent,
        ) {
            self.void(Call::CopyTexture(region(src), region(dst), extent));
        }

        fn copy_buffer_to_texture(
            &mut self,
            src: &CopyBufferTexture<'_, Self>,
            dst: &TextureRegion<'_, Self>,
            extent: Extent,
        ) {
            self.void(Call::CopyBufferToTexture(side(src), region(dst), extent));
        }

        fn copy_texture_to_buffer(
            &mut self,
            src: &TextureRegion<'_, Self>,
            dst: &CopyBufferTexture<'_, Self>,
            extent: Extent,
        ) {
            self.void(Call::CopyTextureToBuffer(region(src), side(dst), extent));
        }

        fn clear_buffer(&mut self, buffer: &H, offset: u64, size: u64, pattern: &[u8]) {
            self.void(Call::ClearBuffer(*buffer, offset, size, pattern.to_vec()));
        }

        fn resolve(
            &mut self,
            src: &TextureRegion<'_, Self>,
            dst: &TextureRegion<'_, Self>,
            format: Format,
        ) {
            self.void(Call::Resolve(region(src), region(dst), format));
        }

        fn submit(&mut self) -> H {
            let h = self.id();
            self.void(Call::Submit(h));
            h
        }

        fn fence_done(&mut self, fence: &H) -> bool {
            matches!(self.run(Call::FenceDone(*fence)), Ok(Answer::Bool(true)))
        }

        fn wait_fence(&mut self, fence: &H) {
            self.void(Call::WaitFence(*fence));
        }

        fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<H, GpuError> {
            let s = source(src);
            self.make(|h| Call::ReadBack(h, s))
        }

        fn read_queries(&mut self, set: &H, first: u32, count: u32) -> Result<H, GpuError> {
            self.make(|h| Call::ReadQueries(h, *set, first, count))
        }

        fn read_state(&mut self, readback: &H) -> ReadState {
            match self.run(Call::ReadState(*readback)) {
                Ok(Answer::State(s)) => s,
                _ => ReadState::Ready,
            }
        }

        fn wait_read(&mut self, readback: &H) {
            self.void(Call::WaitRead(*readback));
        }

        fn finish_read(&mut self, readback: H, dst: &mut [u8]) {
            self.write(&Call::FinishRead(readback, dst.len() as u64));
            if let Some(r) = self.table.readbacks.remove(&readback) {
                self.inner.finish_read(r, dst);
            }
        }

        fn create_surface(&mut self, window: u32) -> Result<H, GpuError> {
            self.make(|h| Call::CreateSurface(h, window))
        }

        fn configure_surface(
            &mut self,
            surface: &H,
            format: Format,
            width: u32,
            height: u32,
            vsync: bool,
        ) -> Result<(), GpuError> {
            self.run(Call::ConfigureSurface(
                *surface, format, width, height, vsync,
            ))
            .map(|_| ())
        }

        fn current_texture(&mut self, surface: &H) -> Result<H, GpuError> {
            self.make(|h| Call::CurrentTexture(h, *surface))
        }

        fn present(&mut self, surface: &H) -> Result<(), GpuError> {
            let r = self.run(Call::Present(*surface)).map(|_| ());
            self.flush();
            r
        }

        fn read_back_now(
            &mut self,
            src: &ReadSource<'_, Self>,
            dst: &mut [u8],
        ) -> Result<(), GpuError> {
            let s = source(src);
            self.write(&Call::ReadBackNow(s, dst.len() as u64));
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
            self.run(Call::DrawFanOrLoop(
                draw(call),
                kind,
                indices,
                provoking,
                first_instance,
                instances,
            ))
            .map(|_| ())
        }

        fn prepare(&mut self, desc: &PrepareDesc<'_, Self>) {
            self.void(Call::Prepare(Box::new(Prepare {
                program: *desc.program,
                blend: *desc.blend,
                depth_stencil: *desc.depth_stencil,
                raster: *desc.raster,
                vertex_layout: *desc.vertex_layout,
                targets: desc.targets,
                topology: desc.topology,
                layouts: desc.layouts,
            })));
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
            self.run(Call::ClearTexture(
                *texture,
                *desc,
                format,
                [mips.start, mips.end],
                [layers.start, layers.end],
                value,
            ))
            .map(|_| ())
        }

        fn clear_rects(
            &mut self,
            view: &H,
            format: Format,
            value: ClearValue,
            rects: &[ScissorRect],
        ) -> Result<(), GpuError> {
            self.run(Call::ClearRects(*view, format, value, rects.to_vec()))
                .map(|_| ())
        }

        fn generate_mips(
            &mut self,
            texture: &H,
            desc: &TextureDesc,
            values: Format,
            mips: std::ops::Range<u32>,
            layers: std::ops::Range<u32>,
        ) -> Result<(), GpuError> {
            self.run(Call::GenerateMips(
                *texture,
                *desc,
                values,
                [mips.start, mips.end],
                [layers.start, layers.end],
            ))
            .map(|_| ())
        }

        fn write_buffer_ring(
            &mut self,
            kind: RingKind,
            parts: &[&[u8]],
            align: u32,
            offsets: &mut [u64],
        ) -> Result<H, GpuError> {
            let h = self.id();
            let owned = parts.iter().map(|p| p.to_vec()).collect();
            self.write(&Call::WriteBufferRing(h, kind, owned, align));
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
            self.run(Call::ResolveQueries(*set, first, count, *dst, dst_offset))
                .map(|_| ())
        }

        fn dispatch(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
            self.run(Call::Dispatch(Box::new(Dispatch {
                program: *call.program,
                bindings: sets(&call.bindings),
                immediates: call.immediates.to_vec(),
                groups: match call.groups {
                    DispatchGroups::Direct(g) => Ok(g),
                    DispatchGroups::Indirect { args, offset } => Err((*args, offset)),
                },
            })))
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
            self.run(Call::CaptureVertices(draw(call), targets, rasterize))
                .map(|_| ())
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
            self.run(Call::WriteTimestamp(*set, index)).map(|_| ())
        }

        fn timestamps_disjoint(&mut self) -> Result<bool, GpuError> {
            self.run(Call::TimestampsDisjoint)
                .map(|a| a == Answer::Bool(true))
        }
    }
}
