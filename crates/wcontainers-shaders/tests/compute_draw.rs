//! Compute shaders with atomic operations, append, consume and counter views, and storage textures
//! (`compute.hlsl`) on the WebGPU back end, against real Direct3D 11 running the same shaders on the
//! same GPU. The values consumed and the texels read are seeded: `WCONTAINERS_COMPUTE_SEED` (default 1) and `WCONTAINERS_COMPUTE_CASES` seeds
//! from it (default 4).

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BufferRange, BufferUsage, DispatchCall, DispatchGroups,
    Extent, Format, Origin, Program, ReadSource, SlotKind, StageSet, TextureDesc, TextureDimension,
    TextureRegion, TextureUsage, TextureViewDesc, VariantKey, ViewDimension, WriteMode,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::{Source, convert};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// One storage view: its bytes before the dispatch, whether it is a raw buffer, the count its
/// counter starts at (none: no counter), and whether it is an append or consume view.
struct View {
    bytes: Vec<u8>,
    raw: bool,
    counter: Option<u32>,
    append: bool,
}

/// One case: a shader, its views by slot and its workgroups.
struct Case {
    shader: &'static str,
    views: Vec<View>,
    groups: u32,
}

fn words(w: &[u32]) -> Vec<u8> {
    w.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn rng(seed: u64) -> impl FnMut() -> u32 {
    let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 16) as u32
    }
}

fn cases(seed: u64) -> Vec<Case> {
    let mut next = rng(seed);
    let mut raw = vec![0u32; 2048];
    raw[642] = !0;
    raw[645] = !0;
    let consumed: Vec<u32> = (0..256).map(|_| next() >> 4).collect();
    let returned: Vec<u32> = (0..32).map(|_| next() >> 4).collect();
    vec![
        Case {
            shader: "cs_atomics",
            views: vec![
                View {
                    bytes: words(&raw),
                    raw: true,
                    counter: None,
                    append: false,
                },
                View {
                    bytes: words(&[i32::MAX as u32, i32::MIN as u32, 5, 0]),
                    raw: false,
                    counter: None,
                    append: false,
                },
                View {
                    bytes: vec![0; 2048],
                    raw: false,
                    counter: Some(0),
                    append: false,
                },
                View {
                    bytes: vec![0; 2048],
                    raw: false,
                    counter: Some(0),
                    append: true,
                },
                View {
                    bytes: words(&consumed),
                    raw: false,
                    counter: Some(256),
                    append: true,
                },
            ],
            groups: 4,
        },
        Case {
            shader: "cs_decrement",
            views: vec![View {
                bytes: words(&returned),
                raw: false,
                counter: Some(32),
                append: false,
            }],
            groups: 1,
        },
    ]
}

/// What a run leaves: each view's bytes and each counter's count.
struct Ran {
    views: Vec<Vec<u8>>,
    counters: Vec<Option<u32>>,
}

fn direct3d(gpu: &Gpu, case: &Case) -> Ran {
    let shader = gpu.cs(&bytecode(&format!("{}.dxbc", case.shader))).unwrap();
    let mut buffers = Vec::new();
    let mut views = Vec::new();
    for v in &case.views {
        let buffer = gpu
            .buffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: v.bytes.len() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                    MiscFlags: if v.raw {
                        D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0 as u32
                    } else {
                        D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32
                    },
                    StructureByteStride: if v.raw { 0 } else { 4 },
                    ..Default::default()
                },
                Some(&v.bytes),
            )
            .unwrap();
        let flags = if v.raw {
            D3D11_BUFFER_UAV_FLAG_RAW.0 as u32
        } else if v.append {
            D3D11_BUFFER_UAV_FLAG_APPEND.0 as u32
        } else if v.counter.is_some() {
            D3D11_BUFFER_UAV_FLAG_COUNTER.0 as u32
        } else {
            0
        };
        let desc = D3D11_UNORDERED_ACCESS_VIEW_DESC {
            Format: if v.raw {
                DXGI_FORMAT_R32_TYPELESS
            } else {
                DXGI_FORMAT_UNKNOWN
            },
            ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
            Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
                Buffer: D3D11_BUFFER_UAV {
                    FirstElement: 0,
                    NumElements: v.bytes.len() as u32 / 4,
                    Flags: flags,
                },
            },
        };
        views.push(Some(gpu.uav(&buffer.clone().into(), Some(&desc)).unwrap()));
        buffers.push(buffer);
    }
    let initial: Vec<u32> = case.views.iter().map(|v| v.counter.unwrap_or(!0)).collect();
    let counts = gpu
        .buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: 4 * case.views.len() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    unsafe {
        let c = &gpu.context;
        c.CSSetShader(&shader, None);
        c.CSSetUnorderedAccessViews(
            0,
            views.len() as u32,
            Some(views.as_ptr()),
            Some(initial.as_ptr()),
        );
        c.Dispatch(case.groups, 1, 1);
        for (i, (v, view)) in case.views.iter().zip(&views).enumerate() {
            if v.counter.is_some() {
                c.CopyStructureCount(&counts, 4 * i as u32, view.as_ref().unwrap());
            }
        }
        let none: Vec<Option<ID3D11UnorderedAccessView>> = vec![None; views.len()];
        c.CSSetUnorderedAccessViews(0, none.len() as u32, Some(none.as_ptr()), None);
    }
    let counts = gpu.read_buffer(&counts).unwrap();
    Ran {
        views: buffers
            .iter()
            .map(|b| gpu.read_buffer(b).unwrap())
            .collect(),
        counters: case
            .views
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.counter
                    .map(|_| u32::from_le_bytes(counts[4 * i..4 * i + 4].try_into().unwrap()))
            })
            .collect(),
    }
}

