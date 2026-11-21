//! The stage helper's routes of a vertex shader alone on the WebGPU back end, against real Direct3D 11:
//! alpha-antialiased lines (and WARP drawing them), both blending each line's colour over the target
//! by its alpha; and the primitive's number for a pixel shader reading it. Seeded vertices:
//! `WCONTAINERS_LINE_SEED` (default 1) and `WCONTAINERS_LINE_CASES` seeds from it (default 4); every
//! case runs per seed. An indexed list Direct3D draws nothing of is passed over for the seed's next
//! (`scene`), and ours must draw nothing of it either.

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BlendComponent, BlendDesc, BlendFactor, BlendOp,
    BufferRange, BufferUsage, ColorAttachment, ColorWrites, CullMode, DepthStencilDesc,
    DrawBaseSource, DrawCall, DrawRange, Extent, FillMode, Format, FrontFace, HelperValue,
    IndexBinding, IndexFormat, LinkedValues, Origin, PassDesc, Program, ProvokingVertex,
    RasterDesc, ReadSource, RenderTargetBlend, ShaderStage, StageSet, StoreOp, TextureRegion,
    TextureUsage, VariantFlags, VariantKey, VertexBinding, VertexFormat, Viewport, helper_binding,
};
use wcontainers_graphics_test::Driver;
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::stage::{InputTopology, StageDraw, StageIndex, Stages};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

/// Vertices per seed.
const VERTICES: usize = 24;
/// Pixels from the target's edges no vertex comes nearer than.
const MARGIN: f32 = 4.0;

struct Case {
    topology: InputTopology,
    count: u32,
    instances: u32,
    /// Drawn through 16-bit indices ([`indices`]).
    indexed: bool,
}

/// Which route of the stage helper draws, and with which pixel shader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    /// `ps_line`, its alpha multiplied by the coverage (`LINE_COVERAGE`).
    Lines,
    /// `ps_primitive`, reading its primitive's number (`PRIMITIVE_ID_FROM_STAGE`).
    Primitives,
}

impl Route {
    fn pixel_shader(self) -> &'static str {
        match self {
            Route::Lines => "ps_line",
            Route::Primitives => "ps_primitive",
        }
    }
}

/// Primitives of every list and strip, one draw instanced (Direct3D numbers each instance's
/// primitives from zero).
const NUMBERED: [Case; 7] = [
    Case {
        topology: InputTopology::TriangleList,
        count: VERTICES as u32,
        instances: 1,
        indexed: false,
    },
    Case {
        topology: InputTopology::TriangleStrip,
        count: 11,
        instances: 2,
        indexed: false,
    },
    Case {
        topology: InputTopology::LineList,
        count: VERTICES as u32,
        instances: 1,
        indexed: false,
    },
    Case {
        topology: InputTopology::LineStrip,
        count: 9,
        instances: 1,
        indexed: false,
    },
    Case {
        topology: InputTopology::PointList,
        count: VERTICES as u32,
        instances: 1,
        indexed: false,
    },
    Case {
        topology: InputTopology::TriangleStrip,
        count: 14,
        instances: 2,
        indexed: true,
    },
    Case {
        topology: InputTopology::LineStrip,
        count: 12,
        instances: 1,
        indexed: true,
    },
];

const CASES: [Case; 3] = [
    Case {
        topology: InputTopology::LineList,
        count: VERTICES as u32,
        instances: 1,
        indexed: false,
    },
    Case {
        topology: InputTopology::LineStrip,
        count: 9,
        instances: 1,
        indexed: false,
    },
    Case {
        topology: InputTopology::LineStrip,
        count: 12,
        instances: 1,
        indexed: true,
    },
];

/// Seeded vertices inside the target, at depth 0.5, with opaque colours.
fn vertices(seed: u64) -> Vec<f32> {
    let mut rng = fastrand::Rng::with_seed(seed);
    let side = SIZE as f32 - 2.0 * MARGIN;
    let mut v = Vec::new();
    for _ in 0..VERTICES {
        let (x, y) = (MARGIN + rng.f32() * side, MARGIN + rng.f32() * side);
        v.extend([
            x / SIZE as f32 * 2.0 - 1.0,
            1.0 - y / SIZE as f32 * 2.0,
            0.5,
        ]);
        v.extend([rng.f32(), rng.f32(), rng.f32(), 1.0]);
    }
    v
}

