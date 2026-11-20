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
    RenderTargetBlend, ShaderModule, ShaderStage, SlotKind, StageSet, StepMode, StoreOp,
    SystemValue, TextureRegion, TextureUsage, VariantFlags, VariantKey, VertexAttribute,
    VertexBinding, VertexFormat, VertexLayoutDesc, VertexStream, Viewport, helper_binding,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::stage::{
    FinalDraw, GeometryReplay, InputTopology, PASSTHROUGH_HELPERS, PASSTHROUGH_LAYER_COUNT,
    ReplayDraw, STREAM_BUFFERS, STREAM_FILLED_WORDS, STREAMS, StageDraw, StageError, StageIndex,
    Stages, StreamOutput, StreamOutputEntry, StreamTargets,
};
use wcontainers_shaders::{Converted, Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

/// Vertices per seed, each of `COLOURED_VERTEX_BYTES`.
const VERTICES: usize = 64;

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

const CASES: [Case; 20] = [
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
    Case {
        vs: "vs_geometry",
        gs: "gs_cut",
        topology: InputTopology::TriangleStrip,
        count: 14,
        first: 2,
        instances: 2,
        first_instance: 1,
        indexed: Some(3),
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_lines",
        topology: InputTopology::LineStrip,
        count: 11,
        first: 1,
        instances: 1,
        first_instance: 0,
        indexed: Some(0),
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_line_adjacent",
        topology: InputTopology::LineStripAdjacency,
        count: 20,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: Some(2),
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_adjacent",
        topology: InputTopology::TriangleStripAdjacency,
        count: 17,
        first: 1,
        instances: 2,
        first_instance: 1,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_adjacent",
        topology: InputTopology::TriangleStripAdjacency,
        count: 24,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: Some(2),
    },
];

/// Vertex shaders writing cull distances, drawn without a geometry shader: the stage helper's cull
/// route.
const CULLED: [Case; 8] = [
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
    Case {
        vs: "vs_cull",
        gs: "",
        topology: InputTopology::TriangleStrip,
        count: 16,
        first: 1,
        instances: 2,
        first_instance: 0,
        indexed: Some(1),
    },
    Case {
        vs: "vs_cull",
        gs: "",
        topology: InputTopology::LineStrip,
        count: 12,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: Some(0),
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
    let floats = COLOURED_VERTEX_BYTES as usize / std::mem::size_of::<f32>();
    let below = |places: &[u32], plane: fn(f32, f32) -> f32| {
        places.iter().all(|&k| {
            let v = vertex(k) * floats;
            plane(vertices[v], vertices[v + 1]) < 0.0
        })
    };
    primitive_places(case, n, indices)
        .iter()
        .filter(|p| below(p, |x, _| x + 0.3) || below(p, |_, y| 0.2 - y))
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
/// them: the stage helper reads the buffer as whole words, as fronts hold it. A strip's have restart
/// indices, one in the middle of the draw and others at seeded places.
fn indices(seed: u64, case: &Case) -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x5eed);
    let base = case.indexed.unwrap_or(0);
    let low = (-base).max(0) as u16;
    let high = (VERTICES as i32 - base.max(0)) as u16;
    let middle = case.first + case.count / 2;
    (0..(case.first + case.count).next_multiple_of(2))
        .flat_map(|i| {
            let index = rng.u16(low..high);
            let restart = strip(case.topology) && (i == middle || rng.u32(..RESTART_ONE_IN) == 0);
            if restart { RESTART } else { index }.to_le_bytes()
        })
        .collect()
}

/// One strip index in this many is a restart, besides the one in the middle.
const RESTART_ONE_IN: u32 = 8;

fn strip(t: InputTopology) -> bool {
    matches!(
        t,
        InputTopology::LineStrip
            | InputTopology::TriangleStrip
            | InputTopology::LineStripAdjacency
            | InputTopology::TriangleStripAdjacency
    )
}

/// The draw's primitives, each as the places of its vertices among the draw's: a strip's restart
/// indices end it, as Direct3D's do.
fn primitive_places(case: &Case, n: u32, indices: &[u8]) -> Vec<Vec<u32>> {
    let index = |i: u32| {
        let at = std::mem::size_of::<u16>() * (case.first + i) as usize;
        u16::from_le_bytes([indices[at], indices[at + 1]])
    };
    if !strip(case.topology) {
        return (0..case.count / n)
            .map(|p| (p * n..p * n + n).collect())
            .collect();
    }
    (0..(case.count + 1).saturating_sub(n))
        .filter(|&i| case.indexed.is_none() || (i..i + n).all(|k| index(k) != RESTART))
        .map(|i| (i..i + n).collect())
        .collect()
}

/// The geometry shader's constants: the quad and offset size, then the tint.
fn constants() -> Vec<u8> {
    [0.08f32, 0.06, 0.0, 0.0, 0.9, 0.8, 1.0, 1.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

/// A depth range drawn with depth clamped instead of clipped, into a Depth32Float buffer cleared
/// to 1 and written by every fragment, whose texels the draw returns instead of the target's.
type Clamped = Option<[f32; 2]>;

fn direct3d(gpu: &Gpu, case: &Case, vertices: &[f32], indices: &[u8], clamped: Clamped) -> Vec<u8> {
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
    let _buffer = gpu
        .vertices(0, bytes_of(vertices), COLOURED_VERTEX_BYTES)
        .unwrap();
    let constants = gpu
        .immutable(&constants(), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let mut raster = d3d11::solid();
    raster.DepthClipEnable = clamped.is_none().into();
    let raster = gpu.rasterizer(&raster).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    let depth = clamped.map(|_| {
        let (texture, view) = gpu.depth(SIZE, SIZE, DXGI_FORMAT_D32_FLOAT).unwrap();
        let state = gpu
            .depth_stencil(&D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D11_COMPARISON_ALWAYS,
                ..Default::default()
            })
            .unwrap();
        (texture, view, state)
    });
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
        c.OMSetRenderTargets(Some(&[Some(rtv)]), depth.as_ref().map(|d| &d.1));
        gpu.viewport(SIZE, SIZE);
        if let (Some((_, view, state)), Some([near, far])) = (&depth, clamped) {
            c.ClearDepthStencilView(view, D3D11_CLEAR_DEPTH.0, 1.0, 0);
            c.OMSetDepthStencilState(state, 0);
            c.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: SIZE as f32,
                Height: SIZE as f32,
                MinDepth: near,
                MaxDepth: far,
            }]));
        }
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
        c.OMSetDepthStencilState(None, 0);
    }
    match &depth {
        Some((texture, _, _)) => read(gpu, texture, Format::Depth32Float),
        None => read(gpu, &target, Format::Rgba8Unorm),
    }
}

