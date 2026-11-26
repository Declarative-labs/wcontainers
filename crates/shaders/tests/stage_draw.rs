//! The stage helper's routes of a vertex shader alone on the WebGPU back end, against real Direct3D 11:
//! alpha-antialiased lines (and WARP drawing them), both blending each line's colour over the target
//! by its alpha, and the same lines replayed on the WebGL 2 back end (ANGLE); and the primitive's
//! number for a pixel shader reading it; aliased lines into a multisampled target by Direct3D's rule
//! (and WARP drawing them); a vertex shader writing storage, run as compute; and a
//! geometry shader reading a typed buffer. Seeded vertices: `WCONTAINERS_LINE_SEED` (default 1) and
//! `WCONTAINERS_LINE_CASES` seeds from it (default 4); every case runs per seed. An indexed list
//! Direct3D draws nothing of is passed over for the seed's next (`scene`), and ours must draw nothing
//! of it either.

#![cfg(windows)]

mod common;

use common::*;
// The trait by its old name: graphics-test's Direct3D 11 device is named `Gpu` in this file.
use wcontainers_gpu::{
    Gpu as Backend, caps::Features, draw::ColorAttachment, draw::DrawCall, draw::DrawRange,
    draw::IndexBinding, draw::PassDesc, draw::StoreOp, draw::VertexBinding, draw::Viewport,
    format::Format, reflect::DrawBaseSource, reflect::HelperValue, reflect::LinkedValues,
    reflect::SlotKind, reflect::VariantFlags, reflect::VariantKey, resource::BindingEntry,
    resource::BindingResource, resource::BufferRange, resource::BufferUsage, resource::Extent,
    resource::Origin, resource::ReadSource, resource::TextureRegion, resource::TextureUsage,
    shader::Program, shader::ShaderModule, shader::ShaderStage, shader::StageSet, slow::SlowPath,
    state::BlendComponent, state::BlendDesc, state::BlendFactor, state::BlendOp,
    state::ColorWrites, state::CullMode, state::DepthStencilDesc, state::FillMode,
    state::FrontFace, state::IndexFormat, state::ProvokingVertex, state::RasterDesc,
    state::RenderTargetBlend, state::StepMode, state::VertexAttribute, state::VertexFormat,
    state::VertexLayoutDesc, state::VertexStream,
};
use wcontainers_graphics_test::Driver;
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::stage::{
    FinalDraw, GeometryReplay, InputTopology, ReplayDraw, StageDraw, StageIndex, Stages,
};
use wcontainers_shaders::{Converted, Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Interface, s};

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
    /// `ps_line` drawing aliased lines into a multisampled target by Direct3D's rule
    /// (`LINE_DIAMOND`), unblended, through [`ALIASED_VIEWPORT`].
    Aliased,
}

/// The aliased route's target samples, and its viewport (x, y, width, height), placed away from the
/// target's corner.
const ALIASED_SAMPLES: u32 = 4;
const ALIASED_VIEWPORT: [f32; 4] = [3.0, 5.0, SIZE as f32 - 8.0, SIZE as f32 - 6.0];

/// [`ALIASED_VIEWPORT`] as the interface gives a viewport, depth from 0 to 1.
fn aliased_viewport() -> Viewport {
    let [x, y, width, height] = ALIASED_VIEWPORT.map(f32::to_bits);
    Viewport {
        x,
        y,
        width,
        height,
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }
}

impl Route {
    fn pixel_shader(self) -> &'static str {
        match self {
            Route::Lines | Route::Aliased => "ps_line",
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
        InputTopology::PatchList(n) => D3D_PRIMITIVE_TOPOLOGY(
            D3D11_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST.0 + i32::from(n) - 1,
        ),
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
    let aliased = route == Route::Aliased;
    let (target, rtv) = match aliased {
        false => gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap(),
        true => {
            let mut desc = d3d11::tex2d(
                SIZE,
                SIZE,
                DXGI_FORMAT_R8G8B8A8_UNORM,
                D3D11_BIND_RENDER_TARGET,
            );
            desc.SampleDesc.Count = ALIASED_SAMPLES;
            let texture = gpu.texture2d(&desc, &[]).unwrap();
            let view = gpu.rtv(&texture.cast().unwrap(), None).unwrap();
            (texture, view)
        }
    };
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
        if aliased {
            let [x, y, width, height] = ALIASED_VIEWPORT;
            c.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: x,
                TopLeftY: y,
                Width: width,
                Height: height,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
        }
        match &index_buffer {
            Some(ib) => {
                c.IASetIndexBuffer(ib, DXGI_FORMAT_R16_UINT, 0);
                c.DrawIndexedInstanced(case.count, case.instances, 0, 0, 0);
            }
            None => c.DrawInstanced(case.count, case.instances, 0, 0),
        }
        c.OMSetBlendState(None, None, !0);
    }
    if aliased {
        let (resolved, _) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
        unsafe {
            gpu.context
                .ResolveSubresource(&resolved, 0, &target, 0, DXGI_FORMAT_R8G8B8A8_UNORM)
        };
        return read(gpu, &resolved, Format::Rgba8Unorm);
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

/// `vs_line` converted with `vs_key` and `route`'s pixel shader linked to it as a front links them:
/// the pixel program's coverage or primitive number past every value either program passes. Returns
/// both and the pixel program's key.
fn linked(dxil: bool, route: Route, vs_key: &VariantKey) -> (Converted, Converted, VariantKey) {
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
    let plain = convert(read(&ps_code), &VariantKey::default(), None).unwrap();
    let vs = convert(read(&vs_code), vs_key, None).unwrap();
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
            Route::Aliased => VariantFlags::LINE_DIAMOND,
        },
        linked: LinkedValues {
            location,
            clip_distances: 0,
        },
        ..Default::default()
    };
    let ps = convert(read(&ps_code), &ps_key, None).unwrap();
    (vs, ps, ps_key)
}

