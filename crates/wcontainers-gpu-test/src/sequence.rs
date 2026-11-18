//! Seeded random call sequences: [`steps`] generates them (a proptest strategy, so a failing one
//! shrinks), and [`run`] makes each step a valid call on any back end, choosing among the objects it
//! made so far. Every step's numbers are taken modulo what exists when it runs, so every generated
//! sequence is valid.

use proptest::prelude::*;
use wcontainers_gpu::*;

/// One step; its numbers choose sizes, objects and ranges when it runs.
#[derive(Clone, Debug)]
pub enum Step {
    Buffer(u16),
    Texture {
        format: u8,
        size: [u8; 2],
        layers: u8,
        mips: u8,
    },
    WriteBuffer {
        which: u16,
        at: [u16; 2],
        mode: u8,
        seed: u8,
    },
    WriteTexture {
        which: u16,
        at: [u8; 6],
        mode: u8,
        pitch: u8,
        seed: u8,
    },
    CopyBuffer {
        which: [u16; 2],
        at: [u16; 3],
    },
    CopyTexture {
        which: [u16; 2],
        at: [u8; 8],
    },
    BufferToTexture {
        which: [u16; 2],
        at: [u8; 6],
        offset: u16,
        pitch: u8,
    },
    TextureToBuffer {
        which: [u16; 2],
        at: [u8; 6],
        offset: u16,
        pitch: u8,
    },
    ClearBuffer {
        which: u16,
        at: [u16; 2],
        pattern: u8,
        seed: u8,
    },
    ReadBuffer {
        which: u16,
        at: [u16; 2],
    },
    ReadTexture {
        which: u16,
        at: [u8; 6],
    },
    /// A pass on a texture's first level and layer: its load, and triangles of random corners, depth
    /// and colours, each blended or not; or, where `multisampled` and the format averages, a pass on
    /// a four-sample texture of that size resolved into it. `options` choose the rest.
    Pass {
        which: u16,
        load: u8,
        triangles: Vec<([f32; 7], [f32; 4], bool)>,
        multisampled: bool,
        options: PassOptions,
    },
    ClearTexture {
        which: u16,
        value: [f32; 4],
    },
    Destroy(u16),
    Submit,
    /// A dispatch updating each word of a buffer, where the back end has compute.
    Dispatch(u16),
    /// Vertices captured into a buffer, one to three triangles, where it has vertex capture.
    Capture {
        which: u16,
        triangles: u8,
    },
    /// A timestamp written into one of two, both read back, where it has timestamps.
    Timestamp(u8),
    /// A depth or stencil texture of its own, one level and layer of it written, a region copied
    /// beside itself in that subresource, each aspect read back, then destroyed.
    DepthCopy {
        format: u8,
        size: [u8; 2],
        layers: u8,
        mips: u8,
        at: [u8; 6],
        seed: u8,
    },
}

/// What else a pass does, each where present: a depth-stencil target (its cleared depth and
/// stencil, tested and written by the draws, both aspects read back after a single-sampled pass), a
/// scissor rectangle, indexed draws (16 or 32-bit, instanced from a first instance), a tint read
/// from a uniform binding at one of two aligned offsets, and an occlusion query read back.
#[derive(Clone, Debug)]
pub struct PassOptions {
    pub depth: Option<(f32, u8)>,
    pub scissor: Option<[u8; 4]>,
    pub indexed: Option<u8>,
    pub tint: Option<([f32; 4], bool)>,
    pub query: bool,
}

/// Sequences of 1 to `max` steps.
pub fn steps(max: usize) -> impl Strategy<Value = Vec<Step>> {
    let corner = -1.5f32..1.5;
    let unit = 0f32..1.0;
    let step = prop_oneof![
        any::<u16>().prop_map(Step::Buffer),
        (any::<u8>(), any::<[u8; 2]>(), any::<u8>(), any::<u8>()).prop_map(
            |(format, size, layers, mips)| Step::Texture {
                format,
                size,
                layers,
                mips
            }
        ),
        (any::<u16>(), any::<[u16; 2]>(), any::<u8>(), any::<u8>()).prop_map(
            |(which, at, mode, seed)| Step::WriteBuffer {
                which,
                at,
                mode,
                seed
            }
        ),
        (
            any::<u16>(),
            any::<[u8; 6]>(),
            any::<u8>(),
            any::<u8>(),
            any::<u8>()
        )
            .prop_map(|(which, at, mode, pitch, seed)| Step::WriteTexture {
                which,
                at,
                mode,
                pitch,
                seed
            }),
        (any::<[u16; 2]>(), any::<[u16; 3]>())
            .prop_map(|(which, at)| Step::CopyBuffer { which, at }),
        (any::<[u16; 2]>(), any::<[u8; 8]>())
            .prop_map(|(which, at)| Step::CopyTexture { which, at }),
        (
            any::<[u16; 2]>(),
            any::<[u8; 6]>(),
            any::<u16>(),
            any::<u8>()
        )
            .prop_map(|(which, at, offset, pitch)| Step::BufferToTexture {
                which,
                at,
                offset,
                pitch
            }),
        (
            any::<[u16; 2]>(),
            any::<[u8; 6]>(),
            any::<u16>(),
            any::<u8>()
        )
            .prop_map(|(which, at, offset, pitch)| Step::TextureToBuffer {
                which,
                at,
                offset,
                pitch
            }),
        (any::<u16>(), any::<[u16; 2]>(), any::<u8>(), any::<u8>()).prop_map(
            |(which, at, pattern, seed)| Step::ClearBuffer {
                which,
                at,
                pattern,
                seed
            }
        ),
        (any::<u16>(), any::<[u16; 2]>()).prop_map(|(which, at)| Step::ReadBuffer { which, at }),
        (any::<u16>(), any::<[u8; 6]>()).prop_map(|(which, at)| Step::ReadTexture { which, at }),
        (
            any::<u16>(),
            any::<u8>(),
            prop::collection::vec(
                (
                    prop::array::uniform7(corner),
                    prop::array::uniform4(unit.clone()),
                    any::<bool>()
                ),
                0..4
            ),
            any::<bool>(),
            (
                prop::option::of((unit.clone(), any::<u8>())),
                prop::option::of(any::<[u8; 4]>()),
                prop::option::of(any::<u8>()),
                prop::option::of((prop::array::uniform4(unit.clone()), any::<bool>())),
                any::<bool>()
            )
                .prop_map(|(depth, scissor, indexed, tint, query)| PassOptions {
                    depth,
                    scissor,
                    indexed,
                    tint,
                    query
                })
        )
            .prop_map(
                |(which, load, triangles, multisampled, options)| Step::Pass {
                    which,
                    load,
                    triangles,
                    multisampled,
                    options
                }
            ),
        (any::<u16>(), prop::array::uniform4(unit))
            .prop_map(|(which, value)| Step::ClearTexture { which, value }),
        any::<u16>().prop_map(Step::Destroy),
        Just(Step::Submit),
        any::<u16>().prop_map(Step::Dispatch),
        (any::<u16>(), any::<u8>())
            .prop_map(|(which, triangles)| Step::Capture { which, triangles }),
        any::<u8>().prop_map(Step::Timestamp),
        (
            any::<u8>(),
            any::<[u8; 2]>(),
            any::<u8>(),
            any::<u8>(),
            any::<[u8; 6]>(),
            any::<u8>()
        )
            .prop_map(|(format, size, layers, mips, at, seed)| Step::DepthCopy {
                format,
                size,
                layers,
                mips,
                at,
                seed
            }),
    ];
    prop::collection::vec(step, 1..=max)
}