/// One index in this many is a restart, besides the one in the middle of the draw.
const RESTART_ONE_IN: u32 = 6;

/// Seeded 16-bit indices of an indexed case's strip, with restart indices: one in the middle of the
/// draw and others at seeded places; an even number of them, as the stage helper reads whole words.
/// Only lists holding a whole primitive are given; `attempt` picks one among them, in order.
fn indices(seed: u64, case: &Case, attempt: u32) -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x5eed);
    let first_primitive = if case.topology == InputTopology::LineStrip {
        2
    } else {
        3
    };
    let mut list = || loop {
        let indices: Vec<u8> = (0..case.count.next_multiple_of(2))
            .flat_map(|i| {
                let index = rng.u16(..VERTICES as u16);
                let restart = i == case.count / 2 || rng.u32(..RESTART_ONE_IN) == 0;
                if restart { RESTART } else { index }.to_le_bytes()
            })
            .collect();
        if whole_strip_primitive(&indices, 0..case.count, first_primitive) {
            return indices;
        }
    };
    (0..attempt).for_each(|_| drop(list()));
    list()
}

fn d3d_topology(t: InputTopology) -> D3D_PRIMITIVE_TOPOLOGY {
    match t {
        InputTopology::PointList => D3D11_PRIMITIVE_TOPOLOGY_POINTLIST,
        InputTopology::LineList => D3D11_PRIMITIVE_TOPOLOGY_LINELIST,
        InputTopology::LineStrip => D3D11_PRIMITIVE_TOPOLOGY_LINESTRIP,
        InputTopology::TriangleList => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        _ => D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
    }
}

fn direct3d(
    gpu: &Gpu,
    case: &Case,
    (vertices, indices): (&[f32], &[u8]),
    blending: bool,
    route: Route,
) -> Vec<u8> {
    let (vs, ps) = (
        bytecode("vs_line.dxbc"),
        bytecode(&format!("{}.dxbc", route.pixel_shader())),
    );
    let elements = [
        d3d11::element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
        d3d11::element(
            s!("COLOR"),
            0,
            DXGI_FORMAT_R32G32B32A32_FLOAT,
            0,
            POSITION_BYTES,
            0,
        ),
    ];
    gpu.program(&vs, &ps, &elements).unwrap();
    let _buffer = gpu
        .vertices(0, bytes_of(vertices), COLOURED_VERTEX_BYTES)
        .unwrap();
    let raster = gpu
        .rasterizer(&D3D11_RASTERIZER_DESC {
            AntialiasedLineEnable: (route == Route::Lines).into(),
            ..d3d11::solid()
        })
        .unwrap();
    let mut blend = D3D11_BLEND_DESC::default();
    blend.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: blending.into(),
        SrcBlend: D3D11_BLEND_SRC_ALPHA,
        DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ONE,
        DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    let blend = gpu.blend(&blend).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    let index_buffer = case
        .indexed
        .then(|| gpu.immutable(indices, D3D11_BIND_INDEX_BUFFER).unwrap());
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&raster);
        c.OMSetBlendState(&blend, None, !0);
        c.IASetPrimitiveTopology(d3d_topology(case.topology));
        c.ClearRenderTargetView(&rtv, if blending { &CLEAR } else { &[0.0; 4] });
        c.OMSetRenderTargets(Some(&[Some(rtv)]), None);
        gpu.viewport(SIZE, SIZE);
        match &index_buffer {
            Some(ib) => {
                c.IASetIndexBuffer(ib, DXGI_FORMAT_R16_UINT, 0);
                c.DrawIndexedInstanced(case.count, case.instances, 0, 0, 0);
            }
            None => c.DrawInstanced(case.count, case.instances, 0, 0),
        }
        c.OMSetBlendState(None, None, !0);
    }
    read(gpu, &target, Format::Rgba8Unorm)
}

/// Each line's colour over the target by its alpha, as [`direct3d`] blends.
fn blended() -> BlendDesc {
    BlendDesc {
        targets: [RenderTargetBlend {
            blend: Some((
                BlendComponent {
                    src: BlendFactor::SrcAlpha,
                    dst: BlendFactor::OneMinusSrcAlpha,
                    op: BlendOp::Add,
                },
                BlendComponent {
                    src: BlendFactor::One,
                    dst: BlendFactor::OneMinusSrcAlpha,
                    op: BlendOp::Add,
                },
            )),
            writes: ColorWrites::all(),
        }; 8],
        same_for_all: true,
        alpha_to_coverage: false,
        logic_op: None,
    }
}

