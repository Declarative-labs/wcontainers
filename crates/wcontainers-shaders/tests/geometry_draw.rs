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
    LinkedValues, Origin, PassDesc, Program, ProvokingVertex, RasterDesc, ReadSource,
    RenderTargetBlend, ShaderStage, SlotKind, StageSet, StoreOp, SystemValue, TextureRegion,
    TextureUsage, VariantFlags, VariantKey, VertexBinding, VertexFormat, Viewport, helper_binding,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::stage::{
    InputTopology, StageDraw, StageIndex, Stages, StreamOutput, StreamOutputEntry, StreamTargets,
};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

/// Vertices per seed: position (x, y, z) and colour (r, g, b, a), 28 bytes each.
const VERTICES: usize = 64;
const STRIDE: u32 = 28;

struct Case {
    vs: &'static str,
    gs: &'static str,
    topology: InputTopology,
    count: u32,
    first: u32,
    instances: u32,
    first_instance: u32,
    /// Indexed with this base vertex.
    indexed: Option<i32>,
}

const CASES: [Case; 15] = [
    Case {
        vs: "vs_geometry",
        gs: "gs_quads",
        topology: InputTopology::PointList,
        count: 24,
        first: 3,
        instances: 2,
        first_instance: 1,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_quads",
        topology: InputTopology::PointList,
        count: 20,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: Some(5),
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_cut",
        topology: InputTopology::TriangleList,
        count: 18,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_cut",
        topology: InputTopology::TriangleStrip,
        count: 9,
        first: 4,
        instances: 2,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_cut",
        topology: InputTopology::TriangleList,
        count: 12,
        first: 6,
        instances: 1,
        first_instance: 0,
        indexed: Some(-2),
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_instances",
        topology: InputTopology::TriangleList,
        count: 6,
        first: 0,
        instances: 2,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_lines",
        topology: InputTopology::LineList,
        count: 10,
        first: 1,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_lines",
        topology: InputTopology::LineStrip,
        count: 7,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_points",
        topology: InputTopology::PointList,
        count: 30,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_adjacent",
        topology: InputTopology::TriangleListAdjacency,
        count: 18,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_adjacent",
        topology: InputTopology::TriangleListAdjacency,
        count: 12,
        first: 0,
        instances: 2,
        first_instance: 3,
        indexed: Some(4),
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_line_adjacent",
        topology: InputTopology::LineListAdjacency,
        count: 12,
        first: 1,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_line_adjacent",
        topology: InputTopology::LineStripAdjacency,
        count: 9,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_clip",
        topology: InputTopology::TriangleList,
        count: 30,
        first: 3,
        instances: 2,
        first_instance: 1,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_clip",
        topology: InputTopology::TriangleStrip,
        count: 14,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
];

/// Vertex shaders writing cull distances, drawn without a geometry shader: the stage helper's cull
/// route.
const CULLED: [Case; 6] = [
    Case {
        vs: "vs_cull",
        gs: "",
        topology: InputTopology::TriangleList,
        count: 30,
        first: 3,
        instances: 2,
        first_instance: 1,
        indexed: None,
    },
    Case {
        vs: "vs_cull",
        gs: "",
        topology: InputTopology::TriangleStrip,
        count: 15,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_cull",
        gs: "",
        topology: InputTopology::TriangleList,
        count: 24,
        first: 3,
        instances: 1,
        first_instance: 0,
        indexed: Some(4),
    },
    Case {
        vs: "vs_cull",
        gs: "",
        topology: InputTopology::LineList,
        count: 20,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_cull",
        gs: "",
        topology: InputTopology::LineStrip,
        count: 9,
        first: 1,
        instances: 2,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_cull",
        gs: "",
        topology: InputTopology::PointList,
        count: 30,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: Some(-1),
    },
];

/// The primitives of a `vs_cull` draw that its first two cull distances drop (all vertices left of
/// x = -0.3, or all above y = 0.2), counted on the CPU so that a case dropping none is seen.
fn dropped_by_planes(case: &Case, vertices: &[f32], indices: &[u8]) -> u32 {
    let n = match case.topology {
        InputTopology::PointList => 1,
        InputTopology::LineList | InputTopology::LineStrip => 2,
        _ => 3,
    };
    let strip = matches!(
        case.topology,
        InputTopology::LineStrip | InputTopology::TriangleStrip
    );
    let primitives = if strip {
        (case.count + 1).saturating_sub(n)
    } else {
        case.count / n
    };
    let vertex = |i: u32| -> usize {
        let place = case.first + i;
        match case.indexed {
            Some(base) => {
                let at = std::mem::size_of::<u16>() * place as usize;
                let index = u16::from_le_bytes([indices[at], indices[at + 1]]);
                (i32::from(index) + base) as usize
            }
            None => place as usize,
        }
    };
    let floats = STRIDE as usize / std::mem::size_of::<f32>();
    let below = |first: u32, plane: fn(f32, f32) -> f32| {
        (0..n).all(|k| {
            let v = vertex(first + k) * floats;
            plane(vertices[v], vertices[v + 1]) < 0.0
        })
    };
    (0..primitives)
        .filter(|&p| {
            let first = if strip { p } else { p * n };
            below(first, |x, _| x + 0.3) || below(first, |_, y| 0.2 - y)
        })
        .count() as u32
}

fn lines(t: InputTopology) -> bool {
    matches!(t, InputTopology::LineList | InputTopology::LineStrip)
}

/// Whether texel (x, y)'s centre is within a pixel of `vs_cull`'s clip plane (x - y + 0.9 = 0 in clip
/// space, each instance moved right by its offset), measured across the plane.
fn near_clip_plane(case: &Case, x: u32, y: u32) -> bool {
    let pixel = 2.0 / SIZE as f32;
    let (cx, cy) = (
        (x as f32 + 0.5) * pixel - 1.0,
        1.0 - (y as f32 + 0.5) * pixel,
    );
    (0..case.instances).any(|i| {
        let distance = (cx - i as f32 * 0.25 - cy + 0.9) / std::f32::consts::SQRT_2;
        distance.abs() <= pixel * std::f32::consts::SQRT_2
    })
}

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
        InputTopology::PatchList(n) => D3D_PRIMITIVE_TOPOLOGY(32 + i32::from(n)),
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

/// Seeded 16-bit indices, kept inside the vertices with the case's base vertex, an even number of
/// them: the stage helper reads the buffer as whole words, as fronts hold it.
fn indices(seed: u64, case: &Case) -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x5eed);
    let base = case.indexed.unwrap_or(0);
    let low = (-base).max(0) as u16;
    let high = (VERTICES as i32 - base.max(0)) as u16;
    (0..(case.first + case.count).next_multiple_of(2))
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
    let (vs, ps) = (
        bytecode(&format!("{}.dxbc", case.vs)),
        bytecode("ps_geometry.dxbc"),
    );
    let elements = [
        d3d11::element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
        d3d11::element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 12, 0),
    ];
    gpu.program(&vs, &ps, &elements).unwrap();
    let geometry =
        (!case.gs.is_empty()).then(|| gpu.gs(&bytecode(&format!("{}.dxbc", case.gs))).unwrap());
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
        c.GSSetShader(geometry.as_ref(), None);
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

