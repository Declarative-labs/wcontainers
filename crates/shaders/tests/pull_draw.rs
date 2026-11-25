//! Pulled vertex inputs (`VariantKey::pulled`) drawn through the WebGPU back end against real Direct3D
//! 11 fetching the same bytes with its input assembler: thirteen formats, the defaults of the
//! components a format lacks, per-vertex streams and per-instance ones of step rates 3, 1 and 0 from a
//! first instance of 4, a stream bound at an offset, and reads past each stream's end. One point per
//! vertex and instance carries one input's bits to its pixel of an RGBA32_UINT target.
//!
//! Also drawn with each stream moved to an offset and stride that are not multiples of four, as
//! OpenGL allows, against Direct3D 11 reading the same elements where it requires them.
//!
//! The streams' bytes are seeded (`WCONTAINERS_PULL_SEED`, default 1; `WCONTAINERS_PULL_CASES` draws
//! that many seeds from it, default 1). Windows only, and needs a GPU: run as a GPU job
//! (docs/status-graphics-shaders.md).

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    Gpu, BindingEntry, BindingResource, BlendDesc, BufferRange, BufferUsage, ColorAttachment,
    ColorWrites, CullMode, DepthStencilDesc, DrawCall, DrawRange, Extent, FillMode, Format,
    FrontFace, HelperValue, Interpolation, Origin, PassDesc, Program, ProvokingVertex, RasterDesc,
    ReadSource, RenderTargetBlend, StageSet, StoreOp, TextureRegion, TextureUsage, Topology,
    VariantKey, VertexFormat, VertexLayoutDesc, Viewport,
};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_POINTLIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{PCSTR, s};

const POINTS: u32 = 64;
const FIRST_INSTANCE: u32 = 4;

/// One input: semantic, stream, offset in the element, Direct3D's format and ours.
struct Input {
    semantic: PCSTR,
    stream: usize,
    offset: u32,
    dxgi: DXGI_FORMAT,
    format: VertexFormat,
}

/// One stream: stride, elements, the offset it is bound at, and its step rate (`None` per vertex).
struct Stream {
    stride: u32,
    elements: u32,
    offset: u32,
    rate: Option<u32>,
}

const STREAMS: [Stream; 4] = [
    Stream {
        stride: 48,
        elements: 56,
        offset: 0,
        rate: None,
    },
    Stream {
        stride: 4,
        elements: 16,
        offset: 0,
        rate: Some(3),
    },
    Stream {
        stride: 4,
        elements: 60,
        offset: 8,
        rate: Some(1),
    },
    Stream {
        stride: 4,
        elements: 8,
        offset: 0,
        rate: Some(0),
    },
];

/// In the order of `pull.hlsl`'s inputs, so input n is at location n on both routes.
fn inputs() -> Vec<Input> {
    use VertexFormat as V;
    let i = |semantic, stream, offset, dxgi, format| Input {
        semantic,
        stream,
        offset,
        dxgi,
        format,
    };
    vec![
        i(s!("A"), 0, 0, DXGI_FORMAT_R8G8B8A8_UNORM, V::Unorm8x4),
        i(s!("B"), 0, 4, DXGI_FORMAT_R16G16_SINT, V::Sint16x2),
        i(
            s!("C"),
            0,
            8,
            DXGI_FORMAT_R10G10B10A2_UNORM,
            V::Unorm10_10_10_2,
        ),
        i(
            s!("D"),
            0,
            12,
            DXGI_FORMAT_R11G11B10_FLOAT,
            V::Float11_11_10,
        ),
        i(s!("E"), 0, 16, DXGI_FORMAT_R16G16B16A16_FLOAT, V::Float16x4),
        i(s!("F"), 0, 24, DXGI_FORMAT_R8G8_SNORM, V::Snorm8x2),
        i(s!("G"), 0, 26, DXGI_FORMAT_R16_UNORM, V::Unorm16x1),
        i(s!("H"), 0, 28, DXGI_FORMAT_B8G8R8A8_UNORM, V::Bgra8Unorm),
        i(s!("I"), 0, 32, DXGI_FORMAT_R32G32B32_UINT, V::Uint32x3),
        i(
            s!("L"),
            0,
            44,
            DXGI_FORMAT_R10G10B10A2_UINT,
            V::Uint10_10_10_2,
        ),
        i(s!("J"), 1, 0, DXGI_FORMAT_R8G8B8A8_SINT, V::Sint8x4),
        i(s!("K"), 2, 0, DXGI_FORMAT_R16G16_SNORM, V::Snorm16x2),
        i(s!("M"), 3, 0, DXGI_FORMAT_R32_FLOAT, V::Float32x1),
    ]
}

