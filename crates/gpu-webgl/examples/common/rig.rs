//! The rig's Direct3D 11 pictures of polygon filling, depth bias and depth clipping (its `raster`
//! and `depth` scenes, crates/graphics-test/src/d3d11/scenes), drawn through the
//! interface with the same vertices, cells and states, for the features WebGL extensions give.

use wcontainers_gpu::*;

/// Position and colour, as the rig's `Colored`.
type Colored = [f32; 7];

/// One output of a rig scene: its reference's name, size, whether it holds depth (as RGBA floats,
/// depth then stencil, as the rig stores depth) and the feature it needs past WebGL 2's core.
pub struct Output {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub depth: bool,
    pub needs: Features,
}

pub const OUTPUTS: [Output; 5] = [
    Output {
        name: "raster-fill",
        width: 256,
        height: 64,
        depth: false,
        needs: Features::POLYGON_MODE,
    },
    Output {
        name: "depth-bias",
        width: 256,
        height: 64,
        depth: false,
        needs: Features::DEPTH_BIAS_CLAMP,
    },
    Output {
        name: "depth-bias_depth",
        width: 256,
        height: 64,
        depth: true,
        needs: Features::DEPTH_BIAS_CLAMP,
    },
    Output {
        name: "depth-clip",
        width: 192,
        height: 64,
        depth: false,
        needs: Features::DEPTH_CLAMP,
    },
    Output {
        name: "depth-clip_depth",
        width: 192,
        height: 64,
        depth: true,
        needs: Features::DEPTH_CLAMP,
    },
];

const PROGRAM: &str = "
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}
@vertex fn vs(@location(0) p: vec3<f32>, @location(1) c: vec4<f32>) -> VsOut {
    return VsOut(vec4<f32>(p, 1.0), c);
}
@fragment fn ps(i: VsOut) -> @location(0) vec4<f32> {
    return i.color;
}
";

/// The rig's `hue`.
fn hue(i: u32, n: u32) -> [f32; 3] {
    let h = i as f32 / n as f32 * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}

/// The rig's `windings`: a clockwise red triangle and an anticlockwise green one.
fn windings() -> Vec<Colored> {
    let red = |x: f32, y: f32| [x, y, 0.5, 0.9, 0.2, 0.2, 1.0];
    let green = |x: f32, y: f32| [x, y, 0.5, 0.2, 0.9, 0.3, 1.0];
    vec![
        red(-0.5, 0.8),
        red(-0.1, -0.8),
        red(-0.9, -0.8),
        green(0.5, 0.8),
        green(0.1, -0.8),
        green(0.9, -0.8),
    ]
}

/// The rig's `fan`: three triangles coloured by corner.
fn fan() -> Vec<Colored> {
    let points = [
        [0.0f32, 0.0],
        [0.8, -0.6],
        [0.7, 0.6],
        [-0.3, 0.85],
        [-0.85, -0.2],
    ];
    let mut v = Vec::new();
    for k in 1..4 {
        for p in [points[0], points[k], points[k + 1]] {
            let [r, g, b] = hue(v.len() as u32, 9);
            v.push([p[0], p[1], 0.5, r, g, b, 1.0]);
        }
    }
    v
}

/// The rig's `quad`: two triangles over x0..x1 and y0..y1, at depth z0 on the left to z1 on the
/// right.
fn quad(x0: f32, y0: f32, x1: f32, y1: f32, z0: f32, z1: f32, color: [f32; 4]) -> [Colored; 6] {
    let [r, g, b, a] = color;
    let v = |x: f32, y: f32| {
        let z = if x == x0 { z0 } else { z1 };
        [x, y, z, r, g, b, a]
    };
    [
        v(x0, y0),
        v(x0, y1),
        v(x1, y1),
        v(x0, y0),
        v(x1, y1),
        v(x1, y0),
    ]
}

const GREEN: [f32; 4] = [0.2, 0.85, 0.3, 1.0];
const BLUE: [f32; 4] = [0.2, 0.4, 0.95, 1.0];
const YELLOW: [f32; 4] = [0.95, 0.85, 0.2, 1.0];

/// The rig's `cell`: square `i` of `size` pixels, `columns` to a row, with a depth range.
fn cell(i: u32, columns: u32, size: f32, depth: [f32; 2]) -> Viewport {
    Viewport {
        x: ((i % columns) as f32 * size).to_bits(),
        y: ((i / columns) as f32 * size).to_bits(),
        width: size.to_bits(),
        height: size.to_bits(),
        min_depth: depth[0].to_bits(),
        max_depth: depth[1].to_bits(),
    }
}

/// Draws coloured triangles in a pass, cell by cell.
struct Drawing<B: Backend> {
    program: B::Program,
    blend: B::Blend,
    layout: B::VertexLayout,
    minus_one: bool,
}

