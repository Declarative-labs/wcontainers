//! The rig's three Direct3D 11 reference scenes (triangle, quads, instanced), drawn through the
//! interface with WGSL programs, natively and in the browser.

use anyhow::{Context, Result, anyhow};
use wcontainers_gpu::{
    AspectOps, Backend, BindingEntry, BindingResource, BlendComponent, BlendDesc, BlendFactor,
    BlendOp, BorderColor, BufferDesc, BufferRange, BufferUsage, ColorAttachment, ColorWrites,
    CompareFn, CullMode, DepthAttachment, DepthStencilDesc, DrawCall, DrawRange, Extent, FillMode,
    Filter, Format, FrontFace, GpuError, IndexBinding, IndexFormat, LoadOp, MapIntent, Origin,
    PassDesc, Program, ProvokingVertex, RasterDesc, ReadSource, RenderTargetBlend, SamplerDesc,
    ShaderModule, ShaderStage, StageSet, StepMode, StoreOp, TextureDesc, TextureDimension,
    TextureRegion, TextureUsage, TextureViewDesc, Topology, VertexAttribute, VertexBinding,
    VertexFormat, VertexLayoutDesc, VertexStream, ViewDimension, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

use crate::scene::{self, HEIGHT, Scene, WIDTH};

type B = WebGpu;

/// A change to a scene that must make its comparison with the reference fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Twist {
    None,
    /// The quads' colour source factor is one instead of the source alpha.
    BlendFactor,
    /// The quads' depth test passes on greater instead of less.
    DepthFunction,
}

impl Twist {
    pub fn parse(s: &str) -> Result<Twist> {
        Ok(match s {
            "none" | "" => Twist::None,
            "blend-factor" => Twist::BlendFactor,
            "depth-function" => Twist::DepthFunction,
            _ => return Err(anyhow!("twists are none, blend-factor, depth-function")),
        })
    }
}

const QUADS_WGSL: &str = r#"
@group(1) @binding(32) var tex: texture_2d<f32>;
@group(1) @binding(16) var smp: sampler;
struct VsOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f, @location(1) color: vec4f }
@vertex fn vs(@location(0) pos: vec3f, @location(1) uv: vec2f, @location(2) color: vec4f) -> VsOut {
    return VsOut(vec4f(pos, 1.0), uv, color);
}
@fragment fn ps(i: VsOut) -> @location(0) vec4f { return textureSample(tex, smp, i.uv) * i.color; }
"#;

const INSTANCED_WGSL: &str = r#"
struct Scene { placement: array<vec4f, 16>, colors: array<vec4f, 16>, scale: vec4f }
@group(0) @binding(0) var<uniform> scene: Scene;
struct VsOut { @builtin(position) pos: vec4f, @location(0) color: vec4f }
@vertex fn vs(@location(0) pos: vec2f, @builtin(instance_index) id: u32) -> VsOut {
    let p = scene.placement[id];
    let turned = vec2f(pos.x * p.z - pos.y * p.w, pos.x * p.w + pos.y * p.z);
    return VsOut(vec4f(turned * scene.scale.xy + p.xy, 0.0, 1.0), scene.colors[id]);
}
@fragment fn ps(i: VsOut) -> @location(0) vec4f { return i.color; }
"#;

fn gpu<T>(r: Result<T, GpuError>) -> Result<T> {
    r.map_err(|e| anyhow!("{e:?}"))
}

/// Makes the back end on the first high-performance adapter of `backends`, which must be a GPU;
/// returns it with the adapter's description.
pub async fn device(backends: wgpu::Backends) -> Result<(WebGpu, String)> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let instance = wgpu::Instance::new(desc);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await
        .context("no adapter")?;
    let info = adapter.get_info();
    anyhow::ensure!(
        info.device_type != wgpu::DeviceType::Cpu,
        "the adapter is the software {}, not a GPU",
        info.name
    );
    let (device, queue) = adapter
        .request_device(&WebGpu::device_descriptor(&adapter))
        .await?;
    let name = format!(
        "{} ({:?}, {:?}, {} {})",
        info.name, info.backend, info.device_type, info.driver, info.driver_info
    );
    Ok((WebGpu::new(&adapter, device, queue), name))
}