fn ours<B: Backend>(
    b: &mut B,
    case: &Case,
    (vertices, indices): (&[f32], &[u8]),
    dxil: bool,
    blending: bool,
    route: Route,
) -> Vec<u8> {
    let kind = if dxil { "dxil" } else { "dxbc" };
    let code = |name: &str| bytecode(&format!("{name}.{kind}"));
    let (vs_code, ps_code) = (code("vs_line"), code(route.pixel_shader()));
    fn source(c: &[u8], dxil: bool) -> Source<'_> {
        if dxil {
            Source::Dxil(c)
        } else {
            Source::Dxbc(c)
        }
    }
    let read = |c| source(c, dxil);
    // Linked as a front links them: the pixel program's coverage or primitive number past every value
    // either program passes, every vertex input pulled.
    let plain = convert(read(&ps_code), &VariantKey::default(), None).unwrap();
    let vs_key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        draw_base: DrawBaseSource::Helpers,
        ..Default::default()
    };
    let vs = convert(read(&vs_code), &vs_key, None).unwrap();
    let location = vs
        .reflection
        .outputs
        .iter()
        .chain(&plain.reflection.inputs)
        .filter(|v| v.system.is_none())
        .map(|v| v.location + 1)
        .max()
        .unwrap_or(0);
    let ps_key = VariantKey {
        flags: match route {
            Route::Lines => VariantFlags::LINE_COVERAGE,
            Route::Primitives => VariantFlags::PRIMITIVE_ID_FROM_STAGE,
        },
        linked: LinkedValues {
            location,
            clip_distances: 0,
        },
        ..Default::default()
    };
    let ps = convert(read(&ps_code), &ps_key, None).unwrap();
    let mut stages = match route {
        Route::Lines => Stages::antialiased_lines(b, &vs, &ps_key).unwrap(),
        Route::Primitives => Stages::primitive_numbered(b, &vs, &ps_key).unwrap(),
    };

    // The vertex stage's sets: the pulled streams in set 0, their layout in its helper block.
    let vertex_buffer = buffer(
        b,
        bytes_of(vertices),
        BufferUsage::VERTEX | BufferUsage::STORAGE,
    );
    let size = std::mem::size_of_val(vertices) as u64;
    let entries: Vec<_> = vs
        .reflection
        .bindings
        .iter()
        .map(|r| BindingEntry {
            binding: r.binding,
            stages: StageSet::COMPUTE,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: &vertex_buffer,
                    offset: 0,
                    size,
                },
                writable: false,
            },
        })
        .collect();
    let vertex_set = b.create_binding_set(&entries).unwrap();
    let mut helpers = vec![0u8; vs.reflection.helper_size as usize];
    for h in &vs.reflection.helpers {
        let words = match h.value {
            HelperValue::PulledStream { input } => [
                if input == 0 { 0 } else { POSITION_BYTES },
                COLOURED_VERTEX_BYTES,
                0,
                0,
            ],
            HelperValue::DrawBase => [0; 4],
            _ => panic!("{h:?}"),
        };
        let at = h.offset as usize;
        for (i, w) in words.iter().enumerate() {
            let word = std::mem::size_of::<u32>();
            helpers[at + word * i..at + word * (i + 1)].copy_from_slice(&w.to_le_bytes());
        }
    }
    let helper_buffer = buffer(b, &helpers, BufferUsage::UNIFORM);
    let vertex_helpers = b
        .create_binding_set(&[BindingEntry {
            binding: helper_binding(ShaderStage::Compute),
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &helper_buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap();
    let index_buffer = case
        .indexed
        .then(|| buffer(b, indices, BufferUsage::INDEX | BufferUsage::STORAGE));
    let draw = StageDraw {
        vertex_sets: [Some(&vertex_set), None, None, Some(&vertex_helpers)],
        geometry_sets: [None; 4],
        index: index_buffer.as_ref().map(|buffer| StageIndex {
            buffer,
            size: indices.len() as u64,
            offset: 0,
            format: IndexFormat::U16,
        }),
        topology: case.topology,
        range: match case.indexed {
            true => DrawRange::Indexed {
                first: 0,
                count: case.count,
                base_vertex: 0,
                first_instance: 0,
                instances: case.instances,
            },
            false => DrawRange::Vertices {
                first: 0,
                count: case.count,
                first_instance: 0,
                instances: case.instances,
            },
        },
        stream_output: None,
    };
    let last = match route {
        Route::Lines => stages.run_antialiased_lines(b, &draw, [SIZE as f32; 2]),
        Route::Primitives => stages.run(b, &draw),
    }
    .unwrap()
    .expect("primitives");

    let vertex = b.create_shader(stages.passthrough().unwrap()).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap();
    let vertex_layout = b
        .create_vertex_layout(&stages.vertex_layout().unwrap())
        .unwrap();
    let blend = b
        .create_blend(&if blending {
            blended()
        } else {
            BlendDesc {
                targets: [RenderTargetBlend {
                    blend: None,
                    writes: ColorWrites::all(),
                }; 8],
                ..blended()
            }
        })
        .unwrap();
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })
        .unwrap();
    let raster = b
        .create_raster(&RasterDesc {
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
        })
        .unwrap();
    let (target, target_view) = texture(
        b,
        Format::Rgba8Unorm,
        SIZE,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
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
        load: wcontainers_gpu::LoadOp::Clear(
            if blending { CLEAR } else { [0.0; 4] }.map(f32::to_bits),
        ),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        bindings: [None; 4],
        vertex_buffers: &[Some(VertexBinding {
            buffer: &last.buffer,
            offset: 0,
        })],
        index: last.index_offset.map(|offset| IndexBinding {
            buffer: &last.buffer,
            offset,
            format: IndexFormat::U32,
        }),
        topology: last.topology,
        restart: false,
        viewports: &[Viewport {
            x: 0.0f32.to_bits(),
            y: 0.0f32.to_bits(),
            width: (SIZE as f32).to_bits(),
            height: (SIZE as f32).to_bits(),
            min_depth: 0.0f32.to_bits(),
            max_depth: 1.0f32.to_bits(),
        }],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: match last.index_offset {
            Some(_) => DrawRange::Indexed {
                first: 0,
                count: last.count,
                base_vertex: 0,
                first_instance: 0,
                instances: 1,
            },
            None => DrawRange::Vertices {
                first: 0,
                count: last.count,
                first_instance: 0,
                instances: 1,
            },
        },
    });
    b.end_pass();
    let mut out = vec![0; (SIZE * SIZE * 4) as usize];
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
                width: SIZE,
                height: SIZE,
                depth_or_layers: 1,
            },
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    stages.destroy(b);
    out
}

