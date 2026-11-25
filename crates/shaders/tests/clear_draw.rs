//! The built-in clear program on the WebGPU back end against real Direct3D 11's
//! `ClearUnorderedAccessViewUint` and `ClearUnorderedAccessViewFloat` on the same GPU: seeded
//! formats (those both have as typed storage views), views (one 2D layer, a range of array layers,
//! a range of 3D slices), texels and values. Held to WARP, which clears as the functional
//! specification says, and float clears to WARP or the GPU's Direct3D; the GPU's differences are
//! printed and counted. `WCONTAINERS_CLEAR_SEED` (default 1) and
//! `WCONTAINERS_CLEAR_CASES` (default 64) cases from it.

#![cfg(windows)]

mod common;

use common::*;
use wcontainers_gpu::{
    Gpu, BindingEntry, BindingResource, BufferRange, BufferUsage, DispatchCall, DispatchGroups,
    Extent, FORMAT_FACTS, Format, FormatUse, Origin, Program, ReadSource, StageSet, TextureDesc,
    TextureDimension, TextureRegion, TextureUsage, TextureViewDesc, ViewDimension, WriteMode,
};
use wcontainers_graphics_test::d3d11::{self, Gpu};
use wcontainers_shaders::ConvertError;
use wcontainers_shaders::builtin::{
    CLEAR_PARAMS_BINDING, CLEAR_VIEW_BINDING, clear_groups, clear_params, storage_clear,
    storage_clear_value,
};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::core::Interface;

/// The formats WGSL has storage views of, and `A8Unorm`, which the back ends hold as `rgba8unorm`.
const FORMATS: [Format; 41] = [
    Format::R8Unorm,
    Format::R8Snorm,
    Format::R8Uint,
    Format::R8Sint,
    Format::R16Unorm,
    Format::R16Snorm,
    Format::R16Uint,
    Format::R16Sint,
    Format::R16Float,
    Format::Rg8Unorm,
    Format::Rg8Snorm,
    Format::Rg8Uint,
    Format::Rg8Sint,
    Format::R32Uint,
    Format::R32Sint,
    Format::R32Float,
    Format::Rg16Unorm,
    Format::Rg16Snorm,
    Format::Rg16Uint,
    Format::Rg16Sint,
    Format::Rg16Float,
    Format::Rgba8Unorm,
    Format::Rgba8Snorm,
    Format::Rgba8Uint,
    Format::Rgba8Sint,
    Format::Bgra8Unorm,
    Format::Rgb10a2Unorm,
    Format::Rgb10a2Uint,
    Format::Rg11b10Float,
    Format::Rg32Uint,
    Format::Rg32Sint,
    Format::Rg32Float,
    Format::Rgba16Unorm,
    Format::Rgba16Snorm,
    Format::Rgba16Uint,
    Format::Rgba16Sint,
    Format::Rgba16Float,
    Format::Rgba32Uint,
    Format::Rgba32Sint,
    Format::Rgba32Float,
    Format::A8Unorm,
];

/// Float clear values, from which each case picks four.
const FLOATS: [f32; 16] = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.5,
    0.2,
    -0.7,
    2.5,
    -3.25,
    f32::NAN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    1e-40,
    65504.0,
    70000.0,
    1e30,
];

/// One clear: a texture of `size` (width, height, layers or depth), the view's layers or slices
/// `first..first + count`, its texels before, and the values.
struct Case {
    format: Format,
    dimension: ViewDimension,
    size: [u32; 3],
    first: u32,
    count: u32,
    texels: Vec<u8>,
    values: [u32; 4],
    float: bool,
}