/// Parses and validates WGSL into the common form and makes it a shader.
pub fn shader(
    b: &mut B,
    source: &str,
    entry: &str,
    stage: ShaderStage,
) -> Result<<B as Backend>::Shader> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| anyhow!("{}", e.emit_to_string(source)))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)?;
    gpu(b.create_shader(&ShaderModule {
        module,
        info,
        entry_point: entry.into(),
        stage,
        key: [0; 32],
    }))
}

fn program(b: &mut B, source: &str) -> Result<<B as Backend>::Program> {
    let vs = shader(b, source, "vs", ShaderStage::Vertex)?;
    let ps = shader(b, source, "ps", ShaderStage::Fragment)?;
    gpu(b.create_program(&Program::Render {
        vertex: &vs,
        fragment: Some(&ps),
    }))
}

/// A buffer holding `data`.
pub fn buffer(b: &mut B, data: &[u8], usage: BufferUsage) -> Result<<B as Backend>::Buffer> {
    let buf = gpu(b.create_buffer(&BufferDesc {
        size: data.len() as u64,
        usage: usage | BufferUsage::COPY_DST,
        intent: MapIntent::Rarely,
    }))?;
    b.write_buffer(&buf, 0, data, WriteMode::Ordered);
    Ok(buf)
}

pub fn opaque() -> BlendDesc {
    BlendDesc {
        targets: [RenderTargetBlend {
            blend: None,
            writes: ColorWrites::all(),
        }; 8],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    }
}

/// Direct3D 11's solid fill without culling, as the reference scenes draw.
pub fn solid() -> RasterDesc {
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
        provoking: ProvokingVertex::First,
        depth_minus_one: false,
        sample_mask: !0,
    }
}

pub fn no_depth() -> DepthStencilDesc {
    DepthStencilDesc {
        depth_compare: None,
        depth_write: false,
        stencil: None,
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    }
}

pub fn layout(
    streams: &[(u32, StepMode)],
    attributes: &[(u8, u32, VertexFormat)],
) -> VertexLayoutDesc {
    let mut d = VertexLayoutDesc {
        streams: [None; 32],
        attributes: [None; 32],
    };
    for (i, (stride, step)) in streams.iter().enumerate() {
        d.streams[i] = Some(VertexStream {
            stride: *stride,
            step: *step,
        });
    }
    for (location, (stream, offset, format)) in attributes.iter().enumerate() {
        d.attributes[location] = Some(VertexAttribute {
            location: location as u32,
            stream: *stream,
            offset: *offset,
            format: *format,
        });
    }
    d
}

pub fn texture_2d(
    b: &mut B,
    format: Format,
    width: u32,
    height: u32,
    usage: TextureUsage,
) -> Result<(<B as Backend>::Texture, <B as Backend>::TextureView)> {
    let t = gpu(b.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width,
        height,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage,
        cube: false,
        reinterpret: false,
    }))?;
    let v = gpu(b.create_view(
        &t,
        &TextureViewDesc {
            format,
            dimension: ViewDimension::D2,
            base_mip: 0,
            mip_count: 1,
            base_layer: 0,
            layer_count: 1,
            stencil: false,
        },
    ))?;
    Ok((t, v))
}

/// What a scene draws with, made once; `draw` records it into a pass.
pub struct Prepared {
    program: <B as Backend>::Program,
    blend: <B as Backend>::Blend,
    depth_stencil: <B as Backend>::DepthStencil,
    raster: <B as Backend>::Raster,
    vertex_layout: <B as Backend>::VertexLayout,
    vertices: <B as Backend>::Buffer,
    index: Option<<B as Backend>::Buffer>,
    sets: [Option<<B as Backend>::BindingSet>; 4],
    range: (u32, u32, bool),
    depth: bool,
}