/// The most a texel's channel may differ from the RTX 3080's, in steps of 8 bits. Direct3D leaves the
/// coverage's filter kernel to the implementation and asks only "reasonable" tolerances (functional
/// specification 3.4.4): the RTX 3080 filters over the pixel, WARP over a wider kernel, and the two
/// differ by more than this allowance, which the test checks.
const ALLOWANCE: u64 = 64;

/// Lines in pixels (x, y of each end, y down), whose alpha [`print_coverage`] prints: on pixel
/// centres, between rows, a quarter off a row, diagonal, shallow, steep, short, and ends between
/// pixel centres.
const MEASURED: [[f32; 4]; 12] = [
    [8.5, 8.5, 24.5, 8.5],
    [8.5, 12.0, 24.5, 12.0],
    [8.5, 16.25, 24.5, 16.25],
    [30.5, 4.5, 46.5, 20.5],
    [8.3, 24.7, 40.1, 31.2],
    [50.2, 4.3, 55.7, 40.9],
    [10.5, 40.5, 12.5, 40.5],
    [20.2, 50.5, 30.7, 50.5],
    [4.5, 56.5, 14.5, 56.5],
    [4.5, 59.9, 14.5, 59.9],
    [40.5, 44.5, 60.5, 54.5],
    [40.5, 58.0, 60.5, 61.0],
];