/// What the stage helper made: the picture, or the stream output buffers' bytes and the eight words
/// of filled sizes and counts.
enum Made {
    Picture(Vec<u8>),
    Streamed(Vec<Vec<u8>>, Vec<u32>),
}

/// Stream output buffers' sizes, the first small enough to fill in some cases, and the second's
/// first filled size (an append).
const STREAM_SIZES: [u64; 2] = [32 * 3 * 9, 4096];
const STREAM_START: [u32; 2] = [0, 24];

/// The stream output declaration of the tests: the position and the colour's last three components
/// in buffer 0 (stride 32), then a gap and the first two numbers in buffer 1 (stride 12). A geometry
/// shader writing no position (`gs_unplaced`) skips the position's place instead.
fn declaration(gs: &str) -> StreamOutput {
    let entry = |semantic: Option<&str>, start, count, slot| StreamOutputEntry {
        stream: 0,
        semantic: semantic.map(String::from),
        semantic_index: 0,
        start_component: start,
        component_count: count,
        slot,
    };
    StreamOutput {
        entries: vec![
            if gs == "gs_unplaced" {
                entry(None, 0, 4, 0)
            } else {
                entry(Some("SV_Position"), 0, 4, 0)
            },
            entry(Some("COLOR"), 1, 3, 0),
            entry(None, 0, 1, 1),
            entry(Some("TEXCOORD"), 0, 2, 1),
        ],
        strides: [32, 12, 0, 0],
    }
}