fn ours<B: Backend>(b: &mut B, case: &Case) -> Ran {
    let c = convert(
        Source::Dxbc(&bytecode(&format!("{}.dxbc", case.shader))),
        &VariantKey::default(),
        None,
    )
    .unwrap();
    assert!(c.reflection.helpers.is_empty());
    let usage = BufferUsage::STORAGE | BufferUsage::COPY_SRC;
    let buffers: Vec<_> = case
        .views
        .iter()
        .map(|v| buffer(b, &v.bytes, usage))
        .collect();
    let counters: Vec<_> = case
        .views
        .iter()
        .map(|v| v.counter.map(|n| buffer(b, &n.to_le_bytes(), usage)))
        .collect();
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| {
            let slot = r.register as usize;
            let range = match r.kind {
                SlotKind::Storage => BufferRange {
                    buffer: &buffers[slot],
                    offset: 0,
                    size: case.views[slot].bytes.len() as u64,
                },
                SlotKind::Counter => BufferRange {
                    buffer: counters[slot].as_ref().unwrap(),
                    offset: 0,
                    size: 4,
                },
                _ => panic!("{r:?}"),
            };
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range,
                    writable: true,
                },
            }
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    let shader = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&shader)).unwrap();
    b.dispatch(&DispatchCall {
        program: &program,
        bindings: [Some(&set), None, None, None],
        immediates: &[],
        groups: DispatchGroups::Direct([case.groups, 1, 1]),
    })
    .unwrap();
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
    let views = buffers
        .iter()
        .zip(&case.views)
        .map(|(buffer, v)| read(buffer, v.bytes.len() as u64))
        .collect();
    let counters = counters
        .iter()
        .map(|c| {
            c.as_ref()
                .map(|c| u32::from_le_bytes(read(c, 4).try_into().unwrap()))
        })
        .collect();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    Ran { views, counters }
}