fn ours<B: Backend>(
    b: &mut B,
    case: &Case,
    (vertices, indices): (&[f32], &[u8]),
    dxil: bool,
    blending: bool,
    route: Route,
) -> Vec<u8> {
    // Every vertex input pulled.
    let vs_key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        draw_base: DrawBaseSource::Helpers,
        ..Default::default()
    };
    let (vs, ps, ps_key) = linked(dxil, route, &vs_key);
    let mut stages = match route {
        Route::Lines | Route::Aliased => Stages::antialiased_lines(b, &vs, &ps_key).unwrap(),
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
            binding: wcontainers_gpu::shader::ShaderStage::helper_binding(ShaderStage::Compute),
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
        Route::Aliased => stages.run_lines(b, &draw, &aliased_viewport()),
    }
    .unwrap()
    .expect("primitives");
    let layout = stages.vertex_layout().unwrap();
    let out = drawn(
        b,
        stages.passthrough().unwrap(),
        &layout,
        &ps,
        &last,
        blending,
        route == Route::Aliased,
    );
    stages.destroy(b);
    out
}

/// `case` through the replay route ([`GeometryReplay::antialiased_lines`],
/// [`GeometryReplay::primitive_numbered`]), as a front on a back end without compute draws it: the
/// vertex shader's inputs as drawn, its clip depth rewritten where the back end lacks clip depth from
/// zero.
fn ours_replayed<B: Backend>(
    b: &mut B,
    case: &Case,
    (vertices, indices): (&[f32], &[u8]),
    dxil: bool,
    blending: bool,
    route: Route,
) -> Vec<u8> {
    let zero_one = b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE);
    let vs_key = VariantKey {
        draw_base: DrawBaseSource::Helpers,
        flags: match zero_one {
            true => VariantFlags::empty(),
            false => VariantFlags::CLIP_DEPTH_REWRITE,
        },
        ..Default::default()
    };
    let (vs, ps, ps_key) = linked(dxil, route, &vs_key);
    let mut replay = match route {
        Route::Lines | Route::Aliased => {
            GeometryReplay::antialiased_lines(b, &vs, &ps_key).unwrap()
        }
        Route::Primitives => GeometryReplay::primitive_numbered(b, &vs, &ps_key).unwrap(),
    };
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
    for h in &vs.reflection.helpers {
        assert!(matches!(h.value, HelperValue::DrawBase), "{h:?}");
        helpers[h.offset as usize..][..8].fill(0);
    }
    let helper_buffer = buffer(b, &helpers, BufferUsage::UNIFORM);
    let vertex_helpers = b
        .create_binding_set(&[BindingEntry {
            binding: wcontainers_gpu::shader::ShaderStage::helper_binding(ShaderStage::Vertex),
            stages: StageSet::VERTEX,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &helper_buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap();
    // The route reads an indexed strip's indices back.
    let index_buffer = case
        .indexed
        .then(|| buffer(b, indices, BufferUsage::INDEX | BufferUsage::COPY_SRC));
    let draw = ReplayDraw {
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
        geometry_sets: [None; 4],
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
        Route::Lines => replay.run_antialiased_lines(b, draw, [SIZE as f32; 2]),
        Route::Primitives => replay.run(b, draw),
        Route::Aliased => replay.run_lines(b, draw, &aliased_viewport()),
    };
    let last = last.unwrap().expect("primitives");
    let out = drawn(
        b,
        replay.passthrough(),
        &replay.vertex_layout(),
        &ps,
        &last,
        blending,
        route == Route::Aliased,
    );
    replay.destroy(b);
    out
}

/// `last` drawn through `passthrough` (its records laid out as `layout`) and the pixel program `ps`
/// into a cleared target, blended by alpha with `blending`; returns the target's texels. With
/// `aliased`, into [`ALIASED_SAMPLES`] samples through [`ALIASED_VIEWPORT`], resolved.
fn drawn<B: Backend>(
    b: &mut B,
    passthrough: &ShaderModule,
    layout: &VertexLayoutDesc,
    ps: &Converted,
    last: &FinalDraw<B>,
    blending: bool,
    aliased: bool,
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
            depth_minus_one: !b.caps().features.contains(Features::CLIP_DEPTH_ZERO_ONE),
            sample_mask: !0,
        })
        .unwrap();
    let (samples, viewport) = match aliased {
        true => (ALIASED_SAMPLES, ALIASED_VIEWPORT),
        false => (1, [0.0, 0.0, SIZE as f32, SIZE as f32]),
    };
    let (target, target_view) = texture_levels(
        b,
        Format::Rgba8Unorm,
        SIZE,
        1,
        samples,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let resolved = aliased.then(|| {
        texture(
            b,
            Format::Rgba8Unorm,
            SIZE,
            TextureUsage::RENDER | TextureUsage::COPY_SRC,
        )
    });
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &target_view,
        depth_slice: 0,
        resolve: resolved.as_ref().map(|(_, view)| view),
        load: wcontainers_gpu::draw::LoadOp::Clear(
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
            x: viewport[0].to_bits(),
            y: viewport[1].to_bits(),
            width: viewport[2].to_bits(),
            height: viewport[3].to_bits(),
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
                texture: resolved.as_ref().map_or(&target, |(t, _)| t),
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
    assert_eq!(b.status(), wcontainers_gpu::error::DeviceStatus::Ok);
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
    let (case, vertices) = measured([0.0, 0.0, SIZE as f32, SIZE as f32]);
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

/// [`MEASURED`]'s lines as a list of white vertices placed through `viewport` (x, y, width, height).
fn measured(viewport: [f32; 4]) -> (Case, Vec<f32>) {
    let [vx, vy, width, height] = viewport;
    let mut vertices = Vec::new();
    for [x0, y0, x1, y1] in MEASURED {
        for (x, y) in [(x0, y0), (x1, y1)] {
            vertices.extend([
                (x - vx) / width * 2.0 - 1.0,
                1.0 - (y - vy) / height * 2.0,
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
    (case, vertices)
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
    let mut gl = webgl();
    let mut gl_no_clip = webgl_without(&["EXT_clip_control"]);
    assert!(
        !gl_no_clip
            .caps()
            .features
            .contains(Features::CLIP_DEPTH_ZERO_ONE)
    );
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
        print_coverage("ours replayed on webgl", |case, v| {
            ours_replayed(&mut gl, case, (v, &[]), false, false, Route::Lines)
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
                        let replayed = ours_replayed(
                            &mut gl,
                            case,
                            (&vertices, indices),
                            dxil,
                            true,
                            Route::Lines,
                        );
                        let no_clip = ours_replayed(
                            &mut gl_no_clip,
                            case,
                            (&vertices, indices),
                            dxil,
                            true,
                            Route::Lines,
                        );
                        [picture, replayed, no_clip]
                            .iter()
                            .all(|p| drawn_texels(p) == 0)
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
            // 0: the compute route on WebGPU; 1: replayed on WebGL 2; 2: the same without
            // EXT_clip_control.
            for (dxil, replayed) in [false, true]
                .into_iter()
                .flat_map(|d| [(d, 0), (d, 1), (d, 2)])
            {
                let picture = match replayed {
                    0 => ours(
                        &mut backend,
                        case,
                        (&vertices, &indices),
                        dxil,
                        true,
                        Route::Lines,
                    ),
                    1 => ours_replayed(
                        &mut gl,
                        case,
                        (&vertices, &indices),
                        dxil,
                        true,
                        Route::Lines,
                    ),
                    _ => ours_replayed(
                        &mut gl_no_clip,
                        case,
                        (&vertices, &indices),
                        dxil,
                        true,
                        Route::Lines,
                    ),
                };
                let route = [
                    "",
                    " replayed on webgl",
                    " replayed on webgl without clip control",
                ][replayed];
                for (driver, reference) in &references {
                    let name = format!(
                        "{name} {}{route} against {driver:?}",
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
                            &format!("lines-{seed}-{i}-{kind}-{replayed}-ours"),
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

/// How two resolved pictures of aliased lines differ: the pixels one covers (alpha not zero) and the
/// other does not, and the largest channel difference of pixels both cover.
fn aliased_apart(a: &[u8], b: &[u8]) -> (Vec<usize>, u8) {
    let alpha = |t: &[u8]| t[3];
    let mut coverage = Vec::new();
    let mut colour = 0;
    for (i, (x, y)) in a.chunks(4).zip(b.chunks(4)).enumerate() {
        if (alpha(x) == 0) != (alpha(y) == 0) {
            coverage.push(i);
        } else if alpha(x) != 0 {
            colour = x
                .iter()
                .zip(y)
                .map(|(p, q)| p.abs_diff(*q))
                .fold(colour, u8::max);
        }
    }
    (coverage, colour)
}

/// Aliased lines (`MultisampleEnable` and `AntialiasedLineEnable` off) into four samples through a
/// viewport away from the target's corner, by the stage helper's line route with `LINE_DIAMOND`: by
/// compute on WebGPU, and replayed on WebGL 2 with and without EXT_clip_control, from the fxc and the
/// dxc build, against real Direct3D 11 on the RTX 3080 and WARP, resolved. Ours passes where its
/// covered pixels are no further from the RTX 3080's than WARP's are, and the colours of pixels both
/// cover no further apart than the two references' (at least one step): Direct3D covers every
/// sample of each pixel its rule picks, so a pixel ours covered in part would resolve towards the
/// clear colour, further than that. [`MEASURED`]'s lines (on pixel centres,
/// between rows, diagonal, ends between centres), then the seeded cases of [`CASES`]
/// (`WCONTAINERS_LINE_SEED`, `WCONTAINERS_LINE_CASES`).
#[test]
fn aliased_lines_into_a_multisampled_target_draw_as_direct3d_does() {
    let mut backend = webgpu();
    let mut gl = webgl();
    let mut gl_no_clip = webgl_without(&["EXT_clip_control"]);
    let gpus: Vec<(Driver, Gpu)> = [Driver::Hardware, Driver::Warp]
        .into_iter()
        .map(|d| {
            (
                d,
                d3d11::create(d, false, &[D3D_FEATURE_LEVEL_11_0]).unwrap(),
            )
        })
        .collect();
    let seed: u64 = std::env::var("WCONTAINERS_LINE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_LINE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut scenes = vec![(
        "measured".to_string(),
        measured(ALIASED_VIEWPORT),
        Vec::new(),
    )];
    for seed in seed..seed + seeds {
        for case in &CASES {
            let name = format!(
                "seed {seed} {:?}{}",
                case.topology,
                if case.indexed { " indexed" } else { "" }
            );
            let indices = match case.indexed {
                true => indices(seed, case, 0),
                false => Vec::new(),
            };
            let case = Case {
                topology: case.topology,
                count: case.count,
                instances: case.instances,
                indexed: case.indexed,
            };
            scenes.push((name, (case, vertices(seed)), indices));
        }
    }
    let mut failures = Vec::new();
    let mut compared = 0;
    let before = backend.slow_paths().counts[SlowPath::AliasedLinesAsQuads as usize];
    for (name, (case, vertices), indices) in &scenes {
        let drawn_from = (vertices.as_slice(), indices.as_slice());
        let references: Vec<Vec<u8>> = gpus
            .iter()
            .map(|(_, gpu)| direct3d(gpu, case, drawn_from, false, Route::Aliased))
            .collect();
        let (between, spread) = aliased_apart(&references[0], &references[1]);
        eprintln!(
            "{name}: {} pixels covered by the RTX 3080, {} covered by one of it and WARP, colours \
             at most {spread} apart",
            drawn_texels(&references[0]),
            between.len(),
        );
        for (dxil, replayed) in [false, true]
            .into_iter()
            .flat_map(|d| [(d, 0), (d, 1), (d, 2)])
        {
            let picture = match replayed {
                0 => ours(&mut backend, case, drawn_from, dxil, false, Route::Aliased),
                1 => ours_replayed(&mut gl, case, drawn_from, dxil, false, Route::Aliased),
                _ => ours_replayed(
                    &mut gl_no_clip,
                    case,
                    drawn_from,
                    dxil,
                    false,
                    Route::Aliased,
                ),
            };
            let route = [
                "",
                " replayed on webgl",
                " replayed on webgl without clip control",
            ][replayed];
            let name = format!("{name} {}{route}", if dxil { "dxil" } else { "dxbc" });
            let (coverage, colour) = aliased_apart(&references[0], &picture);
            let warp = aliased_apart(&references[1], &picture).0.len();
            compared += 1;
            eprintln!(
                "{name}: {} pixels covered by one of the RTX 3080 and ours ({warp} of WARP and \
                 ours), colours at most {colour} apart",
                coverage.len()
            );
            if std::env::var_os("WCONTAINERS_SHADERS_OUT").is_some() {
                let kind = if dxil { "dxil" } else { "dxbc" };
                let file = name.replace(' ', "-");
                keep(
                    &format!("aliased-{file}-reference"),
                    &references[0],
                    Format::Rgba8Unorm,
                );
                keep(
                    &format!("aliased-{file}-{kind}-{replayed}-ours"),
                    &picture,
                    Format::Rgba8Unorm,
                );
            }
            if coverage.len() > between.len() || colour > spread.max(1) {
                for &t in coverage.iter().take(16) {
                    let (px, py) = (t as u32 % SIZE, t as u32 / SIZE);
                    let at = t * 4;
                    eprintln!(
                        "  ({px}, {py}): direct3d {:?}, ours {:?}",
                        &references[0][at..at + 4],
                        &picture[at..at + 4]
                    );
                }
                failures.push(name);
            }
        }
    }
    assert!(backend.slow_paths().counts[SlowPath::AliasedLinesAsQuads as usize] > before);
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A pixel shader reading its primitive's number, drawn through the stage helper's primitive route
/// (`PRIMITIVE_ID_FROM_STAGE`, for adapters without the primitive index): by compute on WebGPU, and
/// replayed on WebGL 2 with and without EXT_clip_control; every texel equal to real Direct3D 11's,
/// from the fxc and the dxc build.
#[test]
fn primitive_numbers_from_the_stage_helper_draw_as_direct3d_does() {
    let mut backend = webgpu();
    let mut gl = webgl();
    let mut gl_no_clip = webgl_without(&["EXT_clip_control"]);
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
                        let replayed = [&mut gl, &mut gl_no_clip].map(|gl| {
                            ours_replayed(gl, case, drawn_from, dxil, false, Route::Primitives)
                        });
                        [&picture, &replayed[0], &replayed[1]]
                            .iter()
                            .all(|p| drawn_texels(p) == 0)
                    })
                },
            )
            .map_err(|e| failures.push(format!("{name}: {e}"))) else {
                continue;
            };
            let drawn_from = (&vertices[..], &indices[..]);
            let drawn = drawn_texels(&reference);
            // 0: the compute route on WebGPU; 1: replayed on WebGL 2; 2: the same without
            // EXT_clip_control.
            for (dxil, replayed) in [false, true]
                .into_iter()
                .flat_map(|d| [(d, 0), (d, 1), (d, 2)])
            {
                let picture = match replayed {
                    0 => ours(
                        &mut backend,
                        case,
                        drawn_from,
                        dxil,
                        false,
                        Route::Primitives,
                    ),
                    1 => ours_replayed(&mut gl, case, drawn_from, dxil, false, Route::Primitives),
                    _ => ours_replayed(
                        &mut gl_no_clip,
                        case,
                        drawn_from,
                        dxil,
                        false,
                        Route::Primitives,
                    ),
                };
                let route = [
                    "",
                    " replayed on webgl",
                    " replayed on webgl without clip control",
                ][replayed];
                let name = format!("{name} {}{route}", if dxil { "dxil" } else { "dxbc" });
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

/// Bytes of the words `vs_store_drawn` writes: two words for each of 32 vertices of two instances.
const STORED_WORD_BYTES: usize = 2 * 32 * 2 * 4;
/// Texels a side of the texture it writes at (vertex, instance), zero before the draw.
const STORED_TEXELS: u32 = SIZE;

/// What a draw of `vs_store_drawn` leaves: the target's texels, the words and the texture's texels.
struct Stored {
    picture: Vec<u8>,
    words: Vec<u8>,
    texels: Vec<u8>,
}

/// `vs_store_drawn` and `ps_line` drawn by real Direct3D 11.1 (UAVs in every stage), unblended, with
/// its buffer and texture zero.
fn direct3d_stored(gpu: &Gpu, case: &Case, (vertices, indices): (&[f32], &[u8])) -> Stored {
    let (vs, ps) = (bytecode("vs_store_drawn.dxbc"), bytecode("ps_line.dxbc"));
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
    let raster = gpu.rasterizer(&d3d11::solid()).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    let zeros = vec![0u8; STORED_WORD_BYTES];
    let words = gpu
        .buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: zeros.len() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0 as u32,
                ..Default::default()
            },
            Some(&zeros),
        )
        .unwrap();
    let words_view = gpu
        .uav(
            &words.cast().unwrap(),
            Some(&D3D11_UNORDERED_ACCESS_VIEW_DESC {
                Format: DXGI_FORMAT_R32_TYPELESS,
                ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
                Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
                    Buffer: D3D11_BUFFER_UAV {
                        FirstElement: 0,
                        NumElements: zeros.len() as u32 / 4,
                        Flags: D3D11_BUFFER_UAV_FLAG_RAW.0 as u32,
                    },
                },
            }),
        )
        .unwrap();
    let texel_zeros = vec![0u8; (4 * STORED_TEXELS * STORED_TEXELS) as usize];
    let texels = gpu
        .texture2d(
            &d3d11::tex2d(
                STORED_TEXELS,
                STORED_TEXELS,
                DXGI_FORMAT_R32_UINT,
                D3D11_BIND_UNORDERED_ACCESS,
            ),
            &[d3d11::data(&texel_zeros, 4 * STORED_TEXELS, 0)],
        )
        .unwrap();
    let texels_view = gpu.uav(&texels.cast().unwrap(), None).unwrap();
    let index_buffer = case
        .indexed
        .then(|| gpu.immutable(indices, D3D11_BIND_INDEX_BUFFER).unwrap());
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&raster);
        c.IASetPrimitiveTopology(d3d_topology(case.topology));
        c.ClearRenderTargetView(&rtv, &[0.0; 4]);
        let views = [Some(words_view), Some(texels_view)];
        c.OMSetRenderTargetsAndUnorderedAccessViews(
            Some(&[Some(rtv)]),
            None,
            1,
            views.len() as u32,
            Some(views.as_ptr()),
            None,
        );
        gpu.viewport(SIZE, SIZE);
        match &index_buffer {
            Some(ib) => {
                c.IASetIndexBuffer(ib, DXGI_FORMAT_R16_UINT, 0);
                c.DrawIndexedInstanced(case.count, case.instances, 0, 0, 0);
            }
            None => c.DrawInstanced(case.count, case.instances, 0, 0),
        }
        c.OMSetRenderTargets(None, None::<&ID3D11DepthStencilView>);
    }
    Stored {
        picture: read(gpu, &target, Format::Rgba8Unorm),
        words: gpu.read_buffer(&words).unwrap(),
        texels: read(gpu, &texels, Format::R32Uint),
    }
}

/// The same through the stage helper's storage route ([`Stages::stored`]) on the WebGPU back end, from
/// the fxc or the dxc build; asserts the route counts its slow path once.
fn ours_stored<B: Backend>(
    b: &mut B,
    case: &Case,
    (vertices, indices): (&[f32], &[u8]),
    dxil: bool,
) -> Stored {
    let kind = if dxil { "dxil" } else { "dxbc" };
    let (vs_code, ps_code) = (
        bytecode(&format!("vs_store_drawn.{kind}")),
        bytecode(&format!("ps_line.{kind}")),
    );
    let source = |c| match dxil {
        true => Source::Dxil(c),
        false => Source::Dxbc(c),
    };
    let vs_key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        draw_base: DrawBaseSource::Helpers,
        // The texture's slot: its register on the DXBC route, its rank among the registers on the
        // DXIL route.
        storage_formats: (0..=2).map(|slot| (slot, Format::R32Uint)).collect(),
        ..Default::default()
    };
    let vs = convert(source(&vs_code), &vs_key, None).unwrap();
    assert!(wcontainers_shaders::stage::writes_storage(&vs));
    let ps = convert(source(&ps_code), &VariantKey::default(), None).unwrap();
    let mut stages = Stages::stored(b, &vs, None).unwrap();

    let vertex_buffer = buffer(
        b,
        bytes_of(vertices),
        BufferUsage::VERTEX | BufferUsage::STORAGE,
    );
    let size = std::mem::size_of_val(vertices) as u64;
    let words = buffer(
        b,
        &[0; STORED_WORD_BYTES],
        BufferUsage::STORAGE | BufferUsage::COPY_SRC,
    );
    // WebGPU makes textures zero.
    let (texels, texels_view) = texture(
        b,
        Format::R32Uint,
        STORED_TEXELS,
        TextureUsage::STORAGE | TextureUsage::COPY_SRC,
    );
    let entries: Vec<_> = vs
        .reflection
        .bindings
        .iter()
        .map(|r| {
            let resource = match (r.added, r.register) {
                (Some(_), _) => BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &vertex_buffer,
                        offset: 0,
                        size,
                    },
                    writable: false,
                },
                (None, 1) => BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &words,
                        offset: 0,
                        size: STORED_WORD_BYTES as u64,
                    },
                    writable: true,
                },
                (None, _) => BindingResource::StorageTexture {
                    view: &texels_view,
                    writable: true,
                },
            };
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource,
            }
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
            binding: wcontainers_gpu::shader::ShaderStage::helper_binding(ShaderStage::Compute),
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
    let before = b.slow_paths().counts[SlowPath::VertexStorageAsCompute as usize];
    let last = stages.run(b, &draw).unwrap().expect("primitives");
    assert_eq!(
        b.slow_paths().counts[SlowPath::VertexStorageAsCompute as usize],
        before + 1
    );
    assert_eq!(
        b.slow_paths().counts[SlowPath::CullDistanceAsGeometry as usize],
        0
    );
    let layout = stages.vertex_layout().unwrap();
    let picture = drawn(
        b,
        stages.passthrough().unwrap(),
        &layout,
        &ps,
        &last,
        false,
        false,
    );
    stages.destroy(b);
    let mut stored_words = vec![0; STORED_WORD_BYTES];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &words,
            offset: 0,
            size: STORED_WORD_BYTES as u64,
        },
        &mut stored_words,
    )
    .unwrap();
    let mut stored_texels = vec![0; (4 * STORED_TEXELS * STORED_TEXELS) as usize];
    b.read_back_now(
        &ReadSource::Texture {
            region: TextureRegion {
                texture: &texels,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: STORED_TEXELS,
                height: STORED_TEXELS,
                depth_or_layers: 1,
            },
        },
        &mut stored_texels,
    )
    .unwrap();
    Stored {
        picture,
        words: stored_words,
        texels: stored_texels,
    }
}

