//! Early depth and stencil tests run in the program (`VariantFlags::EARLY_TESTS`) on the WebGPU back
//! end, against real Direct3D 11 running `[earlydepthstencil]` on the same GPU (`early.hlsl`): a
//! first draw writes depth and stencil over part of the target, a second, whose pixel shader marks
//! each pixel it runs for in a storage buffer, is tested against them. The marks and the colours must
//! be equal. Seeded: `WCONTAINERS_EARLY_SEED` (default 1) and `WCONTAINERS_EARLY_CASES` seeds from it
//! (default 32).

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    AspectOps, Backend, BindingEntry, BindingResource, BlendDesc, BufferRange, BufferUsage,
    ColorAttachment, ColorWrites, CompareFn, CullMode, DepthAttachment, DepthStencilDesc, DrawCall,
    DrawRange, Extent, FillMode, Format, FrontFace, HELPER_SET, HelperValue, LoadOp, Origin,
    PassDesc, Program, ProvokingVertex, RasterDesc, ReadSource, RenderTargetBlend, ShaderStage,
    SlotKind, StageSet, StencilFace, StencilOp, StoreOp, TextureRegion, TextureUsage,
    TextureViewDesc, Topology, VariantFlags, VariantKey, VertexLayoutDesc, ViewDimension, Viewport,
    helper_binding,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{Converted, Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

const SIZE: u32 = 64;

/// One scene: the depth and stencil cleared, the first draw's corners (one depth) and stencil value,
/// the second draw's corners and tests (none: off; stencil front and back), reference and read mask.
#[derive(Clone, Debug)]
struct Case {
    clear_depth: f32,
    clear_stencil: u8,
    occluder: [[f32; 4]; 3],
    occluder_stencil: u8,
    tested: [[f32; 4]; 3],
    depth: Option<CompareFn>,
    stencil: Option<(CompareFn, CompareFn)>,
    reference: u8,
    read_mask: u8,
}

const COMPARES: [CompareFn; 8] = [
    CompareFn::Never,
    CompareFn::Less,
    CompareFn::Equal,
    CompareFn::LessEqual,
    CompareFn::Greater,
    CompareFn::NotEqual,
    CompareFn::GreaterEqual,
    CompareFn::Always,
];

fn rng(seed: u64) -> impl FnMut() -> u32 {
    let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 16) as u32
    }
}

fn case(seed: u64) -> Case {
    let mut next = rng(seed);
    let mut unit = move || (next() % 4096) as f32 / 4096.0;
    // A large triangle: one corner in each of three regions of the target and past it, in either
    // winding (so either stencil face), each corner at depth `z` or at its own.
    let triangle = |z: Option<f32>, unit: &mut dyn FnMut() -> f32| {
        let regions = [
            (-1.4, -0.2, -1.4, -0.2),
            (0.2, 1.4, -1.4, 1.4),
            (-1.4, 1.4, 0.2, 1.4),
        ];
        let mut corners = regions.map(|(x0, x1, y0, y1)| {
            let x = x0 + unit() * (x1 - x0);
            let y = y0 + unit() * (y1 - y0);
            [x, y, z.unwrap_or_else(&mut *unit), 1.0]
        });
        if unit() < 0.5 {
            corners.swap(0, 1);
        }
        corners
    };
    let z = unit();
    let occluder = triangle(Some(z), &mut unit);
    let tested = triangle(None, &mut unit);
    let pick = |unit: &mut dyn FnMut() -> f32| COMPARES[(unit() * 8.0) as usize];
    let depth = (unit() < 0.875).then(|| pick(&mut unit));
    let stencil = (unit() < 0.75).then(|| (pick(&mut unit), pick(&mut unit)));
    let byte = |unit: &mut dyn FnMut() -> f32| (unit() * 256.0) as u8;
    let clear_stencil = byte(&mut unit);
    let occluder_stencil = byte(&mut unit);
    // Often one of the stored values, so equal and not-equal tests take both answers.
    let reference = match (unit() * 4.0) as u32 {
        0 => clear_stencil,
        1 => occluder_stencil,
        _ => byte(&mut unit),
    };
    Case {
        clear_depth: unit(),
        clear_stencil,
        occluder,
        occluder_stencil,
        tested,
        depth,
        stencil,
        reference,
        read_mask: if unit() < 0.5 { 0xff } else { byte(&mut unit) },
    }
}

fn floats(v: &[[f32; 4]; 3]) -> Vec<u8> {
    v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect()
}

