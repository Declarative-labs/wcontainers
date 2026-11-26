//! The graphics rig's first three Direct3D 11 scenes drawn through the interface, on any back end, with
//! their programs written in WGSL. Each returns the target's texels, RGBA8, rows top first.

use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, helpers::*, occlusion::*, passes::*, reflect::*,
    resource::*, shader::*, slow::*, state::*, *,
};

#[path = "../../../../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
pub mod scene;

pub use scene::Scene;
use scene::{HEIGHT, WIDTH};

/// A change to how a scene is drawn. The altered ones change the picture, and the comparison must
/// catch them; the others reach the same picture by another route, and must still match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    None,
    /// Quads: the colour blend's destination factor One instead of one minus source alpha.
    BlendFactor,
    /// Quads: the depth test Greater instead of Less.
    DepthFunction,
    /// Quads: the same draw made first into another target, then a new texture made and written
    /// between it and the draw into the real target, with the same program and binding set.
    StaleUnit,
    /// Quads: the index buffer also holds other data, and gets its indices by a copy from a buffer of
    /// other data.
    TwinCopy,
    /// Quads: the vertices sit after four others, reached by the draw's base vertex.
    BaseVertex,
    /// Triangle (clip z 0): drawn over depth cleared to 0.25 and tested Less, which passes only if
    /// clip depth runs from zero.
    ClearNear,
    /// Quads: drawn as two fans from guest indices with a restart between them.
    Fan,
    /// Quads: drawn as two fans of sequential vertices.
    FanSequential,
    /// Triangle: its outline as a line strip from indices 0, 1, 2, 0.
    LineStrip,
    /// Triangle: its outline as a line loop of its three vertices, which must equal `LineStrip`.
    LineLoop,
    /// Triangle: its colour read from a second stream of stride 0 holding red, so every pixel the
    /// triangle covers is red.
    StrideZero,
    /// Triangle: its colour flat, from the first vertex (red) where the first may provoke, otherwise
    /// from the last with the vertices rotated so red is last.
    FlatRed,
    /// Triangle: then `CLEARED` cleared to red by `clear_rects`, which draws with the interface's
    /// built-in programs.
    ClearRects,
}

/// The square `Change::ClearRects` clears to red.
pub const CLEARED: ScissorRect = ScissorRect {
    x: 64,
    y: 64,
    width: 64,
    height: 64,
};

/// What a picture must equal.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expected {
    /// The scene's Direct3D picture.
    Reference,
    /// The Direct3D picture with every pixel the scene covers red.
    CoveredRed,
    /// The Direct3D picture with `CLEARED` red.
    RectRed,
    /// Another picture of this run, exactly.
    Same(Change),
    /// Nothing: the picture is what another is compared with, and must only hold something.
    Base,
}

impl Change {
    /// Every picture a run draws.
    pub const PICTURES: [(Scene, Change); 16] = [
        (Scene::Triangle, Change::None),
        (Scene::Quads, Change::None),
        (Scene::Instanced, Change::None),
        (Scene::Quads, Change::BlendFactor),
        (Scene::Quads, Change::DepthFunction),
        (Scene::Quads, Change::StaleUnit),
        (Scene::Quads, Change::TwinCopy),
        (Scene::Quads, Change::BaseVertex),
        (Scene::Triangle, Change::ClearNear),
        (Scene::Quads, Change::Fan),
        (Scene::Quads, Change::FanSequential),
        (Scene::Triangle, Change::LineStrip),
        (Scene::Triangle, Change::LineLoop),
        (Scene::Triangle, Change::StrideZero),
        (Scene::Triangle, Change::FlatRed),
        (Scene::Triangle, Change::ClearRects),
    ];