fn ours<B: Backend>(
    b: &mut B,
    case: &Case,
    vertices: &[f32],
    indices: &[u8],
    stream: Option<&StreamOutput>,
) -> Made {
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
    let gs = (!case.gs.is_empty()).then(|| {
        convert(
            Source::Dxbc(&bytecode(&format!("{}.dxbc", case.gs))),
            &gs_key,
            None,
        )
        .unwrap()
    });
    let vs_key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        draw_base: DrawBaseSource::Helpers,
        ..Default::default()
    };
    let vs_code = bytecode(&format!("{}.dxbc", case.vs));
    let vs = convert(Source::Dxbc(&vs_code), &vs_key, None).unwrap();
    // Clip distances from the last vertex stage, clipped in the pixel program as on a back end
    // without them (wgpu's Direct3D 12 path), linked past every value either program passes;
    // counted from the reflection as a front counts them.
    let last_stage = gs.as_ref().unwrap_or(&vs);
    let clips: u32 = last_stage
        .reflection
        .outputs
        .iter()
        .filter(|v| v.system == Some(SystemValue::ClipDistance))
        .map(|v| v.mask.count_ones())
        .sum();
    let (vs, gs, ps) = if clips == 0 {
        (vs, gs, ps)
    } else {
        let location = last_stage
            .reflection
            .outputs
            .iter()
            .chain(&ps.reflection.inputs)
            .filter(|v| v.system.is_none())
            .map(|v| v.location + 1)
            .max()
            .unwrap_or(0);
        let linked = LinkedValues {
            location,
            clip_distances: clips as u8,
        };
        let key = |base: &VariantKey, samples| VariantKey {
            flags: VariantFlags::CLIP_DISTANCE,
            linked,
            samples,
            ..base.clone()
        };
        let ps_code = bytecode("ps_geometry.dxbc");
        let ps = convert(
            Source::Dxbc(&ps_code),
            &key(&VariantKey::default(), 1),
            None,
        )
        .unwrap();
        match gs {
            Some(_) => {
                let gs_code = bytecode(&format!("{}.dxbc", case.gs));
                let gs = convert(Source::Dxbc(&gs_code), &key(&gs_key, 0), None).unwrap();
                (vs, Some(gs), ps)
            }
            None => {
                let vs = convert(Source::Dxbc(&vs_code), &key(&vs_key, 0), None).unwrap();
                (vs, None, ps)
            }
        }
    };
    // Without a geometry stage or stream output, the vertex stage writes cull distances.
    let mut stages = match (&gs, stream) {
        (None, None) => Stages::culled(b, &vs, None).unwrap(),
        _ => Stages::new(b, &vs, gs.as_ref(), stream).unwrap(),
    };

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
        .iter()
        .flat_map(|gs| &gs.reflection.bindings)
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
    let geometry_set = gs.as_ref().map(|gs| {
        assert!(gs.reflection.helpers.is_empty());
        b.create_binding_set(&geometry_entries).unwrap()
    });
    // Stream output: buffers filled with a pattern, so bytes not written compare too.
    let usage = BufferUsage::STORAGE | BufferUsage::COPY_SRC;
    let targets: Vec<_> = STREAM_SIZES
        .iter()
        .map(|&size| buffer(b, &vec![0xcd; size as usize], usage))
        .collect();
    let mut start = [0u32; 8];
    start[..2].copy_from_slice(&STREAM_START);
    let start: Vec<u8> = start.iter().flat_map(|w| w.to_le_bytes()).collect();
    let filled = buffer(b, &start, usage);
    let whole = |buffer, size| BufferRange {
        buffer,
        offset: 0,
        size,
    };
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
                geometry_sets: [geometry_set.as_ref(), None, None, None],
                index: index_buffer.as_ref().map(|buffer| StageIndex {
                    buffer,
                    size: indices.len() as u64,
                    offset: 0,
                    format: IndexFormat::U16,
                }),
                topology: case.topology,
                range,
                stream_output: stream.map(|_| StreamTargets {
                    buffers: [
                        Some(whole(&targets[0], STREAM_SIZES[0])),
                        Some(whole(&targets[1], STREAM_SIZES[1])),
                        None,
                        None,
                    ],
                    filled: whole(&filled, 32),
                }),
            },
        )
        .unwrap();
    if stream.is_some() {
        let mut read = |buffer, size: u64| {
            let mut out = vec![0; size as usize];
            b.read_back_now(
                &ReadSource::Buffer {
                    buffer,
                    offset: 0,
                    size,
                },
                &mut out,
            )
            .unwrap();
            out
        };
        let streamed = targets
            .iter()
            .zip(STREAM_SIZES)
            .map(|(t, size)| read(t, size))
            .collect();
        let words = read(&filled, 32)
            .chunks(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect();
        assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
        return Made::Streamed(streamed, words);
    }
    let last = last.expect("primitives");

    // The draw after the stages: the helper's vertex program with the pixel program.
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
    Made::Picture(out)
}