const PROGRAMS: &str = "
struct V { @builtin(position) p: vec4<f32>, @location(0) c: vec4<f32> }
@vertex fn vs(@location(0) p: vec3<f32>, @location(1) c: vec4<f32>) -> V {
    return V(vec4<f32>(p.xy, p.z * 0.5 + 0.5, 1.0), c);
}
@fragment fn float_out(v: V) -> @location(0) vec4<f32> {
    return v.c;
}
@fragment fn uint_out(v: V) -> @location(0) vec4<u32> {
    return vec4<u32>(v.c * 255.0);
}
@fragment fn sint_out(v: V) -> @location(0) vec4<i32> {
    return vec4<i32>(v.c * 255.0) - 128;
}
@group(0) @binding(0) var<uniform> tint: vec4<f32>;
@fragment fn tinted(v: V) -> @location(0) vec4<f32> {
    return v.c * tint;
}
@group(0) @binding(1) var<storage, read_write> words: array<u32>;
@compute @workgroup_size(1) fn step(@builtin(global_invocation_id) g: vec3<u32>) {
    words[g.x] = words[g.x] * 3u + g.x + 1u;
}
struct Counted { @builtin(position) p: vec4<f32>, @location(0) c: vec4<f32> }
@vertex fn counted(@builtin(vertex_index) i: u32, @builtin(instance_index) k: u32) -> Counted {
    return Counted(vec4<f32>(f32(i), f32(k), 0.5, 1.0), vec4<f32>(f32(i) * 0.25, f32(k), 0.0, 1.0));
}
";

/// The formats random textures take, of each kind and size, where the back end draws into them.
const FORMATS: [Format; 10] = [
    Format::Rgba8Unorm,
    Format::Bgra8Unorm,
    Format::Rgba8UnormSrgb,
    Format::R8Unorm,
    Format::Rg16Float,
    Format::Rgba16Float,
    Format::R32Float,
    Format::R32Uint,
    Format::Rgba16Uint,
    Format::R8Sint,
];

/// The formats of [`Step::DepthCopy`], where the back end copies them out.
const DEPTH_FORMATS: [Format; 6] = [
    Format::Depth16Unorm,
    Format::Depth24Plus,
    Format::Depth32Float,
    Format::Stencil8,
    Format::Depth24PlusStencil8,
    Format::Depth32FloatStencil8,
];

/// What the steps draw with, made once.
struct Kit<B: Backend> {
    float_program: B::Program,
    uint_program: B::Program,
    sint_program: B::Program,
    tinted_program: B::Program,
    opaque: B::Blend,
    added: B::Blend,
    depth: B::DepthStencil,
    /// Depth tested and written, stencil incremented where the depth test passes.
    tested: B::DepthStencil,
    raster: B::Raster,
    scissored: B::Raster,
    layout: B::VertexLayout,
    /// The dispatch's program, where the back end has compute.
    step: Option<B::Program>,
    /// A vertex program reading no stream, with a layout of none, where it has vertex capture.
    counted: Option<(B::Program, B::VertexLayout)>,
    timestamps: Option<B::QuerySet>,
}