    pub fn name(self) -> &'static str {
        match self {
            Change::None => "",
            Change::BlendFactor => "blend_factor",
            Change::DepthFunction => "depth_function",
            Change::StaleUnit => "stale_unit",
            Change::TwinCopy => "twin_copy",
            Change::BaseVertex => "base_vertex",
            Change::ClearNear => "clear_near",
            Change::Fan => "fan",
            Change::FanSequential => "fan_sequential",
            Change::LineStrip => "line_strip",
            Change::LineLoop => "line_loop",
            Change::StrideZero => "stride_zero",
            Change::FlatRed => "flat_red",
            Change::ClearRects => "clear_rects",
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_name(name: &str) -> Option<Change> {
        Change::PICTURES
            .iter()
            .map(|(_, c)| *c)
            .find(|c| c.name() == name)
    }

    /// Whether the picture differs from the scene's reference.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn altered(self) -> bool {
        matches!(self, Change::BlendFactor | Change::DepthFunction)
    }

    /// What the picture is compared with.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn expected(self) -> Expected {
        match self {
            Change::StrideZero | Change::FlatRed => Expected::CoveredRed,
            Change::ClearRects => Expected::RectRed,
            Change::LineLoop => Expected::Same(Change::LineStrip),
            Change::LineStrip => Expected::Base,
            _ => Expected::Reference,
        }
    }
}

/// What a front does without [`Features::CLIP_DEPTH_ZERO_ONE`] (`ClipDepthRewrite`), for these test
/// programs, whose vertex entries each end `return VsOut(...);` with the position first: clip z
/// from zero is rewritten as 2z - w, from minus one.
fn clip_depth_rewrite(source: &str) -> String {
    let mut out = String::with_capacity(source.len() + 128);
    for line in source.lines() {
        match line.trim_end().strip_suffix(';') {
            Some(statement) if line.trim_start().starts_with("return VsOut(") => {
                out += &statement.replacen("return VsOut(", "return clip_depth(VsOut(", 1);
                out += ");\n";
            }
            _ => {
                out += line;
                out += "\n";
            }
        }
    }
    out + "fn clip_depth(o: VsOut) -> VsOut { var r = o; r.pos.z = 2.0 * r.pos.z - r.pos.w; return r; }\n"
}

/// A validated module of one entry point of WGSL source.
fn module(source: &str, entry: &str, stage: ShaderStage) -> Result<ShaderModule, String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| format!("{e:?}"))?;
    Ok(ShaderModule {
        module,
        info,
        entry_point: entry.into(),
        stage,
        key: [0; 32],
    })
}

fn program<B: Gpu>(b: &mut B, source: &str) -> Result<B::Program, String> {
    let rewritten;
    let source = if b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE) {
        source
    } else {
        b.record_slow_path(SlowPath::ClipDepthRewrite);
        rewritten = clip_depth_rewrite(source);
        &rewritten
    };
    let vs = b
        .create_shader(&module(source, "vs", ShaderStage::Vertex)?)
        .map_err(|e| format!("{e:?}"))?;
    let ps = b
        .create_shader(&module(source, "ps", ShaderStage::Fragment)?)
        .map_err(|e| format!("{e:?}"))?;
    let p = b
        .create_program(&Program::Render {
            vertex: &vs,
            fragment: Some(&ps),
        })
        .map_err(|e| format!("{e:?}"))?;
    b.destroy(Object::Shader(vs));
    b.destroy(Object::Shader(ps));
    Ok(p)
}

fn bits(v: [f32; 4]) -> [u32; 4] {
    v.map(f32::to_bits)
}

/// The layout of one stream at `stride`; with `change` StrideZero, the triangle's colour (location 1)
/// comes from stream 1 at stride 0 instead.
fn layout(stride: u32, attributes: &[(u32, VertexFormat)], change: Change) -> VertexLayoutDesc {
    let mut l = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    l.streams[0] = Some(VertexStream {
        stride,
        step: StepMode::Vertex,
    });
    for (i, (offset, format)) in attributes.iter().enumerate() {
        l.attributes[i] = Some(VertexAttribute {
            location: i as u32,
            stream: 0,
            offset: *offset,
            format: *format,
        });
    }
    if change == Change::StrideZero {
        l.streams[1] = Some(VertexStream {
            stride: 0,
            step: StepMode::Vertex,
        });
        l.attributes[1] = Some(VertexAttribute {
            location: 1,
            stream: 1,
            offset: 0,
            format: VertexFormat::Float32x3,
        });
    }
    l
}