/// Real Direct3D 11's stream output of `case` (`gs` empty: from the vertex shader) under
/// [`declaration`], nothing drawn: the buffers' bytes, and the primitives written and needed.
fn direct3d_streamed(
    gpu: &Gpu,
    case: &Case,
    vertices: &[f32],
    indices: &[u8],
) -> (Vec<Vec<u8>>, [u64; 2]) {
    let vs = bytecode("vs_geometry.dxbc");
    let elements = [
        d3d11::element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
        d3d11::element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 12, 0),
    ];
    gpu.program(&vs, &bytecode("ps_geometry.dxbc"), &elements)
        .unwrap();
    let code = match case.gs {
        "" => vs.clone(),
        gs => bytecode(&format!("{gs}.dxbc")),
    };
    let so = declaration(case.gs);
    let names: Vec<std::ffi::CString> = so
        .entries
        .iter()
        .map(|e| std::ffi::CString::new(e.semantic.clone().unwrap_or_default()).unwrap())
        .collect();
    let entries: Vec<D3D11_SO_DECLARATION_ENTRY> = so
        .entries
        .iter()
        .zip(&names)
        .map(|(e, name)| D3D11_SO_DECLARATION_ENTRY {
            Stream: e.stream.into(),
            SemanticName: match e.semantic {
                Some(_) => windows::core::PCSTR(name.as_ptr().cast()),
                None => windows::core::PCSTR::null(),
            },
            SemanticIndex: e.semantic_index,
            StartComponent: e.start_component,
            ComponentCount: e.component_count,
            OutputSlot: e.slot,
        })
        .collect();
    let geometry: ID3D11GeometryShader = d3d11::made(|o| unsafe {
        gpu.device.CreateGeometryShaderWithStreamOutput(
            &code,
            Some(&entries),
            Some(&so.strides[..2]),
            D3D11_SO_NO_RASTERIZED_STREAM,
            None::<&ID3D11ClassLinkage>,
            Some(o),
        )
    })
    .unwrap();
    let _buffer = gpu.vertices(0, bytes_of(vertices), STRIDE).unwrap();
    let constants = gpu
        .immutable(&constants(), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let targets: Vec<ID3D11Buffer> = STREAM_SIZES
        .iter()
        .map(|&size| {
            gpu.buffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: size as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_STREAM_OUTPUT.0 as u32,
                    ..Default::default()
                },
                Some(&vec![0xcd; size as usize]),
            )
            .unwrap()
        })
        .collect();
    let query: ID3D11Query = d3d11::made(|q| unsafe {
        gpu.device.CreateQuery(
            &D3D11_QUERY_DESC {
                Query: D3D11_QUERY_SO_STATISTICS_STREAM0,
                MiscFlags: 0,
            },
            Some(q),
        )
    })
    .unwrap();
    let index_buffer = case
        .indexed
        .map(|_| gpu.immutable(indices, D3D11_BIND_INDEX_BUFFER).unwrap());
    let mut statistics = D3D11_QUERY_DATA_SO_STATISTICS::default();
    unsafe {
        let c = &gpu.context;
        c.GSSetShader(&geometry, None);
        c.GSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.IASetPrimitiveTopology(d3d_topology(case.topology));
        let buffers = [Some(targets[0].clone()), Some(targets[1].clone())];
        c.SOSetTargets(2, Some(buffers.as_ptr()), Some(STREAM_START.as_ptr()));
        c.Begin(&query);
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
        c.End(&query);
        loop {
            let hr = (windows::core::Interface::vtable(c).GetData)(
                windows::core::Interface::as_raw(c),
                windows::core::Interface::as_raw(&query),
                (&mut statistics as *mut D3D11_QUERY_DATA_SO_STATISTICS).cast(),
                std::mem::size_of::<D3D11_QUERY_DATA_SO_STATISTICS>() as u32,
                0,
            );
            if hr == windows::Win32::Foundation::S_OK {
                break;
            }
            hr.ok().unwrap();
        }
        c.SOSetTargets(0, None, None);
        c.GSSetShader(None, None);
    }
    let bytes = targets
        .iter()
        .map(|t| gpu.read_buffer(t).unwrap())
        .collect();
    (
        bytes,
        [
            statistics.NumPrimitivesWritten,
            statistics.PrimitivesStorageNeeded,
        ],
    )
}

