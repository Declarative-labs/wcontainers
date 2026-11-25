//! Atomic minimums and maximums of the other signedness than their memory's (`AtomicExchangeLoop`)
//! in a pixel shader on the WebGPU back end, against real Direct3D 11 on the same GPU (`early.hlsl`'s
//! `ps_contend`): every pixel of a seeded triangle updates the same words, some pixels discard
//! before, and the triangle's edges inside the target make helper lanes. The words, the marks and
//! the colours must be equal, and every loop must end. Seeded: `WCONTAINERS_CONTENTION_SEED`
//! (default 1) and `WCONTAINERS_CONTENTION_CASES` seeds from it (default 8).

#![cfg(windows)]

mod common;

use common::*;
// The trait by its old name: graphics-test's Direct3D 11 device is named `Gpu` in this file.
use wcontainers_gpu::{
    BindingEntry, BindingResource, BlendDesc, BufferRange, BufferUsage, ColorAttachment,
    ColorWrites, CullMode, DepthStencilDesc, DrawCall, DrawRange, Extent, FillMode, Format,
    FrontFace, Gpu as Backend, HELPER_SET, LoadOp, Origin, PassDesc, Program, ProvokingVertex,
    RasterDesc, ReadSource, RenderTargetBlend, ShaderStage, SlotKind, SlowPath, StageSet, StoreOp,
    TextureRegion, TextureUsage, Topology, VariantKey, VertexLayoutDesc, Viewport, helper_binding,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{Converted, Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

fn rng(seed: u64) -> impl FnMut() -> u32 {
    let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 16) as u32
    }
}

/// A large triangle with one corner in each of three regions of the target and past it, in either
/// winding.
fn triangle(seed: u64) -> [[f32; 4]; 3] {
    let mut next = rng(seed);
    let mut unit = move || (next() % 4096) as f32 / 4096.0;
    let regions = [
        (-1.4, -0.2, -1.4, -0.2),
        (0.2, 1.4, -1.4, 1.4),
        (-1.4, 1.4, 0.2, 1.4),
    ];
    let mut corners = regions.map(|(x0, x1, y0, y1)| {
        let x = x0 + unit() * (x1 - x0);
        let y = y0 + unit() * (y1 - y0);
        [x, y, 0.5, 1.0]
    });
    if unit() < 0.5 {
        corners.swap(0, 1);
    }
    corners
}

fn floats(v: &[[f32; 4]; 3]) -> Vec<u8> {
    v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect()
}

/// The words before the draw: the signed minimum's and maximum's starting values, the unsigned
/// ones', then a mark per pixel.
fn initial() -> Vec<u8> {
    let mut w = vec![0u32; 4 + (SIZE * SIZE) as usize];
    w[0] = i32::MAX as u32;
    w[1] = i32::MIN as u32;
    w[2] = u32::MAX;
    w.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Real Direct3D 11: the words and the target's texels.
fn direct3d(gpu: &Gpu, corners: &[[f32; 4]; 3]) -> (Vec<u8>, Vec<u8>) {
    let vs = gpu.vs(&bytecode("vs_early.dxbc")).unwrap();
    let ps = gpu.ps(&bytecode("ps_contend.dxbc")).unwrap();
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    let bytes = initial();
    let words = gpu
        .buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: bytes.len() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0 as u32,
                ..Default::default()
            },
            Some(&bytes),
        )
        .unwrap();
    let desc = D3D11_UNORDERED_ACCESS_VIEW_DESC {
        Format: DXGI_FORMAT_R32_TYPELESS,
        ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
        Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
            Buffer: D3D11_BUFFER_UAV {
                FirstElement: 0,
                NumElements: bytes.len() as u32 / 4,
                Flags: D3D11_BUFFER_UAV_FLAG_RAW.0 as u32,
            },
        },
    };
    let uav = gpu.uav(&words.cast().unwrap(), Some(&desc)).unwrap();
    let corners = gpu
        .immutable(&floats(corners), D3D11_BIND_CONSTANT_BUFFER)
        .unwrap();
    let raster = gpu.rasterizer(&d3d11::solid()).unwrap();
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&raster);
        c.IASetInputLayout(None);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        gpu.viewport(SIZE, SIZE);
        c.ClearRenderTargetView(&rtv, &[0.0; 4]);
        c.VSSetShader(&vs, None);
        c.PSSetShader(&ps, None);
        c.VSSetConstantBuffers(0, Some(&[Some(corners)]));
        c.OMSetRenderTargetsAndUnorderedAccessViews(
            Some(&[Some(rtv)]),
            None,
            2,
            1,
            Some(&Some(uav)),
            None,
        );
        c.Draw(3, 0);
        c.OMSetRenderTargets(None, None::<&ID3D11DepthStencilView>);
    }
    (
        gpu.read_buffer(&words).unwrap(),
        read(gpu, &target, Format::Rgba8Unorm),
    )
}