/// Makes what depends on the order invocations ran in comparable: the value the last exchange left
/// (word 648 of `cs_atomics`' raw view) is marked as the values the exchanges returned are, and the
/// appended values are sorted.
fn ordered(case: &Case, ran: &mut Ran) {
    if case.shader != "cs_atomics" {
        return;
    }
    let raw = &mut ran.views[0];
    let last = u32::from_le_bytes(raw[2592..2596].try_into().unwrap()) as usize;
    raw[2592..2596].fill(0);
    if 1024 + last < 2048 {
        raw[4 * (1024 + last)..4 * (1024 + last) + 4].copy_from_slice(&1u32.to_le_bytes());
    }
    let n = ran.counters[3].unwrap_or(0).min(512) as usize;
    let mut appended: Vec<u32> = ran.views[3][..4 * n]
        .chunks(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect();
    appended.sort_unstable();
    ran.views[3][..4 * n].copy_from_slice(&words(&appended));
}

#[test]
fn atomics_and_counters_match_direct3d() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    let mut compared = 0;
    for seed in seed..seed + seeds {
        for case in cases(seed) {
            let name = format!("seed {seed} {}", case.shader);
            let mut reference = direct3d(&gpu, &case);
            let mut made = ours(&mut backend, &case);
            ordered(&case, &mut reference);
            ordered(&case, &mut made);
            compared += 1;
            let mut apart = 0;
            for (slot, (a, o)) in reference.views.iter().zip(&made.views).enumerate() {
                for (w, (x, y)) in a.chunks(4).zip(o.chunks(4)).enumerate() {
                    if x != y {
                        apart += 1;
                        if apart <= 8 {
                            eprintln!(
                                "  view {slot} word {w}: direct3d {:#010x}, ours {:#010x}",
                                u32::from_le_bytes(x.try_into().unwrap()),
                                u32::from_le_bytes(y.try_into().unwrap())
                            );
                        }
                    }
                }
            }
            eprintln!(
                "{name}: counters direct3d {:?}, ours {:?}; {apart} words apart",
                reference.counters, made.counters
            );
            if apart > 0 || reference.counters != made.counters {
                failures.push(name);
            }
        }
    }
    eprintln!("{compared} comparisons, {} failed", failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `cs_images`' storage textures: slot, format, its Direct3D format, layers, and whether it starts
/// with seeded texels (the one read and written again).
const IMAGES: [(u32, Format, DXGI_FORMAT, u32, bool); 4] = [
    (0, Format::Rgba8Unorm, DXGI_FORMAT_R8G8B8A8_UNORM, 1, false),
    (1, Format::R32Float, DXGI_FORMAT_R32_FLOAT, 1, true),
    (
        2,
        Format::Rgba16Uint,
        DXGI_FORMAT_R16G16B16A16_UINT,
        3,
        false,
    ),
    (3, Format::Rg32Sint, DXGI_FORMAT_R32G32_SINT, 1, false),
];
const IMAGE_SIZE: u32 = 16;

fn image_texels(seed: u64, format: Format, layers: u32, seeded: bool) -> Vec<u8> {
    let n = (IMAGE_SIZE * IMAGE_SIZE * layers * format.block().2) as usize;
    if !seeded {
        return vec![0; n];
    }
    let mut next = rng(seed);
    (0..n / 4)
        .flat_map(|_| ((next() % 2000) as f32 / 16.0 - 60.0).to_le_bytes())
        .collect()
}

/// Each texture's texels after real Direct3D 11 runs `cs_images`, layer after layer.
fn direct3d_images(gpu: &Gpu, seed: u64) -> Vec<Vec<u8>> {
    let shader = gpu.cs(&bytecode("cs_images.dxbc")).unwrap();
    let mut textures = Vec::new();
    let mut views = Vec::new();
    for &(_, format, dxgi, layers, seeded) in &IMAGES {
        let mut desc = d3d11::tex2d(IMAGE_SIZE, IMAGE_SIZE, dxgi, D3D11_BIND_UNORDERED_ACCESS);
        desc.ArraySize = layers;
        let texels = image_texels(seed, format, layers, seeded);
        let row = IMAGE_SIZE * format.block().2;
        let data: Vec<_> = texels
            .chunks((row * IMAGE_SIZE) as usize)
            .map(|layer| d3d11::data(layer, row, row * IMAGE_SIZE))
            .collect();
        let texture = gpu.texture2d(&desc, &data).unwrap();
        views.push(Some(gpu.uav(&texture.clone().into(), None).unwrap()));
        textures.push((texture, format, layers));
    }
    unsafe {
        let c = &gpu.context;
        c.CSSetShader(&shader, None);
        c.CSSetUnorderedAccessViews(0, views.len() as u32, Some(views.as_ptr()), None);
        c.Dispatch(IMAGE_SIZE / 8, IMAGE_SIZE / 8, 1);
        let none: Vec<Option<ID3D11UnorderedAccessView>> = vec![None; views.len()];
        c.CSSetUnorderedAccessViews(0, none.len() as u32, Some(none.as_ptr()), None);
    }
    textures
        .iter()
        .map(|(texture, format, layers)| {
            let row = (IMAGE_SIZE * format.block().2) as usize;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            unsafe { texture.GetDesc(&mut desc) };
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            let staging = gpu.texture2d(&desc, &[]).unwrap();
            let mut out = Vec::new();
            unsafe {
                gpu.context.CopyResource(&staging, texture);
                for layer in 0..*layers {
                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    gpu.context
                        .Map(&staging, layer, D3D11_MAP_READ, 0, Some(&mut mapped))
                        .unwrap();
                    for y in 0..IMAGE_SIZE as usize {
                        let start = mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize);
                        out.extend_from_slice(std::slice::from_raw_parts(start, row));
                    }
                    gpu.context.Unmap(&staging, layer);
                }
            }
            out
        })
        .collect()
}

/// The same through the WebGPU back end, each view's format given in the variant.
fn ours_images<B: Backend>(b: &mut B, seed: u64) -> Vec<Vec<u8>> {
    let key = VariantKey {
        storage_formats: IMAGES.iter().map(|&(slot, f, ..)| (slot, f)).collect(),
        ..Default::default()
    };
    let c = convert(Source::Dxbc(&bytecode("cs_images.dxbc")), &key, None).unwrap();
    assert!(c.reflection.helpers.is_empty());
    let mut textures = Vec::new();
    for &(_, format, _, layers, seeded) in &IMAGES {
        let texture = b
            .create_texture(&TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: IMAGE_SIZE,
                height: IMAGE_SIZE,
                depth_or_layers: layers,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::STORAGE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            })
            .unwrap();
        let view = b
            .create_view(
                &texture,
                &TextureViewDesc {
                    format,
                    dimension: if layers > 1 {
                        ViewDimension::D2Array
                    } else {
                        ViewDimension::D2
                    },
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: layers,
                    stencil: false,
                },
            )
            .unwrap();
        let row = IMAGE_SIZE * format.block().2;
        b.write_texture(
            &TextureRegion {
                texture: &texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: IMAGE_SIZE,
                height: IMAGE_SIZE,
                depth_or_layers: layers,
            },
            &image_texels(seed, format, layers, seeded),
            row,
            IMAGE_SIZE,
            WriteMode::Ordered,
        );
        textures.push((texture, view, format, layers));
    }
    let entries: Vec<_> = c
        .reflection
        .bindings
        .iter()
        .map(|r| {
            assert_eq!(r.kind, SlotKind::Storage, "{r:?}");
            BindingEntry {
                binding: r.binding,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageTexture {
                    view: &textures[r.register as usize].1,
                    writable: true,
                },
            }
        })
        .collect();
    let set = b.create_binding_set(&entries).unwrap();
    let shader = b.create_shader(&c.modules[0]).unwrap();
    let program = b.create_program(&Program::Compute(&shader)).unwrap();
    b.dispatch(&DispatchCall {
        program: &program,
        bindings: [Some(&set), None, None, None],
        immediates: &[],
        groups: DispatchGroups::Direct([IMAGE_SIZE / 8, IMAGE_SIZE / 8, 1]),
    })
    .unwrap();
    let out = textures
        .iter()
        .map(|(texture, _, format, layers)| {
            let mut all = Vec::new();
            for layer in 0..*layers {
                let mut out = vec![0; (IMAGE_SIZE * IMAGE_SIZE * format.block().2) as usize];
                b.read_back_now(
                    &ReadSource::Texture {
                        region: TextureRegion {
                            texture,
                            mip: 0,
                            layer,
                            origin: Origin::default(),
                            stencil: false,
                        },
                        extent: Extent {
                            width: IMAGE_SIZE,
                            height: IMAGE_SIZE,
                            depth_or_layers: 1,
                        },
                    },
                    &mut out,
                )
                .unwrap();
                all.extend(out);
            }
            all
        })
        .collect();
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    out
}

#[test]
fn storage_textures_match_direct3d() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_COMPUTE_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let seeds: u64 = std::env::var("WCONTAINERS_COMPUTE_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(4);
    let mut failures = Vec::new();
    for seed in seed..seed + seeds {
        let reference = direct3d_images(&gpu, seed);
        let made = ours_images(&mut backend, seed);
        for ((a, o), &(slot, format, ..)) in reference.iter().zip(&made).zip(&IMAGES) {
            let apart = a.chunks(4).zip(o.chunks(4)).filter(|(x, y)| x != y).count();
            eprintln!("seed {seed} u{slot} {format:?}: {apart} words apart");
            if apart > 0 {
                failures.push(format!("seed {seed} u{slot}"));
            }
        }
    }
    eprintln!("{} seeds, {} failed", seeds, failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}