/// The vertices a draw of `case` runs the vertex shader for by Direct3D's rules: those of its whole
/// primitives. Direct3D may run it for the others (an indexed strip's pieces too short for a
/// primitive) or not.
fn vertices_of_primitives(case: &Case, indices: &[u8]) -> Vec<bool> {
    let mut used = vec![false; VERTICES];
    if !case.indexed {
        used[..case.count as usize].fill(true);
        return used;
    }
    let least = match case.topology {
        InputTopology::LineStrip => 2,
        _ => 3,
    };
    let words: Vec<u16> = indices[..2 * case.count as usize]
        .chunks(2)
        .map(|w| u16::from_le_bytes([w[0], w[1]]))
        .collect();
    for piece in words.split(|i| *i == RESTART) {
        if piece.len() >= least {
            piece.iter().for_each(|i| used[*i as usize] = true);
        }
    }
    used
}

/// The places of `stored`'s words and texels that differ from `reference`'s where Direct3D's rules
/// fix them: every vertex of a whole primitive, of each instance drawn. Elsewhere ours may hold
/// zero (not run) or Direct3D's.
fn stored_differences(
    case: &Case,
    used: &[bool],
    reference: &Stored,
    ours: &Stored,
) -> Vec<String> {
    let word =
        |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[4 * at..][..4].try_into().unwrap());
    let mut out = Vec::new();
    for instance in 0..2usize {
        for (v, used) in used.iter().enumerate() {
            let fixed = instance < case.instances as usize && *used;
            let places = [
                (
                    "word",
                    &reference.words,
                    &ours.words,
                    2 * (instance * 32 + v),
                ),
                (
                    "word",
                    &reference.words,
                    &ours.words,
                    2 * (instance * 32 + v) + 1,
                ),
                (
                    "texel",
                    &reference.texels,
                    &ours.texels,
                    instance * STORED_TEXELS as usize + v,
                ),
            ];
            for (what, theirs, mine, at) in places {
                let (t, m) = (word(theirs, at), word(mine, at));
                if t != m && (fixed || m != 0) {
                    out.push(format!(
                        "instance {instance} vertex {v} {what} {at}: direct3d {t:#x}, ours {m:#x}"
                    ));
                }
            }
        }
    }
    out
}