fn kit<B: Backend>(b: &mut B) -> Result<Kit<B>, GpuError> {
    let module = naga::front::wgsl::parse_str(PROGRAMS).map_err(|_| GpuError::Invalid("wgsl"))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|_| GpuError::Invalid("wgsl"))?;
    let shader = |b: &mut B, entry: &str, stage| {
        b.create_shader(&ShaderModule {
            module: module.clone(),
            info: info.clone(),
            entry_point: entry.into(),
            stage,
            key: *blake3::hash(entry.as_bytes()).as_bytes(),
        })
    };
    let vs = shader(b, "vs", ShaderStage::Vertex)?;
    let float_fs = shader(b, "float_out", ShaderStage::Fragment)?;
    let uint_fs = shader(b, "uint_out", ShaderStage::Fragment)?;
    let float_program = b.create_program(&Program::Render {
        vertex: &vs,
        fragment: Some(&float_fs),
    })?;
    let uint_program = b.create_program(&Program::Render {
        vertex: &vs,
        fragment: Some(&uint_fs),
    })?;
    let sint_fs = shader(b, "sint_out", ShaderStage::Fragment)?;
    let sint_program = b.create_program(&Program::Render {
        vertex: &vs,
        fragment: Some(&sint_fs),
    })?;
    let tinted_fs = shader(b, "tinted", ShaderStage::Fragment)?;
    let tinted_program = b.create_program(&Program::Render {
        vertex: &vs,
        fragment: Some(&tinted_fs),
    })?;
    let blend = |b: &mut B, blend| {
        b.create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend,
                writes: ColorWrites::all(),
            }; MAX_COLOR_TARGETS],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
        })
    };
    let add = BlendComponent {
        src: BlendFactor::One,
        dst: BlendFactor::One,
        op: BlendOp::Add,
    };
    let opaque = blend(b, None)?;
    let added = blend(b, Some((add, add)))?;
    let depth = b.create_depth_stencil(&DepthStencilDesc {
        depth_compare: None,
        depth_write: false,
        stencil: None,
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    })?;
    let face = StencilFace {
        compare: CompareFn::Always,
        fail: StencilOp::Keep,
        depth_fail: StencilOp::Keep,
        pass: StencilOp::IncrementClamp,
    };
    let tested = b.create_depth_stencil(&DepthStencilDesc {
        depth_compare: Some(CompareFn::Less),
        depth_write: true,
        stencil: Some((face, face)),
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    })?;
    let raster_desc = RasterDesc {
        fill: FillMode::Solid,
        cull: CullMode::None,
        front: FrontFace::CounterClockwise,
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking: ProvokingVertex::First,
        depth_minus_one: false,
        sample_mask: !0,
    };
    let raster = b.create_raster(&raster_desc)?;
    let scissored = b.create_raster(&RasterDesc {
        scissor: true,
        ..raster_desc
    })?;
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    desc.streams[0] = Some(VertexStream {
        stride: 28,
        step: StepMode::Vertex,
    });
    desc.attributes[0] = Some(VertexAttribute {
        location: 0,
        stream: 0,
        offset: 0,
        format: VertexFormat::Float32x3,
    });
    desc.attributes[1] = Some(VertexAttribute {
        location: 1,
        stream: 0,
        offset: 12,
        format: VertexFormat::Float32x4,
    });
    let layout = b.create_vertex_layout(&desc)?;
    let has = b.caps().features;
    let step = if has.contains(Features::COMPUTE) {
        let c = shader(b, "step", ShaderStage::Compute)?;
        Some(b.create_program(&Program::Compute(&c))?)
    } else {
        None
    };
    let counted = if has.contains(Features::TRANSFORM_FEEDBACK) {
        let v = shader(b, "counted", ShaderStage::Vertex)?;
        let program = b.create_program(&Program::Render {
            vertex: &v,
            fragment: None,
        })?;
        let none = b.create_vertex_layout(&VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        })?;
        Some((program, none))
    } else {
        None
    };
    let timestamps = if has.contains(Features::TIMESTAMPS) {
        Some(b.create_query_set(QueryKind::Timestamp, 2)?)
    } else {
        None
    };
    Ok(Kit {
        step,
        counted,
        timestamps,
        float_program,
        uint_program,
        sint_program,
        tinted_program,
        opaque,
        added,
        depth,
        tested,
        raster,
        scissored,
        layout,
    })
}

/// `n` modulo `m`, for choosing; 0 when there is nothing to choose from.
fn pick(n: impl Into<u64>, m: usize) -> usize {
    if m == 0 {
        0
    } else {
        (n.into() % m as u64) as usize
    }
}

/// A sub-range of `0..len`: its start and length from two numbers, at least one byte or texel.
fn span(a: impl Into<u64>, b: impl Into<u64>, len: u64) -> (u64, u64) {
    let start = a.into() % len;
    (start, 1 + b.into() % (len - start))
}

fn bytes(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(131).wrapping_add(seed))
        .collect()
}

fn texel_size(f: Format) -> u32 {
    f.block().2
}

/// A region of level `mip` of a texture, chosen by six numbers: mip, layer, x, y, width, height.
fn region_of(desc: &TextureDesc, at: &[u8]) -> (u32, u32, Origin, Extent) {
    let mip = pick(at[0], desc.mip_levels as usize) as u32;
    let layer = pick(at[1], desc.depth_or_layers as usize) as u32;
    let (w, h) = ((desc.width >> mip).max(1), (desc.height >> mip).max(1));
    let (x, width) = span(at[2], at[3], u64::from(w));
    let (y, height) = span(at[4], at[5], u64::from(h));
    (
        mip,
        layer,
        Origin {
            x: x as u32,
            y: y as u32,
            z: 0,
        },
        Extent {
            width: width as u32,
            height: height as u32,
            depth_or_layers: 1,
        },
    )
}

fn region<B: Backend>(
    t: &B::Texture,
    mip: u32,
    layer: u32,
    origin: Origin,
) -> TextureRegion<'_, B> {
    TextureRegion {
        texture: t,
        mip,
        layer,
        origin,
        stencil: false,
    }
}

/// Texels of one level and layer of a texture (by index), a rectangle from `min` to `max`.
#[derive(Clone, Copy)]
struct Area {
    texture: usize,
    mip: u32,
    layer: u32,
    min: [u32; 2],
    max: [u32; 2],
}

impl Area {
    fn of(texture: usize, mip: u32, layer: u32, origin: Origin, e: Extent) -> Self {
        Self {
            texture,
            mip,
            layer,
            min: [origin.x, origin.y],
            max: [origin.x + e.width, origin.y + e.height],
        }
    }

    /// Every texel of a level and layer.
    fn whole(texture: usize, mip: u32, layer: u32) -> Self {
        Self {
            texture,
            mip,
            layer,
            min: [0; 2],
            max: [u32::MAX; 2],
        }
    }

    fn meets(&self, o: &Self) -> bool {
        (self.texture, self.mip, self.layer) == (o.texture, o.mip, o.layer)
            && (0..2).all(|k| self.min[k] < o.max[k] && o.min[k] < self.max[k])
    }
}