/// Direct3D 11's default rasteriser state with culling off, as the rig's `solid()`; clip depth from
/// minus one where the program was rewritten.
fn solid(depth_minus_one: bool) -> RasterDesc {
    RasterDesc {
        fill: FillMode::Solid,
        cull: CullMode::None,
        front: FrontFace::Clockwise,
        depth_bias: 0,
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking: ProvokingVertex::Last,
        depth_minus_one,
        sample_mask: u32::MAX,
    }
}

fn opaque() -> BlendDesc {
    BlendDesc {
        targets: [RenderTargetBlend {
            blend: None,
            writes: ColorWrites::all(),
        }; MAX_COLOR_TARGETS],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    }
}

fn no_depth() -> DepthStencilDesc {
    DepthStencilDesc {
        depth_compare: None,
        depth_write: false,
        stencil: None,
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    }
}

fn buffer<B: Gpu>(b: &mut B, usage: BufferUsage, bytes: &[u8]) -> Result<B::Buffer, String> {
    let buffer = b
        .create_buffer(&BufferDesc {
            size: bytes.len() as u64,
            usage: usage | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .map_err(|e| format!("{e:?}"))?;
    b.write_buffer(&buffer, 0, bytes, WriteMode::Discard);
    Ok(buffer)
}

fn texture<B: Gpu>(
    b: &mut B,
    format: Format,
    size: u32,
    usage: TextureUsage,
) -> Result<B::Texture, String> {
    b.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: size,
        height: size,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage,
        cube: false,
        reinterpret: false,
    })
    .map_err(|e| format!("{e:?}"))
}

fn whole_view<B: Gpu>(b: &mut B, t: &B::Texture, format: Format) -> Result<B::TextureView, String> {
    b.create_view(
        t,
        &TextureViewDesc {
            format,
            dimension: ViewDimension::D2,
            base_mip: 0,
            mip_count: 1,
            base_layer: 0,
            layer_count: 1,
            stencil: false,
        },
    )
    .map_err(|e| format!("{e:?}"))
}

/// The scene data's bytes, as they lie in memory (plain f32 and u16 arrays, with no padding).
fn as_bytes<T: bytemuck::Pod>(v: &[T]) -> Vec<u8> {
    bytemuck::cast_slice(v).to_vec()
}