impl<B: Backend> Drawing<B> {
    fn new(b: &mut B) -> Result<Drawing<B>, String> {
        let minus_one = !b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE);
        let source = if minus_one {
            // What a front does without CLIP_DEPTH_ZERO_ONE: clip z from zero rewritten as 2z - w.
            PROGRAM.replace("vec4<f32>(p, 1.0)", "vec4<f32>(p.xy, 2.0 * p.z - 1.0, 1.0)")
        } else {
            PROGRAM.to_owned()
        };
        let shader = |b: &mut B, entry, stage| {
            let module = naga::front::wgsl::parse_str(&source).map_err(|e| e.to_string())?;
            let info = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .map_err(|e| format!("{e:?}"))?;
            b.create_shader(&ShaderModule {
                module,
                info,
                entry_point: String::from(entry),
                stage,
                key: [0; 32],
            })
            .map_err(|e| format!("{e:?}"))
        };
        let vs = shader(b, "vs", ShaderStage::Vertex)?;
        let ps = shader(b, "ps", ShaderStage::Fragment)?;
        let program = b
            .create_program(&Program::Render {
                vertex: &vs,
                fragment: Some(&ps),
            })
            .map_err(|e| format!("{e:?}"))?;
        let blend = b
            .create_blend(&BlendDesc {
                targets: [RenderTargetBlend {
                    blend: None,
                    writes: ColorWrites::all(),
                }; MAX_COLOR_TARGETS],
                same_for_all: true,
                alpha_to_coverage: false,
                logic_op: None,
            })
            .map_err(|e| format!("{e:?}"))?;
        let mut layout = VertexLayoutDesc {
            streams: [None; MAX_VERTEX_STREAMS],
            attributes: [None; MAX_VERTEX_STREAMS],
        };
        layout.streams[0] = Some(VertexStream {
            stride: 28,
            step: StepMode::Vertex,
        });
        for (i, (offset, format)) in [(0, VertexFormat::Float32x3), (12, VertexFormat::Float32x4)]
            .into_iter()
            .enumerate()
        {
            layout.attributes[i] = Some(VertexAttribute {
                location: i as u32,
                stream: 0,
                offset,
                format,
            });
        }
        let layout = b
            .create_vertex_layout(&layout)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Drawing {
            program,
            blend,
            layout,
            minus_one,
        })
    }

    /// The rig's `solid()` changed by `change`.
    fn raster(&self, b: &mut B, change: impl FnOnce(&mut RasterDesc)) -> Result<B::Raster, String> {
        let mut d = RasterDesc {
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
            depth_minus_one: self.minus_one,
            sample_mask: u32::MAX,
        };
        change(&mut d);
        b.create_raster(&d).map_err(|e| format!("{e:?}"))
    }

    fn draw(
        &self,
        b: &mut B,
        raster: &B::Raster,
        depth: &B::DepthStencil,
        viewport: Viewport,
        vertices: &[Colored],
    ) -> Result<(), String> {
        let bytes: Vec<u8> = vertices
            .iter()
            .flatten()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let buffer = b
            .create_buffer(&BufferDesc {
                size: bytes.len() as u64,
                usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            })
            .map_err(|e| format!("{e:?}"))?;
        b.write_buffer(&buffer, 0, &bytes, WriteMode::Ordered);
        b.draw(&DrawCall {
            program: &self.program,
            blend: &self.blend,
            depth_stencil: depth,
            raster,
            vertex_layout: &self.layout,
            bindings: [None; 4],
            vertex_buffers: &[Some(VertexBinding {
                buffer: &buffer,
                offset: 0,
            })],
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            range: DrawRange::Vertices {
                first: 0,
                count: vertices.len() as u32,
                first_instance: 0,
                instances: 1,
            },
            stencil_reference: [0; 2],
            blend_constant: [0; 4],
            immediates: &[],
            viewports: &[viewport],
            scissors: &[],
        });
        Ok(())
    }
}

fn depth_block<B: Backend>(
    b: &mut B,
    compare: Option<CompareFn>,
    write: bool,
) -> Result<B::DepthStencil, String> {
    b.create_depth_stencil(&DepthStencilDesc {
        depth_compare: compare,
        depth_write: write,
        stencil: None,
        stencil_read_mask: [0xff; 2],
        stencil_write_mask: [0xff; 2],
        depth_bounds: None,
    })
    .map_err(|e| format!("{e:?}"))
}

fn texture<B: Backend>(
    b: &mut B,
    format: Format,
    w: u32,
    h: u32,
) -> Result<(B::Texture, B::TextureView), String> {
    let t = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: w,
            height: h,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        })
        .map_err(|e| format!("{e:?}"))?;
    let v = b
        .create_view(
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
        )
        .map_err(|e| format!("{e:?}"))?;
    Ok((t, v))
}

fn read<B: Backend>(b: &mut B, t: &B::Texture, w: u32, h: u32) -> Result<Vec<u8>, String> {
    let mut got = vec![0u8; (w * h * 4) as usize];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: t,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: w,
                height: h,
                depth_or_layers: 1,
            },
        },
        &mut got,
    )
    .map_err(|e| format!("{e:?}"))?;
    Ok(got)
}

