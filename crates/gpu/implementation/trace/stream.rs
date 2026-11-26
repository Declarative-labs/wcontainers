//! The stream format: every call of the interface as plain data naming objects by number, saved with
//! postcard (shaders as naga modules), and [`Replay`], which runs a stream on any back end.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wcontainers_gpu::{draw::*, error::*, format::*, resource::*, shader::*, state::*, *};

/// An object of the stream, numbered in the order the stream made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct H(pub u32);

/// A shader in the common form.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Shader {
    pub(crate) module: naga::Module,
    pub(crate) entry_point: String,
    pub(crate) stage: ShaderStage,
    pub(crate) key: [u8; 32],
}

impl Shader {
    pub(crate) fn of(m: &ShaderModule) -> Shader {
        Shader {
            module: m.module.clone(),
            entry_point: m.entry_point.clone(),
            stage: m.stage,
            key: m.key,
        }
    }

    /// The module validated again, as the front handed it over.
    pub(crate) fn module(&self) -> Result<ShaderModule, GpuError> {
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
pub(crate) enum Kind {
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
pub(crate) struct Region {
    pub(crate) texture: H,
    pub(crate) mip: u32,
    pub(crate) layer: u32,
    pub(crate) origin: Origin,
    pub(crate) stencil: bool,
}

/// The buffer side of a copy: buffer, offset, row pitch, rows per layer.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Side(pub(crate) H, pub(crate) u64, pub(crate) u32, pub(crate) u32);

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) enum Source {
    Buffer(H, u64, u64),
    Texture(Region, Extent),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Resource {
    Uniform(H, u64, u64),
    Storage(H, u64, u64, bool),
    Texture(H),
    StorageTexture(H, bool),
    Sampler(H),
    TexelBuffer(H, u64, u64, Format, bool),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) binding: u32,
    pub(crate) stages: StageSet,
    pub(crate) resource: Resource,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Color {
    pub(crate) view: H,
    pub(crate) depth_slice: u32,
    pub(crate) resolve: Option<H>,
    pub(crate) load: LoadOp<[u32; 4]>,
    pub(crate) store: StoreOp,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Depth {
    pub(crate) view: H,
    pub(crate) depth: Option<AspectOps<u32>>,
    pub(crate) stencil: Option<AspectOps<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Pass {
    pub(crate) colors: [Option<Color>; MAX_COLOR_TARGETS],
    pub(crate) depth: Option<Depth>,
    pub(crate) empty: Option<EmptyPass>,
    pub(crate) occlusion: Option<H>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) enum Range {
    Vertices([u32; 4]),
    /// First, count, base vertex, first instance, instances.
    Indexed(u32, u32, i32, u32, u32),
    Indirect(H, u64),
    IndexedIndirect(H, u64),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Draw {
    pub(crate) program: H,
    pub(crate) blend: H,
    pub(crate) depth_stencil: H,
    pub(crate) raster: H,
    pub(crate) vertex_layout: H,
    pub(crate) bindings: [Option<H>; MAX_BINDING_SETS],
    pub(crate) vertex_buffers: Vec<Option<(H, u64)>>,
    pub(crate) index: Option<(H, u64, IndexFormat)>,
    pub(crate) topology: Topology,
    pub(crate) restart: bool,
    pub(crate) viewports: Vec<Viewport>,
    pub(crate) scissors: Vec<ScissorRect>,
    pub(crate) blend_constant: [u32; 4],
    pub(crate) stencil_reference: [u8; 2],
    pub(crate) immediates: Vec<u8>,
    pub(crate) range: Range,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Prepare {
    pub(crate) program: H,
    pub(crate) blend: H,
    pub(crate) depth_stencil: H,
    pub(crate) raster: H,
    pub(crate) vertex_layout: H,
    pub(crate) targets: TargetLayout,
    pub(crate) topology: Topology,
    pub(crate) layouts: [BindingLayoutKey; MAX_BINDING_SETS],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Indices {
    Sequential(u32, u32),
    Guest(Vec<u8>, IndexFormat, i32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Dispatch {
    pub(crate) program: H,
    pub(crate) bindings: [Option<H>; MAX_BINDING_SETS],
    pub(crate) immediates: Vec<u8>,
    pub(crate) groups: Result<[u32; 3], (H, u64)>,
}

/// One call of the interface. A call that makes an object names the number it gets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Call {
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
    pub(crate) fn name(&self) -> &'static str {
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
pub(crate) struct Table<B: Gpu> {
    pub(crate) buffers: HashMap<H, B::Buffer>,
    pub(crate) textures: HashMap<H, B::Texture>,
    pub(crate) views: HashMap<H, B::TextureView>,
    pub(crate) samplers: HashMap<H, B::Sampler>,
    pub(crate) shaders: HashMap<H, B::Shader>,
    pub(crate) programs: HashMap<H, B::Program>,
    pub(crate) blends: HashMap<H, B::Blend>,
    pub(crate) depth_stencils: HashMap<H, B::DepthStencil>,
    pub(crate) rasters: HashMap<H, B::Raster>,
    pub(crate) layouts: HashMap<H, B::VertexLayout>,
    pub(crate) sets: HashMap<H, B::BindingSet>,
    pub(crate) query_sets: HashMap<H, B::QuerySet>,
    pub(crate) readbacks: HashMap<H, B::Readback>,
    pub(crate) fences: HashMap<H, B::Fence>,
    pub(crate) surfaces: HashMap<H, B::Surface>,
}

impl<B: Gpu> Default for Table<B> {
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

impl<B: Gpu> Table<B> {
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

    pub(crate) fn source(&self, s: &Source) -> Result<ReadSource<'_, B>, GpuError> {
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
pub(crate) enum Answer {
    Done,
    Bool(bool),
    State(ReadState),
}

/// Times the back end's own part of a call: the clock's reading is taken just around it.
pub(crate) struct Timer<'c> {
    pub(crate) clock: Option<&'c dyn Fn() -> f64>,
    pub(crate) spent: f64,
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
pub(crate) fn apply<B: Gpu>(
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
pub(crate) fn decode(bytes: &[u8]) -> Result<(Call, &[u8]), String> {
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
pub struct Replay<B: Gpu> {
    bytes: Vec<u8>,
    at: usize,
    table: Table<B>,
    scratch: Vec<u8>,
    /// Each texture's format, to name copies by theirs.
    formats: HashMap<H, Format>,
}

impl<B: Gpu> Replay<B> {
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