/// Draws `scene` with `change` applied and reads the target back.
pub fn draw<B: Gpu>(b: &mut B, which: Scene, change: Change) -> Result<Vec<u8>, String> {
    let target = texture(
        b,
        Format::Rgba8Unorm,
        WIDTH,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )?;
    let target_view = whole_view(b, &target, Format::Rgba8Unorm)?;
    let minus_one = !b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE);
    let first_provokes = b.caps().features.contains(Features::PROVOKING_VERTEX_FIRST);
    let mut raster_desc = solid(minus_one);
    if change == Change::FlatRed && first_provokes {
        raster_desc.provoking = ProvokingVertex::First;
    }
    let raster = b
        .create_raster(&raster_desc)
        .map_err(|e| format!("{e:?}"))?;
    let viewport = [Viewport {
        x: 0,
        y: 0,
        width: (WIDTH as f32).to_bits(),
        height: (HEIGHT as f32).to_bits(),
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }];
    let quads_wgsl = include_str!("quads.wgsl");
    let instanced_wgsl = include_str!("instanced.wgsl");
    let (source, vertices, stride, attributes): (_, Vec<_>, _, &[(u32, VertexFormat)]) = match which
    {
        Scene::Triangle => (
            scene::TRIANGLE_WGSL,
            as_bytes(&scene::TRIANGLE),
            20,
            &[(0, VertexFormat::Float32x2), (8, VertexFormat::Float32x3)],
        ),
        Scene::Quads => (
            quads_wgsl,
            as_bytes(&scene::QUADS),
            36,
            &[
                (0, VertexFormat::Float32x3),
                (12, VertexFormat::Float32x2),
                (20, VertexFormat::Float32x4),
            ],
        ),
        Scene::Instanced => (
            instanced_wgsl,
            as_bytes(&scene::INSTANCE_SHAPE),
            8,
            &[(0, VertexFormat::Float32x2)],
        ),
    };
    let vertices = &vertices[..];
    let flat;
    let source = if change == Change::FlatRed {
        flat = source.replacen(
            "@location(0) color: vec3f,",
            "@location(0) @interpolate(flat) color: vec3f,",
            1,
        );
        &flat[..]
    } else {
        source
    };
    let program = program(b, source)?;
    let mut moved = Vec::new();
    let (vertices, base_vertex) = if change == Change::BaseVertex {
        moved.resize(4 * stride as usize, 0);
        moved.extend_from_slice(vertices);
        (&moved[..], 4)
    } else if change == Change::FlatRed && !first_provokes {
        // The same triangle with red last.
        moved.extend_from_slice(&vertices[stride as usize..]);
        moved.extend_from_slice(&vertices[..stride as usize]);
        (&moved[..], 0)
    } else {
        (vertices, 0)
    };
    let attributes = if change == Change::StrideZero {
        &attributes[..1]
    } else {
        attributes
    };
    let vertex_buffer = buffer(b, BufferUsage::VERTEX, vertices)?;
    let red = buffer(b, BufferUsage::VERTEX, &as_bytes(&[1.0f32, 0.0, 0.0]))?;
    let vertex_layout = b
        .create_vertex_layout(&layout(stride, attributes, change))
        .map_err(|e| format!("{e:?}"))?;

    let mut blend_desc = opaque();
    let mut depth_desc = no_depth();
    let mut depth_target = None;
    let mut index = None;
    let mut sets: [Option<B::BindingSet>; MAX_BINDING_SETS] = Default::default();
    let mut keep = Vec::new();
    if which == Scene::Quads {
        let (src, dst) = match change {
            Change::BlendFactor => (BlendFactor::SrcAlpha, BlendFactor::One),
            _ => (BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha),
        };
        blend_desc.targets[0].blend = Some((
            BlendComponent {
                src,
                dst,
                op: BlendOp::Add,
            },
            BlendComponent {
                src: BlendFactor::One,
                dst: BlendFactor::OneMinusSrcAlpha,
                op: BlendOp::Add,
            },
        ));
        depth_desc.depth_compare = Some(match change {
            Change::DepthFunction => CompareFn::Greater,
            _ => CompareFn::Less,
        });
        depth_desc.depth_write = true;
        let index_bytes = as_bytes(&scene::QUAD_INDICES);
        let indices = &index_bytes[..];
        index = Some(if change == Change::TwinCopy {
            let twin = buffer(
                b,
                BufferUsage::INDEX | BufferUsage::VERTEX,
                &vec![0; indices.len()],
            )?;
            let from = buffer(b, BufferUsage::VERTEX | BufferUsage::COPY_SRC, indices)?;
            b.copy_buffer(&from, 0, &twin, 0, indices.len() as u64);
            b.destroy(Object::Buffer(from));
            twin
        } else {
            buffer(b, BufferUsage::INDEX, indices)?
        });
        let size = scene::TEXTURE_SIZE;
        let checker = texture(
            b,
            Format::Rgba8Unorm,
            size,
            TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        )?;
        b.write_texture(
            &TextureRegion {
                texture: &checker,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: size,
                height: size,
                depth_or_layers: 1,
            },
            &scene::texture(),
            size * 4,
            size,
            WriteMode::Discard,
        );
        let view = whole_view(b, &checker, Format::Rgba8Unorm)?;
        let sampler = b
            .create_sampler(&SamplerDesc {
                min: Filter::Linear,
                mag: Filter::Linear,
                mip: Filter::Linear,
                address: [AddressMode::Repeat; 3],
                lod_range: [0, f32::MAX.to_bits()],
                anisotropy: 1,
                compare: None,
                border: BorderColor::TransparentBlack,
            })
            .map_err(|e| format!("{e:?}"))?;
        let set = b
            .create_binding_set(&[
                BindingEntry {
                    binding: 32,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&view),
                },
                BindingEntry {
                    binding: 16,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Sampler(&sampler),
                },
            ])
            .map_err(|e| format!("{e:?}"))?;
        sets[1] = Some(set);
        keep.push((checker, view, sampler));
    }
    if change == Change::LineStrip {
        index = Some(buffer(b, BufferUsage::INDEX, &as_bytes(&[0u16, 1, 2, 0]))?);
    }
    let mut depth_clear = 1.0f32;
    if which == Scene::Quads || change == Change::ClearNear {
        if change == Change::ClearNear {
            depth_desc.depth_compare = Some(CompareFn::Less);
            depth_clear = 0.25;
        }
        let depth = texture(b, Format::Depth32Float, WIDTH, TextureUsage::RENDER)?;
        depth_target = Some((whole_view(b, &depth, Format::Depth32Float)?, depth));
    }
    let constants;
    if which == Scene::Instanced {
        constants = buffer(
            b,
            BufferUsage::UNIFORM,
            &as_bytes(&scene::instance_constants()),
        )?;
        let size = (scene::instance_constants().len() * 16) as u64;
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::VERTEX,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &constants,
                    offset: 0,
                    size,
                }),
            }])
            .map_err(|e| format!("{e:?}"))?;
        sets[0] = Some(set);
    }
    let blend = b.create_blend(&blend_desc).map_err(|e| format!("{e:?}"))?;
    let depth_stencil = b
        .create_depth_stencil(&depth_desc)
        .map_err(|e| format!("{e:?}"))?;

    // The other target of `StaleUnit`, drawn into first.
    let other = if change == Change::StaleUnit {
        let t = texture(b, Format::Rgba8Unorm, WIDTH, TextureUsage::RENDER)?;
        Some((whole_view(b, &t, Format::Rgba8Unorm)?, t))
    } else {
        None
    };
    let pass = |view| {
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Clear(bits(scene::CLEAR)),
            store: StoreOp::Store,
        });
        if let Some((view, _)) = &depth_target {
            pass.depth = Some(DepthAttachment {
                view,
                depth: Some(AspectOps {
                    load: LoadOp::Clear(depth_clear.to_bits()),
                    store: StoreOp::Discard,
                    read_only: false,
                }),
                stencil: None,
            });
        }
        pass
    };
    let streams = [
        Some(VertexBinding {
            buffer: &vertex_buffer,
            offset: 0,
        }),
        Some(VertexBinding {
            buffer: &red,
            offset: 0,
        }),
    ];
    let range = match which {
        Scene::Triangle if change == Change::LineStrip => DrawRange::Indexed {
            first: 0,
            count: 4,
            base_vertex: 0,
            first_instance: 0,
            instances: 1,
        },
        Scene::Triangle => DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 0,
            instances: 1,
        },
        Scene::Quads => DrawRange::Indexed {
            first: 0,
            count: scene::QUAD_INDICES.len() as u32,
            base_vertex,
            first_instance: 0,
            instances: 1,
        },
        Scene::Instanced => DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 0,
            instances: scene::INSTANCES,
        },
    };
    let call = DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        bindings: [sets[0].as_ref(), sets[1].as_ref(), None, None],
        vertex_buffers: &streams,
        index: index.as_ref().map(|buffer| IndexBinding {
            buffer,
            offset: 0,
            format: IndexFormat::U16,
        }),
        topology: if change == Change::LineStrip {
            Topology::LineStrip
        } else {
            Topology::TriangleList
        },
        restart: change == Change::Fan,
        viewports: &viewport,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range,
    };
    let mut extra = Vec::new();
    if let Some((other_view, _)) = &other {
        b.begin_pass(&pass(other_view));
        b.draw(&call);
        b.end_pass();
        let size = scene::TEXTURE_SIZE;
        let magenta = texture(
            b,
            Format::Rgba8Unorm,
            size,
            TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        )?;
        b.write_texture(
            &TextureRegion {
                texture: &magenta,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: size,
                height: size,
                depth_or_layers: 1,
            },
            &[255, 0, 255, 255].repeat((size * size) as usize),
            size * 4,
            size,
            WriteMode::Discard,
        );
        let magenta_view = whole_view(b, &magenta, Format::Rgba8Unorm)?;
        extra.push((magenta_view, magenta));
    }
    let converted = |b: &mut B| {
        let s = b.slow_paths();
        s.counts[SlowPath::FanConverted as usize] + s.counts[SlowPath::LoopConverted as usize]
    };
    let before = converted(b);
    let fan = |first, count| IndexSource::Sequential { first, count };
    let err = |e| format!("{e:?}");
    b.begin_pass(&pass(&target_view));
    match change {
        Change::Fan => {
            let indices = as_bytes(&[0u16, 1, 2, 3, 0xffff, 4, 5, 6, 7]);
            let source = IndexSource::Guest {
                indices: &indices,
                format: IndexFormat::U16,
                base_vertex: 0,
            };
            let kind = FanOrLoop::TriangleFan;
            b.draw_fan_or_loop(&call, kind, source, ProvokingVertex::Last, 0, 1)
                .map_err(err)?;
        }
        Change::FanSequential => {
            for first in [0, 4] {
                let kind = FanOrLoop::TriangleFan;
                b.draw_fan_or_loop(&call, kind, fan(first, 4), ProvokingVertex::Last, 0, 1)
                    .map_err(err)?;
            }
        }
        Change::LineLoop => {
            let kind = FanOrLoop::LineLoop;
            b.draw_fan_or_loop(&call, kind, fan(0, 3), ProvokingVertex::Last, 0, 1)
                .map_err(err)?;
        }
        _ => b.draw(&call),
    }
    b.end_pass();
    if b.caps().features.contains(Features::FAN_AND_LOOP) && converted(b) != before {
        return Err("a fan or loop was converted although FAN_AND_LOOP is present".into());
    }
    if change == Change::ClearRects {
        let red = ClearValue::Color(bits([1.0, 0.0, 0.0, 1.0]));
        b.clear_rects(&target_view, Format::Rgba8Unorm, red, &[CLEARED])
            .map_err(err)?;
    }
    let mut texels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: &target,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: WIDTH,
                height: HEIGHT,
                depth_or_layers: 1,
            },
        },
        &mut texels,
    )
    .map_err(|e| format!("{e:?}"))?;
    let fence = b.submit();
    b.wait_fence(&fence);
    if let DeviceStatus::Lost(reason) = b.status() {
        return Err(format!("device lost: {reason:?}"));
    }
    for (view, texture) in other.into_iter().chain(extra) {
        b.destroy(Object::TextureView(view));
        b.destroy(Object::Texture(texture));
    }
    b.destroy(Object::TextureView(target_view));
    b.destroy(Object::Texture(target));
    if let Some((view, depth)) = depth_target {
        b.destroy(Object::TextureView(view));
        b.destroy(Object::Texture(depth));
    }
    for (texture, view, sampler) in keep {
        b.destroy(Object::TextureView(view));
        b.destroy(Object::Texture(texture));
        b.destroy(Object::Sampler(sampler));
    }
    b.destroy(Object::Program(program));
    b.destroy(Object::Buffer(vertex_buffer));
    b.destroy(Object::Buffer(red));
    if let Some(index) = index {
        b.destroy(Object::Buffer(index));
    }
    Ok(texels)
}

/// The capability list as text, for the record and for checking against the specification.
pub fn caps_report(caps: &Caps) -> String {
    use serde::Deserialize;
    use serde::de::value::U32Deserializer;
    let mut out = format!(
        "features: {:?}\nlimits: {:#?}\nformats:\n",
        caps.features, caps.limits
    );
    for i in 0..FORMAT_COUNT {
        let uses = caps.formats.uses[i];
        if !uses.is_empty() {
            // The format numbered `i`, which serde's derive reads as the variant's position.
            let number = U32Deserializer::<serde::de::value::Error>::new(i as u32);
            let format = Format::deserialize(number).expect("every index below COUNT is a format");
            out += &format!(
                "  {format:?}: {uses:?} samples {:#b}\n",
                caps.formats.samples[i]
            );
        }
    }
    out
}