/// A vertex shader writing a raw buffer and a typed texture (Direct3D 11.1's UAVs in the vertex stage),
/// drawn through the stage helper's storage route on WebGPU, which lacks
/// `Features::VERTEX_STORAGE_WRITE` (`VertexStorageAsCompute`): the words and texels each vertex of
/// a primitive writes, and the picture, equal to real Direct3D 11.1's on the RTX 3080, from the fxc
/// and the dxc build.
#[test]
fn vertex_storage_writes_through_the_stage_helper_match_direct3d() {
    let mut backend = webgpu();
    assert!(
        !backend
            .caps()
            .features
            .contains(Features::VERTEX_STORAGE_WRITE)
    );
    let gpu = d3d11::create(Driver::Hardware, false, &[D3D_FEATURE_LEVEL_11_1]).unwrap();
    eprintln!("direct3d 11.1: {}", gpu.adapter().unwrap());
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
                |indices| direct3d_stored(&gpu, case, (&vertices, indices)),
                |reference| drawn_texels(&reference.picture) != 0,
                |indices, _| {
                    [false, true].into_iter().all(|dxil| {
                        let ours = ours_stored(&mut backend, case, (&vertices, indices), dxil);
                        drawn_texels(&ours.picture) == 0
                    })
                },
            )
            .map_err(|e| failures.push(format!("{name}: {e}"))) else {
                continue;
            };
            let used = vertices_of_primitives(case, &indices);
            for dxil in [false, true] {
                let name = format!("{name} {}", if dxil { "dxil" } else { "dxbc" });
                let ours = ours_stored(&mut backend, case, (&vertices, &indices), dxil);
                let d = apart(&reference.picture, &ours.picture);
                let count = d.iter().filter(|x| **x != 0).count();
                let stored = stored_differences(case, &used, &reference, &ours);
                compared += 1;
                eprintln!(
                    "{name}: {} texels drawn by direct3d, {count} differ; {} vertices written, {} stored values differ",
                    drawn_texels(&reference.picture),
                    used.iter().filter(|u| **u).count() * case.instances as usize,
                    stored.len()
                );
                if drawn_texels(&reference.picture) == 0 || count != 0 || !stored.is_empty() {
                    for (t, _) in d.iter().enumerate().filter(|(_, x)| **x != 0).take(16) {
                        let (px, py) = (t as u32 % SIZE, t as u32 / SIZE);
                        let at = t * 4;
                        eprintln!(
                            "  ({px}, {py}): direct3d {:?}, ours {:?}",
                            &reference.picture[at..at + 4],
                            &ours.picture[at..at + 4]
                        );
                    }
                    stored.iter().take(16).for_each(|s| eprintln!("  {s}"));
                    failures.push(name);
                }
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The typed buffer `gs_typed` reads: four `R32G32B32A32_FLOAT` elements (an offset in x and y, a
/// colour scale, an alpha).
const OFFSETS: [[f32; 4]; 4] = [
    [0.03125, -0.0625, 0.5, 1.0],
    [-0.046875, 0.015625, 0.75, 0.5],
    [0.0, 0.0390625, 1.0, 0.25],
    [0.0625, 0.0, 0.25, 1.0],
];

/// The triangle cases of [`NUMBERED`].
fn triangle_cases() -> impl Iterator<Item = &'static Case> {
    NUMBERED.iter().filter(|c| {
        matches!(
            c.topology,
            InputTopology::TriangleList | InputTopology::TriangleStrip
        )
    })
}

/// `vs_line`, the named geometry shader (with [`OFFSETS`] bound for `gs_typed`) and `ps_line` drawn
/// by real Direct3D 11, unblended.
fn direct3d_geometry(
    gpu: &Gpu,
    geometry: &str,
    case: &Case,
    (vertices, indices): (&[f32], &[u8]),
) -> Vec<u8> {
    let (vs, gs, ps) = (
        bytecode("vs_line.dxbc"),
        bytecode(&format!("{geometry}.dxbc")),
        bytecode("ps_line.dxbc"),
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
    let gs = gpu.gs(&gs).unwrap();
    let _buffer = gpu
        .vertices(0, bytes_of(vertices), COLOURED_VERTEX_BYTES)
        .unwrap();
    let offsets = gpu
        .immutable(bytes_of(OFFSETS.as_flattened()), D3D11_BIND_SHADER_RESOURCE)
        .unwrap();
    let view = gpu
        .srv(
            &offsets.cast().unwrap(),
            Some(&D3D11_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                ViewDimension: D3D_SRV_DIMENSION_BUFFER,
                Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                    Buffer: D3D11_BUFFER_SRV {
                        Anonymous1: D3D11_BUFFER_SRV_0 { FirstElement: 0 },
                        Anonymous2: D3D11_BUFFER_SRV_1 {
                            NumElements: OFFSETS.len() as u32,
                        },
                    },
                },
            }),
        )
        .unwrap();
    let raster = gpu.rasterizer(&d3d11::solid()).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    let index_buffer = case
        .indexed
        .then(|| gpu.immutable(indices, D3D11_BIND_INDEX_BUFFER).unwrap());
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&raster);
        c.GSSetShader(&gs, None);
        c.GSSetShaderResources(0, Some(&[Some(view)]));
        c.IASetPrimitiveTopology(d3d_topology(case.topology));
        c.ClearRenderTargetView(&rtv, &[0.0; 4]);
        c.OMSetRenderTargets(Some(&[Some(rtv)]), None);
        gpu.viewport(SIZE, SIZE);
        match &index_buffer {
            Some(ib) => {
                c.IASetIndexBuffer(ib, DXGI_FORMAT_R16_UINT, 0);
                c.DrawIndexedInstanced(case.count, case.instances, 0, 0, 0);
            }
            None => c.DrawInstanced(case.count, case.instances, 0, 0),
        }
        c.GSSetShader(None, None);
    }
    read(gpu, &target, Format::Rgba8Unorm)
}

