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
    /// A pass on a texture's first level and layer: its load, and triangles of random corners and
    /// colours, each blended or not.
    Pass {
        which: u16,
        load: u8,
        triangles: Vec<([f32; 6], [f32; 4], bool)>,
    },
    ClearTexture {
        which: u16,
        value: [f32; 4],
    },
    Destroy(u16),
    Submit,
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
        (any::<u16>(), any::<[u8; 6]>(), any::<u8>())
            .prop_map(|(which, at, seed)| Step::WriteTexture { which, at, seed }),
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
                    prop::array::uniform6(corner),
                    prop::array::uniform4(unit.clone()),
                    any::<bool>()
                ),
                0..4
            )
        )
            .prop_map(|(which, load, triangles)| Step::Pass {
                which,
                load,
                triangles
            }),
        (any::<u16>(), prop::array::uniform4(unit))
            .prop_map(|(which, value)| Step::ClearTexture { which, value }),
        any::<u16>().prop_map(Step::Destroy),
        Just(Step::Submit),
    ];
    prop::collection::vec(step, 1..=max)
}

const PROGRAMS: &str = "
struct V { @builtin(position) p: vec4<f32>, @location(0) c: vec4<f32> }
@vertex fn vs(@location(0) p: vec2<f32>, @location(1) c: vec4<f32>) -> V {
    return V(vec4<f32>(p, 0.5, 1.0), c);
}
@fragment fn float_out(v: V) -> @location(0) vec4<f32> {
    return v.c;
}
@fragment fn uint_out(v: V) -> @location(0) vec4<u32> {
    return vec4<u32>(v.c * 255.0);
}
";

/// The formats random textures take: one 8-bit normalised, one float and one integer.
const FORMATS: [Format; 3] = [Format::Rgba8Unorm, Format::R32Float, Format::R32Uint];