/// A pass on `target` cleared to `clear`, with `depth` cleared to 1.
fn pass<'a, B: Backend>(
    target: &'a B::TextureView,
    depth: Option<&'a B::TextureView>,
    clear: [u32; 4],
) -> PassDesc<'a, B> {
    PassDesc {
        colors: core::array::from_fn(|i| {
            (i == 0).then_some(ColorAttachment {
                view: target,
                depth_slice: 0,
                load: LoadOp::Clear(clear),
                store: StoreOp::Store,
                resolve: None,
            })
        }),
        depth: depth.map(|view| DepthAttachment {
            view,
            depth: Some(AspectOps {
                load: LoadOp::Clear(1f32.to_bits()),
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: None,
        }),
        occlusion: None,
        empty: None,
    }
}

/// Draws the scene of `output` and returns its bytes: RGBA8 rows top first, or for depth RGBA f32
/// (depth, stencil 0, 0, 1). None where the back end lacks what the scene needs.
pub fn draw<B: Backend>(b: &mut B, output: &Output) -> Result<Option<Vec<u8>>, String> {
    if !b.caps().features.contains(output.needs) {
        return Ok(None);
    }
    let d = Drawing::new(b)?;
    let (w, h) = (output.width, output.height);
    let (target, target_view) = texture(b, Format::Rgba8Unorm, w, h)?;
    let clear = crate::scenes::scene::CLEAR.map(f32::to_bits);
    let full = [0.0, 1.0];
    let depth_texture = match output.name.split('_').next() {
        Some("raster-fill") => {
            let none = depth_block(b, None, false)?;
            b.begin_pass(&pass::<B>(&target_view, None, clear));
            let mut v = fan();
            v.extend(windings());
            for (i, (fill, cull)) in [
                (FillMode::Line, CullMode::None),
                (FillMode::Line, CullMode::Back),
                (FillMode::Line, CullMode::Front),
                (FillMode::Solid, CullMode::None),
            ]
            .into_iter()
            .enumerate()
            {
                let r = d.raster(b, |r| {
                    r.fill = fill;
                    r.cull = cull;
                })?;
                d.draw(b, &r, &none, cell(i as u32, 4, 64.0, full), &v)?;
            }
            b.end_pass();
            None
        }
        Some("depth-bias") => {
            let (depth, depth_view) = texture(b, Format::Depth24PlusStencil8, w, h)?;
            let less = depth_block(b, Some(CompareFn::Less), true)?;
            let solid = d.raster(b, |_| {})?;
            b.begin_pass(&pass::<B>(&target_view, Some(&depth_view), clear));
            for (i, (bias, slope, clamp)) in [
                (0, 0.0f32, 0.0f32),
                (-3, 0.0, 0.0),
                (0, -1.5, 0.0),
                (-3000, -1.5, -0.00002),
            ]
            .into_iter()
            .enumerate()
            {
                let at = cell(i as u32, 4, 64.0, full);
                d.draw(
                    b,
                    &solid,
                    &less,
                    at,
                    &quad(-0.8, -0.8, 0.8, 0.8, 0.2, 0.8, BLUE),
                )?;
                let biased = d.raster(b, |r| {
                    r.depth_bias = (bias as f32).to_bits();
                    r.depth_bias_slope = slope.to_bits();
                    r.depth_bias_clamp = clamp.to_bits();
                })?;
                d.draw(
                    b,
                    &biased,
                    &less,
                    at,
                    &quad(-0.8, -0.8, 0.8, 0.8, 0.2, 0.8, YELLOW),
                )?;
            }
            b.end_pass();
            Some(depth)
        }
        _ => {
            let (depth, depth_view) = texture(b, Format::Depth32Float, w, h)?;
            let always = depth_block(b, Some(CompareFn::Always), true)?;
            let green = quad(-0.9, -0.9, 0.9, 0.9, -0.5, 1.5, GREEN);
            b.begin_pass(&pass::<B>(&target_view, Some(&depth_view), clear));
            for (i, clip) in [true, false].into_iter().enumerate() {
                let r = d.raster(b, |r| r.depth_clip = clip)?;
                d.draw(b, &r, &always, cell(i as u32, 3, 64.0, full), &green)?;
            }
            let solid = d.raster(b, |_| {})?;
            let narrow = cell(2, 3, 64.0, [0.25, 0.75]);
            d.draw(
                b,
                &solid,
                &always,
                narrow,
                &quad(-0.9, -0.9, 0.9, 0.9, 0.0, 1.0, GREEN),
            )?;
            b.end_pass();
            Some(depth)
        }
    };
    let bytes = match (output.depth, depth_texture) {
        (true, Some(depth)) => {
            let unorm24 = output.name.starts_with("depth-bias");
            read(b, &depth, w, h)?
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|t| {
                    let v = u32::from_le_bytes(*t);
                    let d = if unorm24 {
                        (v & 0xff_ffff) as f32 / 16_777_215.0
                    } else {
                        f32::from_bits(v)
                    };
                    [d, 0.0, 0.0, 1.0]
                })
                .flat_map(f32::to_le_bytes)
                .collect()
        }
        _ => read(b, &target, w, h)?,
    };
    if b.status() != DeviceStatus::Ok {
        return Err(format!("{}: the device was lost", output.name));
    }
    Ok(Some(bytes))
}