/// The same through the stage helper on the WebGPU back end, a typed buffer read through a storage
/// buffer (`TexelBufferRoute::Storage`), from the fxc or the dxc build.
fn ours_geometry<B: Backend>(
    b: &mut B,
    geometry: &str,
    case: &Case,
    (vertices, indices): (&[f32], &[u8]),
    dxil: bool,
) -> Vec<u8> {
    let kind = if dxil { "dxil" } else { "dxbc" };
    let code = |name: &str| bytecode(&format!("{name}.{kind}"));
    let (vs_code, gs_code, ps_code) = (code("vs_line"), code(geometry), code("ps_line"));
    let source = |c| match dxil {
        true => Source::Dxil(c),
        false => Source::Dxbc(c),
    };
    let vs_key = VariantKey {
        pulled: vec![(0, VertexFormat::Float32x3), (1, VertexFormat::Float32x4)],
        draw_base: DrawBaseSource::Helpers,
        ..Default::default()
    };
    let gs_key = VariantKey {
        texel_buffers: wcontainers_gpu::reflect::TexelBufferRoute::Storage,
        texel_formats: vec![(0, Format::Rgba32Float)],
        ..Default::default()
    };
    let vs = convert(source(&vs_code), &vs_key, None).unwrap();
    let gs = convert(source(&gs_code), &gs_key, None).unwrap();
    let ps = convert(source(&ps_code), &VariantKey::default(), None).unwrap();
    let mut stages = Stages::new(b, &vs, Some(&gs), None).unwrap();

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
            binding: wcontainers_gpu::shader::ShaderStage::helper_binding(ShaderStage::Compute),
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &helper_buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap();
    // The geometry stage's sets: the typed buffer at its slot, its view in the helper block.
    let offsets = bytes_of(OFFSETS.as_flattened());
    let offsets_buffer = buffer(b, offsets, BufferUsage::STORAGE);
    let geometry_entries: Vec<_> = gs
        .reflection
        .bindings
        .iter()
        .map(|r| {
            assert_eq!((r.kind, r.register), (SlotKind::Texture, 0), "{r:?}");
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &offsets_buffer,
                        offset: 0,
                        size: offsets.len() as u64,
                    },
                    writable: false,
                },
            }
        })
        .collect();
    let geometry_set =
        (!geometry_entries.is_empty()).then(|| b.create_binding_set(&geometry_entries).unwrap());
    let mut gs_helpers = vec![0u8; gs.reflection.helper_size as usize];
    for h in &gs.reflection.helpers {
        let HelperValue::TexelBufferView { .. } = h.value else {
            panic!("{h:?}");
        };
        let at = h.offset as usize;
        gs_helpers[at..at + 4].copy_from_slice(&0u32.to_le_bytes());
        gs_helpers[at + 4..at + 8].copy_from_slice(&(OFFSETS.len() as u32).to_le_bytes());
    }
    let gs_helper_buffer =
        (!gs_helpers.is_empty()).then(|| buffer(b, &gs_helpers, BufferUsage::UNIFORM));
    let geometry_helpers = gs_helper_buffer.as_ref().map(|helper_buffer| {
        b.create_binding_set(&[BindingEntry {
            binding: wcontainers_gpu::shader::ShaderStage::helper_binding(ShaderStage::Compute),
            stages: StageSet::COMPUTE,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: helper_buffer,
                offset: 0,
                size: gs_helpers.len() as u64,
            }),
        }])
        .unwrap()
    });
    let index_buffer = case
        .indexed
        .then(|| buffer(b, indices, BufferUsage::INDEX | BufferUsage::STORAGE));
    let draw = StageDraw {
        vertex_sets: [Some(&vertex_set), None, None, Some(&vertex_helpers)],
        geometry_sets: [geometry_set.as_ref(), None, None, geometry_helpers.as_ref()],
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
    let last = stages.run(b, &draw).unwrap().expect("primitives");
    let layout = stages.vertex_layout().unwrap();
    let picture = drawn(
        b,
        stages.passthrough().unwrap(),
        &layout,
        &ps,
        &last,
        false,
        false,
    );
    stages.destroy(b);
    picture
}