/// What the steps draw with, made once.
struct Kit<B: Backend> {
    float_program: B::Program,
    uint_program: B::Program,
    opaque: B::Blend,
    added: B::Blend,
    depth: B::DepthStencil,
    raster: B::Raster,
    layout: B::VertexLayout,
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
    let raster = b.create_raster(&RasterDesc {
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
    })?;
    let mut desc = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    desc.streams[0] = Some(VertexStream {
        stride: 24,
        step: StepMode::Vertex,
    });
    desc.attributes[0] = Some(VertexAttribute {
        location: 0,
        stream: 0,
        offset: 0,
        format: VertexFormat::Float32x2,
    });
    desc.attributes[1] = Some(VertexAttribute {
        location: 1,
        stream: 0,
        offset: 8,
        format: VertexFormat::Float32x4,
    });
    let layout = b.create_vertex_layout(&desc)?;
    Ok(Kit {
        float_program,
        uint_program,
        opaque,
        added,
        depth,
        raster,
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

/// Runs `steps` on `b` as valid calls, and gives every byte read back, in order.
pub fn run<B: Backend>(b: &mut B, steps: &[Step]) -> Result<Vec<Vec<u8>>, GpuError> {
    let kit = kit(b)?;
    let mut buffers: Vec<(B::Buffer, u64)> = Vec::new();
    let mut textures: Vec<(B::Texture, TextureDesc)> = Vec::new();
    let mut read = Vec::new();
    let all = BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::VERTEX;
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
                let desc = TextureDesc {
                    dimension: TextureDimension::D2,
                    format: FORMATS[pick(*format, FORMATS.len())],
                    width: 1 + u32::from(size[0] % 8),
                    height: 1 + u32::from(size[1] % 8),
                    depth_or_layers: 1 + u32::from(*layers % 2),
                    mip_levels: 1 + u32::from(*mips % 2),
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
                let (buffer, size) = &buffers[pick(*which, buffers.len())];
                let (offset, len) = span(at[0], at[1], *size);
                let mode = [
                    WriteMode::Ordered,
                    WriteMode::Discard,
                    WriteMode::NoOverwrite,
                ][pick(*mode, 3)];
                // No unfinished command may read what a NoOverwrite write touches: wait for all.
                if mode == WriteMode::NoOverwrite {
                    let fence = b.submit();
                    b.wait_fence(&fence);
                }
                b.write_buffer(buffer, offset, &bytes(len as usize, *seed), mode);
            }
            Step::WriteTexture { which, at, seed } if !textures.is_empty() => {
                let (t, desc) = &textures[pick(*which, textures.len())];
                let (mip, layer, origin, e) = region_of(desc, at);
                let pitch = e.width * texel_size(desc.format);
                let data = bytes((pitch * e.height) as usize, *seed);
                b.write_texture(
                    &region::<B>(t, mip, layer, origin),
                    e,
                    &data,
                    pitch,
                    e.height,
                    WriteMode::Ordered,
                );
            }
            Step::CopyBuffer { which, at } if !buffers.is_empty() => {
                let (src, ss) = &buffers[pick(which[0], buffers.len())];
                let (dst, ds) = &buffers[pick(which[1], buffers.len())];
                let (from, len) = span(at[0], at[1], *ss);
                let len = len.min(*ds);
                let to = u64::from(at[2]) % (ds - len + 1);
                let same = pick(which[0], buffers.len()) == pick(which[1], buffers.len());
                if same && from < to + len && to < from + len {
                    continue;
                }
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
                let dst_origin = Origin {
                    x: (dw - e.width).min(so.x),
                    y: (dh - e.height).min(so.y),
                    z: 0,
                };
                if i == j && smip == dmip && slayer == dlayer {
                    continue;
                }
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
                let (buffer, size) = &buffers[pick(which[0], buffers.len())];
                let (t, desc) = &textures[pick(which[1], textures.len())];
                let (mip, layer, origin, e) = region_of(desc, at);
                let texel = texel_size(desc.format);
                let row = e.width * texel;
                let row_pitch = row + texel * u32::from(*pitch % 3);
                let need = u64::from(row_pitch * (e.height - 1) + row);
                if need > *size {
                    continue;
                }
                let offset = u64::from(*offset) % (size - need + 1);
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
                let (buffer, size) = &buffers[pick(*which, buffers.len())];
                let unit = [1u64, 2, 4, 8, 16][pick(*pattern, 5)];
                if unit > *size {
                    continue;
                }
                let blocks = size / unit;
                let (first, count) = span(at[0], at[1], blocks);
                b.clear_buffer(
                    buffer,
                    first * unit,
                    count * unit,
                    &bytes(unit as usize, *seed),
                );
            }
            Step::ReadBuffer { which, at } if !buffers.is_empty() => {
                let (buffer, size) = &buffers[pick(*which, buffers.len())];
                let (offset, len) = span(at[0], at[1], *size);
                let rb = b.read_back(&ReadSource::Buffer {
                    buffer,
                    offset,
                    size: len,
                })?;
                finish(b, rb, len as usize, &mut read);
            }
            Step::ReadTexture { which, at } if !textures.is_empty() => {
                let (t, desc) = &textures[pick(*which, textures.len())];
                let (mip, layer, origin, e) = region_of(desc, at);
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
            } if !textures.is_empty() => {
                let (t, desc) = &textures[pick(*which, textures.len())];
                let uint = desc.format == Format::R32Uint;
                let vertices: Vec<u8> = triangles
                    .iter()
                    .flat_map(|(corners, color, _)| {
                        (0..3).flat_map(move |v| {
                            [corners[2 * v], corners[2 * v + 1]]
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
                let view = b.create_view(
                    t,
                    &TextureViewDesc {
                        format: desc.format,
                        dimension: ViewDimension::D2,
                        base_mip: 0,
                        mip_count: 1,
                        base_layer: 0,
                        layer_count: 1,
                        stencil: false,
                    },
                )?;
                let load = match load % 3 {
                    0 => LoadOp::Load,
                    1 => LoadOp::Clear([u32::from(*load), 0, 0, 0]),
                    _ => LoadOp::DontCare,
                };
                let mut pass = PassDesc {
                    colors: Default::default(),
                    depth: None,
                    empty: None,
                    occlusion: None,
                };
                pass.colors[0] = Some(ColorAttachment {
                    view: &view,
                    depth_slice: 0,
                    resolve: None,
                    load,
                    store: StoreOp::Store,
                });
                b.begin_pass(&pass);
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
                    b.draw(&DrawCall {
                        program: if uint {
                            &kit.uint_program
                        } else {
                            &kit.float_program
                        },
                        blend: if *blended && !uint {
                            &kit.added
                        } else {
                            &kit.opaque
                        },
                        depth_stencil: &kit.depth,
                        raster: &kit.raster,
                        vertex_layout: &kit.layout,
                        bindings: [None; MAX_BINDING_SETS],
                        vertex_buffers: &streams,
                        index: None,
                        topology: Topology::TriangleList,
                        restart: false,
                        viewports: &viewport,
                        scissors: &[],
                        blend_constant: [0; 4],
                        stencil_reference: [0; 2],
                        immediates: &[],
                        range: DrawRange::Vertices {
                            first: 3 * n as u32,
                            count: 3,
                            first_instance: 0,
                            instances: 1,
                        },
                    });
                }
                b.end_pass();
                b.destroy(Object::TextureView(view));
                b.destroy(Object::Buffer(vb));
            }
            Step::ClearTexture { which, value } if !textures.is_empty() => {
                let (t, desc) = &textures[pick(*which, textures.len())];
                let value = if desc.format == Format::R32Uint {
                    value.map(|v| (v * 255.0) as u32)
                } else {
                    value.map(f32::to_bits)
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
                } else {
                    b.destroy(Object::Texture(textures.remove(i - buffers.len()).0));
                }
            }
            Step::Submit => {
                b.submit();
            }
            _ => {}
        }
    }
    Ok(read)
}