/// Runs `steps` on `b` as valid calls, and gives every byte read back, in order.
pub fn run<B: Backend>(b: &mut B, steps: &[Step]) -> Result<Vec<Vec<u8>>, GpuError> {
    let kit = kit(b)?;
    let mut buffers: Vec<(B::Buffer, u64)> = Vec::new();
    let mut textures: Vec<(B::Texture, TextureDesc)> = Vec::new();
    let mut read = Vec::new();
    let all = BufferUsage::COPY_SRC
        | BufferUsage::COPY_DST
        | BufferUsage::VERTEX
        | BufferUsage::STORAGE
        | BufferUsage::CAPTURE;
    let drawn = FormatUse::RENDER | FormatUse::SAMPLE;
    let formats: Vec<Format> = FORMATS
        .into_iter()
        .filter(|&f| b.caps().formats.get(f).contains(drawn))
        .collect();
    let depth_formats: Vec<Format> = DEPTH_FORMATS
        .into_iter()
        .filter(|&f| b.caps().formats.get(f).contains(FormatUse::COPY_OUT))
        .collect();
    // Buffer ranges (by index into `buffers`) commands or writes touched since the last wait for
    // all work: what a no-overwrite write keeps off without waiting.
    let mut pending: Vec<(usize, u64, u64)> = Vec::new();
    // The same for texels.
    let mut texels: Vec<Area> = Vec::new();
    let finish = |b: &mut B, rb: B::Readback, len: usize, read: &mut Vec<Vec<u8>>| {
        b.submit();
        b.wait_read(&rb);
        let mut out = vec![0; len];
        b.finish_read(rb, &mut out);
        read.push(out);
    };
    for step in steps {
        match step {
            Step::Buffer(size) => {
                let size = 4 + u64::from(*size % 253);
                let buffer = b.create_buffer(&BufferDesc {
                    size,
                    usage: all,
                    intent: MapIntent::Rarely,
                })?;
                buffers.push((buffer, size));
            }
            Step::Texture {
                format,
                size,
                layers,
                mips,
            } => {
                let (width, height) = (1 + u32::from(size[0] % 8), 1 + u32::from(size[1] % 8));
                // A second level where the full chain has one.
                let second = width.max(height) > 1 && *mips % 2 == 1;
                let desc = TextureDesc {
                    dimension: TextureDimension::D2,
                    format: formats[pick(*format, formats.len())],
                    width,
                    height,
                    depth_or_layers: 1 + u32::from(*layers % 2),
                    mip_levels: 1 + u32::from(second),
                    samples: 1,
                    usage: TextureUsage::RENDER
                        | TextureUsage::SAMPLE
                        | TextureUsage::COPY_SRC
                        | TextureUsage::COPY_DST,
                    cube: false,
                    reinterpret: false,
                };
                textures.push((b.create_texture(&desc)?, desc));
            }
            Step::WriteBuffer {
                which,
                at,
                mode,
                seed,
            } if !buffers.is_empty() => {
                let i = pick(*which, buffers.len());
                let (buffer, size) = &buffers[i];
                let (offset, len) = span(at[0], at[1], *size);
                let mode = [
                    WriteMode::Ordered,
                    WriteMode::Discard,
                    WriteMode::NoOverwrite,
                ][pick(*mode, 3)];
                // A no-overwrite write touches nothing an unfinished command uses or an ordered
                // write not waited for wrote; where it would, wait for all first.
                if mode == WriteMode::NoOverwrite
                    && pending
                        .iter()
                        .any(|&(j, s, e)| j == i && s < offset + len && offset < e)
                {
                    let fence = b.submit();
                    b.wait_fence(&fence);
                    pending.clear();
                    texels.clear();
                }
                // A discard gives the buffer fresh contents that nothing earlier uses.
                match mode {
                    WriteMode::Ordered => pending.push((i, offset, offset + len)),
                    WriteMode::Discard => pending.retain(|p| p.0 != i),
                    WriteMode::NoOverwrite => {}
                }
                b.write_buffer(buffer, offset, &bytes(len as usize, *seed), mode);
            }
            Step::WriteTexture {
                which,
                at,
                mode,
                pitch,
                seed,
            } if !textures.is_empty() => {
                let i = pick(*which, textures.len());
                let (t, desc) = &textures[i];
                let (mip, layer, origin, e) = region_of(desc, at);
                let texel = texel_size(desc.format);
                let row = e.width * texel;
                let row_pitch = row + texel * u32::from(*pitch % 3);
                let data = bytes((row_pitch * (e.height - 1) + row) as usize, *seed);
                let mode = [
                    WriteMode::Ordered,
                    WriteMode::Discard,
                    WriteMode::NoOverwrite,
                ][pick(*mode, 3)];
                let area = Area::of(i, mip, layer, origin, e);
                if mode == WriteMode::NoOverwrite && texels.iter().any(|a| a.meets(&area)) {
                    let fence = b.submit();
                    b.wait_fence(&fence);
                    pending.clear();
                    texels.clear();
                }
                match mode {
                    WriteMode::Ordered => texels.push(area),
                    WriteMode::Discard => texels.retain(|a| a.texture != i),
                    WriteMode::NoOverwrite => {}
                }
                b.write_texture(
                    &region::<B>(t, mip, layer, origin),
                    e,
                    &data,
                    row_pitch,
                    e.height,
                    mode,
                );
            }
            Step::CopyBuffer { which, at } if !buffers.is_empty() => {
                let (src, ss) = &buffers[pick(which[0], buffers.len())];
                let (dst, ds) = &buffers[pick(which[1], buffers.len())];
                let (from, len) = span(at[0], at[1], *ss);
                let len = len.min(*ds);
                let to = u64::from(at[2]) % (ds - len + 1);
                let (i, j) = (pick(which[0], buffers.len()), pick(which[1], buffers.len()));
                if i == j && from < to + len && to < from + len {
                    continue;
                }
                pending.push((i, from, from + len));
                pending.push((j, to, to + len));
                b.copy_buffer(src, from, dst, to, len);
            }
            Step::CopyTexture { which, at } if !textures.is_empty() => {
                let (i, j) = (
                    pick(which[0], textures.len()),
                    pick(which[1], textures.len()),
                );
                let ((s, sd), (d, dd)) = (&textures[i], &textures[j]);
                if sd.format != dd.format {
                    continue;
                }
                let (smip, slayer, so, se) = region_of(sd, &at[..6]);
                let dmip = pick(at[6], dd.mip_levels as usize) as u32;
                let dlayer = pick(at[7], dd.depth_or_layers as usize) as u32;
                let (dw, dh) = ((dd.width >> dmip).max(1), (dd.height >> dmip).max(1));
                let e = Extent {
                    width: se.width.min(dw),
                    height: se.height.min(dh),
                    depth_or_layers: 1,
                };
                let mut dst_origin = Origin {
                    x: (dw - e.width).min(so.x),
                    y: (dh - e.height).min(so.y),
                    z: 0,
                };
                // Within one subresource, the region beside the source: right, left, below or
                // above it, where one fits.
                if i == j && smip == dmip && slayer == dlayer {
                    let (x, y) = (so.x, so.y);
                    let (w, h) = (e.width, e.height);
                    if x + 2 * w <= dw {
                        dst_origin.x = x + w;
                    } else if x >= w {
                        dst_origin.x = x - w;
                    } else if y + 2 * h <= dh {
                        dst_origin.y = y + h;
                    } else if y >= h {
                        dst_origin.y = y - h;
                    }
                }
                let (from, to) = (
                    Area::of(i, smip, slayer, so, e),
                    Area::of(j, dmip, dlayer, dst_origin, e),
                );
                if from.meets(&to) {
                    continue;
                }
                texels.push(from);
                texels.push(to);
                b.copy_texture(
                    &region::<B>(s, smip, slayer, so),
                    &region::<B>(d, dmip, dlayer, dst_origin),
                    e,
                );
            }
            Step::BufferToTexture {
                which,
                at,
                offset,
                pitch,
            }
            | Step::TextureToBuffer {
                which,
                at,
                offset,
                pitch,
            } if !buffers.is_empty() && !textures.is_empty() => {
                let i = pick(which[0], buffers.len());
                let (buffer, size) = &buffers[i];
                let k = pick(which[1], textures.len());
                let (t, desc) = &textures[k];
                let (mip, layer, origin, e) = region_of(desc, at);
                let texel = texel_size(desc.format);
                let row = e.width * texel;
                let row_pitch = row + texel * u32::from(*pitch % 3);
                let need = u64::from(row_pitch * (e.height - 1) + row);
                if need > *size {
                    continue;
                }
                // A whole number of texels from the buffer's start.
                let texel = u64::from(texel);
                let offset = u64::from(*offset) % ((size - need) / texel + 1) * texel;
                pending.push((i, offset, offset + need));
                texels.push(Area::of(k, mip, layer, origin, e));
                let side = CopyBufferTexture {
                    buffer,
                    offset,
                    row_pitch,
                    rows_per_layer: e.height,
                };
                let r = region::<B>(t, mip, layer, origin);
                if matches!(step, Step::BufferToTexture { .. }) {
                    b.copy_buffer_to_texture(&side, &r, e);
                } else {
                    b.copy_texture_to_buffer(&r, &side, e);
                }
            }
            Step::ClearBuffer {
                which,
                at,
                pattern,
                seed,
            } if !buffers.is_empty() => {
                let i = pick(*which, buffers.len());
                let (buffer, size) = &buffers[i];
                let unit = [1u64, 2, 4, 8, 16][pick(*pattern, 5)];
                if unit > *size {
                    continue;
                }
                let blocks = size / unit;
                let (first, count) = span(at[0], at[1], blocks);
                pending.push((i, first * unit, (first + count) * unit));
                b.clear_buffer(
                    buffer,
                    first * unit,
                    count * unit,
                    &bytes(unit as usize, *seed),
                );
            }
            Step::ReadBuffer { which, at } if !buffers.is_empty() => {
                let i = pick(*which, buffers.len());
                let (buffer, size) = &buffers[i];
                let (offset, len) = span(at[0], at[1], *size);
                pending.push((i, offset, offset + len));
                let rb = b.read_back(&ReadSource::Buffer {
                    buffer,
                    offset,
                    size: len,
                })?;
                finish(b, rb, len as usize, &mut read);
            }
            Step::ReadTexture { which, at } if !textures.is_empty() => {
                let i = pick(*which, textures.len());
                let (t, desc) = &textures[i];
                let (mip, layer, origin, e) = region_of(desc, at);
                texels.push(Area::of(i, mip, layer, origin, e));
                let rb = b.read_back(&ReadSource::Texture {
                    region: region::<B>(t, mip, layer, origin),
                    extent: e,
                })?;
                let len = (e.width * e.height * texel_size(desc.format)) as usize;
                finish(b, rb, len, &mut read);
            }
            Step::Pass {
                which,
                load,
                triangles,
                multisampled,
                options,
            } if !textures.is_empty() => {
                let i = pick(*which, textures.len());
                let (t, desc) = &textures[i];
                texels.push(Area::whole(i, 0, 0));
                pass(
                    b,
                    &kit,
                    t,
                    desc,
                    *load,
                    triangles,
                    *multisampled,
                    options,
                    &mut read,
                )?;
            }
            Step::ClearTexture { which, value } if !textures.is_empty() => {
                let i = pick(*which, textures.len());
                let (t, desc) = &textures[i];
                for mip in 0..desc.mip_levels {
                    for layer in 0..desc.depth_or_layers {
                        texels.push(Area::whole(i, mip, layer));
                    }
                }
                let value = match desc.format.kind() {
                    FormatKind::Uint => value.map(|v| (v * 255.0) as u32),
                    FormatKind::Sint => value.map(|v| ((v * 255.0) as i32 - 128) as u32),
                    _ => value.map(f32::to_bits),
                };
                b.clear_texture(
                    t,
                    desc,
                    desc.format,
                    0..desc.mip_levels,
                    0..desc.depth_or_layers,
                    ClearValue::Color(value),
                )?;
            }
            Step::Destroy(which) => {
                let n = buffers.len() + textures.len();
                if n == 0 {
                    continue;
                }
                let i = pick(*which, n);
                if i < buffers.len() {
                    b.destroy(Object::Buffer(buffers.remove(i).0));
                    pending.retain(|p| p.0 != i);
                    for p in &mut pending {
                        if p.0 > i {
                            p.0 -= 1;
                        }
                    }
                } else {
                    let i = i - buffers.len();
                    b.destroy(Object::Texture(textures.remove(i).0));
                    texels.retain(|a| a.texture != i);
                    for a in &mut texels {
                        if a.texture > i {
                            a.texture -= 1;
                        }
                    }
                }
            }
            Step::Submit => {
                b.submit();
            }
            Step::Dispatch(which) if kit.step.is_some() && !buffers.is_empty() => {
                let i = pick(*which, buffers.len());
                let (buffer, size) = &buffers[i];
                let words = size / 4;
                let set = b.create_binding_set(&[BindingEntry {
                    binding: 1,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer,
                            offset: 0,
                            size: words * 4,
                        },
                        writable: true,
                    },
                }])?;
                pending.push((i, 0, words * 4));
                b.dispatch(&DispatchCall {
                    program: kit.step.as_ref().ok_or(GpuError::Invalid("no compute"))?,
                    bindings: [Some(&set), None, None, None],
                    immediates: &[],
                    groups: DispatchGroups::Direct([words as u32, 1, 1]),
                })?;
                b.destroy(Object::BindingSet(set));
            }
            Step::Capture { which, triangles } if kit.counted.is_some() && !buffers.is_empty() => {
                let i = pick(*which, buffers.len());
                let (buffer, size) = &buffers[i];
                // One vec4 of 16 bytes a vertex, as many triangles as fit, at most three.
                let triangles = u64::from(1 + triangles % 3).min(size / 48);
                let (program, none) = kit
                    .counted
                    .as_ref()
                    .ok_or(GpuError::Invalid("no capture"))?;
                if triangles > 0 {
                    pending.push((i, 0, triangles * 48));
                    b.capture_vertices(
                        &DrawCall {
                            program,
                            blend: &kit.opaque,
                            depth_stencil: &kit.depth,
                            raster: &kit.raster,
                            vertex_layout: none,
                            bindings: [None; MAX_BINDING_SETS],
                            vertex_buffers: &[],
                            index: None,
                            topology: Topology::TriangleList,
                            restart: false,
                            viewports: &[],
                            scissors: &[],
                            blend_constant: [0; 4],
                            stencil_reference: [0; 2],
                            immediates: &[],
                            range: DrawRange::Vertices {
                                first: 0,
                                count: triangles as u32 * 3,
                                first_instance: 0,
                                instances: 1,
                            },
                        },
                        &[BufferRange {
                            buffer,
                            offset: 0,
                            size: triangles * 48,
                        }],
                        false,
                    )?;
                }
            }
            Step::DepthCopy {
                format,
                size,
                layers,
                mips,
                at,
                seed,
            } if !depth_formats.is_empty() => {
                depth_copy(
                    b,
                    depth_formats[pick(*format, depth_formats.len())],
                    *size,
                    *layers,
                    *mips,
                    at,
                    *seed,
                    &mut read,
                )?;
            }
            Step::Timestamp(n) if kit.timestamps.is_some() => {
                let set = kit
                    .timestamps
                    .as_ref()
                    .ok_or(GpuError::Invalid("no timestamps"))?;
                b.write_timestamp(set, u32::from(*n % 2))?;
                let rb = b.read_queries(set, 0, 2)?;
                finish(b, rb, 16, &mut read);
            }
            _ => {}
        }
    }
    Ok(read)
}