/// Prints the alpha `draw` gives [`MEASURED`]'s white lines over a transparent target, unblended:
/// the coverage, and which pixels are drawn at all.
fn print_coverage(name: &str, draw: impl FnOnce(&Case, &[f32]) -> Vec<u8>) {
    let mut vertices = Vec::new();
    for [x0, y0, x1, y1] in MEASURED {
        for (x, y) in [(x0, y0), (x1, y1)] {
            vertices.extend([
                x / SIZE as f32 * 2.0 - 1.0,
                1.0 - y / SIZE as f32 * 2.0,
                0.5,
            ]);
            vertices.extend([1.0; 4]);
        }
    }
    let case = Case {
        topology: InputTopology::LineList,
        count: (vertices.len() / (COLOURED_VERTEX_BYTES as usize / 4)) as u32,
        instances: 1,
        indexed: false,
    };
    let picture = draw(&case, &vertices);
    eprintln!("{name}: alpha, '.' where nothing is drawn");
    for y in 0..SIZE {
        let row: String = (0..SIZE)
            .map(|x| {
                let t = &picture[((y * SIZE + x) * 4) as usize..][..4];
                if t[0] == 0 {
                    "  .".to_string()
                } else {
                    format!("{:3}", t[3])
                }
            })
            .collect();
        eprintln!("{y:2} {row}");
    }
}

/// The largest channel difference of each texel, as a picture of one byte per texel.
fn apart(a: &[u8], b: &[u8]) -> Vec<u64> {
    a.chunks(4)
        .zip(b.chunks(4))
        .map(|(x, y)| {
            x.iter()
                .zip(y)
                .map(|(p, q)| u64::from(p.abs_diff(*q)))
                .max()
                .unwrap()
        })
        .collect()
}

