//! Geometry shaders through the stage helper (the vertex and geometry stages as compute, then the
//! pass-through draw) on the WebGPU back end, against real Direct3D 11 drawing the same shaders on the
//! same GPU. Seeded vertices: `WCONTAINERS_GEOMETRY_SEED` (default 1) and `WCONTAINERS_GEOMETRY_CASES`
//! seeds from it (default 4); every case runs per seed.

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    AddedBinding, Backend, BindingEntry, BindingResource, BlendDesc, BufferRange, BufferUsage,
    ColorAttachment, ColorWrites, CullMode, DepthStencilDesc, DrawBaseSource, DrawCall, DrawRange,
    Extent, FillMode, Format, FrontFace, HelperValue, IndexBinding, IndexFormat, Interpolation,
    Origin, PassDesc, Program, ProvokingVertex, RasterDesc, ReadSource, RenderTargetBlend,
    ShaderStage, SlotKind, StageSet, StoreOp, TextureRegion, TextureUsage, VariantKey,
    VertexBinding, VertexFormat, Viewport, helper_binding,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::stage::{InputTopology, StageDraw, StageIndex, Stages};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

/// Vertices per seed: position (x, y, z) and colour (r, g, b, a), 28 bytes each.
const VERTICES: usize = 64;
const STRIDE: u32 = 28;

struct Case {
    gs: &'static str,
    topology: InputTopology,
    count: u32,
    first: u32,
    instances: u32,
    first_instance: u32,
    /// Indexed with this base vertex.
    indexed: Option<i32>,
}

const CASES: [Case; 13] = [
    Case {
        gs: "gs_quads",
        topology: InputTopology::PointList,
        count: 24,
        first: 3,
        instances: 2,
        first_instance: 1,
        indexed: None,
    },
    Case {
        gs: "gs_quads",
        topology: InputTopology::PointList,
        count: 20,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: Some(5),
    },
    Case {
        gs: "gs_cut",
        topology: InputTopology::TriangleList,
        count: 18,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "gs_cut",
        topology: InputTopology::TriangleStrip,
        count: 9,
        first: 4,
        instances: 2,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "gs_cut",
        topology: InputTopology::TriangleList,
        count: 12,
        first: 6,
        instances: 1,
        first_instance: 0,
        indexed: Some(-2),
    },
    Case {
        gs: "gs_instances",
        topology: InputTopology::TriangleList,
        count: 6,
        first: 0,
        instances: 2,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "gs_lines",
        topology: InputTopology::LineList,
        count: 10,
        first: 1,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "gs_lines",
        topology: InputTopology::LineStrip,
        count: 7,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "gs_points",
        topology: InputTopology::PointList,
        count: 30,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "gs_adjacent",
        topology: InputTopology::TriangleListAdjacency,
        count: 18,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "gs_adjacent",
        topology: InputTopology::TriangleListAdjacency,
        count: 12,
        first: 0,
        instances: 2,
        first_instance: 3,
        indexed: Some(4),
    },
    Case {
        gs: "gs_line_adjacent",
        topology: InputTopology::LineListAdjacency,
        count: 12,
        first: 1,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "gs_line_adjacent",
        topology: InputTopology::LineStripAdjacency,
        count: 9,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
];

fn d3d_topology(t: InputTopology) -> D3D_PRIMITIVE_TOPOLOGY {
    match t {
        InputTopology::PointList => D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        InputTopology::LineList => D3D_PRIMITIVE_TOPOLOGY_LINELIST,
        InputTopology::LineStrip => D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
        InputTopology::TriangleList => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        InputTopology::TriangleStrip => D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        InputTopology::LineListAdjacency => D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ,
        InputTopology::LineStripAdjacency => D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ,
        InputTopology::TriangleListAdjacency => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ,
        InputTopology::TriangleStripAdjacency => D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ,
    }
}

/// Seeded vertices inside the target, at depth 0.5, with opaque colours.
fn vertices(seed: u64) -> Vec<f32> {
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut v = Vec::new();
    for _ in 0..VERTICES {
        v.extend([rng.f32() * 1.6 - 0.8, rng.f32() * 1.6 - 0.8, 0.5]);
        v.extend([rng.f32(), rng.f32(), rng.f32(), 1.0]);
    }
    v
}

/// Seeded 16-bit indices, kept inside the vertices with the case's base vertex.
fn indices(seed: u64, case: &Case) -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x5eed);
    let base = case.indexed.unwrap_or(0);
    let low = (-base).max(0) as u16;
    let high = (VERTICES as i32 - base.max(0)) as u16;
    (0..case.first + case.count)
        .flat_map(|_| rng.u16(low..high).to_le_bytes())
        .collect()
}