/// [`Step::DepthCopy`]: depth in eighths (which a draw writes exactly), stencil any; the region
/// beside the source to its right, or below it where the level is one texel wide. Each aspect is read
/// before and after the copy, and the after must be the before with the region moved.
#[allow(clippy::too_many_arguments)]
fn depth_copy<B: Backend>(
    b: &mut B,
    format: Format,
    size: [u8; 2],
    layers: u8,
    mips: u8,
    at: &[u8; 6],
    seed: u8,
    read: &mut Vec<Vec<u8>>,
) -> Result<(), GpuError> {
    let (width, height) = (2 + u32::from(size[0] % 7), 1 + u32::from(size[1] % 8));
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width,
        height,
        depth_or_layers: 1 + u32::from(layers % 2),
        mip_levels: 1 + u32::from(mips % 2),
        samples: 1,
        usage: TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
        cube: false,
        reinterpret: false,
    };
    let mip = pick(at[0], desc.mip_levels as usize) as u32;
    let layer = pick(at[1], desc.depth_or_layers as usize) as u32;
    let (w, h) = ((width >> mip).max(1), (height >> mip).max(1));
    // Along x where the level is at least two wide, else along y; nowhere at one by one.
    let (along, across) = if w >= 2 { (w, h) } else { (h, w) };
    if along < 2 {
        return Ok(());
    }
    let long = 1 + u32::from(at[2]) % (along / 2);
    let start = u32::from(at[3]) % (along - 2 * long + 1);
    let (offset, wide) = span(at[4], at[5], u64::from(across));
    let (offset, wide) = (offset as u32, wide as u32);
    let (src, dst, e) = if w >= 2 {
        (
            Origin {
                x: start,
                y: offset,
                z: 0,
            },
            Origin {
                x: start + long,
                y: offset,
                z: 0,
            },
            Extent {
                width: long,
                height: wide,
                depth_or_layers: 1,
            },
        )
    } else {
        (
            Origin {
                x: offset,
                y: start,
                z: 0,
            },
            Origin {
                x: offset,
                y: start + long,
                z: 0,
            },
            Extent {
                width: wide,
                height: long,
                depth_or_layers: 1,
            },
        )
    };
    let t = b.create_texture(&desc)?;
    let whole = Extent {
        width: w,
        height: h,
        depth_or_layers: 1,
    };
    let at_aspect = |stencil| TextureRegion {
        stencil,
        ..region::<B>(&t, mip, layer, Origin { x: 0, y: 0, z: 0 })
    };
    let kind = format.kind();
    let has_depth = kind != FormatKind::Stencil;
    let has_stencil = kind != FormatKind::Depth;
    let depth_bytes = if format == Format::Depth16Unorm { 2 } else { 4 };
    let texels = (w * h) as usize;
    if has_depth {
        let data: Vec<u8> = (0..texels)
            .flat_map(|i| {
                let mut texel = [0; 4];
                let eighth = (i * 131 + usize::from(seed)) % 9;
                crate::texel::encode_depth(format, eighth as f64 / 8.0, &mut texel);
                texel.into_iter().take(depth_bytes)
            })
            .collect();
        b.write_texture(
            &at_aspect(false),
            whole,
            &data,
            w * depth_bytes as u32,
            h,
            WriteMode::Ordered,
        );
    }
    if has_stencil {
        b.write_texture(
            &at_aspect(true),
            whole,
            &bytes(texels, seed),
            w,
            h,
            WriteMode::Ordered,
        );
    }
    let aspects: Vec<(bool, usize)> = [(false, depth_bytes), (true, 1)]
        .into_iter()
        .filter(|&(stencil, _)| if stencil { has_stencil } else { has_depth })
        .collect();
    let read_aspect = |b: &mut B, stencil: bool, texel: usize| -> Result<Vec<u8>, GpuError> {
        let rb = b.read_back(&ReadSource::Texture {
            region: at_aspect(stencil),
            extent: whole,
        })?;
        b.submit();
        b.wait_read(&rb);
        let mut out = vec![0; texels * texel];
        b.finish_read(rb, &mut out);
        Ok(out)
    };
    let before = aspects
        .iter()
        .map(|&(stencil, texel)| read_aspect(b, stencil, texel))
        .collect::<Result<Vec<_>, _>>()?;
    read.extend(before.iter().cloned());
    b.copy_texture(
        &region::<B>(&t, mip, layer, src),
        &region::<B>(&t, mip, layer, dst),
        e,
    );
    for (&(stencil, texel), mut expected) in aspects.iter().zip(before) {
        let out = read_aspect(b, stencil, texel)?;
        // The source region moved onto the destination's texels; every other texel as it was.
        for y in 0..e.height {
            for x in 0..e.width {
                let at = |o: Origin| ((o.y + y) * w + o.x + x) as usize * texel;
                let from = at(src);
                expected.copy_within(from..from + texel, at(dst));
            }
        }
        assert!(
            out == expected,
            "a copy within one {format:?} subresource ({}) moved {e:?} from {src:?} to {dst:?} \
             wrongly at level {mip} of {w} by {h}",
            if stencil { "stencil" } else { "depth" },
        );
        read.push(out);
    }
    b.destroy(Object::Texture(t));
    Ok(())
}