/// A geometry shader reading a typed buffer (every fifth primitive past the view's four elements),
/// run as compute by the stage helper on WebGPU, the buffer read through a storage buffer: every
/// texel equal to real Direct3D 11's on the RTX 3080, from the fxc and the dxc build.
#[test]
fn typed_buffers_in_geometry_shaders_draw_as_direct3d_does() {
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
        for case in triangle_cases() {
            let name = format!(
                "seed {seed} {:?} x{}{}",
                case.topology,
                case.instances,
                if case.indexed { " indexed" } else { "" }
            );
            let Ok((indices, reference)) = scene(
                case.indexed,
                |attempt| indices(seed, case, attempt),
                |indices| direct3d_geometry(&gpu, "gs_typed", case, (&vertices, indices)),
                |reference| drawn_texels(reference) != 0,
                |indices, _| {
                    [false, true].into_iter().all(|dxil| {
                        let data = (vertices.as_slice(), indices);
                        drawn_texels(&ours_geometry(&mut backend, "gs_typed", case, data, dxil))
                            == 0
                    })
                },
            )
            .map_err(|e| failures.push(format!("{name}: {e}"))) else {
                continue;
            };
            for dxil in [false, true] {
                let name = format!("{name} {}", if dxil { "dxil" } else { "dxbc" });
                let data = (vertices.as_slice(), indices.as_slice());
                let picture = ours_geometry(&mut backend, "gs_typed", case, data, dxil);
                let d = apart(&reference, &picture);
                let count = d.iter().filter(|x| **x != 0).count();
                let drawn = drawn_texels(&reference);
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

/// The patch cases: patches of four control points, lists and indexed, one to three instances, the
/// last two cutting a patch short (Direct3D draws no partial patch).
const PATCH_CASES: [Case; 4] = [
    Case {
        topology: InputTopology::PatchList(4),
        count: VERTICES as u32,
        instances: 1,
        indexed: false,
    },
    Case {
        topology: InputTopology::PatchList(4),
        count: 22,
        instances: 2,
        indexed: false,
    },
    Case {
        topology: InputTopology::PatchList(4),
        count: 16,
        instances: 3,
        indexed: true,
    },
    Case {
        topology: InputTopology::PatchList(4),
        count: 18,
        instances: 1,
        indexed: true,
    },
];

/// Seeded 16-bit indices of a list (no restart index: a list has none), an even number of them.
fn list_indices(seed: u64, count: u32) -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x9a7c);
    (0..count.next_multiple_of(2))
        .flat_map(|_| rng.u16(..VERTICES as u16).to_le_bytes())
        .collect()
}

/// A geometry shader taking patches of four control points, drawn without tessellation (`gs_patch`:
/// each patch a quadrilateral, its points picked at run time, its colour by the patch's number), run
/// as compute by the stage helper on WebGPU: every texel equal to real Direct3D 11's on the RTX 3080
/// drawing fxc's build, for fxc's and dxc's builds.
#[test]
fn geometry_shaders_taking_patches_draw_as_direct3d_does() {
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
        for case in &PATCH_CASES {
            let name = format!(
                "seed {seed} {:?} {} x{}{}",
                case.topology,
                case.count,
                case.instances,
                if case.indexed { " indexed" } else { "" }
            );
            let indices = match case.indexed {
                true => list_indices(seed, case.count),
                false => Vec::new(),
            };
            let data = (vertices.as_slice(), indices.as_slice());
            let reference = direct3d_geometry(&gpu, "gs_patch", case, data);
            for dxil in [false, true] {
                let name = format!("{name} {}", if dxil { "dxil" } else { "dxbc" });
                let picture = ours_geometry(&mut backend, "gs_patch", case, data, dxil);
                let d = apart(&reference, &picture);
                let count = d.iter().filter(|x| **x != 0).count();
                let drawn = drawn_texels(&reference);
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
