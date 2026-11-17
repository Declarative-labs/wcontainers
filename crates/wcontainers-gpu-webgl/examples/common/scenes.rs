//! The graphics rig's first three Direct3D 11 scenes drawn through the interface, on any back end, with
//! their programs written in WGSL. Each returns the target's texels, RGBA8, rows top first.

use wcontainers_gpu::*;

#[path = "../../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
pub mod scene;

pub use scene::Scene;
use scene::{HEIGHT, WIDTH};

/// A deliberate change to the quads scene, which the comparison must catch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    None,
    /// The colour blend's destination factor One instead of one minus source alpha.
    BlendFactor,
    /// The depth test Greater instead of Less.
    DepthFunction,
}

impl Change {
    pub const ALTERED: [Change; 2] = [Change::BlendFactor, Change::DepthFunction];

    pub fn name(self) -> &'static str {
        match self {
            Change::None => "",
            Change::BlendFactor => "blend_factor",
            Change::DepthFunction => "depth_function",
        }
    }
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

fn program<B: Backend>(b: &mut B, source: &str) -> Result<B::Program, String> {
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

fn layout(stride: u32, attributes: &[(u32, VertexFormat)]) -> VertexLayoutDesc {
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
    l
}

/// Direct3D 11's default rasteriser state with culling off, as the rig's `solid()`.
fn solid() -> RasterDesc {
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
        depth_minus_one: false,
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

fn buffer<B: Backend>(b: &mut B, usage: BufferUsage, bytes: &[u8]) -> Result<B::Buffer, String> {
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

fn texture<B: Backend>(
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

fn whole_view<B: Backend>(
    b: &mut B,
    t: &B::Texture,
    format: Format,
) -> Result<B::TextureView, String> {
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

fn as_bytes(v: &[impl Copy]) -> &[u8] {
    // SAFETY: the scene data are plain f32 and u16 arrays, with no padding.
    unsafe { core::slice::from_raw_parts(v.as_ptr().cast(), core::mem::size_of_val(v)) }
}

/// Draws `scene` with `change` applied (quads only) and reads the target back.
pub fn draw<B: Backend>(b: &mut B, which: Scene, change: Change) -> Result<Vec<u8>, String> {
    let target = texture(
        b,
        Format::Rgba8Unorm,
        WIDTH,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )?;
    let target_view = whole_view(b, &target, Format::Rgba8Unorm)?;
    let raster = b.create_raster(&solid()).map_err(|e| format!("{e:?}"))?;
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
    let (source, vertices, stride, attributes): (_, &[u8], _, &[(u32, VertexFormat)]) = match which
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
    let program = program(b, source)?;
    let vertex_buffer = buffer(b, BufferUsage::VERTEX, vertices)?;
    let vertex_layout = b
        .create_vertex_layout(&layout(stride, attributes))
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
        let depth = texture(b, Format::Depth32Float, WIDTH, TextureUsage::RENDER)?;
        depth_target = Some((whole_view(b, &depth, Format::Depth32Float)?, depth));
        index = Some(buffer(
            b,
            BufferUsage::INDEX,
            as_bytes(&scene::QUAD_INDICES),
        )?);
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
    let constants;
    if which == Scene::Instanced {
        constants = buffer(
            b,
            BufferUsage::UNIFORM,
            as_bytes(&scene::instance_constants()),
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

    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &target_view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(bits(scene::CLEAR)),
        store: StoreOp::Store,
    });
    if let Some((view, _)) = &depth_target {
        pass.depth = Some(DepthAttachment {
            view,
            depth: Some(AspectOps {
                load: LoadOp::Clear(1.0f32.to_bits()),
                store: StoreOp::Discard,
                read_only: false,
            }),
            stencil: None,
        });
    }
    b.begin_pass(&pass);
    let streams = [Some(VertexBinding {
        buffer: &vertex_buffer,
        offset: 0,
    })];
    let range = match which {
        Scene::Triangle => DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 0,
            instances: 1,
        },
        Scene::Quads => DrawRange::Indexed {
            first: 0,
            count: scene::QUAD_INDICES.len() as u32,
            base_vertex: 0,
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
    b.draw(&DrawCall {
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
        topology: Topology::TriangleList,
        restart: false,
        viewports: &viewport,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range,
    });
    b.end_pass();
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
    if let Some(index) = index {
        b.destroy(Object::Buffer(index));
    }
    Ok(texels)
}

/// The capability list as text, for the record and for checking against the specification.
pub fn caps_report(caps: &Caps) -> String {
    let mut out = format!(
        "features: {:?}\nlimits: {:#?}\nformats:\n",
        caps.features, caps.limits
    );
    for i in 0..Format::COUNT {
        let uses = caps.formats.uses[i];
        if !uses.is_empty() {
            // SAFETY: every index below COUNT is a Format, which is repr(u16) and fieldless.
            let format: Format = unsafe { core::mem::transmute(i as u16) };
            out += &format!(
                "  {format:?}: {uses:?} samples {:#b}\n",
                caps.formats.samples[i]
            );
        }
    }
    out
}