fn d3d(c: CompareFn) -> D3D11_COMPARISON_FUNC {
    // Direct3D's numbers are the codes CompareFn holds.
    D3D11_COMPARISON_FUNC(c as i32)
}

/// Real Direct3D 11: the marks and the target's texels.
fn direct3d(gpu: &Gpu, case: &Case) -> (Vec<u8>, Vec<u8>) {
    let vs = gpu.vs(&bytecode("vs_early.dxbc")).unwrap();
    let early = gpu.ps(&bytecode("ps_early.dxbc")).unwrap();
    let mark = gpu.ps(&bytecode("ps_mark.dxbc")).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    let (_depth, dsv) = gpu
        .depth(SIZE, SIZE, DXGI_FORMAT_D32_FLOAT_S8X24_UINT)
        .unwrap();
    let marks = gpu
        .buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: 4 * SIZE * SIZE,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32,
                StructureByteStride: 4,
                ..Default::default()
            },
            Some(&vec![0; (4 * SIZE * SIZE) as usize]),
        )
        .unwrap();
    let uav = gpu.uav(&marks.cast().unwrap(), None).unwrap();
    let corners = |c| {
        gpu.immutable(&floats(c), D3D11_BIND_CONSTANT_BUFFER)
            .unwrap()
    };
    let (occluder, tested) = (corners(&case.occluder), corners(&case.tested));
    let raster = gpu.rasterizer(&d3d11::solid()).unwrap();
    let face = |func, op| D3D11_DEPTH_STENCILOP_DESC {
        StencilFailOp: op,
        StencilDepthFailOp: op,
        StencilPassOp: op,
        StencilFunc: func,
    };
    let writing = gpu
        .depth_stencil(&D3D11_DEPTH_STENCIL_DESC {
            DepthEnable: true.into(),
            DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
            DepthFunc: D3D11_COMPARISON_ALWAYS,
            StencilEnable: true.into(),
            StencilReadMask: 0xff,
            StencilWriteMask: 0xff,
            FrontFace: face(D3D11_COMPARISON_ALWAYS, D3D11_STENCIL_OP_REPLACE),
            BackFace: face(D3D11_COMPARISON_ALWAYS, D3D11_STENCIL_OP_REPLACE),
        })
        .unwrap();
    let (front, back) = case
        .stencil
        .unwrap_or((CompareFn::Always, CompareFn::Always));
    let testing = gpu
        .depth_stencil(&D3D11_DEPTH_STENCIL_DESC {
            DepthEnable: case.depth.is_some().into(),
            DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ZERO,
            DepthFunc: d3d(case.depth.unwrap_or(CompareFn::Always)),
            StencilEnable: case.stencil.is_some().into(),
            StencilReadMask: case.read_mask,
            StencilWriteMask: 0,
            FrontFace: face(d3d(front), D3D11_STENCIL_OP_KEEP),
            BackFace: face(d3d(back), D3D11_STENCIL_OP_KEEP),
        })
        .unwrap();
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&raster);
        c.IASetInputLayout(None);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        gpu.viewport(SIZE, SIZE);
        c.ClearRenderTargetView(&rtv, &[0.0; 4]);
        c.ClearDepthStencilView(
            &dsv,
            D3D11_CLEAR_DEPTH.0 | D3D11_CLEAR_STENCIL.0,
            case.clear_depth,
            case.clear_stencil,
        );
        c.VSSetShader(&vs, None);
        c.PSSetShader(&mark, None);
        c.VSSetConstantBuffers(0, Some(&[Some(occluder)]));
        c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), Some(&dsv));
        c.OMSetDepthStencilState(&writing, case.occluder_stencil as u32);
        c.Draw(3, 0);
        c.PSSetShader(&early, None);
        c.VSSetConstantBuffers(0, Some(&[Some(tested)]));
        c.OMSetRenderTargetsAndUnorderedAccessViews(
            Some(&[Some(rtv)]),
            Some(&dsv),
            1,
            1,
            Some(&Some(uav)),
            None,
        );
        c.OMSetDepthStencilState(&testing, case.reference as u32);
        c.Draw(3, 0);
        c.OMSetRenderTargets(None, None::<&ID3D11DepthStencilView>);
        c.OMSetDepthStencilState(None, 0);
    }
    (
        gpu.read_buffer(&marks).unwrap(),
        read(gpu, &target, Format::Rgba8Unorm),
    )
}