fn cases(gpu: &Gpu, b: &impl Gpu, seed: u64, n: u64) -> Vec<Case> {
    let can = |f: Format| {
        let need = D3D11_FORMAT_SUPPORT_TYPED_UNORDERED_ACCESS_VIEW.0 as u32;
        let d3d = unsafe { gpu.device.CheckFormatSupport(dxgi(f)) }.is_ok_and(|s| s & need == need);
        d3d && b.caps().formats.uses[f as usize].contains(FormatUse::STORAGE_WRITE)
    };
    let formats: Vec<Format> = FORMATS.into_iter().filter(|&f| can(f)).collect();
    eprintln!("{} formats: {formats:?}", formats.len());
    (seed..seed + n)
        .map(|seed| {
            let mut r = fastrand::Rng::with_seed(seed);
            let mut next = || r.u32(..);
            let format = formats[next() as usize % formats.len()];
            let dimension =
                [ViewDimension::D2, ViewDimension::D2Array, ViewDimension::D3][next() as usize % 3];
            let layers = if dimension == ViewDimension::D2 {
                1
            } else {
                1 + next() % 5
            };
            let size = [3 + next() % 18, 3 + next() % 18, layers];
            let first = next() % layers;
            let count = 1 + next() % (layers - first);
            let bytes = size[0] * size[1] * layers * FORMAT_FACTS[format as usize].block.2;
            let texels = (0..bytes).map(|_| next() as u8).collect();
            let float = next() % 2 == 0;
            let values = [0; 4].map(|_| {
                if float {
                    FLOATS[next() as usize % FLOATS.len()].to_bits()
                } else if next() % 2 == 0 {
                    next() % 300
                } else {
                    next()
                }
            });
            Case {
                format,
                dimension,
                size,
                first,
                count,
                texels,
                values,
                float,
            }
        })
        .collect()
}

/// Every texel of the texture after real Direct3D 11 clears the case's view, layer or slice after
/// layer or slice.
fn direct3d(gpu: &Gpu, case: &Case) -> Vec<u8> {
    let [w, h, l] = case.size;
    let row = w * FORMAT_FACTS[case.format as usize].block.2;
    let volume = case.dimension == ViewDimension::D3;
    let make = |staging: bool| -> ID3D11Resource {
        let (usage, bind, cpu) = if staging {
            (D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ.0 as u32)
        } else {
            (D3D11_USAGE_DEFAULT, D3D11_BIND_UNORDERED_ACCESS.0 as u32, 0)
        };
        let init: Vec<_> = case
            .texels
            .chunks((row * h) as usize)
            .map(|layer| d3d11::data(layer, row, row * h))
            .collect();
        if volume {
            let desc = D3D11_TEXTURE3D_DESC {
                Width: w,
                Height: h,
                Depth: l,
                MipLevels: 1,
                Format: dxgi(case.format),
                Usage: usage,
                BindFlags: bind,
                CPUAccessFlags: cpu,
                ..Default::default()
            };
            let data = d3d11::data(&case.texels, row, row * h);
            let data = (!staging).then_some(&data as *const _);
            d3d11::made(|o| unsafe { gpu.device.CreateTexture3D(&desc, data, Some(o)) })
                .unwrap()
                .cast()
                .unwrap()
        } else {
            let mut desc = d3d11::tex2d(w, h, dxgi(case.format), D3D11_BIND_FLAG(bind as i32));
            desc.ArraySize = l;
            desc.Usage = usage;
            desc.CPUAccessFlags = cpu;
            let init: &[_] = if staging { &[] } else { &init };
            gpu.texture2d(&desc, init).unwrap().cast().unwrap()
        }
    };
    let texture = make(false);
    let mut desc = D3D11_UNORDERED_ACCESS_VIEW_DESC {
        Format: dxgi(case.format),
        ..Default::default()
    };
    match case.dimension {
        ViewDimension::D2 => desc.ViewDimension = D3D11_UAV_DIMENSION_TEXTURE2D,
        ViewDimension::D2Array => {
            desc.ViewDimension = D3D11_UAV_DIMENSION_TEXTURE2DARRAY;
            desc.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_UAV {
                MipSlice: 0,
                FirstArraySlice: case.first,
                ArraySize: case.count,
            };
        }
        _ => {
            desc.ViewDimension = D3D11_UAV_DIMENSION_TEXTURE3D;
            desc.Anonymous.Texture3D = D3D11_TEX3D_UAV {
                MipSlice: 0,
                FirstWSlice: case.first,
                WSize: case.count,
            };
        }
    }
    let view = gpu.uav(&texture, Some(&desc)).unwrap();
    let staging = make(true);
    let mut out = Vec::new();
    unsafe {
        let c = &gpu.context;
        if case.float {
            c.ClearUnorderedAccessViewFloat(&view, &case.values.map(f32::from_bits));
        } else {
            c.ClearUnorderedAccessViewUint(&view, &case.values);
        }
        c.CopyResource(&staging, &texture);
        let subresources = if volume { 1 } else { l };
        for s in 0..subresources {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            c.Map(&staging, s, D3D11_MAP_READ, 0, Some(&mut mapped))
                .unwrap();
            for z in 0..if volume { l } else { 1 } as usize {
                for y in 0..h as usize {
                    let start = mapped
                        .pData
                        .cast::<u8>()
                        .add(z * mapped.DepthPitch as usize + y * mapped.RowPitch as usize);
                    out.extend_from_slice(std::slice::from_raw_parts(start, row as usize));
                }
            }
            c.Unmap(&staging, s);
        }
    }
    out
}