/// What the stage helper made: the picture, or the bound stream output buffers' bytes and the words
/// of filled sizes and counts.
enum Made {
    Picture(Vec<u8>),
    Streamed(Vec<Vec<u8>>, Vec<u32>),
}

/// The stream output buffers bound: each one's size, and the filled size it starts from.
struct Bound {
    sizes: [Option<u64>; STREAM_BUFFERS],
    start: [u32; STREAM_BUFFERS],
}

/// Two buffers, the first small enough to fill in some cases, the second (128 of its 32 bytes)
/// appended to.
const TWO_BUFFERS: Bound = Bound {
    sizes: [Some(32 * 3 * 9), Some(32 * 128), None, None],
    start: [0, 24, 0, 0],
};

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

/// The case's vertex, geometry and pixel programs, linked as a front links them: the vertex shader
/// under `vs_key`, the geometry shader with `gs_flags` too.
fn linked(
    case: &Case,
    vs_key: VariantKey,
    gs_flags: VariantFlags,
    clamped: Clamped,
) -> (Converted, Option<Converted>, Converted) {
    // Linked as a front links them: the pixel program first, the geometry program's outputs written
    // as it reads them.
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
        flags: gs_flags,
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
    // Depth clamped in the programs as on a back end without depth clamping, linked the same way.
    let clamp = match clamped {
        Some(_) => VariantFlags::DEPTH_CLAMP,
        None => VariantFlags::empty(),
    };
    let clip = match clips {
        0 => VariantFlags::empty(),
        _ => VariantFlags::CLIP_DISTANCE,
    };
    let (vs, gs, ps) = if (clip | clamp).is_empty() {
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
            flags: base.flags | clip | clamp,
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
    (vs, gs, ps)
}

fn ours<B: Backend>(
    b: &mut B,
    case: &Case,
    vertices: &[f32],
    indices: &[u8],
    stream: Option<(&StreamOutput, &Bound)>,
    clamped: Clamped,
) -> Made {
    let (stream, bound) = (stream.map(|s| s.0), stream.map(|s| s.1));
    let vs_key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        draw_base: DrawBaseSource::Helpers,
        ..Default::default()
    };
    let (vs, gs, ps) = linked(case, vs_key, VariantFlags::empty(), clamped);
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
            HelperValue::PulledStream { input } => [
                if input == 0 { 0 } else { POSITION_BYTES },
                COLOURED_VERTEX_BYTES,
                0,
                0,
            ],
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
    let bound = bound.unwrap_or(&TWO_BUFFERS);
    let targets: Vec<_> = bound
        .sizes
        .iter()
        .map(|size| size.map(|size| (buffer(b, &vec![0xcd; size as usize], usage), size)))
        .collect();
    let mut start = [0u32; STREAM_FILLED_WORDS];
    start[..STREAM_BUFFERS].copy_from_slice(&bound.start);
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
                    buffers: std::array::from_fn(|i| {
                        targets[i].as_ref().map(|(t, size)| whole(t, *size))
                    }),
                    filled: whole(&filled, start.len() as u64),
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
            .flatten()
            .map(|(t, size)| read(t, *size))
            .collect();
        let words = read(&filled, start.len() as u64)
            .chunks(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect();
        assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
        return Made::Streamed(streamed, words);
    }
    let last = last.expect("primitives");
    let picture = picture(
        b,
        stages.passthrough().unwrap(),
        &stages.vertex_layout().unwrap(),
        &ps,
        &last,
        clamped,
    );
    Made::Picture(picture)
}