/// Each stream's bound offset and stride in the unaligned draw: every component of the first
/// stream starts at each byte of a word for some vertex.
const UNALIGNED: [(u32, u32); 4] = [(1, 51), (2, 5), (11, 7), (3, 6)];

/// Each stream's bytes (its bound offset included), from `seed`.
fn stream_bytes(seed: u64) -> Vec<Vec<u8>> {
    let mut rng = fastrand::Rng::with_seed(seed);
    STREAMS
        .iter()
        .map(|s| {
            (0..s.offset + s.stride * s.elements)
                .map(|_| rng.u8(..))
                .collect()
        })
        .collect()
}

/// `bytes` moved to the offsets and strides of `UNALIGNED`, the bytes between elements seeded, and
/// each stream padded to whole words.
fn unaligned(bytes: &[Vec<u8>], seed: u64) -> Vec<Vec<u8>> {
    let mut rng = fastrand::Rng::with_seed(seed);
    STREAMS
        .iter()
        .zip(UNALIGNED)
        .zip(bytes)
        .map(|((s, (offset, stride)), old)| {
            let size = (offset + stride * s.elements).next_multiple_of(4);
            let mut new: Vec<u8> = (0..size).map(|_| rng.u8(..)).collect();
            for e in 0..s.elements {
                let from = (s.offset + e * s.stride) as usize;
                let to = (offset + e * stride) as usize;
                new[to..to + s.stride as usize]
                    .copy_from_slice(&old[from..from + s.stride as usize]);
            }
            new
        })
        .collect()
}

fn direct3d_11(
    d3d: &wcontainers_graphics_test::d3d11::Gpu,
    code: &[Vec<u8>; 2],
    bytes: &[Vec<u8>],
) -> Vec<u8> {
    let elements: Vec<_> = inputs()
        .iter()
        .map(|i| {
            let rate = STREAMS[i.stream].rate;
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: i.semantic,
                SemanticIndex: 0,
                Format: i.dxgi,
                InputSlot: i.stream as u32,
                AlignedByteOffset: i.offset,
                InputSlotClass: if rate.is_some() {
                    D3D11_INPUT_PER_INSTANCE_DATA
                } else {
                    D3D11_INPUT_PER_VERTEX_DATA
                },
                InstanceDataStepRate: rate.unwrap_or(0),
            }
        })
        .collect();
    d3d.program(&code[0], &code[1], &elements).unwrap();
    let buffers: Vec<_> = bytes
        .iter()
        .map(|b| Some(d3d.immutable(b, D3D11_BIND_VERTEX_BUFFER).unwrap()))
        .collect();
    let strides: Vec<u32> = STREAMS.iter().map(|s| s.stride).collect();
    let offsets: Vec<u32> = STREAMS.iter().map(|s| s.offset).collect();
    let (target, rtv) = d3d
        .target(POINTS, POINTS, DXGI_FORMAT_R32G32B32A32_UINT)
        .unwrap();
    unsafe {
        let c = &d3d.context;
        c.IASetVertexBuffers(
            0,
            buffers.len() as u32,
            Some(buffers.as_ptr()),
            Some(strides.as_ptr()),
            Some(offsets.as_ptr()),
        );
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_POINTLIST);
        c.RSSetState(
            &d3d.rasterizer(&wcontainers_graphics_test::d3d11::solid())
                .unwrap(),
        );
        c.ClearRenderTargetView(&rtv, &[0.0; 4]);
        c.OMSetRenderTargets(Some(&[Some(rtv)]), None);
        d3d.viewport(POINTS, POINTS);
        c.DrawInstanced(POINTS, POINTS, 0, FIRST_INSTANCE);
    }
    read(d3d, &target, Format::Rgba32Uint)
}