fn converted(name: &str, dxil: bool, key: &VariantKey) -> Converted {
    let ext = if dxil { "dxil" } else { "dxbc" };
    let bytes = bytecode(&format!("{name}.{ext}"));
    let source = if dxil {
        Source::Dxil(&bytes)
    } else {
        Source::Dxbc(&bytes)
    };
    convert(source, key, None).unwrap()
}

/// The same through the WebGPU back end: the second draw's pixel program runs the tests itself,
/// with the pass's depth and stencil read-only and bound to it.
fn ours<B: Backend>(b: &mut B, case: &Case, dxil: bool) -> (Vec<u8>, Vec<u8>) {
    let vs = converted("vs_early", dxil, &VariantKey::default());
    let mark = converted("ps_mark", dxil, &VariantKey::default());
    let key = VariantKey {
        flags: VariantFlags::EARLY_TESTS,
        samples: 1,
        ..Default::default()
    };
    let early = converted("ps_early", dxil, &key);
    let vertex = b.create_shader(&vs.modules[0]).unwrap();
    let programs = [&mark, &early].map(|ps| {
        let fragment = b.create_shader(&ps.modules[0]).unwrap();
        b.create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .unwrap()
    });
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
    let face = |compare, op| StencilFace {
        compare,
        fail: op,
        depth_fail: op,
        pass: op,
    };
    let writing = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: Some(CompareFn::Always),
            depth_write: true,
            stencil: Some((
                face(CompareFn::Always, StencilOp::Replace),
                face(CompareFn::Always, StencilOp::Replace),
            )),
            stencil_read_mask: [0xff; 2],
            stencil_write_mask: [0xff; 2],
            depth_bounds: None,
        })
        .unwrap();
    let testing = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: case.depth,
            depth_write: false,
            stencil: case
                .stencil
                .map(|(f, k)| (face(f, StencilOp::Keep), face(k, StencilOp::Keep))),
            stencil_read_mask: [case.read_mask; 2],
            stencil_write_mask: [0; 2],
            depth_bounds: None,
        })
        .unwrap();

    let (target, target_view) = texture_levels(
        b,
        Format::Rgba8Unorm,
        SIZE,
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    );
    let (depth, depth_view) = texture_levels(
        b,
        Format::Depth32FloatStencil8,
        SIZE,
        1,
        1,
        TextureUsage::RENDER | TextureUsage::SAMPLE,
    );
    let stencil_view = b
        .create_view(
            &depth,
            &TextureViewDesc {
                format: Format::Depth32FloatStencil8,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: true,
            },
        )
        .unwrap();
    let marks_size = (4 * SIZE * SIZE) as u64;
    let marks = buffer(
        b,
        &vec![0; marks_size as usize],
        BufferUsage::STORAGE | BufferUsage::COPY_SRC,
    );
    let corners =
        [&case.occluder, &case.tested].map(|c| buffer(b, &floats(c), BufferUsage::UNIFORM));
    let vertex_helpers = buffer(
        b,
        &vec![0; vs.reflection.helper_size.max(16) as usize],
        BufferUsage::UNIFORM,
    );
    let mut tests = vec![0u8; early.reflection.helper_size as usize];
    for h in &early.reflection.helpers {
        assert_eq!(h.value, HelperValue::EarlyTests);
        let (front, back) = case.stencil.map_or((0, 0), |(f, k)| (f as u32, k as u32));
        let codes = [
            case.depth.map_or(0, |d| d as u32),
            front,
            back,
            case.reference as u32,
            case.read_mask as u32,
        ];
        for (i, c) in codes.iter().enumerate() {
            let at = h.offset as usize + 4 * i;
            tests[at..at + 4].copy_from_slice(&c.to_le_bytes());
        }
    }
    let tests = buffer(b, &tests, BufferUsage::UNIFORM);

    let draw = |b: &mut B, pass: &PassDesc<'_, B>, n: usize| {
        let ps = [&mark, &early][n];
        let mut entries: [Vec<BindingEntry<'_, B>>; 4] = Default::default();
        for (c, stages) in [(&vs, StageSet::VERTEX), (ps, StageSet::FRAGMENT)] {
            for r in &c.reflection.bindings {
                let range = |buffer, size| BufferRange {
                    buffer,
                    offset: 0,
                    size,
                };
                let resource = match r.kind {
                    SlotKind::Uniform => BindingResource::UniformBuffer(range(&corners[n], 48)),
                    SlotKind::Storage => BindingResource::StorageBuffer {
                        range: range(&marks, marks_size),
                        writable: true,
                    },
                    kind => panic!("{kind:?}"),
                };
                entries[r.set as usize].push(BindingEntry {
                    binding: r.binding,
                    stages,
                    resource,
                });
            }
        }
        let helpers = &mut entries[HELPER_SET as usize];
        if vs.reflection.helper_size > 0 {
            helpers.push(BindingEntry {
                binding: helper_binding(ShaderStage::Vertex),
                stages: StageSet::VERTEX,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &vertex_helpers,
                    offset: 0,
                    size: vs.reflection.helper_size as u64,
                }),
            });
        }
        if n == 1 {
            helpers.push(BindingEntry {
                binding: helper_binding(ShaderStage::Fragment),
                stages: StageSet::FRAGMENT,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer: &tests,
                    offset: 0,
                    size: early.reflection.helper_size as u64,
                }),
            });
            for (binding, view) in [
                (wcontainers_gpu::EARLY_DEPTH_BINDING, &depth_view),
                (wcontainers_gpu::EARLY_STENCIL_BINDING, &stencil_view),
            ] {
                helpers.push(BindingEntry {
                    binding,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(view),
                });
            }
        }
        let sets: Vec<_> = entries
            .iter()
            .map(|e| (!e.is_empty()).then(|| b.create_binding_set(e).unwrap()))
            .collect();
        b.begin_pass(pass);
        b.draw(&DrawCall {
            program: &programs[n],
            blend: &blend,
            depth_stencil: [&writing, &testing][n],
            raster: &raster,
            vertex_layout: &vertex_layout,
            bindings: [0, 1, 2, 3].map(|i| sets[i].as_ref()),
            vertex_buffers: &[],
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            viewports: &[Viewport {
                x: 0,
                y: 0,
                width: (SIZE as f32).to_bits(),
                height: (SIZE as f32).to_bits(),
                min_depth: 0,
                max_depth: 1.0f32.to_bits(),
            }],
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [[case.occluder_stencil, case.reference][n]; 2],
            immediates: &[],
            range: DrawRange::Vertices {
                first: 0,
                count: 3,
                first_instance: 0,
                instances: 1,
            },
        });
        b.end_pass();
    };
    for n in 0..2 {
        let first = n == 0;
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: Some(DepthAttachment {
                view: &depth_view,
                depth: Some(AspectOps {
                    load: match first {
                        true => LoadOp::Clear(case.clear_depth.to_bits()),
                        false => LoadOp::Load,
                    },
                    store: StoreOp::Store,
                    read_only: !first,
                }),
                stencil: Some(AspectOps {
                    load: match first {
                        true => LoadOp::Clear(case.clear_stencil),
                        false => LoadOp::Load,
                    },
                    store: StoreOp::Store,
                    read_only: !first,
                }),
            }),
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &target_view,
            depth_slice: 0,
            resolve: None,
            load: match first {
                true => LoadOp::Clear([0; 4]),
                false => LoadOp::Load,
            },
            store: StoreOp::Store,
        });
        draw(b, &pass, n);
    }
    let mut marked = vec![0; marks_size as usize];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &marks,
            offset: 0,
            size: marks_size,
        },
        &mut marked,
    )
    .unwrap();
    let mut texels = vec![0; marks_size as usize];
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
        &mut texels,
    )
    .unwrap();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok, "{case:?}");
    (marked, texels)
}

#[test]
fn early_tests_in_the_program_match_direct3d() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_EARLY_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_EARLY_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(32);
    let mut failures = Vec::new();
    for seed in seed..seed + seeds {
        let case = case(seed);
        let (marks, texels) = direct3d(&gpu, &case);
        let ran = marks.chunks(4).filter(|m| m != &[0; 4]).count();
        for dxil in [false, true] {
            let (our_marks, our_texels) = ours(&mut backend, &case, dxil);
            let apart = marks
                .chunks(4)
                .zip(our_marks.chunks(4))
                .filter(|(a, b)| a != b)
                .count();
            let (texels_apart, largest) = differ(&texels, &our_texels, Format::Rgba8Unorm);
            println!(
                "seed {seed} {}: Direct3D ran {ran} pixels; marks apart {apart}, texels apart \
                 {texels_apart} (largest {largest})",
                if dxil { "dxil" } else { "dxbc" },
            );
            if apart != 0 || texels_apart != 0 {
                failures.push(format!("seed {seed} dxil {dxil}: {case:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