/// The same through the WebGPU back end and the clear program, or the texels untouched where the
/// value is refused (`refused`).
fn ours<B: Gpu>(b: &mut B, case: &Case, refused: &mut Vec<&'static str>) -> Vec<u8> {
    let [w, h, l] = case.size;
    let row = w * FORMAT_FACTS[case.format as usize].block.2;
    let volume = case.dimension == ViewDimension::D3;
    let texture = b
        .create_texture(&TextureDesc {
            dimension: if volume {
                TextureDimension::D3
            } else {
                TextureDimension::D2
            },
            format: case.format,
            width: w,
            height: h,
            depth_or_layers: l,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::STORAGE | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    b.write_texture(
        &TextureRegion {
            texture: &texture,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: w,
            height: h,
            depth_or_layers: l,
        },
        &case.texels,
        row,
        h,
        WriteMode::Ordered,
    );
    match storage_clear_value(case.format, case.values, case.float) {
        Ok(value) => {
            // A 3D view holds every slice; the program clears the case's.
            let (base_layer, layer_count, z) = match case.dimension {
                ViewDimension::D3 => (0, 1, [case.first, case.first + case.count]),
                ViewDimension::D2Array => (case.first, case.count, [0, case.count]),
                _ => (0, 1, [0, 1]),
            };
            let view = b
                .create_view(
                    &texture,
                    &TextureViewDesc {
                        format: case.format,
                        dimension: case.dimension,
                        base_mip: 0,
                        mip_count: 1,
                        base_layer,
                        layer_count,
                        stencil: false,
                    },
                )
                .unwrap();
            let (origin, end) = ([0, 0, z[0]], [w, h, z[1]]);
            let params = buffer(b, &clear_params(value, origin, end), BufferUsage::UNIFORM);
            let set = b
                .create_binding_set(&[
                    BindingEntry {
                        binding: CLEAR_PARAMS_BINDING,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::UniformBuffer(BufferRange {
                            buffer: &params,
                            offset: 0,
                            size: 48,
                        }),
                    },
                    BindingEntry {
                        binding: CLEAR_VIEW_BINDING,
                        stages: StageSet::COMPUTE,
                        resource: BindingResource::StorageTexture {
                            view: &view,
                            writable: true,
                        },
                    },
                ])
                .unwrap();
            let module = storage_clear(case.format, case.dimension).unwrap();
            let shader = b.create_shader(&module).unwrap();
            let program = b.create_program(&Program::Compute(&shader)).unwrap();
            b.dispatch(&DispatchCall {
                program: &program,
                bindings: [Some(&set), None, None, None],
                immediates: &[],
                groups: DispatchGroups::Direct(clear_groups(origin, end)),
            })
            .unwrap();
        }
        Err(ConvertError::Unsupported(reason)) => refused.push(reason),
        Err(e) => panic!("{e:?}"),
    }
    let mut all = Vec::new();
    for at in 0..l {
        let mut out = vec![0; (row * h) as usize];
        b.read_back_now(
            &ReadSource::Texture {
                region: TextureRegion {
                    texture: &texture,
                    mip: 0,
                    layer: if volume { 0 } else { at },
                    origin: Origin {
                        z: if volume { at } else { 0 },
                        ..Origin::default()
                    },
                    stencil: false,
                },
                extent: Extent {
                    width: w,
                    height: h,
                    depth_or_layers: 1,
                },
            },
            &mut out,
        )
        .unwrap();
        all.extend(out);
    }
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
    all
}

#[test]
fn storage_clears_match_direct3d() {
    let mut backend = webgpu();
    let gpu = d3d11::create(
        wcontainers_graphics_test::Driver::Hardware,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let warp = d3d11::create(
        wcontainers_graphics_test::Driver::Warp,
        false,
        &[D3D_FEATURE_LEVEL_11_0],
    )
    .unwrap();
    let seed: u64 = std::env::var("WCONTAINERS_CLEAR_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(1);
    let n: u64 = std::env::var("WCONTAINERS_CLEAR_CASES")
        .map(|s| s.parse().unwrap())
        .unwrap_or(64);
    let mut failures = Vec::new();
    let mut refused = Vec::new();
    let mut gpu_apart = 0;
    for (k, case) in cases(&gpu, &backend, seed, n).iter().enumerate() {
        let seed = seed + k as u64;
        // WARP clears as the functional specification says; the GPU's Direct3D saturates integer
        // clears and reads some packed formats' values as floats, so its texels are counted apart.
        let reference = direct3d(&warp, case);
        let hardware = direct3d(&gpu, case);
        let before = refused.len();
        let made = ours(&mut backend, case, &mut refused);
        let size = FORMAT_FACTS[case.format as usize].block.2 as usize;
        let differ = |a: &[u8], b: &[u8], name: &str| {
            let mut apart = 0;
            for (t, (x, y)) in a.chunks(size).zip(b.chunks(size)).enumerate() {
                if x != y {
                    apart += 1;
                    if apart <= 2 {
                        eprintln!("  texel {t}: {name} {x:02x?}, ours {y:02x?}");
                    }
                }
            }
            apart
        };
        let from_hardware = differ(&hardware, &made, "rtx 3080");
        // A float clear of a normalised format may round a value half way between two codes either
        // way (the specification allows 0.6 of a code); where WARP and the GPU part there, ours
        // must equal the GPU's, whose store it uses.
        let apart = if case.float {
            let mut apart = 0;
            for (t, ((w, h), o)) in reference
                .chunks(size)
                .zip(hardware.chunks(size))
                .zip(made.chunks(size))
                .enumerate()
            {
                if o != w && o != h {
                    apart += 1;
                    if apart <= 2 {
                        eprintln!("  texel {t}: warp {w:02x?}, rtx 3080 {h:02x?}, ours {o:02x?}");
                    }
                }
            }
            apart
        } else {
            differ(&reference, &made, "warp")
        };
        if from_hardware > 0 {
            gpu_apart += 1;
        }
        let cleared = reference != case.texels;
        eprintln!(
            "seed {seed} {:?} {:?} {:?} layers {}+{} {} {:08x?}{}: {apart} texels apart from warp, \
             {from_hardware} from the gpu{}",
            case.format,
            case.dimension,
            case.size,
            case.first,
            case.count,
            if case.float { "float" } else { "uint" },
            case.values,
            refused[before..]
                .first()
                .map_or(String::new(), |r| format!(" refused {r}")),
            if cleared { "" } else { ", direct3d left it" },
        );
        // A value the program cannot store is the front's to clear another way; a float clear of
        // an integer format Direct3D 11 drops, which leaving the texels shows.
        let theirs = refused[before..]
            .first()
            .is_some_and(|&r| r != wcontainers_shaders::refusal::FLOAT_CLEAR_OF_INTEGER);
        if apart > 0 && !theirs {
            failures.push(format!("seed {seed}"));
        }
    }
    eprintln!(
        "{n} cases, {} failed, {} refused: {refused:?}; {gpu_apart} apart from the gpu's direct3d",
        failures.len(),
        refused.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