pub fn prepare(b: &mut B, which: Scene, twist: Twist) -> Result<Prepared> {
    let raster = gpu(b.create_raster(&solid()))?;
    Ok(match which {
        Scene::Triangle => Prepared {
            program: program(b, scene::TRIANGLE_WGSL)?,
            blend: gpu(b.create_blend(&opaque()))?,
            depth_stencil: gpu(b.create_depth_stencil(&no_depth()))?,
            raster,
            vertex_layout: gpu(b.create_vertex_layout(&layout(
                &[(20, StepMode::Vertex)],
                &[
                    (0, 0, VertexFormat::Float32x2),
                    (0, 8, VertexFormat::Float32x3),
                ],
            )))?,
            vertices: buffer(b, bytes(&scene::TRIANGLE), BufferUsage::VERTEX)?,
            index: None,
            sets: Default::default(),
            range: (3, 1, false),
            depth: false,
        },
        Scene::Quads => {
            let texels = scene::texture();
            let (texture, view) = texture_2d(
                b,
                Format::Rgba8Unorm,
                scene::TEXTURE_SIZE,
                scene::TEXTURE_SIZE,
                TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            )?;
            b.write_texture(
                &TextureRegion {
                    texture: &texture,
                    mip: 0,
                    layer: 0,
                    origin: Origin::default(),
                    stencil: false,
                },
                Extent {
                    width: scene::TEXTURE_SIZE,
                    height: scene::TEXTURE_SIZE,
                    depth_or_layers: 1,
                },
                &texels,
                scene::TEXTURE_SIZE * 4,
                scene::TEXTURE_SIZE,
                WriteMode::Ordered,
            );
            let sampler = gpu(b.create_sampler(&SamplerDesc {
                min: Filter::Linear,
                mag: Filter::Linear,
                mip: Filter::Linear,
                address: [wcontainers_gpu::AddressMode::Repeat; 3],
                lod_range: [0, f32::MAX.to_bits()],
                anisotropy: 1,
                compare: None,
                border: BorderColor::TransparentBlack,
            }))?;
            let set = gpu(b.create_binding_set(&[
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
            ]))?;
            let mut blend = opaque();
            let src = match twist {
                Twist::BlendFactor => BlendFactor::One,
                _ => BlendFactor::SrcAlpha,
            };
            blend.targets[0].blend = Some((
                BlendComponent {
                    src,
                    dst: BlendFactor::OneMinusSrcAlpha,
                    op: BlendOp::Add,
                },
                BlendComponent {
                    src: BlendFactor::One,
                    dst: BlendFactor::OneMinusSrcAlpha,
                    op: BlendOp::Add,
                },
            ));
            let mut depth = no_depth();
            depth.depth_compare = Some(match twist {
                Twist::DepthFunction => CompareFn::Greater,
                _ => CompareFn::Less,
            });
            depth.depth_write = true;
            Prepared {
                program: program(b, QUADS_WGSL)?,
                blend: gpu(b.create_blend(&blend))?,
                depth_stencil: gpu(b.create_depth_stencil(&depth))?,
                raster,
                vertex_layout: gpu(b.create_vertex_layout(&layout(
                    &[(36, StepMode::Vertex)],
                    &[
                        (0, 0, VertexFormat::Float32x3),
                        (0, 12, VertexFormat::Float32x2),
                        (0, 20, VertexFormat::Float32x4),
                    ],
                )))?,
                vertices: buffer(b, bytes(&scene::QUADS), BufferUsage::VERTEX)?,
                index: Some(buffer(b, bytes(&scene::QUAD_INDICES), BufferUsage::INDEX)?),
                sets: [None, Some(set), None, None],
                range: (scene::QUAD_INDICES.len() as u32, 1, true),
                depth: true,
            }
        }
        Scene::Instanced => {
            let constants = buffer(b, bytes(&scene::instance_constants()), BufferUsage::UNIFORM)?;
            let set = gpu(b.create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::VERTEX,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &constants,
                    offset: 0,
                    size: 33 * 16,
                }),
            }]))?;
            Prepared {
                program: program(b, INSTANCED_WGSL)?,
                blend: gpu(b.create_blend(&opaque()))?,
                depth_stencil: gpu(b.create_depth_stencil(&no_depth()))?,
                raster,
                vertex_layout: gpu(b.create_vertex_layout(&layout(
                    &[(8, StepMode::Vertex)],
                    &[(0, 0, VertexFormat::Float32x2)],
                )))?,
                vertices: buffer(b, bytes(&scene::INSTANCE_SHAPE), BufferUsage::VERTEX)?,
                index: None,
                sets: [Some(set), None, None, None],
                range: (3, scene::INSTANCES, false),
                depth: false,
            }
        }
    })
}