fn converted(name: &str, dxil: bool) -> Converted {
    let ext = if dxil { "dxil" } else { "dxbc" };
    let bytes = bytecode(&format!("{name}.{ext}"));
    let source = if dxil {
        Source::Dxil(&bytes)
    } else {
        Source::Dxbc(&bytes)
    };
    convert(source, &VariantKey::default(), None).unwrap()
}

/// The same through the WebGPU back end.
fn ours<B: Backend>(b: &mut B, corners: &[[f32; 4]; 3], dxil: bool) -> (Vec<u8>, Vec<u8>) {
    let vs = converted("vs_early", dxil);
    let ps = converted("ps_contend", dxil);
    assert_eq!(ps.reflection.slow_paths, [SlowPath::AtomicExchangeLoop]);
    let vertex = b.create_shader(&vs.modules[0]).unwrap();
    let fragment = b.create_shader(&ps.modules[0]).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
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
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: None,
            depth_write: false,
            stencil: None,
            stencil_read_mask: [0xff; 2],
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
    let bytes = initial();
    let size = bytes.len() as u64;
    let words = buffer(b, &bytes, BufferUsage::STORAGE | BufferUsage::COPY_SRC);
    let corners = buffer(b, &floats(corners), BufferUsage::UNIFORM);
    let vertex_helpers = buffer(
        b,
        &vec![0; vs.reflection.helper_size.max(16) as usize],
        BufferUsage::UNIFORM,
    );
    let mut entries: [Vec<BindingEntry<'_, B>>; 4] = Default::default();
    for (c, stages) in [(&vs, StageSet::VERTEX), (&ps, StageSet::FRAGMENT)] {
        for r in &c.reflection.bindings {
            let range = |buffer, size| BufferRange {
                buffer,
                offset: 0,
                size,
            };
            let resource = match r.kind {
                SlotKind::Uniform => BindingResource::UniformBuffer(range(&corners, 48)),
                SlotKind::Storage => BindingResource::StorageBuffer {
                    range: range(&words, size),
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
    assert_eq!(ps.reflection.helper_size, 0);
    if vs.reflection.helper_size > 0 {
        entries[HELPER_SET as usize].push(BindingEntry {
            binding: helper_binding(ShaderStage::Vertex),
            stages: StageSet::VERTEX,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &vertex_helpers,
                offset: 0,
                size: vs.reflection.helper_size as u64,
            }),
        });
    }
    let sets: Vec<_> = entries
        .iter()
        .map(|e| (!e.is_empty()).then(|| b.create_binding_set(e).unwrap()))
        .collect();
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
        load: LoadOp::Clear([0; 4]),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.draw(&DrawCall {
        program: &program,
        blend: &blend,
        depth_stencil: &depth_stencil,
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
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    let mut read_words = vec![0; size as usize];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &words,
            offset: 0,
            size,
        },
        &mut read_words,
    )
    .unwrap();
    let mut texels = vec![0; (4 * SIZE * SIZE) as usize];
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
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    (read_words, texels)
}

#[test]
fn contended_exchange_loops_match_direct3d() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_CONTENTION_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_CONTENTION_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(8);
    let mut failures = Vec::new();
    for seed in seed..seed + seeds {
        let corners = triangle(seed);
        let (words, texels) = direct3d(&gpu, &corners);
        let ran = words[16..].chunks(4).filter(|m| m != &[0; 4]).count();
        for dxil in [false, true] {
            let (our_words, our_texels) = ours(&mut backend, &corners, dxil);
            let apart = words
                .chunks(4)
                .zip(our_words.chunks(4))
                .filter(|(a, b)| a != b)
                .count();
            let (texels_apart, largest) = differ(&texels, &our_texels, Format::Rgba8Unorm);
            println!(
                "seed {seed} {}: Direct3D ran {ran} pixels; words apart {apart}, texels apart \
                 {texels_apart} (largest {largest}); first words {:?} and ours {:?}",
                if dxil { "dxil" } else { "dxbc" },
                &words[..16],
                &our_words[..16],
            );
            if apart != 0 || texels_apart != 0 {
                failures.push(format!("seed {seed} dxil {dxil}: {corners:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
