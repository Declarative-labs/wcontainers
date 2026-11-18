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
/// in buffer 0 (stride 32), then a gap and the first two numbers in buffer 1 (stride 12).
fn declaration() -> StreamOutput {
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
            entry(Some("SV_Position"), 0, 4, 0),
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
    let vs = convert(Source::Dxbc(&bytecode("vs_geometry.dxbc")), &vs_key, None).unwrap();
    let mut stages = Stages::new(b, &vs, gs.as_ref(), stream).unwrap();

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
    let so = declaration();
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
const STREAMED: [Case; 8] = [
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
        gs: "",
        topology: InputTopology::TriangleStrip,
        count: 10,
        first: 2,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
        gs: "",
        topology: InputTopology::TriangleList,
        count: 12,
        first: 0,
        instances: 2,
        first_instance: 1,
        indexed: Some(3),
    },
    Case {
        gs: "",
        topology: InputTopology::LineList,
        count: 8,
        first: 1,
        instances: 1,
        first_instance: 0,
        indexed: None,
    },
    Case {
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
    let so = declaration();
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
            let Made::Streamed(streamed, words) =
                ours(&mut backend, case, &vertices, &indices, Some(&so))
            else {
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
            let Made::Picture(picture) = ours(&mut backend, case, &vertices, &indices, None) else {
                unreachable!()
            };
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