fn bytes<T: bytemuck::Pod>(v: &[T]) -> &[u8] {
    bytemuck::cast_slice(v)
}

/// Records the scene's draw into the open pass.
pub fn draw(b: &mut B, p: &Prepared) {
    let vertex_buffers = [Some(VertexBinding {
        buffer: &p.vertices,
        offset: 0,
    })];
    let (count, instances, indexed) = p.range;
    b.draw(&DrawCall {
        program: &p.program,
        blend: &p.blend,
        depth_stencil: &p.depth_stencil,
        raster: &p.raster,
        vertex_layout: &p.vertex_layout,
        bindings: [
            p.sets[0].as_ref(),
            p.sets[1].as_ref(),
            p.sets[2].as_ref(),
            p.sets[3].as_ref(),
        ],
        vertex_buffers: &vertex_buffers,
        index: p.index.as_ref().map(|buffer| IndexBinding {
            buffer,
            offset: 0,
            format: IndexFormat::U16,
        }),
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: if indexed {
            DrawRange::Indexed {
                first: 0,
                count,
                base_vertex: 0,
                first_instance: 0,
                instances,
            }
        } else {
            DrawRange::Vertices {
                first: 0,
                count,
                first_instance: 0,
                instances,
            }
        },
    });
}

/// The targets a scene draws into.
pub struct Targets {
    pub color: <B as Backend>::Texture,
    color_view: <B as Backend>::TextureView,
    depth_view: Option<<B as Backend>::TextureView>,
}

pub fn targets(b: &mut B, depth: bool) -> Result<Targets> {
    let (color, color_view) = texture_2d(
        b,
        Format::Rgba8Unorm,
        WIDTH,
        HEIGHT,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )?;
    let depth_view = if depth {
        Some(texture_2d(b, Format::Depth32Float, WIDTH, HEIGHT, TextureUsage::RENDER)?.1)
    } else {
        None
    };
    Ok(Targets {
        color,
        color_view,
        depth_view,
    })
}

/// Begins a pass on `t`, cleared as the reference scenes clear (`clear` true) or loaded.
pub fn begin(b: &mut B, t: &Targets, clear: bool) {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: t.depth_view.as_ref().map(|view| DepthAttachment {
            view,
            depth: Some(AspectOps {
                load: if clear {
                    LoadOp::Clear(1.0f32.to_bits())
                } else {
                    LoadOp::Load
                },
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: None,
        }),
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &t.color_view,
        depth_slice: 0,
        resolve: None,
        load: if clear {
            LoadOp::Clear(scene::CLEAR.map(f32::to_bits))
        } else {
            LoadOp::Load
        },
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
}

/// Starts reading the colour target back.
pub fn read(b: &mut B, t: &Targets) -> Result<<B as Backend>::Readback> {
    gpu(b.read_back(&ReadSource::Texture {
        region: TextureRegion {
            texture: &t.color,
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
    }))
}

/// Draws a scene into fresh targets, starts its read back and submits.
pub fn render(b: &mut B, which: Scene, twist: Twist) -> Result<<B as Backend>::Readback> {
    let p = prepare(b, which, twist)?;
    let t = targets(b, p.depth)?;
    begin(b, &t, true);
    draw(b, &p);
    b.end_pass();
    let rb = read(b, &t)?;
    b.submit();
    Ok(rb)
}

/// RGBA8 rows as a PNG file.
pub fn png(rgba: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, WIDTH, HEIGHT);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()?.write_image_data(rgba)?;
    Ok(out)
}