/// The helper values of each pulled input: offset, stride, divisor and bias as
/// `HelperValue::PulledStream` says, with the stream bound from offset 0 and its elements at the
/// offset and stride `layout` gives.
fn stream_words(input: &Input, layout: &[(u32, u32)]) -> [u32; 4] {
    let s = &STREAMS[input.stream];
    let (offset, stride) = layout[input.stream];
    let offset = offset + input.offset;
    match s.rate {
        None => [offset, stride, 0, 0],
        Some(0) => [offset + FIRST_INSTANCE * stride, 0, 1, 0],
        Some(n) => [offset, stride, n, (n - 1) * FIRST_INSTANCE],
    }
}

/// Each stream's offset and stride as Direct3D binds it.
fn aligned() -> Vec<(u32, u32)> {
    STREAMS.iter().map(|s| (s.offset, s.stride)).collect()
}

fn source(c: &[u8], dxil: bool) -> Source<'_> {
    if dxil {
        Source::Dxil(c)
    } else {
        Source::Dxbc(c)
    }
}

fn ours<B: Gpu>(
    b: &mut B,
    code: &[Vec<u8>; 2],
    dxil: bool,
    bytes: &[Vec<u8>],
    layout: &[(u32, u32)],
) -> Vec<u8> {
    let inputs = inputs();
    let ps = convert(source(&code[1], dxil), &VariantKey::default(), None).unwrap();
    let key = VariantKey {
        pulled: (0..).zip(inputs.iter().map(|i| i.format)).collect(),
        draw_base: wcontainers_gpu::DrawBaseSource::Helpers,
        output_interpolation: ps
            .reflection
            .inputs
            .iter()
            .filter(|v| v.system.is_none() && v.interpolation != Interpolation::Perspective)
            .map(|v| (v.location, v.interpolation))
            .collect(),
        ..Default::default()
    };
    let vs = convert(source(&code[0], dxil), &key, None).unwrap();
    let vertex = b.create_shader(&vs.modules[0]).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap();
    let buffers: Vec<_> = bytes
        .iter()
        .map(|data| buffer(b, data, BufferUsage::STORAGE))
        .collect();

    // The helper block: the draw's bases, then each input's stream.
    let mut helpers = vec![0u8; vs.reflection.helper_size as usize];
    for slot in &vs.reflection.helpers {
        let words = match slot.value {
            HelperValue::DrawBase => [0, FIRST_INSTANCE, 0, 0],
            HelperValue::PulledStream { input } => stream_words(&inputs[input as usize], layout),
            other => panic!("no value for {other:?}"),
        };
        let at = slot.offset as usize;
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let end = (at + 16).min(helpers.len());
        helpers[at..end].copy_from_slice(&bytes[..end - at]);
    }
    let helper_buffer = buffer(b, &helpers, BufferUsage::UNIFORM);
    let mut entries = Vec::new();
    for r in &vs.reflection.bindings {
        let Some(wcontainers_gpu::AddedBinding::PulledStream(location)) = r.added else {
            panic!("no case binds {r:?}");
        };
        let stream = inputs[location as usize].stream;
        entries.push(BindingEntry {
            binding: r.binding,
            stages: StageSet::VERTEX,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: &buffers[stream],
                    offset: 0,
                    size: bytes[stream].len() as u64,
                },
                writable: false,
            },
        });
    }
    assert!(ps.reflection.bindings.is_empty());
    let set = b.create_binding_set(&entries).unwrap();
    let helper_set = b
        .create_binding_set(&[BindingEntry {
            binding: wcontainers_gpu::helper_binding(wcontainers_gpu::ShaderStage::Vertex),
            stages: StageSet::VERTEX,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &helper_buffer,
                offset: 0,
                size: helpers.len() as u64,
            }),
        }])
        .unwrap();
    let vertex_layout = b
        .create_vertex_layout(&VertexLayoutDesc {
            streams: [None; 32],
            attributes: [None; 32],
        })
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
    let (target, view) = texture(
        b,
        Format::Rgba32Uint,
        POINTS,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
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
        load: wcontainers_gpu::LoadOp::Clear([0; 4]),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    let size = (POINTS as f32).to_bits();
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
        raster: &raster,
        vertex_layout: &vertex_layout,
        bindings: [Some(&set), None, None, Some(&helper_set)],
        vertex_buffers: &[],
        index: None,
        topology: Topology::PointList,
        restart: false,
        viewports: &[Viewport {
            x: 0,
            y: 0,
            width: size,
            height: size,
            min_depth: 0.0f32.to_bits(),
            max_depth: 1.0f32.to_bits(),
        }],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: POINTS,
            first_instance: FIRST_INSTANCE,
            instances: POINTS,
        },
    });
    b.end_pass();
    let mut out = vec![0; (POINTS * POINTS * 16) as usize];
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
                width: POINTS,
                height: POINTS,
                depth_or_layers: 1,
            },
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    out
}