/// One pass of [`Step::Pass`] on texture `t`.
#[allow(clippy::too_many_arguments)]
fn pass<B: Backend>(
    b: &mut B,
    kit: &Kit<B>,
    t: &B::Texture,
    desc: &TextureDesc,
    load: u8,
    triangles: &[([f32; 7], [f32; 4], bool)],
    multisampled: bool,
    options: &PassOptions,
    read: &mut Vec<Vec<u8>>,
) -> Result<(), GpuError> {
    let kind = desc.format.kind();
    let float = kind == FormatKind::Float;
    let multisampled = multisampled
        && b.caps()
            .formats
            .get(desc.format)
            .contains(FormatUse::RESOLVE)
        && b.caps().formats.takes_samples(desc.format, 4);
    let samples = if multisampled { 4 } else { 1 };
    let vertices: Vec<u8> = triangles
        .iter()
        .flat_map(|(corners, color, _)| {
            (0..3).flat_map(move |v| {
                [corners[2 * v], corners[2 * v + 1], corners[6]]
                    .into_iter()
                    .chain(*color)
                    .flat_map(f32::to_le_bytes)
            })
        })
        .collect();
    let vb = b.create_buffer(&BufferDesc {
        size: vertices.len().max(4) as u64,
        usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
        intent: MapIntent::Often,
    })?;
    if !vertices.is_empty() {
        b.write_buffer(&vb, 0, &vertices, WriteMode::Discard);
    }
    let one_level = |format| TextureViewDesc {
        format,
        dimension: ViewDimension::D2,
        base_mip: 0,
        mip_count: 1,
        base_layer: 0,
        layer_count: 1,
        stencil: false,
    };
    let view = b.create_view(t, &one_level(desc.format))?;
    let load = match load % 3 {
        0 => LoadOp::Load,
        1 => LoadOp::Clear([u32::from(load), 0, 0, 0]),
        _ => LoadOp::DontCare,
    };
    let ms = if multisampled {
        let ms = b.create_texture(&TextureDesc {
            mip_levels: 1,
            depth_or_layers: 1,
            samples: 4,
            usage: TextureUsage::RENDER,
            ..*desc
        })?;
        let v = b.create_view(&ms, &one_level(desc.format))?;
        Some((ms, v))
    } else {
        None
    };
    let depth_format = Format::Depth24PlusStencil8;
    let depth = match options.depth {
        Some(_)
            if b.caps()
                .formats
                .get(depth_format)
                .contains(FormatUse::DEPTH_STENCIL) =>
        {
            let d = b.create_texture(&TextureDesc {
                format: depth_format,
                mip_levels: 1,
                depth_or_layers: 1,
                samples,
                usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
                ..*desc
            })?;
            let v = b.create_view(&d, &one_level(depth_format))?;
            Some((d, v))
        }
        _ => None,
    };
    let (cleared, stencil) = options.depth.unwrap_or((1.0, 0));
    let mut desc_pass = PassDesc {
        colors: Default::default(),
        depth: depth.as_ref().map(|(_, v)| DepthAttachment {
            view: v,
            depth: Some(AspectOps {
                load: LoadOp::Clear(cleared.to_bits()),
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: Some(AspectOps {
                load: LoadOp::Clear(stencil),
                store: StoreOp::Store,
                read_only: false,
            }),
        }),
        empty: None,
        occlusion: None,
    };
    desc_pass.colors[0] = Some(match &ms {
        Some((_, v)) => ColorAttachment {
            view: v,
            depth_slice: 0,
            resolve: Some(&view),
            load,
            store: StoreOp::Discard,
        },
        None => ColorAttachment {
            view: &view,
            depth_slice: 0,
            resolve: None,
            load,
            store: StoreOp::Store,
        },
    });
    let queries = if options.query {
        Some(b.create_query_set(QueryKind::OcclusionAny, 1)?)
    } else {
        None
    };
    desc_pass.occlusion = queries.as_ref();
    // The tint, at one of two offsets the alignment allows.
    let tint = match options.tint {
        Some((value, second)) if float => {
            let align = u64::from(b.caps().limits.min_uniform_offset_alignment).max(16);
            let ub = b.create_buffer(&BufferDesc {
                size: 2 * align,
                usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
                intent: MapIntent::Often,
            })?;
            let offset = if second { align } else { 0 };
            let data: Vec<u8> = value.iter().flat_map(|f| f.to_le_bytes()).collect();
            b.write_buffer(&ub, offset, &data, WriteMode::Ordered);
            let set = b.create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &ub,
                    offset,
                    size: 16,
                }),
            }])?;
            Some((ub, set))
        }
        _ => None,
    };
    // Indices for each triangle, its corners in reverse, and the instances drawn.
    let bases = b.caps().has(Features::BASE_VERTEX_INSTANCE);
    let indexed = options.indexed.map(|n| {
        let wide = n & 1 == 1;
        let instances = 1 + u32::from(n >> 1 & 1);
        let first_instance = if bases { u32::from(n >> 2 & 1) } else { 0 };
        (wide, instances, first_instance)
    });
    let ib = match indexed {
        Some((wide, ..)) if !triangles.is_empty() => {
            let order = (0..triangles.len() as u32).flat_map(|t| [3 * t + 2, 3 * t + 1, 3 * t]);
            let data: Vec<u8> = if wide {
                order.flat_map(u32::to_le_bytes).collect()
            } else {
                order.flat_map(|i| (i as u16).to_le_bytes()).collect()
            };
            let ib = b.create_buffer(&BufferDesc {
                size: data.len() as u64,
                usage: BufferUsage::INDEX | BufferUsage::COPY_DST,
                intent: MapIntent::Often,
            })?;
            b.write_buffer(&ib, 0, &data, WriteMode::Discard);
            Some(ib)
        }
        _ => None,
    };
    let scissor = options.scissor.map(|at| {
        let (x, width) = span(at[0], at[1], u64::from(desc.width));
        let (y, height) = span(at[2], at[3], u64::from(desc.height));
        [ScissorRect {
            x: x as u32,
            y: y as u32,
            width: width as u32,
            height: height as u32,
        }]
    });

    b.begin_pass(&desc_pass);
    if queries.is_some() {
        b.begin_query(0);
    }
    let viewport = [Viewport {
        x: 0f32.to_bits(),
        y: 0f32.to_bits(),
        width: (desc.width as f32).to_bits(),
        height: (desc.height as f32).to_bits(),
        min_depth: 0f32.to_bits(),
        max_depth: 1f32.to_bits(),
    }];
    let streams = [Some(VertexBinding {
        buffer: &vb,
        offset: 0,
    })];
    for (n, (.., blended)) in triangles.iter().enumerate() {
        let n = n as u32;
        let program = match (kind, &tint) {
            (FormatKind::Uint, _) => &kit.uint_program,
            (FormatKind::Sint, _) => &kit.sint_program,
            (_, Some(_)) => &kit.tinted_program,
            _ => &kit.float_program,
        };
        b.draw(&DrawCall {
            program,
            blend: if *blended && float {
                &kit.added
            } else {
                &kit.opaque
            },
            depth_stencil: if depth.is_some() {
                &kit.tested
            } else {
                &kit.depth
            },
            raster: if scissor.is_some() {
                &kit.scissored
            } else {
                &kit.raster
            },
            vertex_layout: &kit.layout,
            bindings: [tint.as_ref().map(|(_, set)| set), None, None, None],
            vertex_buffers: &streams,
            index: ib.as_ref().map(|ib| IndexBinding {
                buffer: ib,
                offset: 0,
                format: match indexed {
                    Some((true, ..)) => IndexFormat::U32,
                    _ => IndexFormat::U16,
                },
            }),
            topology: Topology::TriangleList,
            restart: false,
            viewports: &viewport,
            scissors: scissor.as_ref().map_or(&[], |s| &s[..]),
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: match (&ib, indexed) {
                (Some(_), Some((_, instances, first_instance))) => DrawRange::Indexed {
                    first: 3 * n,
                    count: 3,
                    base_vertex: 0,
                    first_instance,
                    instances,
                },
                _ => DrawRange::Vertices {
                    first: 3 * n,
                    count: 3,
                    first_instance: 0,
                    instances: 1,
                },
            },
        });
    }
    if queries.is_some() {
        b.end_query();
    }
    b.end_pass();
    if let Some(q) = &queries {
        let rb = b.read_queries(q, 0, 1)?;
        b.submit();
        b.wait_read(&rb);
        let mut out = vec![0; 8];
        b.finish_read(rb, &mut out);
        read.push(out);
    }
    // Both aspects of a single-sampled depth-stencil target, read back.
    if let (Some((d, _)), false) = (&depth, multisampled) {
        for (stencil, texel) in [(false, 4), (true, 1)] {
            let rb = b.read_back(&ReadSource::Texture {
                region: TextureRegion {
                    texture: d,
                    mip: 0,
                    layer: 0,
                    origin: Origin::default(),
                    stencil,
                },
                extent: Extent {
                    width: desc.width,
                    height: desc.height,
                    depth_or_layers: 1,
                },
            })?;
            b.submit();
            b.wait_read(&rb);
            let mut out = vec![0; (desc.width * desc.height * texel) as usize];
            b.finish_read(rb, &mut out);
            read.push(out);
        }
    }
    if let Some((ms, v)) = ms {
        b.destroy(Object::TextureView(v));
        b.destroy(Object::Texture(ms));
    }
    if let Some((d, v)) = depth {
        b.destroy(Object::TextureView(v));
        b.destroy(Object::Texture(d));
    }
    if let Some(q) = queries {
        b.destroy(Object::QuerySet(q));
    }
    if let Some((ub, set)) = tint {
        b.destroy(Object::BindingSet(set));
        b.destroy(Object::Buffer(ub));
    }
    if let Some(ib) = ib {
        b.destroy(Object::Buffer(ib));
    }
    b.destroy(Object::TextureView(view));
    b.destroy(Object::Buffer(vb));
    Ok(())
}