/// Stream output cases: from geometry shaders (strips cut, points, lines, instances) and from the
/// vertex shader alone (`gs` empty), some filling the first buffer.
const STREAMED: [Case; 9] = [
    Case {
        vs: "vs_geometry",
        gs: "gs_unplaced",
        topology: InputTopology::PointList,
        count: 30,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_cut",
        topology: InputTopology::TriangleList,
        count: 18,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_cut",
        topology: InputTopology::TriangleStrip,
        count: 9,
        first: 4,
        instances: 2,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_lines",
        topology: InputTopology::LineStrip,
        count: 7,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_points",
        topology: InputTopology::PointList,
        count: 30,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "",
        topology: InputTopology::TriangleStrip,
        count: 10,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "",
        topology: InputTopology::TriangleList,
        count: 12,
        first: 0,
        instances: 2,
        first_instance: 1,
        indexed: Some(3),
    },
    Case {
        vs: "vs_geometry",
        gs: "",
        topology: InputTopology::LineList,
        count: 8,
        first: 1,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "",
        topology: InputTopology::PointList,
        count: 20,
        first: 0,
        instances: 2,
        first_instance: 0,
        indexed: None,
    },
];

#[test]
fn stream_output_matches_direct3d() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
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
        for case in &STREAMED {
            let indices = indices(seed, case);
            let name = format!(
                "seed {seed} {} {:?} {} from {} x{}{}",
                if case.gs.is_empty() {
                    "vertex"
                } else {
                    case.gs
                },
                case.topology,
                case.count,
                case.first,
                case.instances,
                case.indexed
                    .map(|b| format!(" indexed, base {b}"))
                    .unwrap_or_default()
            );
            let (reference, statistics) = direct3d_streamed(&gpu, case, &vertices, &indices);
            let Made::Streamed(streamed, words) = ours(
                &mut backend,
                case,
                &vertices,
                &indices,
                Some(&declaration(case.gs)),
            ) else {
                unreachable!()
            };
            compared += 1;
            let apart: Vec<usize> = reference
                .iter()
                .zip(&streamed)
                .map(|(a, b)| a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count())
                .collect();
            let counts = [u64::from(words[4]), u64::from(words[5])];
            eprintln!(
                "{name}: direct3d wrote {} of {} primitives, ours {} of {}; words apart per buffer {apart:?}",
                statistics[0], statistics[1], counts[0], counts[1]
            );
            for (b, (a, o)) in reference.iter().zip(&streamed).enumerate() {
                for (w, (x, y)) in a.chunks(4).zip(o.chunks(4)).enumerate() {
                    if x != y {
                        let x = u32::from_le_bytes(x.try_into().unwrap());
                        let y = u32::from_le_bytes(y.try_into().unwrap());
                        eprintln!(
                            "  buffer {b} word {w}: direct3d {x:#010x} ({}), ours {y:#010x} ({})",
                            f32::from_bits(x),
                            f32::from_bits(y)
                        );
                    }
                }
            }
            if apart.iter().any(|&n| n > 0) || counts != statistics || statistics[0] == 0 {
                failures.push(name);
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
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
    // Primitives each cull case's planes drop over the seeds: none would leave culling unshown.
    let mut dropped = [0u32; CULLED.len()];
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        let vertices = vertices(seed);
        for (i, case) in CASES.iter().chain(&CULLED).enumerate() {
            let indices = indices(seed, case);
            let shader = if case.gs.is_empty() { case.vs } else { case.gs };
            let name = format!(
                "seed {seed} {shader} {:?} {} from {} x{}{}",
                case.topology,
                case.count,
                case.first,
                case.instances,
                case.indexed
                    .map(|b| format!(" indexed, base {b}"))
                    .unwrap_or_default()
            );
            if let Some(total) = i.checked_sub(CASES.len()).map(|c| &mut dropped[c]) {
                let planes = dropped_by_planes(case, &vertices, &indices);
                eprintln!("{name}: the planes drop {planes} primitives");
                *total += planes;
            }
            let reference = direct3d(&gpu, case, &vertices, &indices);
            // Texels drawn: those unlike the corner, which no case draws.
            let drawn = reference
                .chunks(4)
                .filter(|t| *t != &reference[..4])
                .count();
            let Made::Picture(picture) = ours(&mut backend, case, &vertices, &indices, None) else {
                unreachable!()
            };
            let (count, largest) = differ(&reference, &picture, Format::Rgba8Unorm);
            // A line clipped in the pixel program keeps the pixels whose centres the distance
            // keeps, where Direct3D clips the line and rasterises what is left by its diamond
            // rule: a line's clipped end may differ by a pixel (Known gaps). Those texels are
            // listed apart; any other difference counts.
            let (ends, largest) = if case.vs == "vs_cull" && lines(case.topology) {
                let mut ends = 0;
                let mut largest = 0;
                for (i, (a, b)) in reference.chunks(4).zip(picture.chunks(4)).enumerate() {
                    let apart = a
                        .iter()
                        .zip(b)
                        .map(|(x, y)| u64::from(x.abs_diff(*y)))
                        .max()
                        .unwrap();
                    if apart == 0 {
                        continue;
                    }
                    if near_clip_plane(case, i as u32 % SIZE, i as u32 / SIZE) {
                        ends += 1;
                    } else {
                        largest = largest.max(apart);
                    }
                }
                (ends, largest)
            } else {
                (0, largest)
            };
            // Clipped in the pixel program, attributes are interpolated over the whole triangle,
            // which rounds apart from Direct3D's over the clipped one by a step or two; a pixel
            // drawn on one side only differs by far more.
            let allowed = if case.gs == "gs_clip" || case.vs == "vs_cull" {
                2
            } else {
                0
            };
            compared += 1;
            eprintln!(
                "{name}: {drawn} texels drawn by direct3d, {count} differ ({ends} at a clipped \
                 line end; largest of the others {largest}, allowed {allowed})"
            );
            if std::env::var_os("WCONTAINERS_SHADERS_OUT").is_some() {
                keep(
                    &format!("geometry-{seed}-{i}-{shader}-d3d"),
                    &reference,
                    Format::Rgba8Unorm,
                );
                keep(
                    &format!("geometry-{seed}-{i}-{shader}-ours"),
                    &picture,
                    Format::Rgba8Unorm,
                );
            }
            if drawn == 0 || largest > allowed {
                // The first texels apart, where and what each drew, to find the cause.
                let apart = reference
                    .chunks(4)
                    .zip(picture.chunks(4))
                    .enumerate()
                    .filter(|(_, (a, b))| a != b)
                    .take(16);
                for (i, (a, b)) in apart {
                    let (x, y) = (i as u32 % SIZE, i as u32 / SIZE);
                    eprintln!("  ({x}, {y}): direct3d {a:?}, ours {b:?}");
                }
                failures.push(name);
            }
        }
    }
    for (case, total) in CULLED.iter().zip(dropped) {
        if total == 0 {
            failures.push(format!(
                "{} {:?}: no primitive dropped",
                case.vs, case.topology
            ));
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A geometry shader writing its layer and viewport (`gs_layers`: the full-screen triangle into each
/// of four layers, layer n through viewport n % 2, with `ps_layer` reading both): real Direct3D 11
/// draws it once; ours runs the stages once and draws the final draw once per layer and viewport, as
/// a front splitting it does (`Stages::selects`), each run with its layer and viewport in the
/// pass-through's and the pixel program's helper constants. Every texel of every layer equal.
#[test]
fn layered_geometry_draws_as_direct3d_11_does() {
    use wcontainers_gpu::{
        DeviceStatus, LoadOp, TextureDesc, TextureDimension, TextureViewDesc, ViewDimension,
    };
    const LAYERS: u32 = 4;
    let viewports = [
        [0.0f32, 0.0, SIZE as f32, SIZE as f32],
        [8.0, 4.0, 40.0, 48.0],
    ];
    let (vs_code, ps_code, gs_code) = (
        bytecode("vs_fullscreen.dxbc"),
        bytecode("ps_layer.dxbc"),
        bytecode("gs_layers.dxbc"),
    );
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    gpu.program(&vs_code, &ps_code, &[]).unwrap();
    let geometry = gpu.gs(&gs_code).unwrap();
    let mut desc = d3d11::tex2d(
        SIZE,
        SIZE,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_BIND_RENDER_TARGET,
    );
    desc.ArraySize = LAYERS;
    let target = gpu.texture2d(&desc, &[]).unwrap();
    let rtv = gpu
        .rtv(&windows::core::Interface::cast(&target).unwrap(), None)
        .unwrap();
    let rects: Vec<D3D11_VIEWPORT> = viewports
        .iter()
        .map(|v| D3D11_VIEWPORT {
            TopLeftX: v[0],
            TopLeftY: v[1],
            Width: v[2],
            Height: v[3],
            MinDepth: 0.0,
            MaxDepth: 1.0,
        })
        .collect();
    unsafe {
        let c = &gpu.context;
        c.GSSetShader(&geometry, None);
        c.ClearRenderTargetView(&rtv, &CLEAR);
        c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        c.RSSetViewports(Some(&rects));
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.Draw(3, 0);
        c.GSSetShader(None, None);
    }

    let mut b = webgpu();
    let plain = VariantKey::default();
    let ps = convert(Source::Dxbc(&ps_code), &plain, None).unwrap();
    let gs = convert(Source::Dxbc(&gs_code), &plain, None).unwrap();
    let vs = convert(Source::Dxbc(&vs_code), &plain, None).unwrap();
    for c in [&vs, &gs] {
        assert!(
            c.reflection.bindings.is_empty(),
            "{:?}",
            c.reflection.bindings
        );
        assert!(
            c.reflection.helpers.is_empty(),
            "{:?}",
            c.reflection.helpers
        );
    }
    let mut stages = Stages::new(&mut b, &vs, Some(&gs), None).unwrap();
    assert!(stages.selects());
    let last = stages
        .run(
            &mut b,
            &StageDraw {
                vertex_sets: [None; 4],
                geometry_sets: [None; 4],
                index: None,
                topology: InputTopology::TriangleList,
                range: DrawRange::Vertices {
                    first: 0,
                    count: 3,
                    first_instance: 0,
                    instances: 1,
                },
                stream_output: None,
            },
        )
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
    let texture = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: SIZE,
            height: SIZE,
            depth_or_layers: LAYERS,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    for layer in 0..LAYERS {
        let view = b
            .create_view(
                &texture,
                &TextureViewDesc {
                    format: Format::Rgba8Unorm,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: layer,
                    layer_count: 1,
                    stencil: false,
                },
            )
            .unwrap();
        for (viewport, v) in viewports.iter().enumerate() {
            // The run's layer and viewport: the pass-through's helper constants, then the pixel
            // program's.
            let mut run = [0u8; 16];
            run[..4].copy_from_slice(&layer.to_le_bytes());
            run[4..8].copy_from_slice(&(viewport as u32).to_le_bytes());
            let mut pixel = vec![0u8; ps.reflection.helper_size.max(16) as usize];
            for h in &ps.reflection.helpers {
                let value = match h.value {
                    HelperValue::Layer => layer,
                    HelperValue::Viewport => viewport as u32,
                    _ => panic!("{h:?}"),
                };
                pixel[h.offset as usize..][..4].copy_from_slice(&value.to_le_bytes());
            }
            let run_buffer = buffer(&mut b, &run, BufferUsage::UNIFORM);
            let pixel_buffer = buffer(&mut b, &pixel, BufferUsage::UNIFORM);
            let set = b
                .create_binding_set(&[
                    BindingEntry {
                        binding: helper_binding(ShaderStage::Vertex),
                        stages: StageSet::VERTEX,
                        resource: BindingResource::UniformBuffer(BufferRange {
                            buffer: &run_buffer,
                            offset: 0,
                            size: 16,
                        }),
                    },
                    BindingEntry {
                        binding: helper_binding(ShaderStage::Fragment),
                        stages: StageSet::FRAGMENT,
                        resource: BindingResource::UniformBuffer(BufferRange {
                            buffer: &pixel_buffer,
                            offset: 0,
                            size: pixel.len() as u64,
                        }),
                    },
                ])
                .unwrap();
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
                load: match viewport {
                    0 => LoadOp::Clear(CLEAR.map(f32::to_bits)),
                    _ => LoadOp::Load,
                },
                store: StoreOp::Store,
            });
            b.begin_pass(&pass);
            b.draw(&DrawCall {
                program: &program,
                blend: &blend,
                depth_stencil: &depth_stencil,
                raster: &raster,
                vertex_layout: &vertex_layout,
                bindings: [None, None, None, Some(&set)],
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
                    x: v[0].to_bits(),
                    y: v[1].to_bits(),
                    width: v[2].to_bits(),
                    height: v[3].to_bits(),
                    min_depth: 0.0f32.to_bits(),
                    max_depth: 1.0f32.to_bits(),
                }],
                scissors: &[],
                blend_constant: [0; 4],
                stencil_reference: [0; 2],
                immediates: &[],
                range: DrawRange::Indexed {
                    first: 0,
                    count: last.count,
                    base_vertex: 0,
                    first_instance: 0,
                    instances: 1,
                },
            });
            b.end_pass();
        }
    }
    let mut failed = Vec::new();
    for layer in 0..LAYERS {
        let reference = gpu.read(&target, 0, layer).unwrap().to_rgba8().into_raw();
        let mut made = vec![0; (SIZE * SIZE * 4) as usize];
        b.read_back_now(
            &ReadSource::Texture {
                region: TextureRegion {
                    texture: &texture,
                    mip: 0,
                    layer,
                    origin: Origin::default(),
                    stencil: false,
                },
                extent: Extent {
                    width: SIZE,
                    height: SIZE,
                    depth_or_layers: 1,
                },
            },
            &mut made,
        )
        .unwrap();
        keep(
            &format!("layered-{layer}-direct3d"),
            &reference,
            Format::Rgba8Unorm,
        );
        keep(&format!("layered-{layer}-ours"), &made, Format::Rgba8Unorm);
        let apart = reference.iter().zip(&made).filter(|(a, b)| a != b).count();
        let drawn = reference
            .chunks(4)
            .filter(|t| *t != &reference[..4])
            .count();
        eprintln!("layer {layer}: {drawn} texels drawn by direct3d, {apart} bytes apart");
        if apart > 0 {
            failed.push(layer);
        }
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert!(failed.is_empty(), "layers {failed:?} differ");
}