/// Texels that differ, by input (row modulo 13); prints the first two of each input.
fn differing_by_input(a: &[u8], b: &[u8]) -> [usize; 13] {
    let mut by = [0; 13];
    let words = |t: &[u8]| -> Vec<u32> {
        t.chunks(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect()
    };
    for (n, (x, y)) in a.chunks(16).zip(b.chunks(16)).enumerate() {
        if x != y {
            let input = (n as u32 / POINTS % 13) as usize;
            if by[input] < 2 {
                println!(
                    "  input {input}, vertex {}, instance {}: {:08x?} against {:08x?}",
                    n as u32 % POINTS,
                    n as u32 / POINTS,
                    words(x),
                    words(y)
                );
            }
            by[input] += 1;
        }
    }
    by
}

#[test]
fn pulled_inputs_read_what_direct3d_fetches() {
    let seed: u64 = std::env::var("WCONTAINERS_PULL_SEED").map_or(1, |s| s.parse().unwrap());
    let cases: u64 = std::env::var("WCONTAINERS_PULL_CASES").map_or(1, |s| s.parse().unwrap());
    let mut gpu = webgpu();
    let d3d = wcontainers_graphics_test::d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let mut failed = Vec::new();
    for case in seed..seed + cases {
        let bytes = stream_bytes(case);
        let fxc = [bytecode("vs_pull.dxbc"), bytecode("ps_pull.dxbc")];
        let reference = direct3d_11(&d3d, &fxc, &bytes);
        // Past the per-vertex stream's end Direct3D reads zeros, and the defaults where the format
        // has fewer components: input 0 (RGBA8) all zero, input 6 (R16) (0, 0, 0, 1.0).
        let one = 1.0f32.to_bits();
        for (input, words) in [(0, [0; 4]), (6, [0, 0, 0, one])] {
            let expected: Vec<u8> = words.iter().flat_map(|w: &u32| w.to_le_bytes()).collect();
            let mut past_end = reference.chunks(16).enumerate().filter(|(n, _)| {
                *n as u32 % POINTS >= STREAMS[0].elements && *n as u32 / POINTS % 13 == input
            });
            assert!(
                past_end.clone().count() > 0 && past_end.all(|(_, t)| *t == expected[..]),
                "Direct3D read input {input} past the stream's end otherwise"
            );
        }
        let moved = unaligned(&bytes, case);
        for (route, dxil, place) in [
            ("dxbc", false, ""),
            ("dxil", true, ""),
            ("dxbc", false, "_unaligned"),
            ("dxil", true, "_unaligned"),
        ] {
            let code = [
                bytecode(&format!("vs_pull.{route}")),
                bytecode(&format!("ps_pull.{route}")),
            ];
            let texels = if place.is_empty() {
                ours(&mut gpu, &code, dxil, &bytes, &aligned())
            } else {
                ours(&mut gpu, &code, dxil, &moved, &UNALIGNED)
            };
            let (count, largest) = differ(&reference, &texels, Format::Rgba32Uint);
            let by_input = differing_by_input(&reference, &texels);
            println!(
                "pulled seed {case} {route}{place} webgpu: {count} of {} texels differ, largest {largest}; by input {by_input:?}",
                POINTS * POINTS
            );
            keep(
                &format!("pulled_{case}_{route}{place}.reference"),
                &reference,
                Format::Rgba32Uint,
            );
            keep(
                &format!("pulled_{case}_{route}{place}.ours"),
                &texels,
                Format::Rgba32Uint,
            );
            if count != 0 {
                failed.push(format!("seed {case} {route}{place}"));
            }
        }
    }
    assert!(failed.is_empty(), "differ from Direct3D 11: {failed:?}");
}