/// The geometry shader's constants: the quad and offset size, then the tint.
fn constants() -> Vec<u8> {
    [0.08f32, 0.06, 0.0, 0.0, 0.9, 0.8, 1.0, 1.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

fn direct3d(gpu: &Gpu, case: &Case, vertices: &[f32], indices: &[u8]) -> Vec<u8> {
    let (vs, ps, gs) = (
        bytecode("vs_geometry.dxbc"),
        bytecode("ps_geometry.dxbc"),
        bytecode(&format!("{}.dxbc", case.gs)),
    );
    let elements = [
        d3d11::element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
        d3d11::element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 12, 0),
    ];
    gpu.program(&vs, &ps, &elements).unwrap();
    let geometry = gpu.gs(&gs).unwrap();
    let _buffer = gpu.vertices(0, bytes_of(vertices), STRIDE).unwrap();
    let constants = gpu
        .immutable(&constants(), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let raster = gpu.rasterizer(&d3d11::solid()).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    let index_buffer = case
        .indexed
        .map(|_| gpu.immutable(indices, D3D11_BIND_INDEX_BUFFER).unwrap());
    unsafe {
        let c = &gpu.context;
        c.GSSetShader(&geometry, None);
        c.GSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.RSSetState(&raster);
        c.IASetPrimitiveTopology(d3d_topology(case.topology));
        c.ClearRenderTargetView(&rtv, &CLEAR);
        c.OMSetRenderTargets(Some(&[Some(rtv)]), None);
        gpu.viewport(SIZE, SIZE);
        match (case.indexed, &index_buffer) {
            (Some(base), Some(ib)) => {
                c.IASetIndexBuffer(ib, DXGI_FORMAT_R16_UINT, 0);
                c.DrawIndexedInstanced(
                    case.count,
                    case.instances,
                    case.first,
                    base,
                    case.first_instance,
                );
            }
            _ => c.DrawInstanced(case.count, case.instances, case.first, case.first_instance),
        }
        c.GSSetShader(None, None);
    }
    read(gpu, &target, Format::Rgba8Unorm)
}

fn ours<B: Backend>(b: &mut B, case: &Case, vertices: &[f32], indices: &[u8]) -> Vec<u8> {
    // Linked as a front links them: the pixel program first, the geometry program's outputs written
    // as it reads them, every vertex input pulled.
    let ps = convert(
        Source::Dxbc(&bytecode("ps_geometry.dxbc")),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    let gs_key = VariantKey {
        output_interpolation: ps
            .reflection
            .inputs
            .iter()
            .filter(|v| v.system.is_none() && v.interpolation != Interpolation::Perspective)
            .map(|v| (v.location, v.interpolation))
            .collect(),
        ..Default::default()
    };
    let gs = convert(
        Source::Dxbc(&bytecode(&format!("{}.dxbc", case.gs))),
        &gs_key,
        None,
    )
    .unwrap();
    let vs_key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        draw_base: DrawBaseSource::Helpers,
        ..Default::default()
    };
    let vs = convert(Source::Dxbc(&bytecode("vs_geometry.dxbc")), &vs_key, None).unwrap();
    let mut stages = Stages::new(b, &vs, &gs).unwrap();

    // The vertex stage's sets: the pulled streams in set 0, their layout in its helper block.
    let vertex_buffer = buffer(
        b,
        bytes_of(vertices),
        BufferUsage::VERTEX | BufferUsage::STORAGE,
    );
    let size = std::mem::size_of_val(vertices) as u64;
    let mut entries = Vec::new();
    for r in &vs.reflection.bindings {
        assert!(
            matches!(r.added, Some(AddedBinding::PulledStream(_))),
            "{r:?}"
        );
        entries.push(BindingEntry {
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
        });
    }
    let vertex_set = b.create_binding_set(&entries).unwrap();
    let mut helpers = vec![0u8; vs.reflection.helper_size as usize];
    // The draw's bases, as the front writes them: Direct3D's vertex and instance numbers leave them
    // out.
    let base = case.indexed.unwrap_or(case.first as i32);
    for h in &vs.reflection.helpers {
        let words = match h.value {
            HelperValue::PulledStream { input } => [if input == 0 { 0 } else { 12 }, STRIDE, 0, 0],
            HelperValue::DrawBase => [base as u32, case.first_instance, 0, 0],
            _ => panic!("{h:?}"),
        };
        let at = h.offset as usize;
        for (i, w) in words.iter().enumerate() {
            helpers[at + 4 * i..at + 4 * i + 4].copy_from_slice(&w.to_le_bytes());
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
    // The geometry stage's set: its constant buffer.
    let constants = constants();
    let constant_buffer = buffer(b, &constants, BufferUsage::UNIFORM);
    let geometry_entries: Vec<_> = gs
        .reflection
        .bindings
        .iter()
        .map(|r| {
            assert_eq!((r.kind, r.register), (SlotKind::Uniform, 0), "{r:?}");
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &constant_buffer,
                    offset: 0,
                    size: constants.len() as u64,
                }),
            }
        })
        .collect();
    let geometry_set = b.create_binding_set(&geometry_entries).unwrap();
    assert!(gs.reflection.helpers.is_empty());
    let index_buffer = case
        .indexed
        .map(|_| buffer(b, indices, BufferUsage::INDEX | BufferUsage::STORAGE));
    let range = match case.indexed {
        Some(base_vertex) => DrawRange::Indexed {
            first: case.first,
            count: case.count,
            base_vertex,
            first_instance: case.first_instance,
            instances: case.instances,
        },
        None => DrawRange::Vertices {
            first: case.first,
            count: case.count,
            first_instance: case.first_instance,
            instances: case.instances,
        },
    };
    let last = stages
        .run(
            b,
            &StageDraw {
                vertex_sets: [Some(&vertex_set), None, None, Some(&vertex_helpers)],
                geometry_sets: [Some(&geometry_set), None, None, None],
                index: index_buffer.as_ref().map(|buffer| StageIndex {
                    buffer,
                    size: indices.len() as u64,
                    offset: 0,
                    format: IndexFormat::U16,
                }),
                topology: case.topology,
                range,
            },
        )
        .unwrap()
        .expect("primitives");

    // The draw after the stages: the helper's vertex program with the pixel program.
    let vertex = b.create_shader(stages.passthrough()).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap();
    let vertex_layout = b.create_vertex_layout(&stages.vertex_layout()).unwrap();
    let blend = b
        .create_blend(&BlendDesc {
            targets: [RenderTargetBlend {
                blend: None,
                writes: ColorWrites::all(),
            }; 8],
            same_for_all: true,
            alpha_to_coverage: false,
            logic_op: None,
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
        load: wcontainers_gpu::LoadOp::Clear(CLEAR.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    let range = match last.index_offset {
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
    };
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
        restart: true,
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
        range,
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
    out
}

#[test]
fn geometry_shaders_draw_as_direct3d_does() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    eprintln!("direct3d 11: {}", gpu.adapter().unwrap());
    let seed: u64 = std::env::var("WCONTAINERS_GEOMETRY_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_GEOMETRY_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        let vertices = vertices(seed);
        for case in &CASES {
            let indices = indices(seed, case);
            let name = format!(
                "seed {seed} {} {:?} {} from {} x{}{}",
                case.gs,
                case.topology,
                case.count,
                case.first,
                case.instances,
                case.indexed
                    .map(|b| format!(" indexed, base {b}"))
                    .unwrap_or_default()
            );
            let reference = direct3d(&gpu, case, &vertices, &indices);
            // Texels drawn: those unlike the corner, which no case draws.
            let drawn = reference
                .chunks(4)
                .filter(|t| *t != &reference[..4])
                .count();
            let picture = ours(&mut backend, case, &vertices, &indices);
            let (count, largest) = differ(&reference, &picture, Format::Rgba8Unorm);
            compared += 1;
            eprintln!(
                "{name}: {drawn} texels drawn by direct3d, {count} differ (largest {largest})"
            );
            if std::env::var_os("WCONTAINERS_SHADERS_OUT").is_some() {
                keep(
                    &format!("geometry-{seed}-{}-d3d", case.gs),
                    &reference,
                    Format::Rgba8Unorm,
                );
                keep(
                    &format!("geometry-{seed}-{}-ours", case.gs),
                    &picture,
                    Format::Rgba8Unorm,
                );
            }
            if drawn == 0 || count > 0 {
                failures.push(name);
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