/// The draw after the stages, `passthrough` with the pixel program, into a cleared target: its texels.
fn picture<B: Backend>(
    b: &mut B,
    passthrough: &ShaderModule,
    layout: &VertexLayoutDesc,
    ps: &Converted,
    last: &FinalDraw<B>,
    clamped: Clamped,
) -> Vec<u8> {
    let vertex = b.create_shader(passthrough).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap();
    let vertex_layout = b.create_vertex_layout(layout).unwrap();
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
            depth_compare: clamped.map(|_| wcontainers_gpu::CompareFn::Always),
            depth_write: clamped.is_some(),
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
    let depth = clamped.map(|_| {
        texture(
            b,
            Format::Depth32Float,
            SIZE,
            TextureUsage::RENDER | TextureUsage::COPY_SRC,
        )
    });
    // The pixel program's depth range, where it clamps.
    let mut helpers = vec![0u8; ps.reflection.helper_size as usize];
    for h in &ps.reflection.helpers {
        assert_eq!(h.value, HelperValue::DepthRange, "{h:?}");
        let range = clamped.expect("a range");
        let at = h.offset as usize;
        for (i, f) in range.iter().enumerate() {
            let at = at + std::mem::size_of::<f32>() * i;
            helpers[at..at + std::mem::size_of::<f32>()].copy_from_slice(&f.to_le_bytes());
        }
    }
    let helper_buffer = (!helpers.is_empty()).then(|| buffer(b, &helpers, BufferUsage::UNIFORM));
    let helper_set = helper_buffer.as_ref().map(|helper_buffer| {
        b.create_binding_set(&[BindingEntry {
            binding: helper_binding(ShaderStage::Fragment),
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: helper_buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap()
    });
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: depth
            .as_ref()
            .map(|(_, view)| wcontainers_gpu::DepthAttachment {
                view,
                depth: Some(wcontainers_gpu::AspectOps {
                    load: wcontainers_gpu::LoadOp::Clear(1.0f32.to_bits()),
                    store: StoreOp::Store,
                    read_only: false,
                }),
                stencil: None,
            }),
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
    let [near, far] = clamped.unwrap_or([0.0, 1.0]);
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
        bindings: [None, None, None, helper_set.as_ref()],
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
            min_depth: near.to_bits(),
            max_depth: far.to_bits(),
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
                texture: depth.as_ref().map_or(&target, |d| &d.0),
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

/// Whether the replay route takes `case`: it refuses strips with adjacency by name.
fn replayed(case: &Case) -> bool {
    !matches!(
        case.topology,
        InputTopology::LineStripAdjacency | InputTopology::TriangleStripAdjacency
    )
}

/// `case` through the replay route (`GeometryReplay`), as a front on a back end without compute
/// draws it: the vertex shader's inputs as drawn, its draw bases from its helper block. With
/// `stream`, what stream output wrote and the filled block; without, the picture.
fn ours_replayed<B: Backend>(
    b: &mut B,
    case: &Case,
    vertices: &[f32],
    indices: &[u8],
    stream: Option<(&StreamOutput, &Bound)>,
) -> Result<Made, StageError> {
    let vs_key = VariantKey {
        draw_base: DrawBaseSource::Helpers,
        ..Default::default()
    };
    let (vs, gs, ps) = linked(case, vs_key, VariantFlags::GEOMETRY_REPLAY, None);
    let gs = gs.expect("a geometry shader");
    let mut replay = GeometryReplay::new(b, &vs, &gs, stream.map(|s| s.0))?;
    let vertex_buffer = buffer(b, bytes_of(vertices), BufferUsage::VERTEX);
    let mut layout = VertexLayoutDesc {
        streams: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
        attributes: [None; wcontainers_gpu::MAX_VERTEX_STREAMS],
    };
    layout.streams[0] = Some(VertexStream {
        stride: COLOURED_VERTEX_BYTES,
        step: StepMode::Vertex,
    });
    for (location, offset, format) in [
        (0, 0, VertexFormat::Float32x3),
        (1, POSITION_BYTES, VertexFormat::Float32x4),
    ] {
        layout.attributes[location as usize] = Some(VertexAttribute {
            location,
            stream: 0,
            offset,
            format,
        });
    }
    let vertex_layout = b.create_vertex_layout(&layout).unwrap();
    let mut helpers = vec![0u8; vs.reflection.helper_size as usize];
    let base = case.indexed.unwrap_or(case.first as i32);
    for h in &vs.reflection.helpers {
        assert!(matches!(h.value, HelperValue::DrawBase), "{h:?}");
        let at = h.offset as usize;
        for (i, w) in [base as u32, case.first_instance].iter().enumerate() {
            helpers[at + 4 * i..at + 4 * i + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
    let helper_buffer = buffer(b, &helpers, BufferUsage::UNIFORM);
    let vertex_helpers = b
        .create_binding_set(&[BindingEntry {
            binding: helper_binding(ShaderStage::Vertex),
            stages: StageSet::VERTEX,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &helper_buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap();
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
                stages: StageSet::VERTEX,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &constant_buffer,
                    offset: 0,
                    size: constants.len() as u64,
                }),
            }
        })
        .collect();
    assert!(gs.reflection.helpers.is_empty());
    let geometry_set = b.create_binding_set(&geometry_entries).unwrap();
    // The route reads an indexed strip's indices back.
    let index_usage = BufferUsage::INDEX | BufferUsage::COPY_SRC;
    let index_buffer = case.indexed.map(|_| buffer(b, indices, index_usage));
    // Stream output: buffers filled with a pattern, so bytes not written compare too.
    let usage = BufferUsage::COPY_SRC | BufferUsage::COPY_DST;
    let bound = stream.map_or(&TWO_BUFFERS, |s| s.1);
    let targets: Vec<_> = bound
        .sizes
        .iter()
        .map(|size| size.map(|size| (buffer(b, &vec![0xcd; size as usize], usage), size)))
        .collect();
    let mut start = [0u32; STREAM_FILLED_WORDS];
    start[..STREAM_BUFFERS].copy_from_slice(&bound.start);
    let start: Vec<u8> = start.iter().flat_map(|w| w.to_le_bytes()).collect();
    let filled = buffer(b, &start, usage);
    let whole = |buffer, size| BufferRange {
        buffer,
        offset: 0,
        size,
    };
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
    let last = replay
        .run(
            b,
            ReplayDraw {
                vertex_sets: [None, None, None, Some(&vertex_helpers)],
                vertex_layout: &vertex_layout,
                vertex_buffers: &[Some(VertexBinding {
                    buffer: &vertex_buffer,
                    offset: 0,
                })],
                index: index_buffer.as_ref().map(|buffer| IndexBinding {
                    buffer,
                    offset: 0,
                    format: IndexFormat::U16,
                }),
                immediates: &[],
                geometry_sets: [Some(&geometry_set), None, None, None],
                topology: case.topology,
                range,
                stream_output: stream.map(|_| StreamTargets {
                    buffers: std::array::from_fn(|i| {
                        targets[i].as_ref().map(|(t, size)| whole(t, *size))
                    }),
                    filled: whole(&filled, start.len() as u64),
                }),
            },
        )?
        .expect("primitives");
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
            .flatten()
            .map(|(t, size)| read(t, *size))
            .collect();
        let words = read(&filled, start.len() as u64)
            .chunks(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect();
        replay.destroy(b);
        return Ok(Made::Streamed(streamed, words));
    }
    let out = picture(
        b,
        replay.passthrough(),
        &replay.vertex_layout(),
        &ps,
        &last,
        None,
    );
    replay.destroy(b);
    Ok(Made::Picture(out))
}

/// Real Direct3D 11's stream output of `case` (`gs` empty: from the vertex shader) under `so` into
/// the `bound` buffers, nothing drawn: the bound buffers' bytes, and each of the first `streams`
/// streams' primitives written and needed.
fn direct3d_streamed(
    gpu: &Gpu,
    case: &Case,
    vertices: &[f32],
    indices: &[u8],
    (so, bound): (&StreamOutput, &Bound),
    streams: usize,
) -> (Vec<Vec<u8>>, Vec<[u64; 2]>) {
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
            Some(
                &so.strides[..so
                    .strides
                    .iter()
                    .rposition(|&s| s != 0)
                    .map_or(0, |i| i + 1)],
            ),
            D3D11_SO_NO_RASTERIZED_STREAM,
            None::<&ID3D11ClassLinkage>,
            Some(o),
        )
    })
    .unwrap();
    let _buffer = gpu
        .vertices(0, bytes_of(vertices), COLOURED_VERTEX_BYTES)
        .unwrap();
    let constants = gpu
        .immutable(&constants(), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let targets: Vec<Option<ID3D11Buffer>> = bound
        .sizes
        .iter()
        .map(|size| {
            size.map(|size| {
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
        })
        .collect();
    let stream_queries = [
        D3D11_QUERY_SO_STATISTICS_STREAM0,
        D3D11_QUERY_SO_STATISTICS_STREAM1,
        D3D11_QUERY_SO_STATISTICS_STREAM2,
        D3D11_QUERY_SO_STATISTICS_STREAM3,
    ];
    let queries: Vec<ID3D11Query> = stream_queries[..streams]
        .iter()
        .map(|&query| {
            d3d11::made(|q| unsafe {
                gpu.device.CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: query,
                        MiscFlags: 0,
                    },
                    Some(q),
                )
            })
            .unwrap()
        })
        .collect();
    let index_buffer = case
        .indexed
        .map(|_| gpu.immutable(indices, D3D11_BIND_INDEX_BUFFER).unwrap());
    let mut statistics = vec![[0u64; 2]; streams];
    unsafe {
        let c = &gpu.context;
        c.GSSetShader(&geometry, None);
        c.GSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.IASetPrimitiveTopology(d3d_topology(case.topology));
        c.SOSetTargets(
            STREAM_BUFFERS as u32,
            Some(targets.as_ptr()),
            Some(bound.start.as_ptr()),
        );
        for query in &queries {
            c.Begin(query);
        }
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
        for (query, counts) in queries.iter().zip(&mut statistics) {
            c.End(query);
            let mut data = D3D11_QUERY_DATA_SO_STATISTICS::default();
            loop {
                let hr = (windows::core::Interface::vtable(c).GetData)(
                    windows::core::Interface::as_raw(c),
                    windows::core::Interface::as_raw(query),
                    (&mut data as *mut D3D11_QUERY_DATA_SO_STATISTICS).cast(),
                    std::mem::size_of::<D3D11_QUERY_DATA_SO_STATISTICS>() as u32,
                    0,
                );
                if hr == windows::Win32::Foundation::S_OK {
                    break;
                }
                hr.ok().unwrap();
            }
            *counts = [data.NumPrimitivesWritten, data.PrimitivesStorageNeeded];
        }
        c.SOSetTargets(0, None, None);
        c.GSSetShader(None, None);
    }
    let bytes = targets
        .iter()
        .flatten()
        .map(|t| gpu.read_buffer(t).unwrap())
        .collect();
    (bytes, statistics)
}

/// Stream output cases: from geometry shaders (strips cut, points, lines, instances) and from the
/// vertex shader alone (`gs` empty), some filling the first buffer.
const STREAMED: [Case; 14] = [
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
    Case {
        vs: "vs_geometry",
        gs: "",
        topology: InputTopology::TriangleStrip,
        count: 13,
        first: 1,
        instances: 2,
        first_instance: 0,
        indexed: Some(2),
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_cut",
        topology: InputTopology::TriangleStrip,
        count: 12,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: Some(1),
    },
    Case {
        vs: "vs_geometry",
        gs: "",
        topology: InputTopology::LineStrip,
        count: 10,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: Some(0),
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_adjacent",
        topology: InputTopology::TriangleStripAdjacency,
        count: 16,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_adjacent",
        topology: InputTopology::TriangleStripAdjacency,
        count: 22,
        first: 0,
        instances: 2,
        first_instance: 0,
        indexed: Some(1),
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
            let so = declaration(case.gs);
            let (reference, statistics) =
                direct3d_streamed(&gpu, case, &vertices, &indices, (&so, &TWO_BUFFERS), 1);
            let statistics = statistics[0];
            let Made::Streamed(streamed, words) = ours(
                &mut backend,
                case,
                &vertices,
                &indices,
                Some((&so, &TWO_BUFFERS)),
                None,
            ) else {
                unreachable!()
            };
            compared += 1;
            if !streamed_alike(&name, &reference, statistics, &streamed, &words) {
                failures.push(name);
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Whether ours wrote what Direct3D wrote (`reference`, its statistics written and made), every word
/// of every buffer and both counts; prints the counts and each word apart.
fn streamed_alike(
    name: &str,
    reference: &[Vec<u8>],
    statistics: [u64; 2],
    streamed: &[Vec<u8>],
    words: &[u32],
) -> bool {
    let apart: Vec<usize> = reference
        .iter()
        .zip(streamed)
        .map(|(a, b)| a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count())
        .collect();
    let counts = [
        u64::from(words[STREAM_BUFFERS]),
        u64::from(words[STREAM_BUFFERS + 1]),
    ];
    eprintln!(
        "{name}: direct3d wrote {} of {} primitives, ours {} of {}; words apart per buffer {apart:?}",
        statistics[0], statistics[1], counts[0], counts[1]
    );
    for (b, (a, o)) in reference.iter().zip(streamed).enumerate() {
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
    apart.iter().all(|&n| n == 0) && counts == statistics && statistics[0] != 0
}

/// Stream output after a geometry shader through the replay route on the WebGL 2 back end (ANGLE
/// on the same GPU), against real Direct3D 11: the geometry cases of
/// [`stream_output_matches_direct3d`]; those the route refuses must be refused by name.
#[test]
fn stream_output_after_replay_matches_direct3d() {
    let mut backend = webgl();
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
        for case in STREAMED.iter().filter(|c| !c.gs.is_empty()) {
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
            let so = declaration(case.gs);
            let made = ours_replayed(
                &mut backend,
                case,
                &vertices,
                &indices,
                Some((&so, &TWO_BUFFERS)),
            );
            if !replayed(case) {
                match made {
                    Err(StageError::Unsupported(reason)) => eprintln!("{name}: refused, {reason}"),
                    _ => failures.push(format!("{name}: not refused by name")),
                }
                continue;
            }
            let Made::Streamed(streamed, words) = made.unwrap() else {
                unreachable!()
            };
            let (reference, statistics) =
                direct3d_streamed(&gpu, case, &vertices, &indices, (&so, &TWO_BUFFERS), 1);
            compared += 1;
            if !streamed_alike(&name, &reference, statistics[0], &streamed, &words) {
                failures.push(name);
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The four-stream test's declaration: stream 0's position and colour's last three components into
/// buffer 0, stream 1's numbers into buffer 1, stream 2's colour's first two, a gap and its numbers'
/// last three into buffer 2, stream 3's place into buffer 3.
fn streams_declaration() -> StreamOutput {
    let entry =
        |stream, semantic: Option<&str>, semantic_index, start, count, slot| StreamOutputEntry {
            stream,
            semantic: semantic.map(String::from),
            semantic_index,
            start_component: start,
            component_count: count,
            slot,
        };
    StreamOutput {
        entries: vec![
            entry(0, Some("SV_Position"), 0, 0, 4, 0),
            entry(0, Some("COLOR"), 0, 1, 3, 0),
            entry(1, Some("TEXCOORD"), 0, 0, 4, 1),
            entry(2, Some("COLOR"), 0, 0, 2, 2),
            entry(2, None, 0, 0, 1, 2),
            entry(2, Some("TEXCOORD"), 0, 1, 3, 2),
            entry(3, Some("TEXCOORD"), 1, 0, 2, 3),
        ],
        strides: [28, 16, 24, 8],
    }
}

/// The four-stream test's buffers: every one bound, the first and third small enough to fill and
/// the second and fourth appended to; then the last unbound, so its stream writes nothing.
const STREAMS_BOUND: [Bound; 2] = [
    Bound {
        sizes: [Some(28 * 20), Some(16 * 64), Some(24 * 8), Some(8 * 80)],
        start: [0, 16, 0, 8],
    },
    Bound {
        sizes: [Some(28 * 40), Some(16 * 64), Some(24 * 30), None],
        start: [28, 0, 0, 0],
    },
];

/// The four-stream test's draws: points, once from the first vertex, once from another with two
/// instances.
const STREAMS_CASES: [Case; 2] = [
    Case {
        vs: "vs_geometry",
        gs: "gs_streams",
        topology: InputTopology::PointList,
        count: 30,
        first: 0,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        vs: "vs_geometry",
        gs: "gs_streams",
        topology: InputTopology::PointList,
        count: 24,
        first: 5,
        instances: 2,
        first_instance: 1,
        indexed: None,
    },
];

#[test]
fn streams_past_the_first_match_direct3d() {
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
    let so = streams_declaration();
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        let vertices = vertices(seed);
        for case in &STREAMS_CASES {
            for (b, bound) in STREAMS_BOUND.iter().enumerate() {
                let indices = indices(seed, case);
                let name = format!(
                    "seed {seed} {} from {} x{}, buffers {b}",
                    case.count, case.first, case.instances
                );
                let (reference, statistics) =
                    direct3d_streamed(&gpu, case, &vertices, &indices, (&so, bound), STREAMS);
                let Made::Streamed(streamed, words) = ours(
                    &mut backend,
                    case,
                    &vertices,
                    &indices,
                    Some((&so, bound)),
                    None,
                ) else {
                    unreachable!()
                };
                compared += 1;
                let counts: Vec<[u64; 2]> = (0..STREAMS)
                    .map(|s| {
                        let at = STREAM_BUFFERS + 2 * s;
                        [u64::from(words[at]), u64::from(words[at + 1])]
                    })
                    .collect();
                let apart: Vec<usize> = reference
                    .iter()
                    .zip(&streamed)
                    .map(|(a, b)| a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count())
                    .collect();
                eprintln!(
                    "{name}: direct3d written and made per stream {statistics:?}, ours {counts:?}; \
                     words apart per buffer {apart:?}"
                );
                for (b, (a, o)) in reference.iter().zip(&streamed).enumerate() {
                    for (w, (x, y)) in a.chunks(4).zip(o.chunks(4)).enumerate() {
                        if x != y {
                            let x = u32::from_le_bytes(x.try_into().unwrap());
                            let y = u32::from_le_bytes(y.try_into().unwrap());
                            eprintln!("  buffer {b} word {w}: direct3d {x:#010x}, ours {y:#010x}");
                        }
                    }
                }
                // A stream whose buffers are all unbound: the RTX 3080 counts some of its
                // primitives as written (25 of 54, 0 of 84, the same on every seed and on a new
                // device), by no rule found, so only those it made are compared. Ours counts none
                // written, as Wine's test_so_statistics_query has Windows do with nothing bound.
                let unbound = |stream: usize| {
                    so.entries
                        .iter()
                        .filter(|e| usize::from(e.stream) == stream)
                        .all(|e| bound.sizes[usize::from(e.slot)].is_none())
                };
                let agree = (0..STREAMS).all(|s| match unbound(s) {
                    true => counts[s][1] == statistics[s][1] && counts[s][0] == 0,
                    false => counts[s] == statistics[s],
                });
                let wrote_all = statistics.iter().all(|s| s[1] > 0);
                if apart.iter().any(|&n| n > 0) || !agree || !wrote_all {
                    failures.push(name);
                }
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
            let reference = direct3d(&gpu, case, &vertices, &indices, None);
            // Texels drawn: those unlike the corner, which no case draws.
            let drawn = reference
                .chunks(4)
                .filter(|t| *t != &reference[..4])
                .count();
            let Made::Picture(picture) = ours(&mut backend, case, &vertices, &indices, None, None)
            else {
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

/// Units in the last place two interpolations of one value may round apart by.
const INTERPOLATION_ULPS: f32 = 4.0;

/// Depth clamped behind a geometry shader, in the stage helper's pass-through and the pixel program
/// (as on a back end without depth clamping), against real Direct3D 11 with `DepthClipEnable` false:
/// points made quads and triangles copied (`gs_quads`, `gs_cut`) at seeded depths reaching past both
/// ends of the clip volume, a depth range of 0.25 to 0.75, depth buffers compared as f32 steps.
#[test]
fn depth_clamped_behind_a_geometry_shader_draws_as_direct3d_does() {
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
    let range = [0.25f32, 0.75];
    let deepest = 2.0f32;
    // The pixel program's depth is an interpolated value, the rasteriser's a plane of its own: they
    // may round apart by a few units in the last place of the value, mapped by the depth range.
    let allowed = INTERPOLATION_ULPS * f32::EPSILON * deepest * (range[1] - range[0]);
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        let mut vertices = vertices(seed);
        let mut rng = fastrand::Rng::with_seed(seed ^ 0xdee9);
        for v in vertices.chunks_mut(COLOURED_VERTEX_BYTES as usize / std::mem::size_of::<f32>()) {
            v[2] = rng.f32() * (deepest + 1.0) - 1.0;
        }
        for case in CASES
            .iter()
            .filter(|c| matches!(c.gs, "gs_quads" | "gs_cut"))
        {
            let indices = indices(seed, case);
            let name = format!("seed {seed} {} {:?} depth clamped", case.gs, case.topology);
            let reference = direct3d(&gpu, case, &vertices, &indices, Some(range));
            let Made::Picture(picture) =
                ours(&mut backend, case, &vertices, &indices, None, Some(range))
            else {
                unreachable!()
            };
            let depths = |t: &[u8]| -> Vec<f32> {
                t.chunks(std::mem::size_of::<f32>())
                    .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                    .collect()
            };
            let (theirs, mine) = (depths(&reference), depths(&picture));
            // Texels Direct3D clamped to either end: none would leave the clamp unshown.
            let at_ends = theirs.iter().filter(|z| range.contains(z)).count();
            let largest = theirs
                .iter()
                .zip(&mine)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            let (count, steps) = differ(&reference, &picture, Format::Depth32Float);
            compared += 1;
            eprintln!(
                "{name}: {at_ends} texels clamped by direct3d, {count} differ, largest {largest:e} \
                 ({steps} steps; allowed {allowed:e})"
            );
            if at_ends == 0 || largest > allowed {
                failures.push(name);
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The geometry shaders of [`geometry_shaders_draw_as_direct3d_does`] through the replay route on the
/// WebGL 2 back end (ANGLE on the same GPU), against real Direct3D 11: the same seeds, cases and
/// allowances, but for the topologies the route refuses by name.
#[test]
fn geometry_shaders_replayed_on_webgl_draw_as_direct3d_does() {
    let mut backend = webgl();
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
        for (i, case) in CASES.iter().enumerate() {
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
            let indices = indices(seed, case);
            let made = ours_replayed(&mut backend, case, &vertices, &indices, None);
            if !replayed(case) {
                match made {
                    Err(StageError::Unsupported(reason)) => eprintln!("{name}: refused, {reason}"),
                    _ => failures.push(format!("{name}: not refused by name")),
                }
                continue;
            }
            let reference = direct3d(&gpu, case, &vertices, &indices, None);
            let drawn = reference
                .chunks(4)
                .filter(|t| *t != &reference[..4])
                .count();
            let Made::Picture(picture) = made.unwrap() else {
                unreachable!()
            };
            let (count, largest) = differ(&reference, &picture, Format::Rgba8Unorm);
            // As on the compute route: clipped in the pixel program, attributes round apart by a step
            // or two.
            let allowed = if case.gs == "gs_clip" { 2 } else { 0 };
            compared += 1;
            eprintln!(
                "{name}: {drawn} texels drawn by direct3d, {count} differ (largest {largest}, \
                 allowed {allowed})"
            );
            if std::env::var_os("WCONTAINERS_SHADERS_OUT").is_some() {
                keep(
                    &format!("replay-{seed}-{i}-{}-d3d", case.gs),
                    &reference,
                    Format::Rgba8Unorm,
                );
                keep(
                    &format!("replay-{seed}-{i}-{}-ours", case.gs),
                    &picture,
                    Format::Rgba8Unorm,
                );
            }
            if drawn == 0 || largest > allowed {
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
    layered_geometry(GS_LAYERS_NAMED);
}

/// As [`layered_geometry_draws_as_direct3d_11_does`] into views of half the layers `gs_layers` names:
/// Direct3D draws the primitives naming a layer past the views' into the first layer, its pixel
/// shader reading the layer named; ours keeps them in layer 0's run (`LAYER_FROM_STAGE`).
#[test]
fn layers_past_the_views_draw_into_the_first_as_direct3d_11_does() {
    layered_geometry(GS_LAYERS_NAMED / 2);
}

/// The layers `gs_layers` names, one per instance.
const GS_LAYERS_NAMED: u32 = 4;

fn layered_geometry(layers: u32) {
    use wcontainers_gpu::{
        DeviceStatus, LoadOp, TextureDesc, TextureDimension, TextureViewDesc, VariantFlags,
        ViewDimension,
    };
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
    desc.ArraySize = layers;
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
    let vs = convert(Source::Dxbc(&vs_code), &plain, None).unwrap();
    // The named layer passes one past every location the geometry and pixel programs list.
    let listed = |c: &wcontainers_shaders::Converted, outputs: bool| {
        let values = if outputs {
            &c.reflection.outputs
        } else {
            &c.reflection.inputs
        };
        values
            .iter()
            .filter(|v| v.system.is_none())
            .map(|v| v.location + 1)
            .max()
            .unwrap_or(0)
    };
    let mut keyed = plain.clone();
    keyed.flags = VariantFlags::LAYER_FROM_STAGE;
    keyed.linked.location = listed(
        &convert(Source::Dxbc(&ps_code), &plain, None).unwrap(),
        false,
    )
    .max(listed(
        &convert(Source::Dxbc(&gs_code), &plain, None).unwrap(),
        true,
    ));
    let ps = convert(Source::Dxbc(&ps_code), &keyed, None).unwrap();
    let gs = convert(Source::Dxbc(&gs_code), &keyed, None).unwrap();
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
            depth_or_layers: layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    for layer in 0..layers {
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
            let values = [layer, viewport as u32, layers];
            let slots = PASSTHROUGH_HELPERS.iter().chain([&PASSTHROUGH_LAYER_COUNT]);
            for (slot, value) in slots.zip(values) {
                run[slot.offset as usize..][..4].copy_from_slice(&value.to_le_bytes());
            }
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
    for layer in 0..layers {
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
            &format!("layered-{layers}-{layer}-direct3d"),
            &reference,
            Format::Rgba8Unorm,
        );
        keep(
            &format!("layered-{layers}-{layer}-ours"),
            &made,
            Format::Rgba8Unorm,
        );
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