#[test]
fn antialiased_lines_draw_as_direct3d_does() {
    let mut backend = webgpu();
    let gpus: Vec<(Driver, Gpu)> = [Driver::Hardware, Driver::Warp]
        .into_iter()
        .map(|d| {
            (
                d,
                d3d11::create(d, false, &[D3D_FEATURE_LEVEL_11_0]).unwrap(),
            )
        })
        .collect();
    for (driver, gpu) in &gpus {
        eprintln!("direct3d 11: {}", gpu.adapter().unwrap());
        if std::env::var_os("WCONTAINERS_LINE_COVERAGE").is_some() {
            print_coverage(&format!("{driver:?}"), |case, v| {
                direct3d(gpu, case, (v, &[]), false, Route::Lines)
            });
        }
    }
    if std::env::var_os("WCONTAINERS_LINE_COVERAGE").is_some() {
        print_coverage("ours", |case, v| {
            ours(&mut backend, case, (v, &[]), false, false, Route::Lines)
        });
    }
    let seed: u64 = std::env::var("WCONTAINERS_LINE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_LINE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    let mut compared = 0;
    let mut spread = 0;
    for seed in seed..seed + seeds {
        let vertices = vertices(seed);
        for (i, case) in CASES.iter().enumerate() {
            let name = format!(
                "seed {seed} {:?}{}",
                case.topology,
                if case.indexed { " indexed" } else { "" }
            );
            let Ok((indices, references)) = scene(
                case.indexed,
                |attempt| indices(seed, case, attempt),
                |indices| -> Vec<(Driver, Vec<u8>)> {
                    gpus.iter()
                        .map(|(d, gpu)| {
                            (
                                *d,
                                direct3d(gpu, case, (&vertices, indices), true, Route::Lines),
                            )
                        })
                        .collect()
                },
                |references| drawn_texels(&references[0].1) != 0,
                |indices, _| {
                    [false, true].into_iter().all(|dxil| {
                        let picture = ours(
                            &mut backend,
                            case,
                            (&vertices, indices),
                            dxil,
                            true,
                            Route::Lines,
                        );
                        drawn_texels(&picture) == 0
                    })
                },
            )
            .map_err(|e| failures.push(format!("{name}: {e}"))) else {
                continue;
            };
            let between = apart(&references[0].1, &references[1].1);
            eprintln!(
                "seed {seed} {:?}: the RTX 3080 and WARP {} texels apart, largest {}",
                case.topology,
                between.iter().filter(|d| **d != 0).count(),
                between.iter().max().unwrap()
            );
            spread = spread.max(*between.iter().max().unwrap());
            for dxil in [false, true] {
                let picture = ours(
                    &mut backend,
                    case,
                    (&vertices, &indices),
                    dxil,
                    true,
                    Route::Lines,
                );
                for (driver, reference) in &references {
                    let name = format!(
                        "{name} {} against {driver:?}",
                        if dxil { "dxil" } else { "dxbc" }
                    );
                    let d = apart(reference, &picture);
                    let drawn = drawn_texels(reference);
                    let largest = *d.iter().max().unwrap();
                    compared += 1;
                    eprintln!(
                        "{name}: {drawn} texels drawn by direct3d, {} differ, largest {largest}",
                        d.iter().filter(|x| **x != 0).count()
                    );
                    if std::env::var_os("WCONTAINERS_SHADERS_OUT").is_some() {
                        let kind = if dxil { "dxil" } else { "dxbc" };
                        keep(
                            &format!("lines-{seed}-{i}-{driver:?}"),
                            reference,
                            Format::Rgba8Unorm,
                        );
                        keep(
                            &format!("lines-{seed}-{i}-{kind}-ours"),
                            &picture,
                            Format::Rgba8Unorm,
                        );
                    }
                    // WARP's kernel is printed beside, not held to the allowance.
                    if *driver == Driver::Warp {
                        continue;
                    }
                    if drawn == 0 || largest > ALLOWANCE {
                        for (t, x) in d
                            .iter()
                            .enumerate()
                            .filter(|(_, x)| **x > ALLOWANCE)
                            .take(16)
                        {
                            let (px, py) = (t as u32 % SIZE, t as u32 / SIZE);
                            let at = t * 4;
                            eprintln!(
                                "  ({px}, {py}) {x} apart: direct3d {:?}, ours {:?}",
                                &reference[at..at + 4],
                                &picture[at..at + 4]
                            );
                        }
                        failures.push(name);
                    }
                }
            }
        }
    }
    eprintln!(
        "{compared} comparisons, {} failed; the two references at most {spread} apart",
        failures.len()
    );
    assert!(
        spread > ALLOWANCE,
        "the allowance exceeds what two Direct3D implementations differ by"
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A pixel shader reading its primitive's number, drawn through the stage helper's primitive route
/// (`PRIMITIVE_ID_FROM_STAGE`, for adapters without the primitive index): every texel equal to real
/// Direct3D 11's, from the fxc and the dxc build.
#[test]
fn primitive_numbers_from_the_stage_helper_draw_as_direct3d_does() {
    let mut backend = webgpu();
    let gpu = d3d11::create(Driver::Hardware, false, &[D3D_FEATURE_LEVEL_11_0]).unwrap();
    eprintln!("direct3d 11: {}", gpu.adapter().unwrap());
    let seed: u64 = std::env::var("WCONTAINERS_LINE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_LINE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        let vertices = vertices(seed);
        for case in &NUMBERED {
            let name = format!(
                "seed {seed} {:?} x{}{}",
                case.topology,
                case.instances,
                if case.indexed { " indexed" } else { "" }
            );
            let Ok((indices, reference)) = scene(
                case.indexed,
                |attempt| indices(seed, case, attempt),
                |indices| direct3d(&gpu, case, (&vertices, indices), false, Route::Primitives),
                |reference| drawn_texels(reference) != 0,
                |indices, _| {
                    [false, true].into_iter().all(|dxil| {
                        let drawn_from = (&vertices[..], indices);
                        let picture = ours(
                            &mut backend,
                            case,
                            drawn_from,
                            dxil,
                            false,
                            Route::Primitives,
                        );
                        drawn_texels(&picture) == 0
                    })
                },
            )
            .map_err(|e| failures.push(format!("{name}: {e}"))) else {
                continue;
            };
            let drawn_from = (&vertices[..], &indices[..]);
            let drawn = drawn_texels(&reference);
            for dxil in [false, true] {
                let picture = ours(
                    &mut backend,
                    case,
                    drawn_from,
                    dxil,
                    false,
                    Route::Primitives,
                );
                let name = format!("{name} {}", if dxil { "dxil" } else { "dxbc" });
                let d = apart(&reference, &picture);
                let count = d.iter().filter(|x| **x != 0).count();
                compared += 1;
                eprintln!("{name}: {drawn} texels drawn by direct3d, {count} differ");
                if drawn == 0 || count != 0 {
                    for (t, _) in d.iter().enumerate().filter(|(_, x)| **x != 0).take(16) {
                        let (px, py) = (t as u32 % SIZE, t as u32 / SIZE);
                        let at = t * 4;
                        eprintln!(
                            "  ({px}, {py}): direct3d {:?}, ours {:?}",
                            &reference[at..at + 4],
                            &picture[at..at + 4]
                        );
                    }
                    failures.push(name);
                }
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
